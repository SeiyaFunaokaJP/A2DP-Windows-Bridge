"""LE Audio cases and checks of run_test.py --le-audio (standard library only).

Each module here belongs to one work package and may define:

    CASES = [Case(...), ...]          test cases it adds (unique names)
    def check(case, ctx): -> [str]    extra checks after every case; return
                                      failure texts (empty list: fine)

ctx is a dict: 'out' (the case's output folder), 'a2dpwb_log' / 'sink_log'
(text of both logs), 'sent' / 'received' (parsed a2dpwb_decode reports, see
run_test.parse_le_report) and 'sink_addresses'. A check should only look at
what its own profile does and return [] for cases it does not concern
(case.tags says which profiles a case exercises).

core.py holds the BAP unicast cases (all layouts of emu_le_audio_sink.py).
"""

from . import bas, ccp, core, csip, mcp, privacy, tmap_gmap, vcp
from .case import MONO_TONES, STEREO_TONES, Case  # noqa: F401  (for the modules)

MODULES = [core, csip, vcp, mcp, ccp, tmap_gmap, bas, privacy]


def cases():
    out = []
    for module in MODULES:
        out.extend(getattr(module, 'CASES', []))
    return out


def check(case, ctx):
    failures = []
    for module in MODULES:
        if hasattr(module, 'check'):
            failures.extend(module.check(case, ctx) or [])
    return failures
