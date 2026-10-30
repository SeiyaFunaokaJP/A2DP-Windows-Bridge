"""TMAP and GMAP on the emulated sinks.

Every sink has a TMAS (bumble.profiles.tmap) with TMAP Role --tmap-role
(default 0x0008, Unicast Media Receiver; 0 = no TMAS), as a TMAP headset
has. With --gmap-role the sink also has a GMAS (bumble.profiles.gmap): e.g.
--gmap-role 0x02 --gmap-ugt-features 0x3C is a Unicast Game Terminal with
the Sink, 64 kbps Sink, Multiplex and Multisink features. The sink PAC
record (bap.py: 16-48 kHz, 7.5 and 10 ms frames, 26-155 octets) already
covers all of GMAP's UGG -> UGT configurations (32_1_gr ... 48_4_gr).

--tmap-read-peer: once BlueCaster has connected and encrypted the link, the
sink reads BlueCaster's TMAS / GMAS (Read By Type) and logs
"peer TMAP Role 0x0004 (UMS)" / "peer GMAP Role 0x01 (UGG), UGG Features
0x03" to sink.log for the run_test.py checks.

--gmap-available-contexts 0xNNNN replaces the sink's Available Audio
Contexts (PACS) after bap.py set them up, e.g. Media without Game.
"""

import asyncio
import logging
import struct

from bumble import gatt
from bumble.device import Peer
from bumble.profiles.gmap import GamingAudioService, GmapRole, UgtFeatures
from bumble.profiles.tmap import Role, TelephonyAndMediaAudioService

log = logging.getLogger('emu_le_audio')

TMAP_NAMES = ['CG', 'CT', 'UMS', 'UMR', 'BMS', 'BMR']
GMAP_NAMES = ['UGG', 'UGT', 'BGS', 'BGR']


def names(value, table):
    return '+'.join(n for i, n in enumerate(table) if value & (1 << i)) or 'none'


def add_arguments(parser):
    parser.add_argument('--tmap-role', type=lambda s: int(s, 0), default=int(Role.UNICAST_MEDIA_RECEIVER),
                        help='TMAP Role of the sinks\' TMAS (default 0x0008 UMR; 0: no TMAS)')
    parser.add_argument('--gmap-role', type=lambda s: int(s, 0), default=0,
                        help='GMAP Role of a GMAS on the sinks (e.g. 0x02 UGT; default: no GMAS)')
    parser.add_argument('--gmap-ugt-features', type=lambda s: int(s, 0), default=0,
                        help='UGT Features of the GMAS (e.g. 0x3C: sink, 64 kbps sink, multiplex, multisink)')
    parser.add_argument('--tmap-read-peer', action='store_true',
                        help='read and log BlueCaster\'s TMAP / GMAP roles once it connected')
    parser.add_argument('--gmap-available-contexts', type=lambda s: int(s, 0),
                        help='Available Audio Contexts (sink) of the PACS instead of bap.py\'s, e.g. 0x0006 '
                             '(Conversational + Media, no Game)')


def configure_sink(sink, args):
    pacs = sink.state.get('bap', {}).get('pacs')
    if args.gmap_available_contexts is not None and pacs is not None:
        pacs.available_audio_contexts.value = struct.pack('<HH', args.gmap_available_contexts, 0)
    if args.tmap_role:
        sink.device.add_service(TelephonyAndMediaAudioService(Role(args.tmap_role)))
    if args.gmap_role:
        sink.device.add_service(GamingAudioService(GmapRole(args.gmap_role),
                                                   ugt_features=UgtFeatures(args.gmap_ugt_features)))


async def read_peer_roles(sink, connection):
    peer = Peer(connection)
    for service_uuid, chr_uuid, label, table, size in (
            (gatt.GATT_TELEPHONY_AND_MEDIA_AUDIO_SERVICE, gatt.GATT_TMAP_ROLE_CHARACTERISTIC, 'TMAP Role',
             TMAP_NAMES, 2),
            (gatt.GATT_GAMING_AUDIO_SERVICE, gatt.GATT_GMAP_ROLE_CHARACTERISTIC, 'GMAP Role', GMAP_NAMES, 1)):
        services = await peer.discover_service(service_uuid)
        if not services:
            log.info('%s: peer has no %s service', sink.name, label.split()[0])
            continue
        values = await peer.read_characteristics_by_uuid(chr_uuid, services[0])
        if not values:
            log.info('%s: peer %s missing', sink.name, label)
            continue
        role = int.from_bytes(values[0][:size], 'little')
        text = f'{sink.name}: peer {label} 0x{role:0{size * 2}x} ({names(role, table)})'
        if chr_uuid == gatt.GATT_GMAP_ROLE_CHARACTERISTIC and role & GmapRole.UNICAST_GAME_GATEWAY:
            features = await peer.read_characteristics_by_uuid(gatt.GATT_UGG_FEATURES_CHARACTERISTIC, services[0])
            text += f', UGG Features 0x{features[0][0]:02x}' if features else ', no UGG Features'
        log.info('%s', text)


async def start(sink, args):
    if args.tmap_role:
        log.info('%s: TMAS, TMAP Role 0x%04x (%s)', sink.name, args.tmap_role, names(args.tmap_role, TMAP_NAMES))
    if args.gmap_role:
        log.info('%s: GMAS, GMAP Role 0x%02x (%s), UGT Features 0x%02x', sink.name, args.gmap_role,
                 names(args.gmap_role, GMAP_NAMES), args.gmap_ugt_features)
    if not args.tmap_read_peer:
        return

    def on_connection(connection):
        def on_encrypted():
            if connection.encryption and not getattr(connection, 'tmap_read', False):
                connection.tmap_read = True

                async def run():
                    try:
                        await read_peer_roles(sink, connection)
                    except Exception as e:  # noqa: BLE001 (logged for the checks)
                        log.info('%s: reading the peer roles failed: %r', sink.name, e)
                asyncio.get_running_loop().create_task(run())
        connection.on(connection.EVENT_CONNECTION_ENCRYPTION_CHANGE, on_encrypted)
    sink.device.on('connection', on_connection)
