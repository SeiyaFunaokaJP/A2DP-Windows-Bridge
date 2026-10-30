"""Virtual Bluetooth link with an A2DP test sink, for testing BlueCaster without hardware.

Runs two Bumble virtual controllers on one in-process link:
  - controller "BlueCaster" is served as H4 over TCP; BlueCaster connects to it with
    `bluecaster-core.exe --cli --hci-tcp 127.0.0.1:<port>`
  - controller "SINK" is driven by a Bumble host acting as an A2DP sink that
    offers SBC (SBC XQ capable), AAC, aptX, aptX HD, aptX LL, LDAC, LHDC V5,
    FastStream, Opus, Opus 05 and aptX Adaptive like a typical headset

The sink's HCI traffic is written to a PacketLogger (.pklg) capture, so
bluecaster_decode --received can check what actually arrived at the sink.

Run with the Python of the tools/emu virtual environment (see setup_env.py).
Prints "READY" once BlueCaster can connect; stops on SIGTERM / Ctrl+C.
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
from bumble.avdtp import (
    AVDTP_AUDIO_MEDIA_TYPE,
    AVDTP_MEDIA_CODEC_SERVICE_CATEGORY,
    AVDTP_UNSUPPORTED_CONFIGURATION_ERROR,
    Discover_Response,
    EndPointInfo,
    Listener,
    MediaCodecCapabilities,
    Set_Configuration_Reject,
)
from bumble.controller import Controller
from bumble.device import Device
from bumble.host import Host
from bumble.link import LocalLink
from bumble.snoop import Snooper
from bumble.transport import open_transport
from bumble.transport.common import AsyncPipeSink

BLUECASTER_ADDRESS = '00:A2:D0:00:00:01'
SINK_ADDRESS = '00:A2:D0:00:00:02'

log = logging.getLogger('emu_sink')


# A BR/EDR + LE dual-mode controller's features. Bumble's default controller
# is LE only (BR/EDR Not Supported): BTstack then never inquires.
DUAL_MODE_FEATURES = (
    hci.LmpFeatureMask.LMP_3_SLOT_PACKETS
    | hci.LmpFeatureMask.LMP_5_SLOT_PACKETS
    | hci.LmpFeatureMask.ENCRYPTION
    | hci.LmpFeatureMask.ROLE_SWITCH
    | hci.LmpFeatureMask.SNIFF_MODE
    | hci.LmpFeatureMask.ENHANCED_DATA_RATE_ACL_2_MBPS_MODE
    | hci.LmpFeatureMask.ENHANCED_DATA_RATE_ACL_3_MBPS_MODE
    | hci.LmpFeatureMask.LMP_3_SLOT_ENHANCED_DATA_RATE_ACL_PACKETS
    | hci.LmpFeatureMask.LMP_5_SLOT_ENHANCED_DATA_RATE_ACL_PACKETS
    | hci.LmpFeatureMask.LE_SUPPORTED_CONTROLLER
    | hci.LmpFeatureMask.SIMULTANEOUS_LE_AND_BR_EDR_TO_SAME_DEVICE_CAPABLE_CONTROLLER
    | hci.LmpFeatureMask.SECURE_SIMPLE_PAIRING_CONTROLLER_SUPPORT
    | hci.LmpFeatureMask.EXTENDED_INQUIRY_RESPONSE
    | hci.LmpFeatureMask.EXTENDED_FEATURES
)


def strip_names(eir):
    """The EIR without its (shortened or complete) local name."""
    out, i = b'', 0
    while i < len(eir) and eir[i]:
        length = eir[i]
        if eir[i + 1] not in (0x08, 0x09):
            out += eir[i:i + 1 + length]
        i += 1 + length
    return out


class EmuController(Controller):
    """Bumble virtual controller plus the classic HCI commands BTstack needs.

    Bumble's controller has no LMP encryption. On a virtual link encryption is
    only bookkeeping, so it is reported as enabled on both ends.
    """

    # BR/EDR ACL buffers like a real USB adapter (e.g. Realtek RTL8761B:
    # 1021 bytes); Bumble's default of 27 bytes is the LE minimum.
    acl_data_packet_length = 1021
    total_num_acl_data_packets = 8

    # Inquiry, which Bumble's controller lacks: every other controller of the
    # link with inquiry scan on answers, the first time after
    # INQUIRY_FIRST_ANSWER and then again every INQUIRY_REPEAT (a real
    # controller hears a device many times per inquiry); the format follows
    # the inquiry mode the host wrote
    INQUIRY_FIRST_ANSWER = 1.0
    INQUIRY_REPEAT = 2.0
    class_of_device = 0
    inquiry_mode = 0
    eir = b''
    # False: the EIR it answers with leaves the name out (a name request
    # gets it)
    eir_name = True
    inquiry_task = None

    def on_hci_write_class_of_device_command(self, command):
        self.class_of_device = command.class_of_device
        return hci.HCI_StatusReturnParameters(hci.HCI_ErrorCode.SUCCESS)

    def on_hci_write_extended_inquiry_response_command(self, command):
        self.eir = bytes(command.extended_inquiry_response)
        return hci.HCI_StatusReturnParameters(hci.HCI_ErrorCode.SUCCESS)

    def on_hci_write_inquiry_mode_command(self, command):
        self.inquiry_mode = command.inquiry_mode
        return hci.HCI_StatusReturnParameters(hci.HCI_ErrorCode.SUCCESS)

    def on_hci_inquiry_command(self, command):
        if self.inquiry_task is not None:
            self._send_hci_command_status(hci.HCI_ErrorCode.COMMAND_DISALLOWED_ERROR, command.op_code)
            return None
        self._send_hci_command_status(hci.HCI_COMMAND_STATUS_PENDING, command.op_code)
        log.info('inquiry for %.2f s (mode %d)', command.inquiry_length * 1.28, self.inquiry_mode)
        self.inquiry_task = asyncio.get_running_loop().create_task(
            self.run_inquiry(command.inquiry_length * 1.28))
        return None

    def on_hci_inquiry_cancel_command(self, _command):
        # No Inquiry Complete after a cancel (Core Vol 4 Part E 7.1.2)
        if self.inquiry_task is not None:
            self.inquiry_task.cancel()
            self.inquiry_task = None
        return hci.HCI_StatusReturnParameters(hci.HCI_ErrorCode.SUCCESS)

    async def run_inquiry(self, length):
        loop = asyncio.get_running_loop()
        end = loop.time() + length
        await asyncio.sleep(min(self.INQUIRY_FIRST_ANSWER, length))
        while loop.time() < end:
            for peer in list(self.link.controllers):
                if peer is not self and isinstance(peer, EmuController) and peer.classic_scan_enable & 0x01:
                    self.send_hci_packet(self.inquiry_result(peer))
            await asyncio.sleep(max(0.0, min(self.INQUIRY_REPEAT, end - loop.time())))
        self.inquiry_task = None
        self.send_hci_packet(hci.HCI_Inquiry_Complete_Event(status=hci.HCI_ErrorCode.SUCCESS))

    def inquiry_result(self, peer):
        address, cod, psrm, clock, rssi = peer.public_address, peer.class_of_device, 1, 0x1234, -55
        if self.inquiry_mode == 2:
            eir = peer.eir if peer.eir_name else strip_names(peer.eir)
            return hci.HCI_Extended_Inquiry_Result_Event(
                num_responses=1, bd_addr=address, page_scan_repetition_mode=psrm, reserved=0,
                class_of_device=cod, clock_offset=clock, rssi=rssi,
                extended_inquiry_response=eir.ljust(240, b'\0')[:240])
        if self.inquiry_mode == 1:
            return hci.HCI_Inquiry_Result_With_RSSI_Event(
                bd_addr=[address], page_scan_repetition_mode=[psrm], reserved=[0],
                class_of_device=[cod], clock_offset=[clock], rssi=[rssi])
        return hci.HCI_Inquiry_Result_Event(
            bd_addr=[address], page_scan_repetition_mode=[psrm], reserved_0=[0], reserved_1=[0],
            class_of_device=[cod], clock_offset=[clock])

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
    """Writes HCI packets in PacketLogger format (as BlueCaster's own capture)."""

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


# Codec of each sink_codec_capabilities() entry, in order (for --in-use)
CODEC_NAMES = ['sbc', 'aac', 'aptx', 'aptxhd', 'aptxll', 'ldac', 'lhdcv5', 'faststream', 'opus', 'opus05', 'aptxad']

# aptX Adaptive as the B&W Px7 S2e lists it (AKS-Logic/aptx-adaptive-linux
# spec/px7s2e-a2dp-caps.md), after the vendor header: an R1 sink, 48 + 44.1
# kHz with its own flag 0x01, stereo + joint stereo, its target latencies (HQ
# 0), the R1 end marker; 40 bytes in all
APTXAD_R1 = [0x19, 0x0A, 0x37, 0x6A, 0x00, 0xAF, 0x64, 0x4B, 0x00, 0x00, 0xAA] + [0] * 23
# An R2 sink: capability extension version 1, the R2.1 features word
# 0x0F000003, setup preferences, the end marker at 24..25
APTXAD_R2 = ([0x39, 0x0A, 0x37, 0x6A, 0x00, 0xAF, 0x64, 0x4B, 0x00, 0x01, 0x03, 0x00, 0x00, 0x0F,
              0x02, 0x03, 0x03, 0x03, 0x00, 0xAA] + [0] * 14)


def sink_codec_capabilities(sbc_dual=True, sbc_max_bitpool=53, aptxad_r2=False, aptxad_rates=None):
    """Endpoints a typical high-end headset offers (all sample rates / modes).
    SBC allows SBC XQ (dual channel, bitpool up to 53) unless sbc_dual is
    False; aptX Adaptive is an R1 sink's unless aptxad_r2, with its rate
    byte (bits 7..3 of byte 6) replaced by aptxad_rates when given."""
    aptxad = list(APTXAD_R2 if aptxad_r2 else APTXAD_R1)
    if aptxad_rates is not None:
        aptxad[0] = (aptxad_rates & 0xF8) | (aptxad[0] & 0x07)
    sbc = SbcMediaCodecInformation
    sbc_modes = sbc.ChannelMode.MONO | sbc.ChannelMode.STEREO | sbc.ChannelMode.JOINT_STEREO
    if sbc_dual:
        sbc_modes |= sbc.ChannelMode.DUAL_CHANNEL
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
                channel_mode=sbc_modes,
                block_length=sbc.BlockLength.BL_4
                | sbc.BlockLength.BL_8
                | sbc.BlockLength.BL_12
                | sbc.BlockLength.BL_16,
                subbands=sbc.Subbands.S_4 | sbc.Subbands.S_8,
                allocation_method=sbc.AllocationMethod.LOUDNESS
                | sbc.AllocationMethod.SNR,
                minimum_bitpool_value=2,
                maximum_bitpool_value=sbc_max_bitpool,
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
        # LHDC V5 (Android a2dp_vendor_lhdcv5_constants.h): 44.1 / 48 / 96 / 192 kHz,
        # 16 / 24-bit, 64..1000 kbps, V5 ver.1 with 5 ms frames, low latency offered
        vendor(0x0000053A, 0x4C35, [0x35, 0x06, 0x11, 0x40, 0x00]),
        # FastStream (PipeWire a2dp_faststream_t) as a CSR headset lists it:
        # music (sink) and voice (source) directions, music at 44.1 / 48 kHz,
        # voice at 16 kHz
        vendor(0x0000000A, 0x0001, [0x03, 0x23]),
        # Opus (Android a2dp_vendor_opus_constants.h): Android's own sink
        # capabilities, 48 kHz (0x80), 20 ms frames (0x10), stereo (0x02)
        vendor(0x000000E0, 0x0001, [0x92]),
        # Opus 05 (PipeWire a2dp-codec-opus.c, its opus_05 sink SEP): main
        # direction stereo at front left + right (0x00000003), every frame
        # duration (0x1F), no bit rate limit; no return channel
        vendor(0x000005F1, 0x1005, [2, 0, 0x03, 0, 0, 0, 0x1F, 0, 0] + [0] * 9),
        vendor(0x000000D7, 0x00AD, aptxad),
    ]


def refuse_dual_channel(sink):
    """The SBC endpoint lists dual channel but rejects a SET_CONFIGURATION
    with it, as some headsets do (SBC XQ then has to fall back to SBC)."""
    accept = sink.on_set_configuration_command

    async def on_set_configuration_command(configuration):
        for capability in configuration:
            if (isinstance(capability, MediaCodecCapabilities)
                    and capability.media_codec_type == A2DP_SBC_CODEC_TYPE
                    and bytes(capability.media_codec_information)[0] & 0x0F
                    == SbcMediaCodecInformation.ChannelMode.DUAL_CHANNEL):
                log.info('refusing a dual channel SBC configuration')
                return Set_Configuration_Reject(
                    service_category=AVDTP_MEDIA_CODEC_SERVICE_CATEGORY,
                    error_code=AVDTP_UNSUPPORTED_CONFIGURATION_ERROR,
                )
        return await accept(configuration)

    sink.on_set_configuration_command = on_set_configuration_command


async def main(args):
    link = LocalLink()
    transport = await open_transport(f'tcp-server:127.0.0.1:{args.port}')
    bluecaster_controller = EmuController(
        'BlueCaster',
        host_source=transport.source,
        host_sink=transport.sink,
        link=link,
        public_address=BLUECASTER_ADDRESS,
    )
    if args.dual_mode:
        bluecaster_controller.lmp_features = DUAL_MODE_FEATURES
    sink_controller = EmuController('SINK', link=link, public_address=SINK_ADDRESS)
    sink_controller.eir_name = not args.no_eir_name
    host = Host(sink_controller, AsyncPipeSink(sink_controller))
    if args.capture:
        host.snooper = PacketLoggerSnooper(args.capture)
    device = Device(name='BlueCaster Test Sink', address=SINK_ADDRESS, host=host)
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
        for _ in range(args.extra_seps):
            # A codec BlueCaster does not know, discovered before the real ones
            server.add_sink(MediaCodecCapabilities(
                media_type=AVDTP_AUDIO_MEDIA_TYPE,
                media_codec_type=A2DP_NON_A2DP_CODEC_TYPE,
                media_codec_information=VendorSpecificMediaCodecInformation(0x0000FFFE, 0x0001, bytes(1)),
            ))
        for name, capabilities in zip(CODEC_NAMES, sink_codec_capabilities(not args.sbc_no_dual, args.sbc_max_bitpool,
                                                                         args.aptxad_r2, args.aptxad_rates)):
            if args.codecs and name not in args.codecs:
                continue
            # BTstack 1.8.1 keeps at most 10 discovered SEPs (a2dp.c
            # AVDTP_MAX_SEP_NUM) and overflows its array beyond that: aptX
            # Adaptive, the 11th, only when asked for with --codecs
            if not args.codecs and name == 'aptxad':
                continue
            sink = server.add_sink(capabilities)
            if name == 'sbc' and args.sbc_reject_dual:
                refuse_dual_channel(sink)
            if name in args.in_use:
                # As a multipoint headset streaming this codec to another
                # device: listed by Discover with in_use = 1
                sink.__class__ = type('InUseSink', (sink.__class__,), {'in_use': property(lambda self: 1)})
        if args.in_use:
            # Bumble's Discover response always reports in_use = 0
            async def on_discover_command(command):
                return Discover_Response([
                    EndPointInfo(e.seid, e.in_use, e.media_type, e.tsep) for e in server.local_endpoints
                ])
            server.on_discover_command = on_discover_command
            # Media is checked from the HCI capture with bluecaster_decode. Bumble
            # would parse every packet as RTP, but classic aptX / aptX LL carry
            # no RTP header, so the parser is bypassed.
            sink.on_avdtp_packet = lambda packet: None

    Listener.for_device(device).on('connection', on_avdtp_connection)
    device.on('connection', lambda c: log.info('ACL connection from %s', c.peer_address))
    print(f'READY 127.0.0.1:{args.port} sink={SINK_ADDRESS}', flush=True)
    await asyncio.get_running_loop().create_future()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--port', type=int, default=9001, help='TCP port for BlueCaster (H4)')
    parser.add_argument('--capture', help='PacketLogger capture of the sink HCI traffic')
    parser.add_argument('--log-level', default='WARNING')
    parser.add_argument('--no-eir-name', action='store_true',
                        help='answer inquiries without the name in the EIR (BlueCaster asks for it)')
    parser.add_argument('--dual-mode', action='store_true',
                        help="report BR/EDR support on BlueCaster's controller (BTstack inquires only then)")
    parser.add_argument('--in-use', nargs='*', default=[], choices=CODEC_NAMES, metavar='CODEC',
                        help='report the SEP of these codecs as in use by another device (multipoint)')
    parser.add_argument('--codecs', nargs='+', choices=CODEC_NAMES, metavar='CODEC',
                        help='offer only these codecs (default: all)')
    parser.add_argument('--extra-seps', type=int, default=0, metavar='N',
                        help='list N more sink SEPs (an unknown vendor codec) before the codecs')
    parser.add_argument('--sbc-no-dual', action='store_true',
                        help='SBC without dual channel (no SBC XQ)')
    parser.add_argument('--sbc-reject-dual', action='store_true',
                        help='list dual channel but reject a configuration with it')
    parser.add_argument('--sbc-max-bitpool', type=int, default=53,
                        help='largest SBC bitpool the sink takes (default 53)')
    parser.add_argument('--aptxad-r2', action='store_true',
                        help='aptX Adaptive as an R2 sink lists it (default: R1, the B&W Px7 S2e)')
    parser.add_argument('--aptxad-rates', type=lambda x: int(x, 0),
                        help='aptX Adaptive rate byte to list instead (e.g. 0xD1: an EarFun Air Pro 4+, '
                             '96 kHz by its 0x80 bit only; 0x51: a Bose QC Ultra, no 96 kHz; issue #86)')
    bumble.logging.setup_basic_logging(parser.parse_known_args()[0].log_level)
    asyncio.run(main(parser.parse_args()))
