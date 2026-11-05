"""BAS cases and checks of run_test.py --le-audio (standard library only).

The sinks run a Battery Service (emu option --bas, le_audio/bas.py) and
A2DPWB reads the level (the headphones' battery, shown in the UI).
"""

import re

from .case import Case

STEREO = r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 1 CIS'
TWO_CIS = r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 2 CIS'

CASES = [
    # The level is read, then followed by a notification
    Case('bas-level', 'stereo', STEREO, sink_args=['--bas', '80', '--bas-change', '4:55'],
         tags=('bas',), duration=8),
    # Two earbuds: the lower level is the one reported
    Case('bas-earbuds', 'earbuds', TWO_CIS, sink_args=['--bas', '90,40'],
         tags=('bas',), duration=8),
]


def _reported(log):
    return [int(x) for x in re.findall(r'BAS: device \d+: battery \d+% \(reported (\d+)%\)', log)]


def check(case, ctx):
    if 'bas' not in case.tags:
        return []
    log, sink_log = ctx['a2dpwb_log'], ctx['sink_log']
    failures = []
    reported = _reported(log)
    if case.name == 'bas-level':
        if 'Battery Level read: 80%' not in sink_log:
            failures.append('A2DPWB did not read the Battery Level')
        if reported != [80, 55]:
            failures.append(f'reported levels {reported}, expected [80, 55]')
    elif case.name == 'bas-earbuds':
        if not reported or reported[-1] != 40:
            failures.append(f'reported levels {reported}, the lowest (40) should be the last')
    return failures
