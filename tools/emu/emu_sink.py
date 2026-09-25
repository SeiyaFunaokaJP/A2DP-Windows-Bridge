"""Virtual Bluetooth link with an A2DP test sink, for testing A2DPWB without hardware.

Runs two Bumble virtual controllers on one in-process link:
  - controller "A2DPWB" is served as H4 over TCP; A2DPWB connects to it with
    `A2DPWB.exe --cli --hci-tcp 127.0.0.1:<port>`
  - controller "SINK" is driven by a Bumble host acting as an A2DP sink that
    offers SBC, AAC, aptX, aptX HD, aptX LL and LDAC like a typical headset

The sink's HCI traffic is written to a PacketLogger (.pklg) capture, so
a2dpwb_decode --received can check what actually arrived at the sink.

Run with the Python of the tools/emu virtual environment (see setup_env.py).
Prints "READY" once A2DPWB can connect; stops on SIGTERM / Ctrl+C.
"""

import argparse
import asyncio
import logging
import struct
import time

import bumble.logging
from bumble import hci
from bumble.a2dp import (
    A2DP_MPEG_2_4_AAC_CODEC_TYPE,
    A2DP_NON_A2DP_CODEC_TYPE,
    A2DP_SBC_CODEC_TYPE,
    AacMediaCodecInformation,
    SbcMediaCodecInformation,
    VendorSpecificMediaCodecInformation,
    make_audio_sink_service_sdp_records,
)
from bumble.avdtp import AVDTP_AUDIO_MEDIA_TYPE, Listener, MediaCodecCapabilities
from bumble.controller import Controller
from bumble.device import Device
from bumble.host import Host
from bumble.link import LocalLink
from bumble.snoop import Snooper
from bumble.transport import open_transport
from bumble.transport.common import AsyncPipeSink

A2DPWB_ADDRESS = '00:A2:D0:00:00:01'
SINK_ADDRESS = '00:A2:D0:00:00:02'

log = logging.getLogger('emu_sink')


class EmuController(Controller):
    """Bumble virtual controller plus the classic HCI commands BTstack needs.

    Bumble's controller has no LMP encryption. On a virtual link encryption is
    only bookkeeping, so it is reported as enabled on both ends.
    """

    # BR/EDR ACL buffers like a real USB adapter (e.g. Realtek RTL8761B:
    # 1021 bytes); Bumble's default of 27 bytes is the LE minimum.
    acl_data_packet_length = 1021
    total_num_acl_data_packets = 8

    def on_hci_write_secure_connections_host_support_command(self, command):
        return hci.HCI_StatusReturnParameters(hci.HCI_ErrorCode.SUCCESS)

    def on_hci_read_encryption_key_size_command(self, command):
        return hci.HCI_Read_Encryption_Key_Size_ReturnParameters(
            status=hci.HCI_ErrorCode.SUCCESS,
            connection_handle=command.connection_handle,
            key_size=16,
        )

    def on_hci_set_connection_encryption_command(self, command):
        connection = self.find_classic_connection_by_handle(command.connection_handle)
        if connection is None:
            self._send_hci_command_status(
                hci.HCI_ErrorCode.UNKNOWN_CONNECTION_IDENTIFIER_ERROR, command.op_code
            )
            return None
        self._send_hci_command_status(hci.HCI_COMMAND_STATUS_PENDING, command.op_code)
        enabled = command.encryption_enable
        self.send_hci_packet(
            hci.HCI_Encryption_Change_Event(
                status=0, connection_handle=connection.handle, encryption_enabled=enabled
            )
        )
        peer = self.link.find_classic_controller(connection.peer_address)
        peer_connection = peer.classic_connections.get(self.public_address) if peer else None
        if peer_connection:
            peer.send_hci_packet(
                hci.HCI_Encryption_Change_Event(
                    status=0,
                    connection_handle=peer_connection.handle,
                    encryption_enabled=enabled,
                )
            )
        return None


class PacketLoggerSnooper(Snooper):
    """Writes HCI packets in PacketLogger format (as A2DPWB's own capture)."""

    def __init__(self, path):
        self.file = open(path, 'wb', buffering=0)  # unbuffered: survives being killed

    def snoop(self, hci_packet, direction):
        h4_type, payload = hci_packet[0], hci_packet[1:]
        to_controller = direction == Snooper.Direction.HOST_TO_CONTROLLER
        if h4_type == hci.HCI_COMMAND_PACKET:
            pl_type = 0x00
        elif h4_type == hci.HCI_EVENT_PACKET:
            pl_type = 0x01
        elif h4_type == hci.HCI_ACL_DATA_PACKET:
            pl_type = 0x02 if to_controller else 0x03
        else:
            return
        now = time.time()
        sec, usec = int(now), int((now - int(now)) * 1e6)
        self.file.write(struct.pack('>IIIB', 9 + len(payload), sec, usec, pl_type) + payload)


def sink_codec_capabilities():
    """Endpoints a typical high-end headset offers (all sample rates / modes)."""
    sbc = SbcMediaCodecInformation
    aac = AacMediaCodecInformation

    def vendor(vendor_id, codec_id, value):
        return MediaCodecCapabilities(
            media_type=AVDTP_AUDIO_MEDIA_TYPE,
            media_codec_type=A2DP_NON_A2DP_CODEC_TYPE,
            media_codec_information=VendorSpecificMediaCodecInformation(
                vendor_id, codec_id, bytes(value)
            ),
        )

    # aptX family: high nibble 0x20 44.1 kHz | 0x10 48 kHz, low nibble 0x02 stereo
    aptx_freq_ch = 0x32
    return [
        MediaCodecCapabilities(
            media_type=AVDTP_AUDIO_MEDIA_TYPE,
            media_codec_type=A2DP_SBC_CODEC_TYPE,
            media_codec_information=sbc(
                sampling_frequency=sbc.SamplingFrequency.SF_48000
                | sbc.SamplingFrequency.SF_44100,
                channel_mode=sbc.ChannelMode.MONO
                | sbc.ChannelMode.DUAL_CHANNEL
                | sbc.ChannelMode.STEREO
                | sbc.ChannelMode.JOINT_STEREO,
                block_length=sbc.BlockLength.BL_4
                | sbc.BlockLength.BL_8
                | sbc.BlockLength.BL_12
                | sbc.BlockLength.BL_16,
                subbands=sbc.Subbands.S_4 | sbc.Subbands.S_8,
                allocation_method=sbc.AllocationMethod.LOUDNESS
                | sbc.AllocationMethod.SNR,
                minimum_bitpool_value=2,
                maximum_bitpool_value=53,
            ),
        ),
        MediaCodecCapabilities(
            media_type=AVDTP_AUDIO_MEDIA_TYPE,
            media_codec_type=A2DP_MPEG_2_4_AAC_CODEC_TYPE,
            media_codec_information=aac(
                object_type=aac.ObjectType.MPEG_2_AAC_LC | aac.ObjectType.MPEG_4_AAC_LC,
                sampling_frequency=aac.SamplingFrequency.SF_48000
                | aac.SamplingFrequency.SF_44100,
                channels=aac.Channels.MONO | aac.Channels.STEREO,
                vbr=1,
                bitrate=320000,
            ),
        ),
        vendor(0x0000004F, 0x0001, [aptx_freq_ch]),               # aptX
        vendor(0x000000D7, 0x0024, [aptx_freq_ch, 0, 0, 0, 0]),   # aptX HD
        vendor(0x0000000A, 0x0002, [aptx_freq_ch, 0x00]),         # aptX LL
        # LDAC: 44.1 / 48 / 88.2 / 96 kHz, mono / dual / stereo
        vendor(0x0000012D, 0x00AA, [0x3C, 0x07]),
    ]


async def main(args):
    link = LocalLink()
    transport = await open_transport(f'tcp-server:127.0.0.1:{args.port}')
    EmuController(
        'A2DPWB',
        host_source=transport.source,
        host_sink=transport.sink,
        link=link,
        public_address=A2DPWB_ADDRESS,
    )
    sink_controller = EmuController('SINK', link=link, public_address=SINK_ADDRESS)
    host = Host(sink_controller, AsyncPipeSink(sink_controller))
    if args.capture:
        host.snooper = PacketLoggerSnooper(args.capture)
    device = Device(name='A2DPWB Test Sink', address=SINK_ADDRESS, host=host)
    device.classic_enabled = True
    device.class_of_device = 0x240404  # Audio / Video, wearable headset
    device.sdp_service_records = {
        0x00010001: make_audio_sink_service_sdp_records(0x00010001)
    }
    await device.power_on()
    await device.set_discoverable(True)
    await device.set_connectable(True)

    def on_avdtp_connection(server):
        log.info('AVDTP connection')
        for capabilities in sink_codec_capabilities():
            sink = server.add_sink(capabilities)
            # Media is checked from the HCI capture with a2dpwb_decode. Bumble
            # would parse every packet as RTP, but classic aptX / aptX LL carry
            # no RTP header, so the parser is bypassed.
            sink.on_avdtp_packet = lambda packet: None

    Listener.for_device(device).on('connection', on_avdtp_connection)
    device.on('connection', lambda c: log.info('ACL connection from %s', c.peer_address))
    print(f'READY 127.0.0.1:{args.port} sink={SINK_ADDRESS}', flush=True)
    await asyncio.get_running_loop().create_future()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--port', type=int, default=9001, help='TCP port for A2DPWB (H4)')
    parser.add_argument('--capture', help='PacketLogger capture of the sink HCI traffic')
    parser.add_argument('--log-level', default='WARNING')
    bumble.logging.setup_basic_logging(parser.parse_known_args()[0].log_level)
    asyncio.run(main(parser.parse_args()))
