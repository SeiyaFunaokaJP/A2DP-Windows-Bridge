"""Auracast broadcast sink (emu_le_audio_sink.py --layout auracast).

A Bumble host on its own BroadcastController (le_audio/broadcast.py) does
what a broadcast sink does: extended scan for a Broadcast Audio
Announcement, periodic advertising sync, BASE and BIGInfo, LE BIG Create Sync
(with --broadcast-code), ISO data paths on the BIS handles. What BlueCaster
advertises is parsed with Bumble's own parsers (bap.BroadcastAudioAnnouncement,
pbp.PublicBroadcastAnnouncement, bap.BasicAudioAnnouncement) after a strict
length walk of the AD and BASE structures, and logged for run_test.py
(le_audio_tests/auracast.py):

    Auracast sink: broadcast from <addr> SID <n>: Broadcast ID 0x<id>, Broadcast Name "<name>"
    Auracast sink: Public Broadcast Announcement: features 0x<f> (<flags>), metadata <entries>
    Auracast sink: BASE: presentation delay <us> us, <n> subgroup(s)
    Auracast sink: BASE subgroup <k>: <codec>, <Hz> Hz, <frame> us, <octets> octets, <blocks> block(s) per SDU, metadata <entries>, <n> BIS
    Auracast sink: BASE BIS <index>: allocation 0x<a> (<locations>)
    Auracast sink: BIGInfo: <n> BIS, ...
    Auracast sink: BIG sync established: BIS <i> -> 0x<handle>, ...
    Auracast sink: BIG sync failed: <reason> (0x<status>)
    Auracast sink: BIG sync lost (reason 0x<r>): BIS <i>: <n> SDUs, ...
    Auracast sink: problem: <text>         (malformed data, unexpected values)

The sink's HCI traffic (ISO data included) goes to the PacketLogger capture
for bluecaster_decode --le-audio --received.
"""

import asyncio
import logging
import struct
import warnings

from bumble import hci
from bumble.device import BigSyncParameters, Device
from bumble.host import Host
from bumble.profiles import bap, le_audio, pbp
from bumble.transport.common import AsyncPipeSink

from .broadcast import BroadcastController
from .controller import PacketLoggerSnooper

log = logging.getLogger('emu_le_audio')
# Device.create_big_sync() is marked experimental ("Only for testing"): it is
warnings.filterwarnings('ignore', message='Only for testing', category=FutureWarning)

SINK_NAME = 'Auracast sink'
CONTROLLER_NAME = 'Sink controller'
BROADCAST_AUDIO_ANNOUNCEMENT = 0x1852
PUBLIC_BROADCAST_ANNOUNCEMENT = 0x1856
BASIC_AUDIO_ANNOUNCEMENT = 0x1851
BROADCAST_NAME = 0x30
LOCATIONS = {0x1: 'FL', 0x2: 'FR', 0x4: 'FC', 0x8: 'LFE', 0x10: 'BL', 0x20: 'BR'}
STATUS_NAMES = {0x3D: 'MIC failure', 0x25: 'encryption mode not acceptable', 0x12: 'invalid parameters'}


def code_bytes(code):
    """Broadcast_Code: the UTF-8 string, zero padded to 16 bytes (as Bumble's auracast app)"""
    raw = code.encode('utf-8')
    if len(raw) > 16:
        raise ValueError('a Broadcast_Code has at most 16 bytes')
    return raw + bytes(16 - len(raw))


def ad_structures(data):
    """[(type, value)] of AD data, or raise ValueError when a length does not fit"""
    out, i = [], 0
    while i < len(data):
        length = data[i]
        if length == 0:
            # Padding: the rest must be zeros
            if any(data[i:]):
                raise ValueError(f'zero-length AD structure at offset {i} followed by data')
            break
        if i + 1 + length > len(data):
            raise ValueError(f'AD structure at offset {i} (length {length}) runs past the end ({len(data)} bytes)')
        out.append((data[i + 1], bytes(data[i + 2:i + 1 + length])))
        i += 1 + length
    return out


def service_data(structures, uuid):
    for ad_type, value in structures:
        if ad_type == 0x16 and len(value) >= 2 and struct.unpack_from('<H', value)[0] == uuid:
            return value[2:]
    return None


def ltv_walk(data, what):
    """Checks LTV lengths (codec configuration, metadata); raises ValueError"""
    i = 0
    while i < len(data):
        length = data[i]
        if length == 0 or i + 1 + length > len(data):
            raise ValueError(f'{what}: LTV at offset {i} (length {length}) does not fit in {len(data)} bytes')
        i += 1 + length


def base_walk(data):
    """Strict length walk of a BASE (BAP 3.7.2.2); raises ValueError"""
    if len(data) < 4:
        raise ValueError(f'BASE of {len(data)} bytes')
    offset, subgroups = 4, data[3]
    if not subgroups:
        raise ValueError('BASE with no subgroup')
    for k in range(subgroups):
        if offset + 7 > len(data):
            raise ValueError(f'BASE subgroup {k + 1} header runs past the end')
        num_bis = data[offset]
        if not num_bis:
            raise ValueError(f'BASE subgroup {k + 1} has no BIS')
        offset += 6
        n = data[offset]
        ltv_walk(data[offset + 1:offset + 1 + n], f'BASE subgroup {k + 1} codec configuration')
        offset += 1 + n
        if offset >= len(data):
            raise ValueError(f'BASE subgroup {k + 1} metadata length missing')
        n = data[offset]
        ltv_walk(data[offset + 1:offset + 1 + n], f'BASE subgroup {k + 1} metadata')
        offset += 1 + n
        for b in range(num_bis):
            if offset + 2 > len(data):
                raise ValueError(f'BASE subgroup {k + 1} BIS {b + 1} runs past the end')
            n = data[offset + 1]
            ltv_walk(data[offset + 2:offset + 2 + n], f'BASE BIS {data[offset]} codec configuration')
            offset += 2 + n
        if offset > len(data):
            raise ValueError(f'BASE subgroup {k + 1} runs past the end')
    if offset != len(data):
        raise ValueError(f'BASE has {len(data) - offset} byte(s) after its last subgroup')


def metadata_text(metadata):
    parts = []
    for entry in metadata.entries:
        try:
            value = entry.decode()
        except Exception:  # noqa: BLE001  (Bumble raises various errors on bad values)
            value = entry.data.hex()
        if entry.tag in (le_audio.Metadata.Tag.STREAMING_AUDIO_CONTEXTS,
                         le_audio.Metadata.Tag.PREFERRED_AUDIO_CONTEXTS):
            value = f'0x{struct.unpack("<H", entry.data)[0]:04X} ({value.name if hasattr(value, "name") else value})'
        name = entry.tag.name.lower().replace('_', ' ') if hasattr(entry.tag, 'name') else f'tag 0x{entry.tag:02X}'
        parts.append(f'{name} {value}')
    return '; '.join(parts) or 'none'


def allocation_text(allocation):
    if allocation is None:
        return 'none'
    names = [LOCATIONS.get(1 << b, f'0x{1 << b:x}') for b in range(32) if allocation & (1 << b)]
    return f'0x{int(allocation):x} ({"+".join(names) or "mono"})'


class BroadcastSink:
    def __init__(self, link, address, capture, broadcast_code):
        self.code = code_bytes(broadcast_code) if broadcast_code else None
        self.controller = BroadcastController(CONTROLLER_NAME, link=link, public_address=address)
        host = Host(self.controller, AsyncPipeSink(self.controller))
        if capture:
            host.snooper = PacketLoggerSnooper(capture)
        self.device = Device(name=SINK_NAME, address=address, host=host)
        self.device.le_enabled = True
        self.device.classic_enabled = False
        self.address = address
        self.broadcast = None      # (address, SID) being followed
        self.pa_sync = None
        self.base = None
        self.base_raw = None
        self.biginfo = None
        self.big_sync = None
        self.syncing = False
        self.sdus = {}             # BIS index -> SDUs received
        self.seq = {}              # BIS index -> last sequence number
        self.gaps = {}

    def problem(self, text):
        log.warning('%s: problem: %s', SINK_NAME, text)

    async def start(self):
        await self.device.power_on()
        self.device.on('advertisement', self.on_advertisement)
        await self.device.start_scanning(active=False, filter_duplicates=False)
        log.info('%s: scanning for broadcasts (%s)', SINK_NAME,
                 'Broadcast_Code ' + repr(self.code.rstrip(b'\0').decode()) if self.code else 'no Broadcast_Code')

    def on_advertisement(self, advertisement):
        if self.broadcast is not None:
            return
        try:
            structures = ad_structures(advertisement.data_bytes)
        except ValueError as e:
            self.problem(f'extended advertising data of {advertisement.address}: {e}')
            return
        announcement = service_data(structures, BROADCAST_AUDIO_ANNOUNCEMENT)
        if announcement is None:
            return
        self.broadcast = (advertisement.address, advertisement.sid)
        if len(announcement) != 3:
            self.problem(f'Broadcast Audio Announcement of {len(announcement)} bytes (3: Broadcast_ID)')
        broadcast_id = bap.BroadcastAudioAnnouncement.from_bytes(announcement).broadcast_id
        names = [v for t, v in structures if t == BROADCAST_NAME]
        name = None
        if not names:
            self.problem('no Broadcast Name (AD type 0x30)')
        else:
            try:
                name = names[0].decode('utf-8')
            except UnicodeDecodeError:
                self.problem(f'Broadcast Name is not UTF-8: {names[0].hex()}')
            if name is not None and not 4 <= len(names[0]) <= 128:
                self.problem(f'Broadcast Name of {len(names[0])} bytes (PBP: 4 to 128)')
        if advertisement.is_connectable or advertisement.is_scannable or advertisement.is_legacy:
            self.problem('the broadcast is advertised connectable, scannable or with legacy PDUs')
        log.info('%s: broadcast from %s SID %d: Broadcast ID 0x%06X, Broadcast Name "%s"', SINK_NAME,
                 advertisement.address, advertisement.sid, broadcast_id, name)
        public = service_data(structures, PUBLIC_BROADCAST_ANNOUNCEMENT)
        if public is None:
            self.problem('no Public Broadcast Announcement (service data 0x1856)')
        else:
            if len(public) < 2 or len(public) != 2 + public[1]:
                self.problem(f'Public Broadcast Announcement of {len(public)} bytes with metadata length '
                             f'{public[1] if len(public) > 1 else None}')
            try:
                ltv_walk(public[2:], 'Public Broadcast Announcement metadata')
                pba = pbp.PublicBroadcastAnnouncement.from_bytes(public)
                flags = [f.name.lower() for f in pbp.PublicBroadcastAnnouncement.Features if f in pba.features]
                log.info('%s: Public Broadcast Announcement: features 0x%02X (%s), metadata %s', SINK_NAME,
                         public[0], ', '.join(flags) or 'none', metadata_text(pba.metadata))
            except (ValueError, IndexError, struct.error) as e:
                self.problem(f'Public Broadcast Announcement {public.hex()}: {e}')
        asyncio.get_running_loop().create_task(self.sync(advertisement))

    async def sync(self, advertisement):
        self.pa_sync = await self.device.create_periodic_advertising_sync(
            advertiser_address=advertisement.address, sid=advertisement.sid, sync_timeout=1.0)
        self.pa_sync.on('establishment', lambda: log.info('%s: periodic advertising sync established',
                                                          SINK_NAME))
        self.pa_sync.on('loss', lambda: log.info('%s: periodic advertising sync lost', SINK_NAME))
        self.pa_sync.on('periodic_advertisement', self.on_periodic_advertisement)
        self.pa_sync.on('biginfo_advertisement', self.on_biginfo)
        await self.device.stop_scanning()

    def on_periodic_advertisement(self, advertisement):
        data = advertisement.data_bytes
        try:
            raw = service_data(ad_structures(data), BASIC_AUDIO_ANNOUNCEMENT)
        except ValueError as e:
            self.problem(f'periodic advertising data: {e}')
            return
        if raw is None:
            if self.base_raw is None:
                self.problem(f'periodic advertising data without a BASE (service data 0x1851): {data.hex()}')
                self.base_raw = b''
            return
        if raw == self.base_raw:
            return
        self.base_raw = raw
        try:
            base_walk(raw)
            self.base = bap.BasicAudioAnnouncement.from_bytes(raw)
        except (ValueError, IndexError, struct.error) as e:
            self.problem(f'BASE {raw.hex()}: {e}')
            return
        log.info('%s: BASE: presentation delay %d us, %d subgroup(s)', SINK_NAME, self.base.presentation_delay,
                 len(self.base.subgroups))
        for k, subgroup in enumerate(self.base.subgroups, 1):
            cfg = subgroup.codec_specific_configuration
            codec = 'LC3' if subgroup.codec_id.codec_id == hci.CodecID.LC3 else str(subgroup.codec_id)
            log.info('%s: BASE subgroup %d: %s, %s Hz, %s us, %s octets, %s block(s) per SDU, allocation %s, '
                     'metadata %s, %d BIS', SINK_NAME, k, codec,
                     cfg.sampling_frequency.hz if cfg.sampling_frequency else None,
                     cfg.frame_duration.us if cfg.frame_duration else None, cfg.octets_per_codec_frame,
                     cfg.codec_frames_per_sdu or 1, allocation_text(cfg.audio_channel_allocation),
                     metadata_text(subgroup.metadata), len(subgroup.bis))
            for bis in subgroup.bis:
                bis_cfg = bis.codec_specific_configuration
                log.info('%s: BASE BIS %d: allocation %s', SINK_NAME, bis.index,
                         allocation_text(bis_cfg.audio_channel_allocation
                                         if bis_cfg.audio_channel_allocation is not None
                                         else cfg.audio_channel_allocation))
        self.maybe_sync_big()

    def on_biginfo(self, info):
        if self.biginfo is None:
            log.info('%s: BIGInfo: %d BIS, NSE %d, ISO interval %s ms, BN %d, PTO %d, IRC %d, max PDU %d, '
                     'SDU interval %d us, max SDU %d, PHY %s, %s, %s', SINK_NAME, info.num_bis, info.nse,
                     info.iso_interval, info.bn, info.pto, info.irc, info.max_pdu, info.sdu_interval, info.max_sdu,
                     getattr(info.phy, 'name', info.phy), getattr(info.framing, 'name', info.framing).lower(),
                     'encrypted' if info.encryption else 'open')
        self.biginfo = info
        self.maybe_sync_big()

    def maybe_sync_big(self):
        if self.syncing or self.base is None or self.biginfo is None:
            return
        self.syncing = True
        asyncio.get_running_loop().create_task(self.sync_big())

    async def sync_big(self):
        indices = [bis.index for subgroup in self.base.subgroups for bis in subgroup.bis]
        try:
            self.big_sync = await self.device.create_big_sync(
                self.pa_sync, BigSyncParameters(big_sync_timeout=0x4000, bis=indices, broadcast_code=self.code))
        except hci.HCI_Error as e:
            log.info('%s: BIG sync failed: %s (0x%02X)', SINK_NAME, STATUS_NAMES.get(e.error_code, e.error_name),
                     e.error_code)
            return
        log.info('%s: BIG sync established: %s', SINK_NAME,
                 ', '.join(f'BIS {i} -> 0x{link.handle:04X}' for i, link in zip(indices, self.big_sync.bis_links)))
        self.big_sync.on(self.big_sync.Event.TERMINATION, self.on_big_sync_lost)
        for index, bis_link in zip(indices, self.big_sync.bis_links):
            self.sdus[index] = 0
            bis_link.sink = lambda packet, index=index: self.on_sdu(index, packet)
            await bis_link.setup_data_path(direction=bis_link.Direction.CONTROLLER_TO_HOST)

    def on_sdu(self, index, packet):
        self.sdus[index] += 1
        seq = packet.packet_sequence_number
        last = self.seq.get(index)
        if last is not None and seq != (last + 1) & 0xFFFF:
            self.gaps[index] = self.gaps.get(index, 0) + 1
        self.seq[index] = seq

    def on_big_sync_lost(self, reason):
        log.info('%s: BIG sync lost (reason 0x%02X): %s', SINK_NAME, reason,
                 ', '.join(f'BIS {i}: {n} SDUs' + (f' ({self.gaps[i]} sequence gaps)' if self.gaps.get(i) else '')
                           for i, n in sorted(self.sdus.items())))


async def start(link, address, capture, broadcast_code):
    sink = BroadcastSink(link, address, capture, broadcast_code)
    await sink.start()
    return sink
