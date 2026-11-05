"""BAS on the emulated sinks: a Battery Service (Bumble's).

With --bas every sink gets a Battery Service and logs

  BAS <sink>: Battery Level read: L%        (A2DPWB's reads)
  BAS <sink>: Battery Level set to L% (notified)

Options:
  --bas L[,L2]       Battery Level (0-100) per sink (the last one for the
                     remaining sinks)
  --bas-change S:L[@N]  S seconds after A2DPWB connected, sink N (default 0)
                     sets its level to L and notifies it (repeatable)
"""

import asyncio
import logging

from bumble.profiles.battery_service import BatteryService

log = logging.getLogger('emu_le_audio')


def add_arguments(parser):
    parser.add_argument('--bas', metavar='L[,L2]', help='Battery Service on every sink, level 0-100 per sink')
    parser.add_argument('--bas-change', action='append', default=[], metavar='S:L[@N]',
                        help='S seconds after A2DPWB connected, sink N sets its battery level L (notified)')


def _level_for(args, index):
    values = [int(v) for v in args.bas.split(',')]
    return values[min(index, len(values) - 1)]


def configure_sink(sink, args):
    if not args.bas:
        return
    state = {'level': _level_for(args, sink.index)}

    def read(_connection):
        log.info('BAS %s: Battery Level read: %d%%', sink.name, state['level'])
        return state['level']

    service = BatteryService(read)
    sink.device.add_service(service)
    sink.state['bas'] = (service, state)


async def start(sink, args):
    entry = sink.state.get('bas')
    if not entry:
        return
    service, state = entry
    log.info('BAS %s: Battery Service, level %d%%', sink.name, state['level'])
    changes = []
    for item in args.bas_change:
        spec, _, index = item.partition('@')
        seconds, _, level = spec.partition(':')
        if int(index or 0) == sink.index:
            changes.append((float(seconds), int(level)))
    if not changes:
        return
    loop = asyncio.get_running_loop()
    started = []

    def on_connection(connection):
        if started:
            return
        started.append(True)

        async def run():
            start_time = loop.time()
            for seconds, level in sorted(changes):
                await asyncio.sleep(max(0.0, start_time + seconds - loop.time()))
                state['level'] = level
                log.info('BAS %s: Battery Level set to %d%% (notified)', sink.name, level)
                await sink.device.notify_subscribers(service.battery_level_characteristic, level)
        loop.create_task(run())
    sink.device.on('connection', on_connection)
