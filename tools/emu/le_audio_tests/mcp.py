"""MCP checks of run_test.py --le-audio: BlueCaster's GMCS (media control server).

The sink is the Media Control Client (emu --mcp-client, le_audio/mcp.py) and
logs what it reads, the notifications and the control point results to
sink.log. BlueCaster runs with --mcp-dry-run: its simulated player answers and
the commands are only logged, nothing reaches the media players of the PC.

    mcp-read         every GMCS characteristic readable with the simulated
                     player's values; the GATT server's Database Hash matches
                     one the client computes from the attributes it reads
    mcp-control      Play / Pause / Next / Previous / Move Relative / Stop:
                     result Success, state / track / position notifications,
                     BlueCaster logs the dry-run commands; ATT MTU 64 so the long
                     track title is read in parts and cut in notifications
    mcp-unsupported  unsupported opcodes -> Opcode Not Supported, a wrong
                     length -> Command Cannot Be Completed, a write before
                     subscribing is carried out without a result
                     notification (as Zephyr's mcs.c)
    mcp-long-read    the track title changes (Next Track) between the parts
                     of a long read -> ATT error 0x80
    mcp-resume       the link drops and BlueCaster reconnects (bond reused): the
                     client does not subscribe again and still gets the
                     notifications (CCC values persist for a bonded client)
    mcp-ccid         both earbuds get CCID List [CCID_GMCS] and Streaming
                     Audio Contexts Media in the Enable metadata
"""

import re

from .case import Case

STEREO = r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 1 CIS'
EARBUDS = r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 2 CIS'

PLAYER = 'BlueCaster test player'
DURATION = 18000                       # simulated track: 180.00 s
OPCODES = 0x00001833                   # Play, Pause, Stop, Move Relative, Previous / Next Track
CCID_GMCS = 1

CASES = [
    Case('mcp-read', 'stereo', STEREO, sink_args=['--mcp-client', '--mcp-check-db-hash'], bluecaster_args=['--mcp-dry-run'],
         tags=('mcp', 'mcp-read')),
    Case('mcp-control', 'stereo', STEREO,
         sink_args=['--mcp-client', '--mcp-mtu', '64',
                    '--mcp-ops', 'play,pause!,next,prev,move-relative:1500,stop'],
         bluecaster_args=['--mcp-dry-run'], tags=('mcp', 'mcp-control'), duration=8),
    Case('mcp-unsupported', 'stereo', STEREO,
         sink_args=['--mcp-client', '--mcp-unsubscribed-write',
                    '--mcp-ops', 'fast-rewind,first-segment,0x77,goto-track,raw:0101'],
         bluecaster_args=['--mcp-dry-run'], tags=('mcp', 'mcp-unsupported'), duration=7),
    Case('mcp-ccid', 'earbuds', EARBUDS, bluecaster_args=['--mcp-dry-run'], tags=('mcp', 'mcp-ccid')),
    Case('mcp-long-read', 'stereo', STEREO, sink_args=['--mcp-client', '--mcp-mtu', '64', '--mcp-long-read-race'],
         bluecaster_args=['--mcp-dry-run'], tags=('mcp', 'mcp-long-read'), duration=7),
    Case('mcp-resume', 'stereo', STEREO,
         sink_args=['--mcp-client', '--mcp-resume', '--drop-link-after', '4'],
         bluecaster_args=['--mcp-dry-run'], tags=('mcp', 'mcp-resume'), streams=2, lossy=True, duration=14),
]


def _reads(sink_log):
    """{characteristic: repr of the value} of the client's reads"""
    return dict(re.findall(r'MCP: read (\w+) = (.*) \(\d+ bytes\)', sink_log))


def _results(sink_log):
    """[(op label, result name)] of the control point writes"""
    return re.findall(r'MCP: control point (\S+) -> (?:opcode 0x[0-9a-f]+ )?(\w[\w ]*?)$', sink_log, re.M)


def _tail(title):
    """A simulated track title without its "Test track N" """
    return re.sub(r'^Test track \d+', '', title)


def _client(sink_log):
    failures = []
    m = re.search(r'MCP: client failed: (.*)', sink_log)
    if m:
        failures.append(f'MCP client failed: {m.group(1)}')
    elif 'MCP: client done' not in sink_log:
        failures.append('the MCP client did not finish (sink.log)')
    return failures


def _check_read(ctx):
    log = ctx['sink_log']
    failures = _client(log)
    r = _reads(log)
    expect = {
        'media_player_name': repr(PLAYER),
        'track_duration': str(DURATION),
        'track_position': '0',
        'playback_speed': '0',
        'seeking_speed': '0',
        'media_state': repr('PAUSED'),
        'media_control_point_opcodes_supported': repr(f'0x{OPCODES:08x}'),
        'content_control_id': str(CCID_GMCS),
    }
    for field, value in expect.items():
        if r.get(field) != value:
            failures.append(f'read {field} = {r.get(field)} (expected {value})')
    if not r.get('track_title', '').startswith("'Test track 1"):
        failures.append(f'read track_title = {r.get("track_title")}')
    for field in ('track_changed', 'media_control_point'):
        if f'MCP: {field} present' not in log:
            failures.append(f'GMCS has no {field}')
    if 'MCP: subscribed' not in log:
        failures.append('the client could not subscribe')
    m = re.search(r'GATT: Database Hash .*', log)
    if not m or not m.group(0).endswith(': match'):
        failures.append(f"Database Hash of BlueCaster's GATT server: {m.group(0) if m else 'not read'}")
    return failures


def _check_control(ctx):
    log, bluecaster = ctx['sink_log'], ctx['bluecaster_log']
    failures = _client(log)
    results = _results(log)
    want = ['play', 'pause', 'next', 'prev', 'move-relative:1500', 'stop']
    if [op for op, _ in results] != want:
        failures.append(f'control point answers {results} (expected one per {want})')
    failures += [f'control point {op} -> {res}' for op, res in results if res != 'SUCCESS']
    for name in ('PLAY', 'PAUSE', 'NEXT_TRACK', 'PREVIOUS_TRACK', 'MOVE_RELATIVE 1500', 'STOP'):
        if f'MCP: dry run: {name} (nothing sent to Windows)' not in bluecaster:
            failures.append(f'BlueCaster did not log the dry-run command {name}')
    if 'media key' in bluecaster or re.search(r'MCP: \w+ -> "', bluecaster):
        failures.append('BlueCaster sent a command to Windows in a dry run')
    for note in ('notify media_state = PLAYING', 'notify media_state = PAUSED', 'notify track_changed',
                 'notify track_position = 1500', 'notify track_position = 0', 'notify track_duration = 18000'):
        if f'MCP: {note}' not in log:
            failures.append(f'no "{note}" at the client')
    titles = re.findall(r"MCP: notify track_title = '(.*)' \((\d+) bytes\)", log)
    if not any(t.startswith('Test track 2') for t, _ in titles):
        failures.append(f'no track title notification for track 2: {titles}')
    m = re.search(r'ATT MTU (\d+)', log)
    mtu = int(m.group(1)) if m else 0
    if mtu != 64:
        failures.append(f'ATT MTU {mtu} (the case asks the sink for 64)')
    full = re.search(r"MCP: read track_title = '(.*)' \((\d+) bytes\)", log)
    if not full or int(full.group(2)) <= mtu - 1:
        failures.append(f'the track title is not a long value: {full.group(0) if full else None}')
    for title, size in titles:
        if int(size) > mtu - 3 or '�' in title or (full and not _tail(full.group(1)).startswith(_tail(title))):
            failures.append(f'track title notification not cut to the MTU at a character: {title!r} ({size} bytes)')
    m = re.search(r"MCP: final media_state = (\w+), track_position = (-?\d+), track_title = '(.*)'", log)
    if not m or m.group(1) != 'PAUSED' or m.group(2) != '0' or not m.group(3).startswith('Test track 1'):
        failures.append(f'final state {m.groups() if m else None} (expected PAUSED at 0 on track 1)')
    return failures


def _check_unsupported(ctx):
    log, bluecaster = ctx['sink_log'], ctx['bluecaster_log']
    failures = _client(log)
    if 'MCP: unsubscribed write accepted' not in log:
        failures.append('a control point write before subscribing was refused')
    # The result goes out after the write response: the client's CCC write
    # can land in between, so the result may or may not be notified
    if 'MCP: control point PLAY (0x01) -> Success' not in bluecaster:
        failures.append('BlueCaster did not carry out the Play written before subscribing (without notification)')
    want = [('fast-rewind', 'OPCODE_NOT_SUPPORTED'), ('first-segment', 'OPCODE_NOT_SUPPORTED'),
            ('0x77', 'OPCODE_NOT_SUPPORTED'), ('goto-track', 'OPCODE_NOT_SUPPORTED'),
            ('raw:0101', 'COMMAND_CANNOT_BE_COMPLETED')]
    results = _results(log)
    if results != want:
        failures.append(f'control point answers {results} (expected {want})')
    done = re.findall(r'MCP: dry run: (\w+)', bluecaster.replace('MCP: dry run: simulated player', ''))
    if done != ['PLAY']:
        failures.append(f'BlueCaster carried out {done} (expected only the PLAY written before subscribing)')
    return failures


def _check_ccid(ctx):
    failures = []
    log = ctx['sink_log']
    lines = re.findall(r'(BlueCaster LE[^:]*): MCP: ASE (\d+) Enable metadata: (.*)', log)
    sinks = {name for name, _, _ in lines}
    if len(sinks) != len(ctx['sink_addresses']):
        failures.append(f'Enable metadata seen on {len(sinks)} sink(s), expected {len(ctx["sink_addresses"])}')
    for name, ase, meta in lines:
        if f'CCID_LIST=[{CCID_GMCS}]' not in meta.split() or 'STREAMING_AUDIO_CONTEXTS=MEDIA' not in meta.split():
            failures.append(f'{name} ASE {ase}: Enable metadata "{meta}" (expected Media and CCID List [1])')
    if f'Enable metadata: contexts 0x0004, CCID list {CCID_GMCS}' not in ctx['bluecaster_log']:
        failures.append('BlueCaster did not log the CCID List of the Enable')
    return failures


def _check_long_read(ctx):
    log, bluecaster = ctx['sink_log'], ctx['bluecaster_log']
    failures = _client(log)
    if 'MCP: long-read-race: Read Blob -> ATT error 0x80' not in log:
        m = re.search(r'MCP: long-read-race: .*', log)
        failures.append(f'the long read across a track change did not fail with 0x80: {m.group(0) if m else None}')
    if not re.search(r'MCP: 0x[0-9a-f]{4} changed during a long read by', bluecaster):
        failures.append('BlueCaster did not log the change during the long read')
    return failures


def _check_resume(ctx):
    log = ctx['sink_log']
    failures = []
    if 'MCP: subscribed' not in log:
        failures.append('the client did not subscribe on the first link')
    if 'MCP: resumed on the new link' not in log:
        failures.append('the client did not run on the second link')
    if 'MCP: resumed: control point play -> SUCCESS' not in log:
        failures.append('no control point notification on the new link without subscribing again')
    if 'MCP: resumed: notify media_state = PLAYING' not in log:
        failures.append('no Media State notification on the new link without subscribing again')
    m = re.search(r'MCP: resumed client failed: (.*)', log)
    if m:
        failures.append(f'resumed client failed: {m.group(1)}')
    if 'LE Audio stream restored' not in ctx['bluecaster_log']:
        failures.append('the stream was not set up again after the link loss')
    return failures


def check(case, ctx):
    if 'mcp' not in case.tags:
        return []
    failures = []
    if f'MCP: GMCS on the PC (CCID {CCID_GMCS}), dry run' not in ctx['bluecaster_log']:
        failures.append('BlueCaster did not set up GMCS in dry-run mode')
    failures += _check_ccid(ctx)
    for tag, fn in (('mcp-read', _check_read), ('mcp-control', _check_control),
                    ('mcp-unsupported', _check_unsupported), ('mcp-long-read', _check_long_read),
                    ('mcp-resume', _check_resume)):
        if tag in case.tags:
            failures += fn(ctx)
    return failures
