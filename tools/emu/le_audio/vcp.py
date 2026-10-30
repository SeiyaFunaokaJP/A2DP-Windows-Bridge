"""VCP on the emulated sinks: a Volume Control Service (VCS) renderer.

With --vcp every sink gets Bumble's VolumeControlService
(bumble.profiles.vcs) and logs, for run_test.py's checks (sink.log):

  VCS <sink>: Volume State read: volume V, mute M, counter C
              (BlueCaster's reads, and Bumble's for each notification)
  VCS <sink>: BlueCaster wrote <OPCODE> [V] (counter C): volume V, mute M, counter C
  VCS <sink>: BlueCaster wrote <OPCODE> [V] (counter C): rejected, error 0x80
  VCS <sink>: button: volume V, counter C (notified)
  VCS <sink>: change counter bumped to C without a notification

Options:
  --vcp                      VCS on every sink
  --vcp-volume V[,V2]        initial Volume_Setting (0-255) per sink (the last
                             one for the remaining sinks; default 100)
  --vcp-muted                the sinks start muted
  --vcp-press S:V[@N]        "button press": S seconds after BlueCaster connected,
                             sink N (default 0) sets volume V and notifies it
                             (repeatable)
  --vcp-press-on-write V[@N]   the first time BlueCaster writes Set Absolute
                             Volume to sink N (default 0), a button sets volume
                             V right after the write (notified while BlueCaster
                             reads the state back)
  --vcp-press-before-write V[@N]  the same, but the button is pressed just
                             before the write arrives: the write has a stale
                             change counter and is rejected with 0x80
  --vcp-bump-counter S       S seconds after BlueCaster first read Volume State,
                             every sink increments its change counter without
                             telling BlueCaster (a missed notification): BlueCaster's
                             next Control Point write gets "Invalid Change
                             Counter" (0x80) and has to read the state again
"""

import asyncio
import logging

from bumble import att
from bumble.profiles.vcs import VolumeControlPointOpcode, VolumeControlService

log = logging.getLogger('emu_le_audio')


class LoggingVolumeControlService(VolumeControlService):
    """Bumble's VCS server, logging what BlueCaster reads and writes."""

    def __init__(self, sink, **kwargs):
        super().__init__(**kwargs)
        self.sink = sink
        self.first_read = asyncio.Event()
        self.press_on_write = None      # (volume, before) for the first Set Absolute Volume

    def describe(self):
        return f'volume {self.volume_setting}, mute {self.muted}, counter {self.change_counter}'

    def _on_read_volume_state(self, connection):
        log.info('VCS %s: Volume State read: %s', self.sink.name, self.describe())
        self.first_read.set()
        return super()._on_read_volume_state(connection)

    def _on_write_volume_control_point(self, connection, value):
        try:
            name = VolumeControlPointOpcode(value[0]).name
        except (ValueError, IndexError):
            name = f'opcode 0x{value[0]:02x}' if value else 'empty'
        operand = f' {value[2]}' if len(value) > 2 else ''
        counter = value[1] if len(value) > 1 else -1
        what = f'VCS {self.sink.name}: BlueCaster wrote {name}{operand} (counter {counter})'
        press = None
        if self.press_on_write and value and value[0] == VolumeControlPointOpcode.SET_ABSOLUTE_VOLUME:
            press, self.press_on_write = self.press_on_write, None
        if press and press[1]:
            self.press_now(press[0], 'before the write')
        try:
            super()._on_write_volume_control_point(connection, value)
        except att.ATT_Error as error:
            log.info('%s: rejected, error 0x%02X', what, int(error.error_code))
            raise
        finally:
            if press and not press[1]:
                asyncio.get_running_loop().call_soon(self.press_now, press[0], 'after the write')
        log.info('%s: %s', what, self.describe())

    def press_now(self, volume, when):
        """A button pressed around BlueCaster's write: set and notify (in a task)."""
        self.volume_setting = volume
        self.change_counter = (self.change_counter + 1) % 256
        log.info('VCS %s: button %s: volume %d, counter %d (notified)', self.sink.name, when, volume,
                 self.change_counter)
        asyncio.get_running_loop().create_task(self.sink.device.notify_subscribers(attribute=self.volume_state))

    async def press(self, volume):
        """A button on the device: a new volume, notified to the subscribers."""
        self.volume_setting = volume
        self.change_counter = (self.change_counter + 1) % 256
        log.info('VCS %s: button: volume %d, counter %d (notified)', self.sink.name, volume,
                 self.change_counter)
        await self.sink.device.notify_subscribers(attribute=self.volume_state)

    def bump_counter(self):
        self.change_counter = (self.change_counter + 1) % 256
        log.info('VCS %s: change counter bumped to %d without a notification', self.sink.name,
                 self.change_counter)


def add_arguments(parser):
    parser.add_argument('--vcp', action='store_true', help='Volume Control Service on every sink')
    parser.add_argument('--vcp-volume', default='100', metavar='V[,V2]',
                        help='initial volume 0-255 per sink (default 100)')
    parser.add_argument('--vcp-muted', action='store_true', help='the sinks start muted')
    parser.add_argument('--vcp-press', action='append', default=[], metavar='S:V[@N]',
                        help='S seconds after BlueCaster connected, sink N sets volume V (button)')
    parser.add_argument('--vcp-press-on-write', metavar='V[@N]',
                        help="right after BlueCaster's first Set Absolute Volume to sink N, a button sets volume V")
    parser.add_argument('--vcp-press-before-write', metavar='V[@N]',
                        help="just before BlueCaster's first Set Absolute Volume to sink N, a button sets volume V")
    parser.add_argument('--vcp-bump-counter', type=float, metavar='S',
                        help='S seconds after the first Volume State read, bump the change counter silently')


def _volume_for(args, index):
    values = [int(v) for v in args.vcp_volume.split(',')]
    return values[min(index, len(values) - 1)]


def configure_sink(sink, args):
    if not args.vcp:
        return
    vcs = LoggingVolumeControlService(sink, volume_setting=_volume_for(args, sink.index),
                                      muted=1 if args.vcp_muted else 0, volume_flags=0x01)
    sink.device.add_service(vcs)
    sink.state['vcp'] = vcs
    for spec, before in ((args.vcp_press_on_write, False), (args.vcp_press_before_write, True)):
        if spec:
            volume, _, index = spec.partition('@')
            if int(index or 0) == sink.index:
                vcs.press_on_write = (int(volume), before)


async def start(sink, args):
    vcs = sink.state.get('vcp')
    if not vcs:
        return
    log.info('VCS %s: Volume Control Service, %s', sink.name, vcs.describe())
    loop = asyncio.get_running_loop()

    presses = []
    for item in args.vcp_press:
        spec, _, index = item.partition('@')
        seconds, _, volume = spec.partition(':')
        if int(index or 0) == sink.index:
            presses.append((float(seconds), int(volume)))
    if presses:
        started = []

        def on_connection(connection):
            if started:
                return
            started.append(True)

            async def run():
                start_time = loop.time()
                for seconds, volume in sorted(presses):
                    await asyncio.sleep(max(0.0, start_time + seconds - loop.time()))
                    await vcs.press(volume)
            loop.create_task(run())
        sink.device.on('connection', on_connection)

    if args.vcp_bump_counter is not None:
        async def bump():
            await vcs.first_read.wait()
            await asyncio.sleep(args.vcp_bump_counter)
            vcs.bump_counter()
        loop.create_task(bump())
