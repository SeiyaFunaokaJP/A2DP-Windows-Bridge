"""Auracast (BAP broadcast) in the emulated controllers.

Bumble's virtual controller has no BIG support and ignores periodic
advertising. BroadcastController adds both roles on the in-process link:

  Broadcaster (A2DPWB's controller):
  - extended advertising sets advertise with their SID and properties
    (ExtAdvPdu), periodic advertising keeps its parameters and data
    (fragments joined) per set
  - LE Create BIG: Command Status, then LE Create BIG Complete with BIS
    handles and parameters derived from the request (BN 1, IRC RTN + 1,
    NSE IRC, PTO 0, ISO interval = SDU interval, max PDU = max SDU)
  - LE Setup / Remove ISO Data Path on the BIS handles
  - HCI ISO data on a BIS: one SDU per SDU interval with Number Of Completed
    Packets, the same pacing and "late SDU" rule as a CIS
    (LeEmuController.schedule_iso_sdu), delivered to every receiver synced
    to that BIS
  - LE Terminate BIG: LE Terminate BIG Complete, receivers get BIG Sync Lost

  Synchronized receiver (the broadcast sink's controller):
  - LE Set Extended Scan Parameters / Enable, LE Extended Advertising
    Reports of the extended advertising sets on the link (SID, periodic
    advertising interval, non-connectable event type)
  - LE Periodic Advertising Create Sync (Cancel, Terminate Sync): Sync
    Established once the set does periodic advertising, then Periodic
    Advertising Reports and, with a BIG on the set, BIGInfo Advertising
    Reports every periodic interval (at least PA_EVENT_MIN_S); Sync Lost when
    the set stops
  - LE BIG Create Sync: BIG Sync Established with BIS handles, or with
    Connection Terminated due to MIC Failure when the BIG is encrypted and
    the Broadcast_Code differs (no audio then); LE BIG Terminate Sync

Both report the LE features and supported commands for this (BTstack uses
extended advertising when LE Set Extended Advertising Enable is listed).
"""

import asyncio
import dataclasses
import logging

from bumble import hci, ll
from bumble.controller import AdvertisingSet

from .controller import LeEmuController

log = logging.getLogger('emu_le_audio')

FEATURES = (hci.LeFeatureMask.LE_EXTENDED_ADVERTISING
            | hci.LeFeatureMask.LE_PERIODIC_ADVERTISING
            | hci.LeFeatureMask.CONNECTED_ISOCHRONOUS_STREAM_CENTRAL
            | hci.LeFeatureMask.CONNECTED_ISOCHRONOUS_STREAM_PERIPHERAL
            | hci.LeFeatureMask.ISOCHRONOUS_BROADCASTER
            | hci.LeFeatureMask.SYNCHRONIZED_RECEIVER)
COMMANDS = {
    hci.HCI_LE_SET_ADVERTISING_SET_RANDOM_ADDRESS_COMMAND,
    hci.HCI_LE_SET_EXTENDED_ADVERTISING_PARAMETERS_COMMAND,
    hci.HCI_LE_SET_EXTENDED_ADVERTISING_DATA_COMMAND,
    hci.HCI_LE_SET_EXTENDED_SCAN_RESPONSE_DATA_COMMAND,
    hci.HCI_LE_SET_EXTENDED_ADVERTISING_ENABLE_COMMAND,
    hci.HCI_LE_READ_MAXIMUM_ADVERTISING_DATA_LENGTH_COMMAND,
    hci.HCI_LE_READ_NUMBER_OF_SUPPORTED_ADVERTISING_SETS_COMMAND,
    hci.HCI_LE_REMOVE_ADVERTISING_SET_COMMAND,
    hci.HCI_LE_CLEAR_ADVERTISING_SETS_COMMAND,
    hci.HCI_LE_SET_PERIODIC_ADVERTISING_PARAMETERS_COMMAND,
    hci.HCI_LE_SET_PERIODIC_ADVERTISING_DATA_COMMAND,
    hci.HCI_LE_SET_PERIODIC_ADVERTISING_ENABLE_COMMAND,
    hci.HCI_LE_SET_EXTENDED_SCAN_PARAMETERS_COMMAND,
    hci.HCI_LE_SET_EXTENDED_SCAN_ENABLE_COMMAND,
    hci.HCI_LE_PERIODIC_ADVERTISING_CREATE_SYNC_COMMAND,
    hci.HCI_LE_PERIODIC_ADVERTISING_CREATE_SYNC_CANCEL_COMMAND,
    hci.HCI_LE_PERIODIC_ADVERTISING_TERMINATE_SYNC_COMMAND,
    hci.HCI_LE_SET_CIG_PARAMETERS_COMMAND,
    hci.HCI_LE_CREATE_CIS_COMMAND,
    hci.HCI_LE_REMOVE_CIG_COMMAND,
    hci.HCI_LE_ACCEPT_CIS_REQUEST_COMMAND,
    hci.HCI_LE_CREATE_BIG_COMMAND,
    hci.HCI_LE_TERMINATE_BIG_COMMAND,
    hci.HCI_LE_BIG_CREATE_SYNC_COMMAND,
    hci.HCI_LE_BIG_TERMINATE_SYNC_COMMAND,
    hci.HCI_LE_SETUP_ISO_DATA_PATH_COMMAND,
    hci.HCI_LE_REMOVE_ISO_DATA_PATH_COMMAND,
}

PA_EVENT_MIN_S = 0.1        # periodic advertising reports at most this often
SYNC_LOST_MAX_S = 1.0       # Sync Lost after the sync timeout, at most this
EXT_REPORT_DATA_MAX = 229   # data per LE Extended Advertising Report
PA_REPORT_DATA_MAX = 247    # data per LE Periodic Advertising Report
BIS_HANDLE_BASE = 0x0100    # BIS handles: 0x0100, 0x0101, ...
TX_POWER_NONE = 0x7F

SUCCESS = hci.HCI_ErrorCode.SUCCESS
UNKNOWN_ADVERTISING_IDENTIFIER = 0x42
COMMAND_DISALLOWED = hci.HCI_ErrorCode.COMMAND_DISALLOWED_ERROR
INVALID_PARAMETERS = hci.HCI_ErrorCode.INVALID_COMMAND_PARAMETERS_ERROR
ENCRYPTION_MODE_NOT_ACCEPTABLE = 0x25
MIC_FAILURE = 0x3D
OPERATION_CANCELLED_BY_HOST = 0x44
REMOTE_USER_TERMINATED = hci.HCI_ErrorCode.REMOTE_USER_TERMINATED_CONNECTION_ERROR
LOCAL_HOST_TERMINATED = 0x16


def code_text(code):
    """A Broadcast_Code as the string it holds (UTF-8, zero padded), else hex"""
    text = bytes(code).rstrip(b'\0')
    try:
        decoded = text.decode('utf-8')
        if decoded.isprintable():
            return repr(decoded)
    except UnicodeDecodeError:
        pass
    return bytes(code).hex()


def phy_name(phy):
    return {1: '1M', 2: '2M', 3: 'Coded'}.get(phy, str(phy))


@dataclasses.dataclass
class ExtAdvPdu(ll.AdvExtInd):
    """An extended advertising event of a set (not a legacy PDU)"""
    sid: int = 0
    properties: int = 0
    source: object = None      # the advertising controller
    handle: int = 0            # its advertising handle


class EmuAdvertisingSet(AdvertisingSet):
    """Bumble's advertising set, sending ExtAdvPdu unless it uses legacy PDUs"""

    def send_extended_advertising_data(self):
        p = self.parameters
        properties = int(p.advertising_event_properties) if p else 0
        if not self.controller.link or properties & 0x10:
            return super().send_extended_advertising_data()
        self.controller.send_advertising_pdu(ExtAdvPdu(
            advertiser_address=self.address, data=bytes(self.data), sid=p.advertising_sid,
            properties=properties, source=self.controller, handle=self.handle))


@dataclasses.dataclass
class PeriodicAdv:
    interval: int = 0           # units of 1.25 ms (Periodic_Advertising_Interval_Max)
    properties: int = 0
    data: bytearray = dataclasses.field(default_factory=bytearray)
    enabled: bool = False
    timer: object = None


@dataclasses.dataclass
class Big:
    handle: int
    adv_handle: int
    command: object             # HCI_LE_Create_BIG_Command
    bis_handles: list           # BIS index i + 1 -> handle
    phy: int
    nse: int
    bn: int
    pto: int
    irc: int
    max_pdu: int
    iso_interval: int           # units of 1.25 ms
    sync_delay: int             # us
    transport_latency: int      # us
    data_paths: set = dataclasses.field(default_factory=set)
    sdus: dict = dataclasses.field(default_factory=dict)   # BIS index -> SDUs taken

    @property
    def encrypted(self):
        return bool(self.command.encryption)


@dataclasses.dataclass
class PaSync:
    handle: int
    source: object              # broadcaster controller
    adv_handle: int
    sid: int
    address: object
    timeout_s: float
    lost_timer: object = None


@dataclasses.dataclass
class BigSync:
    handle: int
    sync_handle: int
    command: object             # HCI_LE_BIG_Create_Sync_Command
    source: object = None
    source_big: int = None
    bis: dict = dataclasses.field(default_factory=dict)    # BIS index -> handle
    data_paths: set = dataclasses.field(default_factory=set)
    established: bool = False


class BroadcastController(LeEmuController):
    """LeEmuController with the broadcaster and synchronized receiver roles
    (see the module doc)."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.le_features = self.le_features | FEATURES
        self.supported_commands = set(self.supported_commands) | COMMANDS
        self.periodic = {}         # advertising handle -> PeriodicAdv
        self.bigs = {}             # BIG handle -> Big (broadcaster)
        self.bis_tx = {}           # BIS handle -> (BIG handle, BIS index)
        self.ext_scan_enable = False
        self.pa_pending = None     # LE Periodic Advertising Create Sync waiting for its set
        self.pa_syncs = {}         # sync handle -> PaSync
        self.next_sync_handle = 1
        self.big_syncs = {}        # BIG handle -> BigSync (receiver)
        self.bis_rx = {}           # BIS handle -> (BIG handle, BIS index)

    # ---- helpers ----

    def _status(self, command, status):
        self._send_hci_command_status(status, command.op_code)

    def _adv_set(self, handle):
        if handle not in self.advertising_sets:
            self.advertising_sets[handle] = EmuAdvertisingSet(controller=self, handle=handle)
        return self.advertising_sets[handle]

    def _free_iso_handle(self, used):
        handle = BIS_HANDLE_BASE
        while handle in used:
            handle += 1
        return handle

    def _receivers(self):
        """The broadcast controllers on the link but this one"""
        return [c for c in getattr(self.link, 'controllers', []) if c is not self and isinstance(c, BroadcastController)]

    def _syncs_of(self, adv_handle):
        """(receiver, PaSync) synced to one of this controller's sets"""
        return [(c, s) for c in self._receivers() for s in list(c.pa_syncs.values())
                if s.source is self and s.adv_handle == adv_handle]

    def _big_of_set(self, adv_handle):
        return next((b for b in self.bigs.values() if b.adv_handle == adv_handle), None)

    # ---- extended advertising (broadcaster) ----

    def on_hci_le_set_advertising_set_random_address_command(self, command):
        self._adv_set(command.advertising_handle)
        return super().on_hci_le_set_advertising_set_random_address_command(command)

    def on_hci_le_set_extended_advertising_parameters_command(self, command):
        self._adv_set(command.advertising_handle)
        log.info('%s: advertising set %d: properties 0x%02X, SID %d, interval %.1f-%.1f ms, own address type %d',
                 self.name, command.advertising_handle, int(command.advertising_event_properties),
                 command.advertising_sid, command.primary_advertising_interval_min * 0.625,
                 command.primary_advertising_interval_max * 0.625, command.own_address_type)
        return super().on_hci_le_set_extended_advertising_parameters_command(command)

    def on_hci_le_set_extended_advertising_enable_command(self, command):
        handles = list(command.advertising_handles) or list(self.advertising_sets)
        log.info('%s: extended advertising %s: set(s) %s', self.name,
                 'enabled' if command.enable else 'disabled', ', '.join(str(h) for h in handles) or 'all')
        result = super().on_hci_le_set_extended_advertising_enable_command(command)
        if command.enable:
            # A receiver waiting to sync to one of these sets
            for c in self._receivers():
                c._try_pending_sync()
        return result

    def on_hci_le_remove_advertising_set_command(self, command):
        handle = command.advertising_handle
        if self._big_of_set(handle) is not None:
            return hci.HCI_StatusReturnParameters(COMMAND_DISALLOWED)
        result = super().on_hci_le_remove_advertising_set_command(command)
        self._stop_periodic(handle)
        self.periodic.pop(handle, None)
        log.info('%s: advertising set %d removed', self.name, handle)
        return result

    def on_hci_le_clear_advertising_sets_command(self, command):
        if self.bigs:
            return hci.HCI_StatusReturnParameters(COMMAND_DISALLOWED)
        for handle in list(self.periodic):
            self._stop_periodic(handle)
        self.periodic.clear()
        log.info('%s: advertising sets cleared', self.name)
        return super().on_hci_le_clear_advertising_sets_command(command)

    # ---- periodic advertising (broadcaster) ----

    def on_hci_le_set_periodic_advertising_parameters_command(self, command):
        handle = command.advertising_handle
        if handle not in self.advertising_sets:
            return hci.HCI_StatusReturnParameters(UNKNOWN_ADVERTISING_IDENTIFIER)
        pa = self.periodic.setdefault(handle, PeriodicAdv())
        pa.interval = command.periodic_advertising_interval_max
        pa.properties = command.periodic_advertising_properties
        log.info('%s: periodic advertising parameters of set %d: interval %.2f-%.2f ms', self.name, handle,
                 command.periodic_advertising_interval_min * 1.25, command.periodic_advertising_interval_max * 1.25)
        return hci.HCI_StatusReturnParameters(SUCCESS)

    def on_hci_le_set_periodic_advertising_data_command(self, command):
        pa = self.periodic.get(command.advertising_handle)
        if pa is None:
            return hci.HCI_StatusReturnParameters(UNKNOWN_ADVERTISING_IDENTIFIER)
        operation = int(command.operation)
        if operation in (1, 3):      # first fragment / complete data
            pa.data = bytearray(command.advertising_data)
        elif operation in (0, 2):    # intermediate / last fragment
            pa.data.extend(command.advertising_data)
        if operation in (2, 3):
            log.info('%s: periodic advertising data of set %d: %s', self.name, command.advertising_handle,
                     bytes(pa.data).hex())
        return hci.HCI_StatusReturnParameters(SUCCESS)

    def on_hci_le_set_periodic_advertising_enable_command(self, command):
        handle = command.advertising_handle
        pa = self.periodic.get(handle)
        if pa is None:
            return hci.HCI_StatusReturnParameters(UNKNOWN_ADVERTISING_IDENTIFIER)
        enable = bool(command.enable & 1)
        log.info('%s: periodic advertising of set %d %s', self.name, handle, 'enabled' if enable else 'disabled')
        if enable and not pa.enabled:
            pa.enabled = True
            loop = asyncio.get_running_loop()
            pa.timer = loop.call_soon(self._periodic_event, handle)
            for c in self._receivers():
                c._try_pending_sync()
        elif not enable and pa.enabled:
            self._stop_periodic(handle)
        return hci.HCI_StatusReturnParameters(SUCCESS)

    def _stop_periodic(self, handle):
        pa = self.periodic.get(handle)
        if pa is None:
            return
        pa.enabled = False
        if pa.timer is not None:
            pa.timer.cancel()
            pa.timer = None
        for receiver, sync in self._syncs_of(handle):
            receiver._lose_sync_later(sync)

    def _periodic_event(self, handle):
        """One periodic advertising event: its data and BIGInfo to the synced receivers"""
        pa = self.periodic.get(handle)
        if pa is None or not pa.enabled:
            return
        big = self._big_of_set(handle)
        for receiver, sync in self._syncs_of(handle):
            receiver.send_periodic_report(sync, bytes(pa.data))
            if big is not None:
                receiver.send_biginfo(sync, self, big)
        interval = max(pa.interval * 1.25e-3, PA_EVENT_MIN_S)
        pa.timer = asyncio.get_running_loop().call_later(interval, self._periodic_event, handle)

    # ---- BIG (broadcaster) ----

    def on_hci_le_create_big_command(self, command):
        if command.big_handle in self.bigs:
            self._status(command, COMMAND_DISALLOWED)
            return None
        pa = self.periodic.get(command.advertising_handle)
        if command.advertising_handle not in self.advertising_sets or pa is None:
            log.warning('%s: LE Create BIG on advertising set %d, which has no periodic advertising parameters',
                        self.name, command.advertising_handle)
            self._status(command, UNKNOWN_ADVERTISING_IDENTIFIER)
            return None
        if not 1 <= command.num_bis <= 31 or not command.sdu_interval or not command.max_sdu:
            self._status(command, INVALID_PARAMETERS)
            return None
        if self._big_of_set(command.advertising_handle) is not None:
            self._status(command, COMMAND_DISALLOWED)
            return None
        self._status(command, SUCCESS)
        if not pa.enabled:
            log.warning('%s: LE Create BIG before periodic advertising of set %d was enabled',
                        self.name, command.advertising_handle)
        phy = 2 if command.phy & 0x02 else 1 if command.phy & 0x01 else 3
        bn = 1
        irc = min(command.rtn + 1, 15)
        nse = min(irc * bn, 31)
        iso_interval = max(4, round(command.sdu_interval / 1250))
        # Sub-interval: one BIS PDU (header, payload, MIC) and T_MSS
        pdu_us = (10 + command.max_sdu + (4 if command.encryption else 0)) * (4 if phy == 2 else 8)
        sync_delay = min(command.num_bis * nse * (pdu_us + 150), iso_interval * 1250)
        used = set(self.bis_tx)
        handles = []
        for _ in range(command.num_bis):
            h = self._free_iso_handle(used)
            used.add(h)
            handles.append(h)
        big = Big(handle=command.big_handle, adv_handle=command.advertising_handle, command=command,
                  bis_handles=handles, phy=phy, nse=nse, bn=bn, pto=0, irc=irc, max_pdu=command.max_sdu,
                  iso_interval=iso_interval, sync_delay=sync_delay, transport_latency=sync_delay)
        self.bigs[command.big_handle] = big
        for index, h in enumerate(handles, 1):
            self.bis_tx[h] = (command.big_handle, index)
        code = bytes(command.broadcast_code)
        log.info('%s: LE Create BIG: BIG %d on advertising set %d, %d BIS, SDU interval %d us, max SDU %d, '
                 'max transport latency %d ms, RTN %d, PHY %s, %s, %s%s', self.name, command.big_handle,
                 command.advertising_handle, command.num_bis, command.sdu_interval, command.max_sdu,
                 command.max_transport_latency, command.rtn, phy_name(phy),
                 'framed' if command.framing else 'unframed', 'encrypted' if command.encryption else 'open',
                 f', Broadcast_Code {code_text(code)}' if command.encryption else '')
        if command.encryption and code[0] == 0 and code[-1] != 0:
            log.warning('%s: the Broadcast_Code starts with zero bytes and ends with data: byte order reversed? '
                        '(the first character belongs in the first byte)', self.name)
        self.send_hci_packet(hci.HCI_LE_Create_BIG_Complete_Event(
            status=SUCCESS, big_handle=big.handle, big_sync_delay=big.sync_delay,
            transport_latency_big=big.transport_latency, phy=phy, nse=nse, bn=bn, pto=0, irc=irc,
            max_pdu=big.max_pdu, iso_interval=iso_interval, connection_handle=handles))
        log.info('%s: BIG %d created: BIS handles %s, ISO interval %.2f ms, NSE %d, BN %d, IRC %d, PTO 0, '
                 'max PDU %d', self.name, big.handle, ', '.join(f'0x{h:04X}' for h in handles),
                 iso_interval * 1.25, nse, bn, irc, big.max_pdu)
        # Receivers synced to the set learn of the BIG at once (BIGInfo)
        for receiver, sync in self._syncs_of(big.adv_handle):
            receiver.send_biginfo(sync, self, big)
        for receiver in self._receivers():
            receiver._try_pending_big_syncs()
        return None

    def on_hci_le_terminate_big_command(self, command):
        big = self.bigs.get(command.big_handle)
        if big is None:
            self._status(command, UNKNOWN_ADVERTISING_IDENTIFIER)
            return None
        self._status(command, SUCCESS)
        self.flush_iso(big.bis_handles)
        sdus = ', '.join(f'BIS {i}: {big.sdus.get(i, 0)}' for i in range(1, len(big.bis_handles) + 1))
        log.info('%s: LE Terminate BIG %d (reason 0x%02X): SDUs taken %s', self.name, big.handle,
                 command.reason, sdus)
        for receiver in self._receivers():
            receiver._source_big_terminated(self, big.handle, command.reason or REMOTE_USER_TERMINATED)
        for h in big.bis_handles:
            self.bis_tx.pop(h, None)
        del self.bigs[big.handle]
        self.send_hci_packet(hci.HCI_LE_Terminate_BIG_Complete_Event(
            big_handle=big.handle, reason=LOCAL_HOST_TERMINATED))
        return None

    # ---- ISO data paths and data (both roles) ----

    def on_hci_le_setup_iso_data_path_command(self, command):
        handle = command.connection_handle
        direction = int(command.data_path_direction)
        if handle in self.bis_tx:
            big = self.bigs[self.bis_tx[handle][0]]
            if direction != 0 or handle in big.data_paths:
                return hci.HCI_StatusAndConnectionHandleReturnParameters(COMMAND_DISALLOWED, handle)
            big.data_paths.add(handle)
            return hci.HCI_StatusAndConnectionHandleReturnParameters(SUCCESS, handle)
        if handle in self.bis_rx:
            sync = self.big_syncs[self.bis_rx[handle][0]]
            if direction != 1 or handle in sync.data_paths:
                return hci.HCI_StatusAndConnectionHandleReturnParameters(COMMAND_DISALLOWED, handle)
            sync.data_paths.add(handle)
            return hci.HCI_StatusAndConnectionHandleReturnParameters(SUCCESS, handle)
        return super().on_hci_le_setup_iso_data_path_command(command)

    def on_hci_le_remove_iso_data_path_command(self, command):
        handle = command.connection_handle
        for owner, groups in ((self.bis_tx, self.bigs), (self.bis_rx, self.big_syncs)):
            if handle in owner:
                group = groups[owner[handle][0]]
                if handle not in group.data_paths:
                    return hci.HCI_StatusAndConnectionHandleReturnParameters(COMMAND_DISALLOWED, handle)
                group.data_paths.discard(handle)
                return hci.HCI_StatusAndConnectionHandleReturnParameters(SUCCESS, handle)
        return super().on_hci_le_remove_iso_data_path_command(command)

    def on_hci_iso_data_packet(self, packet):
        handle = packet.connection_handle
        if handle not in self.bis_tx:
            return super().on_hci_iso_data_packet(packet)
        big_handle, index = self.bis_tx[handle]
        big = self.bigs[big_handle]
        if handle not in big.data_paths:
            log.warning('ISO data on BIS 0x%04X without an ISO data path', handle)
        big.sdus[index] = big.sdus.get(index, 0) + 1

        def receivers():
            return [(c, h) for c in self._receivers() for h in c.bis_receiving(self, big_handle, index)]
        self.schedule_iso_sdu(packet, 'BIS', big.command.sdu_interval, big.command.max_transport_latency,
                              receivers)

    # ---- extended scanning (receiver) ----

    def on_hci_le_set_extended_scan_parameters_command(self, _command):
        if self.ext_scan_enable:
            return hci.HCI_StatusReturnParameters(COMMAND_DISALLOWED)
        return hci.HCI_StatusReturnParameters(SUCCESS)

    def on_hci_le_set_extended_scan_enable_command(self, command):
        self.ext_scan_enable = bool(command.enable)
        return hci.HCI_StatusReturnParameters(SUCCESS)

    def on_advertising_pdu(self, pdu):
        if not isinstance(pdu, ExtAdvPdu):
            return super().on_advertising_pdu(pdu)
        self._try_pending_sync()
        if not self.ext_scan_enable:
            return None
        pa = pdu.source.periodic.get(pdu.handle)
        interval = pa.interval if pa is not None and pa.enabled else 0
        event_type = pdu.properties & 0x07   # connectable, scannable, directed
        data = pdu.data
        chunks = [data[i:i + EXT_REPORT_DATA_MAX] for i in range(0, len(data), EXT_REPORT_DATA_MAX)] or [b'']
        for i, chunk in enumerate(chunks):
            status = 0 if i == len(chunks) - 1 else 1   # complete / incomplete, more to come
            self.send_hci_packet(hci.HCI_LE_Extended_Advertising_Report_Event([
                hci.HCI_LE_Extended_Advertising_Report_Event.Report(
                    event_type=event_type | (status << 5),
                    address_type=pdu.advertiser_address.address_type,
                    address=pdu.advertiser_address,
                    primary_phy=hci.Phy.LE_1M,
                    secondary_phy=hci.Phy.LE_2M,
                    advertising_sid=pdu.sid,
                    tx_power=TX_POWER_NONE,
                    rssi=-50,
                    periodic_advertising_interval=interval,
                    direct_address_type=0,
                    direct_address=hci.Address.ANY,
                    data=chunk,
                )]))
        return None

    # ---- periodic advertising sync (receiver) ----

    def on_hci_le_periodic_advertising_create_sync_command(self, command):
        if self.pa_pending is not None:
            self._status(command, COMMAND_DISALLOWED)
            return None
        self._status(command, SUCCESS)
        self.pa_pending = command
        log.info('%s: LE Periodic Advertising Create Sync: %s SID %d', self.name, command.advertiser_address,
                 command.advertising_sid)
        asyncio.get_running_loop().call_soon(self._try_pending_sync)
        return None

    def _find_set(self, address, sid):
        for c in self._receivers():
            for handle, adv in c.advertising_sets.items():
                p = adv.parameters
                if adv.enabled and p is not None and adv.address is not None \
                        and bytes(adv.address) == bytes(address) and p.advertising_sid == sid:
                    return c, handle
        return None, None

    def _try_pending_sync(self):
        command = self.pa_pending
        if command is None:
            return
        source, adv_handle = self._find_set(command.advertiser_address, command.advertising_sid)
        if source is None:
            return
        pa = source.periodic.get(adv_handle)
        if pa is None or not pa.enabled:
            return
        self.pa_pending = None
        handle = self.next_sync_handle
        self.next_sync_handle += 1
        sync = PaSync(handle=handle, source=source, adv_handle=adv_handle, sid=command.advertising_sid,
                      address=command.advertiser_address,
                      timeout_s=min(command.sync_timeout * 0.01, SYNC_LOST_MAX_S))
        self.pa_syncs[handle] = sync
        params = source.advertising_sets[adv_handle].parameters
        self.send_hci_packet(hci.HCI_LE_Periodic_Advertising_Sync_Established_Event(
            status=SUCCESS, sync_handle=handle, advertising_sid=sync.sid,
            advertiser_address_type=command.advertiser_address.address_type,
            advertiser_address=command.advertiser_address,
            advertiser_phy=int(getattr(params, 'secondary_advertising_phy', 1) or 1),
            periodic_advertising_interval=pa.interval, advertiser_clock_accuracy=0))
        log.info('%s: periodic advertising sync %d established to %s set %d', self.name, handle, source.name,
                 adv_handle)
        # The current event's data now, BIGInfo too
        self.send_periodic_report(sync, bytes(pa.data))
        big = source._big_of_set(adv_handle)
        if big is not None:
            self.send_biginfo(sync, source, big)

    def on_hci_le_periodic_advertising_create_sync_cancel_command(self, _command):
        command = self.pa_pending
        if command is None:
            return hci.HCI_StatusReturnParameters(COMMAND_DISALLOWED)
        self.pa_pending = None
        self.send_hci_packet(hci.HCI_LE_Periodic_Advertising_Sync_Established_Event(
            status=OPERATION_CANCELLED_BY_HOST, sync_handle=0, advertising_sid=command.advertising_sid,
            advertiser_address_type=command.advertiser_address.address_type,
            advertiser_address=command.advertiser_address, advertiser_phy=1, periodic_advertising_interval=0,
            advertiser_clock_accuracy=0))
        return hci.HCI_StatusReturnParameters(SUCCESS)

    def on_hci_le_periodic_advertising_terminate_sync_command(self, command):
        sync = self.pa_syncs.pop(command.sync_handle, None)
        if sync is None:
            return hci.HCI_StatusReturnParameters(UNKNOWN_ADVERTISING_IDENTIFIER)
        if sync.lost_timer is not None:
            sync.lost_timer.cancel()
        log.info('%s: periodic advertising sync %d terminated', self.name, sync.handle)
        return hci.HCI_StatusReturnParameters(SUCCESS)

    def send_periodic_report(self, sync, data):
        chunks = [data[i:i + PA_REPORT_DATA_MAX] for i in range(0, len(data), PA_REPORT_DATA_MAX)] or [b'']
        for i, chunk in enumerate(chunks):
            self.send_hci_packet(hci.HCI_LE_Periodic_Advertising_Report_Event(
                sync_handle=sync.handle, tx_power=TX_POWER_NONE, rssi=-50, cte_type=0xFF,
                data_status=0 if i == len(chunks) - 1 else 1, data=chunk))

    def send_biginfo(self, sync, source, big):
        c = big.command
        self.send_hci_packet(hci.HCI_LE_BIGInfo_Advertising_Report_Event(
            sync_handle=sync.handle, num_bis=c.num_bis, nse=big.nse, iso_interval=big.iso_interval, bn=big.bn,
            pto=big.pto, irc=big.irc, max_pdu=big.max_pdu, sdu_interval=c.sdu_interval, max_sdu=c.max_sdu,
            phy=big.phy, framing=c.framing, encryption=c.encryption))

    def _lose_sync_later(self, sync):
        """The set stopped periodic advertising: Sync Lost after the sync timeout"""
        if sync.lost_timer is not None:
            return

        def lost():
            sync.lost_timer = None
            pa = sync.source.periodic.get(sync.adv_handle)
            if pa is not None and pa.enabled or self.pa_syncs.get(sync.handle) is not sync:
                return
            del self.pa_syncs[sync.handle]
            log.info('%s: periodic advertising sync %d lost', self.name, sync.handle)
            self.send_hci_packet(hci.HCI_LE_Periodic_Advertising_Sync_Lost_Event(sync_handle=sync.handle))
        sync.lost_timer = asyncio.get_running_loop().call_later(sync.timeout_s, lost)

    # ---- BIG sync (receiver) ----

    def on_hci_le_big_create_sync_command(self, command):
        if command.big_handle in self.big_syncs:
            self._status(command, COMMAND_DISALLOWED)
            return None
        if command.sync_handle not in self.pa_syncs:
            self._status(command, UNKNOWN_ADVERTISING_IDENTIFIER)
            return None
        self._status(command, SUCCESS)
        self.big_syncs[command.big_handle] = BigSync(handle=command.big_handle, sync_handle=command.sync_handle,
                                                     command=command)
        log.info('%s: LE BIG Create Sync: BIG %d, BIS %s, %s', self.name, command.big_handle,
                 ', '.join(str(i) for i in command.bis),
                 f'Broadcast_Code {code_text(command.broadcast_code)}' if command.encryption else 'no Broadcast_Code')
        asyncio.get_running_loop().call_soon(self._try_pending_big_syncs)
        return None

    def _try_pending_big_syncs(self):
        for sync in list(self.big_syncs.values()):
            if sync.established:
                continue
            pa = self.pa_syncs.get(sync.sync_handle)
            if pa is None:
                continue
            big = pa.source._big_of_set(pa.adv_handle)
            if big is None:
                continue   # waits for the BIG (BIGInfo)
            self._establish_big_sync(sync, pa.source, big)

    def _big_sync_failed(self, sync, status):
        del self.big_syncs[sync.handle]
        self.send_hci_packet(hci.HCI_LE_BIG_Sync_Established_Event(
            status=status, big_handle=sync.handle, transport_latency_big=0, nse=0, bn=0, pto=0, irc=0,
            max_pdu=0, iso_interval=0, connection_handle=[]))

    def _establish_big_sync(self, sync, source, big):
        c = sync.command
        if not c.bis or any(not 1 <= i <= big.command.num_bis for i in c.bis) or len(set(c.bis)) != len(c.bis):
            log.warning('%s: BIG Create Sync for BIS %s of a BIG with %d', self.name, list(c.bis),
                        big.command.num_bis)
            return self._big_sync_failed(sync, INVALID_PARAMETERS)
        if big.encrypted and not c.encryption:
            log.warning('%s: BIG Create Sync without a Broadcast_Code: the BIG is encrypted', self.name)
            return self._big_sync_failed(sync, ENCRYPTION_MODE_NOT_ACCEPTABLE)
        if big.encrypted and bytes(c.broadcast_code) != bytes(big.command.broadcast_code):
            log.info('%s: BIG sync %d: MIC failure (the Broadcast_Code %s does not decrypt the BIG)', self.name,
                     sync.handle, code_text(c.broadcast_code))
            return self._big_sync_failed(sync, MIC_FAILURE)
        if not big.encrypted and c.encryption:
            log.warning('%s: BIG Create Sync with a Broadcast_Code for an unencrypted BIG', self.name)
        used = set(self.bis_rx)
        for index in c.bis:
            h = self._free_iso_handle(used)
            used.add(h)
            sync.bis[index] = h
            self.bis_rx[h] = (sync.handle, index)
        sync.source, sync.source_big, sync.established = source, big.handle, True
        self.send_hci_packet(hci.HCI_LE_BIG_Sync_Established_Event(
            status=SUCCESS, big_handle=sync.handle, transport_latency_big=big.transport_latency, nse=big.nse,
            bn=big.bn, pto=big.pto, irc=big.irc, max_pdu=big.max_pdu, iso_interval=big.iso_interval,
            connection_handle=list(sync.bis.values())))
        log.info('%s: BIG sync %d established to %s BIG %d: %s', self.name, sync.handle, source.name, big.handle,
                 ', '.join(f'BIS {i} -> 0x{h:04X}' for i, h in sync.bis.items()))
        return None

    def bis_receiving(self, source, big_handle, index):
        """Handles of this receiver that take BIS index of source's BIG (synced, data path set)"""
        return [h for sync in self.big_syncs.values()
                if sync.established and sync.source is source and sync.source_big == big_handle
                for i, h in sync.bis.items() if i == index and h in sync.data_paths]

    def _drop_big_sync(self, sync):
        for h in sync.bis.values():
            self.bis_rx.pop(h, None)
        self.big_syncs.pop(sync.handle, None)

    def _source_big_terminated(self, source, big_handle, reason):
        for sync in list(self.big_syncs.values()):
            if sync.established and sync.source is source and sync.source_big == big_handle:
                self._drop_big_sync(sync)
                log.info('%s: BIG sync %d lost (reason 0x%02X)', self.name, sync.handle, reason)
                self.send_hci_packet(hci.HCI_LE_BIG_Sync_Lost_Event(big_handle=sync.handle, reason=reason))

    def on_hci_le_big_terminate_sync_command(self, command):
        sync = self.big_syncs.get(command.big_handle)
        if sync is None:
            return hci.HCI_LE_BIG_Terminate_Sync_ReturnParameters(UNKNOWN_ADVERTISING_IDENTIFIER,
                                                                  command.big_handle)
        self._drop_big_sync(sync)
        if not sync.established:
            self.send_hci_packet(hci.HCI_LE_BIG_Sync_Established_Event(
                status=OPERATION_CANCELLED_BY_HOST, big_handle=sync.handle, transport_latency_big=0, nse=0,
                bn=0, pto=0, irc=0, max_pdu=0, iso_interval=0, connection_handle=[]))
        log.info('%s: BIG sync %d terminated', self.name, sync.handle)
        return hci.HCI_LE_BIG_Terminate_Sync_ReturnParameters(SUCCESS, command.big_handle)
