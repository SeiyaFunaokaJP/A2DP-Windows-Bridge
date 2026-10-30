"""VCP cases and checks of run_test.py --le-audio (standard library only).

The sinks run a Volume Control Service (emu option --vcp, le_audio/vcp.py).
BlueCaster runs VolumeSync against a simulated Windows volume
(--le-sim-volume "<start>[,<s>:<level>...]": the real Windows volume is never
touched), so the cases go through the same path as the GUI: VolumeSync ->
Core::request_volume() / remote_volume() -> the VCP module -> VCS.

Levels: Windows / VolumeSync 0-127, VCS 0-255; 50 <-> 100, 90 <-> 181,
100 <-> 200, 30 <-> 60 (le_audio::vcp::to_setting / to_level).
"""

import re

from .case import Case

STEREO = r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 1 CIS'
TWO_CIS = r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 2 CIS'

CASES = [
    # Windows volume keys -> headset: the simulated Windows volume first takes
    # the headset's volume over (100/255 = 50/127, nothing written), then
    # changes to 90 -> Set Absolute Volume 181; the muted headset is unmuted
    Case('vcp-set', 'stereo', STEREO, sink_args=['--vcp', '--vcp-volume', '100', '--vcp-muted'],
         bluecaster_args=['--le-sim-volume', '64,3:90'], tags=('vcp',), duration=8),
    # Headset buttons -> Windows: the sink notifies volume 200 -> BlueCaster
    # reports 100/127 -> VolumeSync sets the (simulated) Windows volume; no
    # write back to the headset
    Case('vcp-remote', 'stereo', STEREO, sink_args=['--vcp', '--vcp-volume', '100', '--vcp-press', '6:200'],
         bluecaster_args=['--le-sim-volume', '64'], tags=('vcp',), duration=10),
    # The sink's change counter moves without a notification: the write is
    # rejected with 0x80, BlueCaster reads Volume State again and the retry works
    Case('vcp-change-counter', 'stereo', STEREO, sink_args=['--vcp', '--vcp-volume', '100', '--vcp-bump-counter', '1'],
         bluecaster_args=['--le-sim-volume', '64,4:90'], tags=('vcp',), duration=8),
    # Two earbuds (a group of 2): the right one (140) is set to the left one's
    # volume (100), a Windows change goes to both, a button on the right one
    # goes to the left one and to Windows
    Case('vcp-earbuds', 'earbuds', TWO_CIS,
         sink_args=['--vcp', '--vcp-volume', '100,140', '--vcp-press', '8:60@1'],
         bluecaster_args=['--le-sim-volume', '64,3:90'], tags=('vcp',), duration=12),
    # A button press while BlueCaster's own write runs: Windows -> 181 is written,
    # right after it the headset's button sets 40 (notified while BlueCaster reads
    # the state back). The device's change must not be lost: Windows follows
    # (20/127), 181 is not written again
    Case('vcp-press-during-write', 'stereo', STEREO,
         sink_args=['--vcp', '--vcp-volume', '100', '--vcp-press-on-write', '40'],
         bluecaster_args=['--le-sim-volume', '64,3:90'], tags=('vcp',), duration=8),
    # The same with the button pressed just before the write arrives: the
    # write is rejected (0x80, stale counter) and not retried, as the device's
    # change is newer
    Case('vcp-press-before-write', 'stereo', STEREO,
         sink_args=['--vcp', '--vcp-volume', '100', '--vcp-press-before-write', '40'],
         bluecaster_args=['--le-sim-volume', '64,3:90'], tags=('vcp',), duration=8),
]

WROTE = re.compile(r'VCS (.+?): BlueCaster wrote (\w+)(?: (\d+))? \(counter (\d+)\): (.*)$', re.M)


def _writes(sink_log):
    """[(sink, opcode, operand or None, counter, result)] in order"""
    return [(m.group(1), m.group(2), int(m.group(3)) if m.group(3) else None, int(m.group(4)), m.group(5))
            for m in WROTE.finditer(sink_log)]


def _sim_levels(bluecaster_log):
    """Levels VolumeSync gave the simulated Windows volume, in order"""
    return [int(x) for x in re.findall(r'simulated Windows volume set to (\d+)/127 \(by VolumeSync\)', bluecaster_log)]


def _accepted(writes, sink=None):
    return [(w[1], w[2]) for w in writes if not w[4].startswith('rejected') and (sink is None or w[0] == sink)]


def check(case, ctx):
    if 'vcp' not in case.tags:
        return []
    log, sink_log = ctx['bluecaster_log'], ctx['sink_log']
    failures = []
    writes = _writes(sink_log)
    sims = _sim_levels(log)
    if 'Volume State read:' not in sink_log:
        failures.append('BlueCaster did not read Volume State')
    if not re.search(r'VCP: group volume 100/255 \(50/127\)', log):
        failures.append('BlueCaster did not take the headset volume 100/255 (50/127)')
    if not sims or sims[0] != 50:
        failures.append(f'VolumeSync did not start from the headset volume 50: {sims}')

    if case.name == 'vcp-set':
        if _accepted(writes) != [('SET_ABSOLUTE_VOLUME', 181), ('UNMUTE', None)]:
            failures.append(f'expected Set Absolute Volume 181 and Unmute, the sink saw {writes}')
        if 'volume 181, mute 0' not in sink_log:
            failures.append('the sink did not end at volume 181, unmuted')
    elif case.name == 'vcp-remote':
        if 'VCP: 00:A2:D0:00:00:12: volume changed on the device to 200/255 (100/127)' not in log:
            failures.append('BlueCaster did not log the volume change of the device')
        if sims != [50, 100]:
            failures.append(f'simulated Windows volume steps {sims}, expected [50, 100]')
        if writes:
            failures.append(f'BlueCaster wrote to the headset although only the headset changed: {writes}')
    elif case.name == 'vcp-change-counter':
        rejected = [w for w in writes if w[4].startswith('rejected, error 0x80')]
        if len(rejected) != 1:
            failures.append(f'expected one write rejected with 0x80, the sink saw {writes}')
        if 'Invalid Change Counter (0x80), reading Volume State again' not in log or \
                'Set Absolute Volume 181: retrying with change counter' not in log:
            failures.append('BlueCaster did not report the retry')
        if _accepted(writes) != [('SET_ABSOLUTE_VOLUME', 181)]:
            failures.append(f'the retry did not set volume 181: {writes}')
    elif case.name == 'vcp-earbuds':
        left, right = 'BlueCaster LE L', 'BlueCaster LE R'
        if _accepted(writes, right) != [('SET_ABSOLUTE_VOLUME', 100), ('SET_ABSOLUTE_VOLUME', 181)]:
            failures.append(f'right earbud: expected 100 (group) then 181 (Windows): {_accepted(writes, right)}')
        if _accepted(writes, left) != [('SET_ABSOLUTE_VOLUME', 181), ('SET_ABSOLUTE_VOLUME', 60)]:
            failures.append(f'left earbud: expected 181 (Windows) then 60 (right button): {_accepted(writes, left)}')
        if 'VCP: 00:A2:D0:00:00:13: volume changed on the device to 60/255 (30/127)' not in log:
            failures.append('BlueCaster did not log the button press on the right earbud')
        if sims != [50, 30]:
            failures.append(f'simulated Windows volume steps {sims}, expected [50, 30]')
    elif case.name in ('vcp-press-during-write', 'vcp-press-before-write'):
        before = case.name == 'vcp-press-before-write'
        if 'VCP: 00:A2:D0:00:00:12: volume changed on the device during a write of BlueCaster' not in log:
            failures.append('BlueCaster did not notice the button press during its write')
        if 'VCP: 00:A2:D0:00:00:12: volume changed on the device to 40/255 (20/127)' not in log:
            failures.append('BlueCaster did not follow the volume set on the device (40/255)')
        if sims != [50, 20]:
            failures.append(f'simulated Windows volume steps {sims}, expected [50, 20]')
        want = [] if before else [('SET_ABSOLUTE_VOLUME', 181)]
        if _accepted(writes) != want:
            failures.append(f'writes accepted by the sink {_accepted(writes)}, expected {want}')
        rejected = [w for w in writes if w[4].startswith('rejected')]
        if len(rejected) != (1 if before else 0):
            failures.append(f'rejected writes {rejected}')
        if before and 'not retried: the volume was changed on the device meanwhile' not in log:
            failures.append('BlueCaster did not log why it did not retry')
    return failures
