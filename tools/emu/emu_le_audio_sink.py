"""Virtual LE Audio unicast sink(s) for testing BlueCaster's experimental LE Audio.

Runs Bumble virtual controllers on one in-process link (as emu_sink.py does
for A2DP):
  - controller "BlueCaster" is served as H4 over TCP; BlueCaster connects to it with
    `bluecaster-core.exe --cli --hci-tcp 127.0.0.1:<port> --le-audio -d <sink> ...`
  - one or two sink controllers, each driven by a Bumble host acting as a BAP
    unicast server (PACS, ASCS, CAS; le_audio/bap.py) plus the profile
    modules of le_audio/ (CSIP, VCP, MCP, CCP, TMAP / GMAP)

The controllers carry HCI ISO data between each other (le_audio/
controller.py). Each sink's HCI traffic, ISO data included, is written to a
PacketLogger capture so bluecaster_decode --le-audio --received can check what
arrived.

Layouts (--layout):
  stereo   one headset, Front Left + Right, 2 sink ASEs, 1 or 2 channels per
           ASE (BlueCaster uses one 2-channel ASE: 1 CIS)
  two-ase  one headset, Front Left + Right, 2 sink ASEs of 1 channel (2 CIS)
  earbuds  two earbuds, Front Left / Front Right, 1 sink ASE each (2 CIS on
           2 LE links)
  mono     one device, no location (mono), 1 sink ASE

--privacy: the sinks use resolvable private addresses (a new one after each
link went down); READY then lists their current RPAs, not the public
(identity) addresses.

Run with the Python of the tools/emu virtual environment (see setup_env.py).
Prints "READY <host:port> sinks=<addr>[,<addr>]" once BlueCaster can connect;
stops on SIGTERM / Ctrl+C.
"""

import argparse
import asyncio
import logging
import os

import bumble.logging
from bumble.transport import open_transport

import le_audio
from le_audio.controller import LeEmuController, LeLocalLink
from le_audio.sink import SinkContext, create_device, current_address, power_on_and_advertise

BLUECASTER_ADDRESS = '00:A2:D0:00:00:11'
SINK_ADDRESSES = ['00:A2:D0:00:00:12', '00:A2:D0:00:00:13']
FL, FR = 0x1, 0x2

LAYOUTS = {
    # name: [(name, locations, sink ASEs, channel counts), ...]
    'stereo': [('BlueCaster LE', FL | FR, 2, [1, 2])],
    'two-ase': [('BlueCaster LE', FL | FR, 2, [1])],
    'earbuds': [('BlueCaster LE L', FL, 1, [1]), ('BlueCaster LE R', FR, 1, [1])],
    'mono': [('BlueCaster LE M', 0, 1, [1])],
}

log = logging.getLogger('emu_le_audio')


def capture_path(base, index):
    if not base or index == 0:
        return base
    root, ext = os.path.splitext(base)
    return f'{root}-{index + 1}{ext}'


async def main(args):
    link = LeLocalLink()
    transport = await open_transport(f'tcp-server:127.0.0.1:{args.port}')
    LeEmuController(
        'BlueCaster',
        host_source=transport.source,
        host_sink=transport.sink,
        link=link,
        public_address=BLUECASTER_ADDRESS,
    )
    sinks = []
    for index, (name, locations, ases, counts) in enumerate(LAYOUTS[args.layout]):
        sink = SinkContext(index=index, address=SINK_ADDRESSES[index], name=name, locations=locations,
                           sink_ases=ases, channel_counts=counts, privacy=args.privacy)
        create_device(link, sink, capture_path(args.capture, index))
        le_audio.configure_sink(sink, args)
        sinks.append(sink)
    for sink in sinks:
        await power_on_and_advertise(sink)
        await le_audio.start(sink, args)
    if args.drop_link_after:
        # Link loss test: the first sink ends its LE link once, some time
        # after BlueCaster connected (BlueCaster must set the stream up again)
        def on_connection(connection):
            async def drop():
                await asyncio.sleep(args.drop_link_after)
                log.info('%s: dropping the LE link (--drop-link-after)', sinks[0].name)
                await connection.disconnect()
            if not getattr(main, 'dropped', False):
                main.dropped = True
                asyncio.get_running_loop().create_task(drop())
        sinks[0].device.on('connection', on_connection)
    print(f'READY 127.0.0.1:{args.port} sinks={",".join(current_address(s) for s in sinks)}', flush=True)
    await asyncio.get_running_loop().create_future()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--port', type=int, default=9001, help='TCP port for BlueCaster (H4)')
    parser.add_argument('--capture', help='PacketLogger capture of the (first) sink; '
                                          'further sinks get -2, -3 before the extension')
    parser.add_argument('--layout', choices=sorted(LAYOUTS), default='stereo')
    parser.add_argument('--log-level', default='WARNING')
    parser.add_argument('--drop-link-after', type=float, metavar='SECONDS',
                        help='the first sink drops its LE link once, this long after BlueCaster connected')
    parser.add_argument('--privacy', action='store_true',
                        help='sinks advertise with resolvable private addresses (IRK distributed when paired) '
                             'and take a new one whenever a link went down')
    le_audio.add_arguments(parser)
    bumble.logging.setup_basic_logging(parser.parse_known_args()[0].log_level)
    logging.getLogger('emu_le_audio').setLevel(logging.INFO)
    asyncio.run(main(parser.parse_args()))
