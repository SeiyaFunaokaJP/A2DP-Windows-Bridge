"""CCP on the emulated sink: a Call Control Client of BlueCaster's GTBS.

The PC is the call control server (Generic Telephone Bearer Service, TBS 1.0);
with --ccp the first sink acts as the CCP client, as an earbud would. Bumble
0.0.234 has no TBS / CCP profile, so this uses its generic GATT client: once
BlueCaster has connected and the link is encrypted it discovers GTBS (0x184C),
checks that every mandatory characteristic is there with its properties,
reads them all (long reads included), subscribes to every notifying one and
then runs the --ccp-ops script. Everything goes to sink.log as "CCP: ..."
lines that le_audio_tests/ccp.py checks.

--ccp-mtu N makes the sinks accept at most this ATT MTU (BlueCaster negotiates
517 otherwise, and no TBS value needs a long read then).

--ccp-ops: comma-separated steps, run in order
    wait-incoming            wait for an Incoming Call notification; its call
                             becomes "the call"
    accept[:N] terminate[:N] hold[:N] retrieve[:N]
                             Call Control Point operation on call N (default:
                             the call); waits for the control point
                             notification
    originate:<uri>          Originate; on success the new call is "the call"
    raw:<hex>                write these bytes to the control point
    wait-end                 wait for a Termination Reason notification
    read-calls               read Call State, Bearer List Current Calls,
                             Incoming Call, Incoming Call Target Bearer URI
                             and Call Friendly Name
    read-race                long read of Bearer List Current Calls with a
                             change between its parts (Local Hold of the
                             call): expects Value Changed During Read Long
    interval:<s>             write the Bearer Signal Strength Reporting
                             Interval and read it back
    sleep:<s>
At the end: "CCP: final call state ..." and "CCP: script done".
"""

import asyncio
import logging
import struct

from bumble import att
from bumble.core import UUID
from bumble.device import Peer

log = logging.getLogger('emu_le_audio')

GTBS = UUID.from_16_bits(0x184C, 'Generic Telephone Bearer')

# (uuid16, name, mandatory, properties GTBS must have: R read, W write,
#  w write without response, N notify)
CHARACTERISTICS = [
    (0x2BB3, 'Bearer Provider Name', True, 'RN'),
    (0x2BB4, 'Bearer UCI', True, 'R'),
    (0x2BB5, 'Bearer Technology', True, 'RN'),
    (0x2BB6, 'Bearer URI Schemes Supported List', True, 'R'),
    (0x2BB7, 'Bearer Signal Strength', False, 'RN'),
    (0x2BB8, 'Bearer Signal Strength Reporting Interval', False, 'RWw'),
    (0x2BB9, 'Bearer List Current Calls', True, 'RN'),
    (0x2BBA, 'Content Control ID', True, 'R'),
    (0x2BBB, 'Status Flags', True, 'RN'),
    (0x2BBC, 'Incoming Call Target Bearer URI', False, 'RN'),
    (0x2BBD, 'Call State', True, 'RN'),
    (0x2BBE, 'Call Control Point', True, 'WwN'),
    (0x2BBF, 'Call Control Point Optional Opcodes', True, 'R'),
    (0x2BC0, 'Termination Reason', True, 'N'),
    (0x2BC1, 'Incoming Call', True, 'RN'),
    (0x2BC2, 'Call Friendly Name', False, 'RN'),
]
NAMES = {u: n for u, n, _, _ in CHARACTERISTICS}
PROP_BITS = {'R': 0x02, 'w': 0x04, 'W': 0x08, 'N': 0x10}

STATES = {0: 'Incoming', 1: 'Dialing', 2: 'Alerting', 3: 'Active', 4: 'Locally Held', 5: 'Remotely Held',
          6: 'Locally and Remotely Held'}
OPCODES = {0: 'Accept', 1: 'Terminate', 2: 'Local Hold', 3: 'Local Retrieve', 4: 'Originate', 5: 'Join'}
RESULTS = {0: 'Success', 1: 'Opcode Not Supported', 2: 'Operation Not Possible', 3: 'Invalid Call Index',
           4: 'State Mismatch', 5: 'Lack Of Resources', 6: 'Invalid Outgoing URI'}
OP_STEPS = {'accept': 0, 'terminate': 1, 'hold': 2, 'retrieve': 3}
TIMEOUT = 10.0


def add_arguments(parser):
    parser.add_argument('--ccp', action='store_true',
                        help='the first sink is a CCP client of BlueCaster\'s GTBS (reads, subscribes)')
    parser.add_argument('--ccp-ops', default='',
                        help='CCP client steps after the reads (see le_audio/ccp.py)')
    parser.add_argument('--ccp-mtu', type=int, default=0,
                        help='largest ATT MTU the sinks accept (long reads and cut notifications '
                             'with small values)')


def configure_sink(sink, args):
    if args.ccp_mtu:
        # BlueCaster (the GATT client) asks for the MTU; Bumble's server answers this
        sink.device.gatt_server.max_mtu = args.ccp_mtu


async def start(sink, args):
    if not (args.ccp or args.ccp_ops) or sink.index != 0:
        return

    def on_connection(connection):
        async def run():
            try:
                await CcpClient(sink, connection).run(args.ccp_ops)
            except Exception as e:   # noqa: BLE001 - goes to the log for the checks
                log.info('CCP: error %s: %s', type(e).__name__, e)
        if not sink.state.get('ccp_started'):
            sink.state['ccp_started'] = True
            asyncio.get_running_loop().create_task(run())
    sink.device.on('connection', on_connection)


def text(value):
    return value.decode('utf-8', errors='replace')


def fmt_state(value):
    calls = [f'{value[i]} {STATES.get(value[i + 1], "?")} flags 0x{value[i + 2]:02x}'
             for i in range(0, len(value) - 2, 3)]
    return '[' + ', '.join(calls) + ']'


def fmt_list(value):
    calls, i = [], 0
    while i < len(value):
        n = value[i]
        item = value[i + 1:i + 1 + n]
        if n < 3 or len(item) < n:
            calls.append('truncated item' if len(item) < n else f'malformed item at {i}')
            break
        calls.append(f'{item[0]} {STATES.get(item[1], "?")} flags 0x{item[2]:02x} {text(item[3:])}')
        i += 1 + n
    return '[' + ', '.join(calls) + ']'


def fmt_indexed(value):
    return f'call {value[0]} {text(value[1:])!r}' if value else 'none'


def fmt(uuid16, value):
    if uuid16 in (0x2BB3, 0x2BB4, 0x2BB6):
        return repr(text(value))
    if uuid16 in (0x2BB5, 0x2BB7, 0x2BB8, 0x2BBA) and len(value) == 1:
        return f'0x{value[0]:02x}'
    if uuid16 in (0x2BBB, 0x2BBF) and len(value) == 2:
        return f'0x{struct.unpack("<H", value)[0]:04x}'
    if uuid16 == 0x2BB9:
        return fmt_list(value)
    if uuid16 == 0x2BBD:
        return fmt_state(value)
    if uuid16 in (0x2BBC, 0x2BC1, 0x2BC2):
        return fmt_indexed(value)
    if uuid16 == 0x2BC0 and len(value) == 2:
        return f'call {value[0]} reason 0x{value[1]:02x}'
    if uuid16 == 0x2BBE and len(value) == 3:
        return f'{OPCODES.get(value[0], f"opcode 0x{value[0]:02x}")} call {value[1]} -> ' \
               f'{RESULTS.get(value[2], f"0x{value[2]:02x}")}'
    return value.hex() or '(empty)'


class CcpClient:
    def __init__(self, sink, connection):
        self.sink = sink
        self.connection = connection
        self.peer = Peer(connection)
        self.chars = {}                 # uuid16 -> CharacteristicProxy
        self.queues = {}                # uuid16 -> asyncio.Queue of notified values
        self.call = 0                   # "the call" of the script

    async def wait_encrypted(self):
        if self.connection.is_encrypted:
            return
        done = asyncio.Event()
        self.connection.on('connection_encryption_change', done.set)
        await asyncio.wait_for(done.wait(), 30)
        while not self.connection.is_encrypted:
            done.clear()
            await asyncio.wait_for(done.wait(), 30)

    async def run(self, ops):
        await self.wait_encrypted()
        services = await self.peer.discover_service(GTBS)
        if not services:
            log.info('CCP: no GTBS on BlueCaster')
            return
        service = services[0]
        for c in await self.peer.discover_characteristics(service=service):
            for uuid16 in NAMES:
                if c.uuid == UUID.from_16_bits(uuid16):
                    self.chars[uuid16] = c
        log.info('CCP: GTBS 0x%04x-0x%04x, %d characteristics, ATT MTU %d', service.handle,
                 service.end_group_handle, len(self.chars), self.peer.gatt_client.mtu)
        for uuid16, name, mandatory, props in CHARACTERISTICS:
            c = self.chars.get(uuid16)
            if c is None:
                if mandatory:
                    log.info('CCP: mandatory characteristic missing: %s', name)
                continue
            missing = [p for p in props if not c.properties & PROP_BITS[p]]
            if missing:
                log.info('CCP: %s lacks properties %s (has 0x%02x)', name, ''.join(missing), int(c.properties))
        for uuid16, name, _, props in CHARACTERISTICS:
            if uuid16 in self.chars and 'R' in props:
                await self.read(uuid16)
        # Subscribe to everything, Call State last (BlueCaster's --ccp-sim starts then)
        for uuid16, name, _, props in sorted(CHARACTERISTICS, key=lambda e: e[0] == 0x2BBD):
            c = self.chars.get(uuid16)
            if c is not None and c.properties & PROP_BITS['N']:
                self.queues[uuid16] = asyncio.Queue()
                await self.peer.subscribe(c, lambda value, u=uuid16: self.on_notify(u, value))
        log.info('CCP: subscribed, client ready')
        for step in [s.strip() for s in ops.split(',') if s.strip()]:
            await self.step(step)
        await self.read(0x2BBD, 'final')
        await self.read(0x2BB9, 'final')
        log.info('CCP: script done')

    def on_notify(self, uuid16, value):
        log.info('CCP: notify %s %s (%d bytes)', NAMES[uuid16], fmt(uuid16, value), len(value))
        self.queues[uuid16].put_nowait(bytes(value))

    async def read(self, uuid16, what='read'):
        try:
            value = await self.peer.read_value(self.chars[uuid16])
            log.info('CCP: %s %s = %s (%d bytes)', what, NAMES[uuid16], fmt(uuid16, value), len(value))
            return value
        except att.ATT_Error as e:
            log.info('CCP: %s %s -> ATT error 0x%02x', what, NAMES[uuid16], e.error_code)
            return None

    async def next_notification(self, uuid16):
        return await asyncio.wait_for(self.queues[uuid16].get(), TIMEOUT)

    async def write_cp(self, value, label):
        try:
            await self.peer.write_value(self.chars[0x2BBE], bytes(value), with_response=True)
        except att.ATT_Error as e:
            log.info('CCP: CP %s -> ATT error 0x%02x', label, e.error_code)
            return None
        result = await self.next_notification(0x2BBE)
        log.info('CCP: CP %s -> %s', label, fmt(0x2BBE, result))
        return result

    async def step(self, step):
        name, _, arg = step.partition(':')
        log.info('CCP: step %s', step)
        if name == 'wait-incoming':
            value = await self.next_notification(0x2BC1)
            self.call = value[0]
            log.info('CCP: incoming call %d from %s', self.call, text(value[1:]))
        elif name in OP_STEPS:
            index = int(arg, 0) if arg else self.call
            await self.write_cp([OP_STEPS[name], index], f'{name} {index}')
        elif name == 'originate':
            result = await self.write_cp([4] + list(arg.encode()), f'originate {arg}')
            if result and result[2] == 0:
                self.call = result[1]
        elif name == 'raw':
            await self.write_cp(bytes.fromhex(arg), f'raw {arg}')
        elif name == 'wait-end':
            value = await self.next_notification(0x2BC0)
            log.info('CCP: call %d ended, reason 0x%02x', value[0], value[1])
        elif name == 'read-calls':
            for uuid16 in (0x2BBD, 0x2BB9, 0x2BC1, 0x2BBC, 0x2BC2):
                if uuid16 in self.chars:
                    await self.read(uuid16)
        elif name == 'read-race':
            await self.read_race()
        elif name == 'interval':
            await self.peer.write_value(self.chars[0x2BB8], bytes([int(arg, 0)]), with_response=True)
            await self.read(0x2BB8)
        elif name == 'sleep':
            await asyncio.sleep(float(arg))
        else:
            log.info('CCP: unknown step %s', step)

    async def read_race(self):
        """First part of a long read of the call list, then a change (Local
        Hold of the call), then the next part: the server must answer with
        Value Changed During Read Long (0x80)."""
        client = self.peer.gatt_client
        handle = self.chars[0x2BB9].handle
        first = await client.read_value(handle, no_long_read=True)
        if len(first) < client.mtu - 1:
            log.info('CCP: read-race: the call list (%d bytes) fits one PDU (MTU %d), no long read',
                     len(first), client.mtu)
            return
        await self.write_cp([2, self.call], f'hold {self.call}')
        response = await client.send_request(att.ATT_Read_Blob_Request(attribute_handle=handle,
                                                                       value_offset=len(first)))
        if response.op_code == att.Opcode.ATT_ERROR_RESPONSE:
            log.info('CCP: read-race: Read Blob -> ATT error 0x%02x', response.error_code)
        else:
            log.info('CCP: read-race: Read Blob -> %d bytes, no error', len(response.part_attribute_value))
