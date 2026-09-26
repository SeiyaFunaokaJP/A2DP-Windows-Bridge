"""Virtual Bluetooth link between A2DPWB and the Linux Bluetooth stack (BlueZ).

Linux only, as root (opens /dev/vhci). Runs two Bumble virtual controllers on
one in-process link:
  - controller "A2DPWB" is served as H4 over TCP; A2DPWB connects to it with
    `A2DPWB.exe --cli --hci-tcp <this host>:<port>` (from another PC too)
  - controller "LINUX" is attached to the kernel through /dev/vhci, so BlueZ
    sees it as a new adapter (hciN) and everything above it - bluetoothd,
    PipeWire, tools/linux_sink/a2dpwb_sink.py - runs unmodified

Lets tools/linux_sink be tested end to end without Bluetooth hardware.
--drop and --stall impair the AVDTP media packets A2DPWB sends (random loss,
periodic hold-and-burst like radio interference) to check that the receiver
reports them; signaling is never touched. --silent-links N makes the first N
connections go silent right after they come up (nothing A2DPWB sends gets
through), as radio links sometimes do, to check A2DPWB's connection retry.
--detach-after S makes the first streaming link end S seconds after media
started the way a lost radio link can end: A2DPWB's controller reports the
link terminated by the remote, the Linux side is never told.
Run with the Python of a virtual environment holding tools/emu/requirements.txt.
Prints "READY" once A2DPWB can connect; stops on SIGTERM / Ctrl+C (the hciN
adapter disappears with it). For an HCI capture of the Linux side use
`btmon -w <file>`.
"""

import argparse
import asyncio
import logging
import random
import struct

import bumble.logging
from bumble import hci, lmp
from bumble.link import LocalLink
from bumble.transport import open_transport

from emu_sink import A2DPWB_ADDRESS, EmuController

LINUX_ADDRESS = '00:A2:D0:00:00:03'

log = logging.getLogger('emu_vhci')


class LinuxController(EmuController):
    """The controller BlueZ drives: a BR/EDR + LE dual-mode adapter.

    Bumble's default controller is LE only (BR/EDR Not Supported), which makes
    BlueZ skip everything classic, A2DP included.
    """

    lmp_features = (
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

    # BR/EDR housekeeping commands the Linux kernel sends that Bumble does not
    # implement: opcode -> return parameters after the status byte. None
    # entries echo the connection handle (first two command parameter bytes).
    SIMPLE_REPLIES = {
        0x0C38: bytes([1]),                      # Read Number Of Supported IAC
        0x0C39: bytes([1, 0x33, 0x8B, 0x9E]),    # Read Current IAC LAP: GIAC
        0x0C3A: b'',                             # Write Current IAC LAP
        0x0C1B: bytes([0x00, 0x08, 0x12, 0x00]), # Read Page Scan Activity
        0x0C1C: b'',                             # Write Page Scan Activity
        0x0C1E: b'',                             # Write Inquiry Scan Activity
        0x0C46: bytes([0]),                      # Read Page Scan Type
        0x0C47: b'',                             # Write Page Scan Type
        0x0C45: b'',                             # Write Inquiry Mode
        0x0C58: bytes([4]),                      # Read Inquiry Response TX Power Level
        0x0C5A: bytes([0]),                      # Read Default Erroneous Data Reporting
        0x0C25: bytes([0x60, 0x00]),             # Read Voice Setting
        0x0C0D: bytes([0, 0, 0, 0]),             # Read Stored Link Key
        0x0C12: bytes([0, 0]),                   # Delete Stored Link Key
        0x0C18: b'',                             # Write Page Timeout
        0x0C16: b'',                             # Write Connection Accept Timeout
        0x0C05: b'',                             # Set Event Filter
        0x080F: b'',                             # Write Default Link Policy Settings
        0x080D: None,                            # Write Link Policy Settings
        0x0C37: None,                            # Write Link Supervision Timeout
        0x0C2D: (None, bytes([4])),              # Read Transmit Power Level: 4 dBm
        0x1405: (None, bytes([0x00])),           # Read RSSI: 0 = within the golden range
    }

    # Impairment of media packets from A2DPWB (set from the command line)
    drop_fraction = 0.0
    stall = None          # (hold seconds, period seconds)
    silent_links = 0      # the first N connections carry nothing from A2DPWB
    detach_after = None   # seconds of media before the first link is cut
    AVDTP_PSM = 0x0019

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.held = []
        self.flush_pending = False
        self.t0 = None
        self.dropped = self.held_total = 0
        # L2CAP channels on the AVDTP PSM by their CID on this (Linux) side:
        # the first one open is signaling, the next ones media
        self.pending_requests = {}
        self.avdtp_cids = []
        self.connections_seen = 0
        self.silent = False
        self.detach_scheduled = False

    def on_classic_connection_request(self, peer_address, link_type):
        self.connections_seen += 1
        self.silent = self.connections_seen <= self.silent_links
        if self.silent:
            log.warning('connection %d: silent (--silent-links)', self.connections_seen)
        super().on_classic_connection_request(peer_address, link_type)

    def _on_l2cap_signaling(self, pdu, from_linux):
        """Follows connection setup / teardown on the L2CAP signaling channel
        to know which CIDs carry AVDTP media."""
        off = 4
        while off + 4 <= len(pdu):
            code, ident = pdu[off], pdu[off + 1]
            length = struct.unpack_from('<H', pdu, off + 2)[0]
            body = pdu[off + 4:off + 4 + length]
            if code == 0x02 and len(body) >= 4:    # Connection Request: PSM, source CID
                self.pending_requests[(from_linux, ident)] = struct.unpack_from('<HH', body)
            elif code == 0x03 and len(body) >= 6:  # Connection Response: dest CID, source CID, result
                dcid, scid, result = struct.unpack_from('<HHH', body)
                if result == 1:  # pending: the final response follows
                    off += 4 + length
                    continue
                request = self.pending_requests.pop((not from_linux, ident), None)
                if request and request[0] == self.AVDTP_PSM and result == 0:
                    self.avdtp_cids.append(dcid if from_linux else scid)
                    log.info('AVDTP channel open: CID 0x%04X (open: %s)', self.avdtp_cids[-1],
                             ', '.join(f'0x{c:04X}' for c in self.avdtp_cids))
            elif code == 0x06 and len(body) >= 4:  # Disconnection Request: dest CID, source CID
                dcid, scid = struct.unpack_from('<HH', body)
                cid = scid if from_linux else dcid
                if cid in self.avdtp_cids:
                    self.avdtp_cids.remove(cid)
            off += 4 + length

    def on_hci_acl_data_packet(self, packet):
        data = bytes(packet.data)
        if packet.pb_flag != 1 and len(data) >= 8 and struct.unpack_from('<H', data, 2)[0] == 1:
            self._on_l2cap_signaling(data, from_linux=True)
        super().on_hci_acl_data_packet(packet)

    def on_link_acl_data(self, sender_address, transport, data):
        if self.silent:
            return
        cid = struct.unpack_from('<H', data, 2)[0] if len(data) >= 4 else 0
        if cid == 1:
            self._on_l2cap_signaling(data, from_linux=False)
        if self.detach_after is not None and not self.detach_scheduled and cid in self.avdtp_cids[1:]:
            self.detach_scheduled = True
            asyncio.get_running_loop().call_later(self.detach_after, self._detach, sender_address)
        if (self.drop_fraction or self.stall) and cid in self.avdtp_cids[1:]:
            if random.random() < self.drop_fraction:
                self.dropped += 1
                return
            if self.stall:
                loop = asyncio.get_running_loop()
                now = loop.time()
                if self.t0 is None:
                    self.t0 = now
                hold, period = self.stall
                phase = (now - self.t0) % period
                # hold during the last `hold` seconds of every period
                if phase >= period - hold or self.held:
                    self.held.append((sender_address, transport, data))
                    self.held_total += 1
                    if not self.flush_pending:
                        self.flush_pending = True
                        release = now + (period - phase) if phase >= period - hold else now
                        loop.call_at(release, self._flush)
                    return
        super().on_link_acl_data(sender_address, transport, data)

    def _detach(self, peer_address):
        log.warning('cutting the link (--detach-after): A2DPWB is told, Linux is not')
        self.send_lmp_packet(peer_address, lmp.LmpDetach(hci.HCI_ErrorCode.REMOTE_USER_TERMINATED_CONNECTION_ERROR))
        self.silent = True  # nothing more gets through on the stale link

    def _flush(self):
        held, self.held = self.held, []
        self.flush_pending = False
        for args in held:
            super().on_link_acl_data(*args)

    def send_hci_packet(self, packet):
        # Bumble's SSP emulation always yields an authenticated P-256 link key,
        # which Linux only accepts with AES-CCM encryption (security level 4);
        # E0 makes the kernel drop the link with Authentication Failure.
        if isinstance(packet, hci.HCI_Encryption_Change_Event) and packet.encryption_enabled == 1:
            packet = hci.HCI_Encryption_Change_Event(
                status=packet.status, connection_handle=packet.connection_handle, encryption_enabled=2)
        super().send_hci_packet(packet)

    def _send_event(self, code, params):
        asyncio.get_running_loop().call_soon(
            self.host.on_packet, bytes([hci.HCI_EVENT_PACKET, code, len(params)]) + params)

    def _command_status(self, command, status):
        self._send_event(hci.HCI_COMMAND_STATUS_EVENT,
                         bytes([status, 1]) + command.op_code.to_bytes(2, 'little'))

    def on_hci_command_packet(self, command):
        handler = getattr(self, f'on_{command.name.lower()}', None)
        reply = self.SIMPLE_REPLIES.get(command.op_code, False)
        if handler is None and command.op_code == hci.HCI_READ_CLOCK_OFFSET_COMMAND:
            self._command_status(command, hci.HCI_COMMAND_STATUS_PENDING)
            self._send_event(hci.HCI_READ_CLOCK_OFFSET_COMPLETE_EVENT,
                             bytes([0]) + bytes(command.parameters[:2]) + bytes(2))
            return
        if handler is None and reply is False and not isinstance(command, hci.HCI_SyncCommand):
            # Bumble answers nothing to an unknown asynchronous command, which
            # stalls the kernel's command queue
            log.warning('unsupported command %s', command.name)
            self._command_status(command, hci.HCI_ErrorCode.UNKNOWN_HCI_COMMAND_ERROR)
            return
        if handler is not None or reply is False:
            super().on_hci_command_packet(command)
            return
        if reply is None:
            reply = bytes(command.parameters[:2])
        elif isinstance(reply, tuple):
            reply = bytes(command.parameters[:2]) + reply[1]
        self._send_event(hci.HCI_COMMAND_COMPLETE_EVENT,
                         bytes([1]) + command.op_code.to_bytes(2, 'little') + bytes([0]) + reply)


async def main(args):
    link = LocalLink()
    a2dpwb_transport = await open_transport(f'tcp-server:{args.bind}:{args.port}')
    EmuController(
        'A2DPWB',
        host_source=a2dpwb_transport.source,
        host_sink=a2dpwb_transport.sink,
        link=link,
        public_address=A2DPWB_ADDRESS,
    )
    vhci_transport = await open_transport('vhci')
    LinuxController.drop_fraction = args.drop / 100.0
    LinuxController.silent_links = args.silent_links
    LinuxController.detach_after = args.detach_after
    if args.stall:
        hold_ms, period_s = args.stall.split('/')
        LinuxController.stall = (int(hold_ms) / 1000.0, float(period_s))
    linux = LinuxController(
        'LINUX',
        host_source=vhci_transport.source,
        host_sink=vhci_transport.sink,
        link=link,
        public_address=LINUX_ADDRESS,
    )
    print(f'READY {args.bind}:{args.port} linux={LINUX_ADDRESS}', flush=True)
    try:
        await asyncio.get_running_loop().create_future()
    finally:
        if args.drop or args.stall:
            print(f'impairment: {linux.dropped} media packets dropped, '
                  f'{linux.held_total} held back', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--bind', default='0.0.0.0', help='address the H4 TCP port listens on')
    parser.add_argument('--port', type=int, default=9001, help='TCP port for A2DPWB (H4)')
    parser.add_argument('--drop', type=float, default=0.0, metavar='PERCENT',
                        help='drop this share of the media packets A2DPWB sends')
    parser.add_argument('--stall', metavar='MS/SECONDS',
                        help='hold media packets back for MS milliseconds every SECONDS seconds, '
                             'then deliver them at once (e.g. 300/5)')
    parser.add_argument('--silent-links', type=int, default=0, metavar='N',
                        help='the first N connections carry nothing from A2DPWB after they come up')
    parser.add_argument('--detach-after', type=float, metavar='SECONDS',
                        help='cut the first streaming link after this much media, telling only A2DPWB')
    parser.add_argument('--log-level', default='WARNING')
    bumble.logging.setup_basic_logging(parser.parse_known_args()[0].log_level)
    asyncio.run(main(parser.parse_args()))
