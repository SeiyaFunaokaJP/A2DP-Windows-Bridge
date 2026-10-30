"""MCP on the emulated sinks: a Media Control Client of BlueCaster's GMCS.

The PC is the media control server (GMCS); with --mcp-client the first sink
is the client (as earbuds are): once BlueCaster's link is encrypted it discovers
GMCS with Bumble's GenericMediaControlServiceProxy, reads every
characteristic, subscribes and writes the --mcp-ops control point
operations, logging what it reads, the notifications and the result codes
(run_test.py checks sink.log, le_audio_tests/mcp.py).

Every sink also logs the metadata of each ASE Enable (Streaming Audio
Contexts, CCID List) for the CCID check, with or without --mcp-client.
"""

import asyncio
import logging
import struct

from bumble.att import (ATT_Error, ATT_Find_Information_Request, ATT_Read_Blob_Request, ATT_Read_Request,
                        Opcode)
from bumble.crypto import aes_cmac
from bumble.device import Peer
from bumble.profiles.ascs import AseStateMachine
from bumble.profiles.mcp import (
    GenericMediaControlServiceProxy,
    MediaControlPointOpcode,
    MediaControlPointResultCode,
    MediaState,
)

log = logging.getLogger('emu_le_audio')

OPS = {
    'play': 0x01, 'pause': 0x02, 'fast-rewind': 0x03, 'fast-forward': 0x04, 'stop': 0x05,
    'move-relative': 0x10, 'first-segment': 0x22, 'prev': 0x30, 'next': 0x31, 'goto-track': 0x34,
}
NOTIFY_TIMEOUT = 5.0


def add_arguments(parser):
    parser.add_argument('--mcp-client', action='store_true',
                        help='the first sink is a Media Control Client of the GMCS of BlueCaster')
    parser.add_argument('--mcp-ops', default='',
                        help='control point writes after reading, comma separated: play, pause, stop, '
                             'next, prev, move-relative:<0.01 s>, any opcode as 0xNN, raw:<hex bytes>; '
                             'a trailing ! writes with response')
    parser.add_argument('--mcp-unsubscribed-write', action='store_true',
                        help='first write Play before subscribing to the control point (carried out, no '
                             'result notification)')
    parser.add_argument('--mcp-long-read-race', action='store_true',
                        help='after subscribing: read the first part of the track title, Next Track, then '
                             'the rest (Read Blob): expects ATT error 0x80')
    parser.add_argument('--mcp-check-db-hash', action='store_true',
                        help="read every attribute of BlueCaster's GATT database, compute its Database Hash "
                             '(Core Vol 3 Part G 7.3.1) and compare it with the Database Hash characteristic')
    parser.add_argument('--mcp-resume', action='store_true',
                        help='on the reconnection after a link loss (bonded): do not subscribe again, write '
                             'Play and expect the notifications the first link enabled (persistent CCC)')
    parser.add_argument('--mcp-delay', type=float, default=0.5,
                        help='seconds between encryption and the first GMCS request')
    parser.add_argument('--mcp-mtu', type=int,
                        help='largest ATT MTU the sink accepts (BlueCaster negotiates the MTU)')


def configure_sink(sink, args):
    if args.mcp_mtu and sink.index == 0:
        sink.device.gatt_server.max_mtu = args.mcp_mtu
    ascs = sink.state.get('bap', {}).get('ascs')
    if ascs is None:
        return

    def on_state(ase):
        if ase.state != AseStateMachine.State.ENABLING:
            return
        parts = []
        for entry in ase.metadata.entries:
            value = entry.decode()
            if isinstance(value, bytes):
                value = value.hex()
            elif hasattr(value, 'name') and value.name:
                value = value.name
            parts.append(f'{entry.tag.name}={value}')
        log.info('%s: MCP: ASE %d Enable metadata: %s', sink.name, ase.ase_id, ' '.join(parts) or '(none)')

    for ase in ascs.ase_state_machines.values():
        ase.on('state_change', lambda ase=ase: on_state(ase))


async def start(sink, args):
    if not args.mcp_client or sink.index != 0:
        return
    loop = asyncio.get_running_loop()

    def on_connection(connection):
        sink.state['mcp_links'] = sink.state.get('mcp_links', 0) + 1
        if args.mcp_resume and sink.state['mcp_links'] > 1:
            loop.create_task(run_resumed(sink, args, connection))
        else:
            loop.create_task(run_client(sink, args, connection))

    sink.device.on('connection', on_connection)


async def wait_encrypted(connection):
    if not connection.is_encrypted:
        encrypted = asyncio.get_running_loop().create_future()
        connection.once(connection.EVENT_CONNECTION_ENCRYPTION_CHANGE,
                        lambda: encrypted.done() or encrypted.set_result(True))
        await asyncio.wait_for(encrypted, 30)


async def run_resumed(sink, args, connection):
    """A reconnection of the bonded client: its CCC values persist on the
    server (Core Vol 3 Part G 3.3.3.3), so notifications come without
    subscribing again. Handlers go straight into Bumble's GATT client."""
    name = sink.name
    try:
        await wait_encrypted(connection)
        await asyncio.sleep(args.mcp_delay)
        peer = Peer(connection)
        gmcs = await peer.discover_service_and_create_proxy(GenericMediaControlServiceProxy)
        if gmcs is None:
            log.info('%s: MCP: resumed: no GMCS', name)
            return
        client = peer.gatt_client
        cp_answers = asyncio.Queue()
        states = asyncio.Queue()
        client.notification_subscribers.setdefault(gmcs.media_control_point.handle, set()).add(
            cp_answers.put_nowait)
        client.notification_subscribers.setdefault(gmcs.media_state.handle, set()).add(states.put_nowait)
        log.info('%s: MCP: resumed on the new link, not subscribing again', name)
        await gmcs.media_control_point.write_value(bytes([OPS['play']]), with_response=True)
        try:
            answer = await asyncio.wait_for(cp_answers.get(), NOTIFY_TIMEOUT)
            log.info('%s: MCP: resumed: control point play -> %s', name,
                     MediaControlPointResultCode(answer[1]).name if len(answer) > 1 else answer.hex())
        except asyncio.TimeoutError:
            log.info('%s: MCP: resumed: control point play -> no notification', name)
        try:
            state = await asyncio.wait_for(states.get(), NOTIFY_TIMEOUT)
            log.info('%s: MCP: resumed: notify media_state = %s', name, MediaState(state[0]).name)
        except asyncio.TimeoutError:
            log.info('%s: MCP: resumed: no media_state notification', name)
        await gmcs.media_control_point.write_value(bytes([OPS['pause']]), with_response=True)
        log.info('%s: MCP: resumed client done', name)
    except Exception as e:  # noqa: BLE001  (log it for the check)
        log.info('%s: MCP: resumed client failed: %r', name, e)


async def check_db_hash(name, peer):
    """The Database Hash BlueCaster serves against one computed here from the
    attributes it serves (an independent check of its hash)."""
    client = peer.gatt_client
    attributes, start = [], 1
    while start <= 0xFFFF:
        response = await client.send_request(ATT_Find_Information_Request(starting_handle=start,
                                                                           ending_handle=0xFFFF))
        if response.op_code == Opcode.ATT_ERROR_RESPONSE or not response.information:
            break
        attributes += response.information
        start = attributes[-1][0] + 1
    message, hash_handle = bytearray(), None
    for handle, attribute_type in attributes:
        if len(attribute_type) != 2:
            continue                                    # 128-bit types are not hashed
        uuid16 = int.from_bytes(attribute_type, 'little')
        if uuid16 in (0x2800, 0x2801, 0x2802, 0x2803, 0x2900):  # declarations, Extended Properties: with value
            value = (await client.send_request(ATT_Read_Request(attribute_handle=handle))).attribute_value
            message += struct.pack('<HH', handle, uuid16) + value
            if uuid16 == 0x2803 and int.from_bytes(value[3:5], 'little') == 0x2B2A:
                hash_handle = int.from_bytes(value[1:3], 'little')
        elif uuid16 in (0x2901, 0x2902, 0x2903, 0x2904, 0x2905):  # other descriptors: handle and type
            message += struct.pack('<HH', handle, uuid16)
    if hash_handle is None:
        log.info('%s: GATT: no Database Hash characteristic', name)
        return
    served = (await client.send_request(ATT_Read_Request(attribute_handle=hash_handle))).attribute_value
    computed = aes_cmac(bytes(message), bytes(16))[::-1]  # sent least significant octet first
    log.info('%s: GATT: Database Hash %s over %d attributes, computed %s: %s', name, bytes(served).hex(),
             len(attributes), computed.hex(), 'match' if bytes(served) == computed else 'MISMATCH')


async def long_read_race(name, peer, gmcs):
    """First part of a long read of the track title, then Next Track (a new
    title), then the next part: the server must answer Value Changed During
    Read Long (0x80)."""
    client = peer.gatt_client
    handle = gmcs.track_title.handle
    first = await client.read_value(handle, no_long_read=True)
    if len(first) < client.mtu - 1:
        log.info('%s: MCP: long-read-race: the title (%d bytes) fits one PDU (MTU %d)', name, len(first),
                 client.mtu)
        return
    await gmcs.media_control_point.write_value(bytes([OPS['next']]), with_response=True)
    await asyncio.wait_for(gmcs.media_control_point_notifications.get(), NOTIFY_TIMEOUT)
    response = await client.send_request(ATT_Read_Blob_Request(attribute_handle=handle, value_offset=len(first)))
    if response.op_code == Opcode.ATT_ERROR_RESPONSE:
        log.info('%s: MCP: long-read-race: Read Blob -> ATT error 0x%02X', name, response.error_code)
    else:
        log.info('%s: MCP: long-read-race: Read Blob -> %d bytes, no error', name,
                 len(response.part_attribute_value))
    # Back to track 1 for the final state
    await gmcs.media_control_point.write_value(bytes([OPS['prev']]), with_response=True)
    await asyncio.wait_for(gmcs.media_control_point_notifications.get(), NOTIFY_TIMEOUT)


def text(value):
    return value.decode('utf-8', errors='replace')


def parse_op(token):
    """('name', bytes, with_response) of an --mcp-ops item"""
    with_response = token.endswith('!')
    token = token.rstrip('!')
    if token.startswith('raw:'):
        return token, bytes.fromhex(token[4:]), with_response
    name, _, arg = token.partition(':')
    opcode = OPS[name] if name in OPS else int(name, 0)
    data = bytes([opcode])
    if arg:
        data += struct.pack('<i', int(arg, 0))
    return token, data, with_response


async def run_client(sink, args, connection):
    name = sink.name
    try:
        await wait_encrypted(connection)
        await asyncio.sleep(args.mcp_delay)
        peer = Peer(connection)
        gmcs = await peer.discover_service_and_create_proxy(GenericMediaControlServiceProxy)
        if gmcs is None:
            log.info('%s: MCP: no GMCS on %s', name, connection.peer_address)
            return
        log.info('%s: MCP: GMCS found on %s, ATT MTU %d', name, connection.peer_address, connection.att_mtu)
        if args.mcp_check_db_hash:
            await check_db_hash(name, peer)

        values = {}
        for field, decode in (
            ('media_player_name', text), ('track_title', text),
            ('track_duration', lambda v: struct.unpack('<i', v)[0]),
            ('track_position', lambda v: struct.unpack('<i', v)[0]),
            ('playback_speed', lambda v: struct.unpack('<b', v)[0]),
            ('seeking_speed', lambda v: struct.unpack('<b', v)[0]),
            ('media_state', lambda v: MediaState(v[0]).name),
            ('media_control_point_opcodes_supported', lambda v: f'0x{struct.unpack("<I", v)[0]:08x}'),
            ('content_control_id', lambda v: v[0]),
        ):
            proxy = getattr(gmcs, field)
            if proxy is None:
                log.info('%s: MCP: %s missing', name, field)
                continue
            raw = await proxy.read_value()
            values[field] = decode(raw)
            log.info('%s: MCP: read %s = %r (%d bytes)', name, field, values[field], len(raw))
        for field in ('track_changed', 'media_control_point'):
            log.info('%s: MCP: %s %s', name, field, 'present' if getattr(gmcs, field) else 'missing')

        if args.mcp_unsubscribed_write:
            try:
                await gmcs.media_control_point.write_value(bytes([OPS['play']]), with_response=True)
                log.info('%s: MCP: unsubscribed write accepted', name)
            except ATT_Error as e:
                log.info('%s: MCP: unsubscribed write -> ATT error 0x%02X', name, e.error_code)

        gmcs.on(gmcs.EVENT_MEDIA_STATE, lambda s: log.info('%s: MCP: notify media_state = %s', name, s.name))
        gmcs.on(gmcs.EVENT_TRACK_CHANGED, lambda: log.info('%s: MCP: notify track_changed', name))
        gmcs.on(gmcs.EVENT_TRACK_TITLE,
                lambda t: log.info('%s: MCP: notify track_title = %r (%d bytes)', name, t, len(t.encode())))
        gmcs.on(gmcs.EVENT_TRACK_DURATION, lambda d: log.info('%s: MCP: notify track_duration = %d', name, d))
        gmcs.on(gmcs.EVENT_TRACK_POSITION, lambda p: log.info('%s: MCP: notify track_position = %d', name, p))
        await gmcs.subscribe_characteristics()
        for field in ('media_player_name', 'media_control_point_opcodes_supported'):
            proxy = getattr(gmcs, field)
            if proxy is not None:
                await proxy.subscribe(lambda v, f=field: log.info('%s: MCP: notify %s = %s', name, f, v.hex()))
        log.info('%s: MCP: subscribed', name)
        if args.mcp_long_read_race:
            await long_read_race(name, peer, gmcs)

        for token in [t.strip() for t in args.mcp_ops.split(',') if t.strip()]:
            label, data, with_response = parse_op(token)
            while not gmcs.media_control_point_notifications.empty():
                gmcs.media_control_point_notifications.get_nowait()
            try:
                await gmcs.media_control_point.write_value(data, with_response=with_response)
            except ATT_Error as e:
                log.info('%s: MCP: control point %s -> ATT error 0x%02X', name, label, e.error_code)
                continue
            try:
                answer = await asyncio.wait_for(gmcs.media_control_point_notifications.get(), NOTIFY_TIMEOUT)
            except asyncio.TimeoutError:
                log.info('%s: MCP: control point %s -> no notification', name, label)
                continue
            opcode = MediaControlPointOpcode(answer[0])
            result = MediaControlPointResultCode(answer[1]) if len(answer) > 1 else None
            log.info('%s: MCP: control point %s -> opcode 0x%02x %s', name, label, opcode,
                     result.name if result is not None else answer.hex())
            await asyncio.sleep(0.3)        # let state notifications arrive in order
        position = struct.unpack('<i', await gmcs.track_position.read_value())[0]
        state = MediaState((await gmcs.media_state.read_value())[0]).name
        title = text(await gmcs.track_title.read_value())
        log.info('%s: MCP: final media_state = %s, track_position = %d, track_title = %r', name, state, position,
                 title)
        log.info('%s: MCP: client done', name)
    except Exception as e:  # noqa: BLE001  (log it for the check)
        log.info('%s: MCP: client failed: %r', name, e)
