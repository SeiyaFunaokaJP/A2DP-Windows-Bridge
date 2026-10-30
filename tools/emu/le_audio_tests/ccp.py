"""CCP cases of run_test.py --le-audio: BlueCaster's GTBS (call control server)
against the CCP client of the emulated sink (le_audio/ccp.py, --ccp).

BlueCaster has no call source, so the incoming calls come from its simulation
(--ccp-sim, le_audio_ccp.h); the sink answers / ends / holds them through
the Call Control Point. Audio stays media only. The checks read the "CCP:"
lines of sink.log (what the client saw) and BlueCaster's log. Standard library
only.
"""

import re

from .case import Case

STEREO = r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 1 CIS'

CALLER = 'tel:+15551234567'
CALLER_NAME = 'BlueCaster test caller with a friendly name longer than one ATT PDU'
TARGET = 'sip:bluecaster@pc.example'
SECOND = 'sip:second.caller.with.a.long.address@bluecaster.example'
MTU = 64   # --ccp-mtu: values beyond 61 bytes need long reads, notifications are cut

CASES = [
    # Read everything with no call; write the signal strength interval
    Case('ccp-read', 'stereo', STEREO, tags=('ccp',),
         sink_args=['--ccp', '--ccp-ops', 'interval:5'], duration=6),
    # BlueCaster simulates an incoming call; the sink reads it (the friendly name
    # with a long read), accepts it, holds and retrieves it; then the far end
    # hangs up (Termination Reason 0x02)
    Case('ccp-incoming-accept', 'stereo', STEREO, tags=('ccp',),
         sink_args=['--ccp', '--ccp-mtu', str(MTU), '--ccp-ops', 'wait-incoming,read-calls,accept,hold,retrieve,wait-end'],
         bluecaster_args=['--ccp-sim', f'1:incoming {CALLER}|{CALLER_NAME}|{TARGET};4:hangup'], duration=8),
    # Two incoming calls: accept the first, the long call list changes during a
    # long read (0x80), the sink rejects the second and ends the first
    # (Termination Reason 0x06 each), the list is empty afterwards
    Case('ccp-terminate', 'stereo', STEREO, tags=('ccp',),
         sink_args=['--ccp', '--ccp-mtu', str(MTU), '--ccp-ops', 'wait-incoming,accept,wait-incoming,read-calls,read-race,'
                                           'terminate,wait-end,terminate:1,wait-end'],
         bluecaster_args=['--ccp-sim', f'1:incoming {CALLER};2:incoming {SECOND}'], duration=8),
    # Bad requests: unknown call index, unsupported opcodes, a short write,
    # bad outgoing URIs, operations in the wrong state; an outgoing call that
    # the far end answers (simulation) and the sink ends
    Case('ccp-invalid', 'stereo', STEREO, tags=('ccp',),
         sink_args=['--ccp', '--ccp-ops', 'accept:9,terminate:0,raw:07,raw:0501,raw:00,originate:nocolon,'
                                           'originate:mailto:x@y,originate:tel:+15550000,accept,retrieve,'
                                           'sleep:3.5,hold,retrieve,terminate,wait-end'],
         bluecaster_args=['--ccp-sim', '2:alerting;3:answered'], duration=8),
]

# Lines sink.log must contain, per case (regular expressions, in order)
COMMON = [
    r'CCP: GTBS 0x[0-9a-f]{4}-0x[0-9a-f]{4}, 16 characteristics',
    r"CCP: read Bearer Provider Name = 'BlueCaster'",
    r"CCP: read Bearer UCI = 'un000'",
    r'CCP: read Bearer Technology = 0x04',
    r"CCP: read Bearer URI Schemes Supported List = 'tel,sip'",
    r'CCP: read Bearer Signal Strength = 0xff',
    r'CCP: read Bearer List Current Calls = \[\] \(0 bytes\)',
    r'CCP: read Content Control ID = 0x02',
    r'CCP: read Status Flags = 0x0000',
    r'CCP: read Call State = \[\] \(0 bytes\)',
    r'CCP: read Call Control Point Optional Opcodes = 0x0001',
    r'CCP: read Incoming Call = none',
    r'CCP: subscribed, client ready',
]
FINAL = [
    r'CCP: final Call State = \[\]',
    r'CCP: final Bearer List Current Calls = \[\]',
    r'CCP: script done',
]
EXPECT = {
    'ccp-read': [
        r'CCP: read Bearer Signal Strength Reporting Interval = 0x05',
    ],
    'ccp-incoming-accept': [
        # Order of Zephyr's tbs.c: Call State (and the list) before the
        # incoming call's own characteristics
        r'CCP: notify Call State \[1 Incoming flags 0x00\]',
        rf"CCP: notify Incoming Call Target Bearer URI call 1 '{re.escape(TARGET)}'",
        # Cut to ATT_MTU - 3 = 61 bytes: index + 60 bytes of the name
        rf"CCP: notify Call Friendly Name call 1 '{re.escape(CALLER_NAME[:60])}' \(61 bytes\)",
        rf"CCP: notify Incoming Call call 1 '{re.escape(CALLER)}'",
        rf'CCP: incoming call 1 from {re.escape(CALLER)}',
        rf'CCP: read Bearer List Current Calls = \[1 Incoming flags 0x00 {re.escape(CALLER)}\]',
        rf"CCP: read Incoming Call Target Bearer URI = call 1 '{re.escape(TARGET)}'",
        rf"CCP: read Call Friendly Name = call 1 '{re.escape(CALLER_NAME)}' \(68 bytes\)",
        # After a control point write: the new Call State, then the result
        r'CCP: notify Call State \[1 Active flags 0x00\]',
        r'CCP: notify Call Control Point Accept call 1 -> Success',
        r'CCP: notify Call State \[1 Locally Held flags 0x00\]',
        r'CCP: notify Call Control Point Local Hold call 1 -> Success',
        r'CCP: notify Call State \[1 Active flags 0x00\]',
        r'CCP: notify Call Control Point Local Retrieve call 1 -> Success',
        # The far end hangs up: Call State, then Termination Reason
        r'CCP: notify Call State \[\]',
        r'CCP: notify Termination Reason call 1 reason 0x02',
    ],
    'ccp-terminate': [
        r'CCP: incoming call 1 from',
        r'CCP: notify Call Control Point Accept call 1 -> Success',
        r'CCP: incoming call 2 from',
        rf'CCP: read Bearer List Current Calls = \[1 Active flags 0x00 {re.escape(CALLER)}, '
        rf'2 Incoming flags 0x00 {re.escape(SECOND)}\] \(80 bytes\)',
        r'CCP: read-race: Read Blob -> ATT error 0x80',
        r'CCP: notify Termination Reason call 2 reason 0x06',
        r'CCP: notify Call Control Point Terminate call 2 -> Success',
        r'CCP: notify Termination Reason call 1 reason 0x06',
        r'CCP: notify Call Control Point Terminate call 1 -> Success',
    ],
    'ccp-invalid': [
        r'CCP: notify Call Control Point Accept call 9 -> Invalid Call Index',
        r'CCP: notify Call Control Point Terminate call 0 -> Invalid Call Index',
        r'CCP: notify Call Control Point opcode 0x07 call 0 -> Opcode Not Supported',
        r'CCP: notify Call Control Point Join call 0 -> Opcode Not Supported',
        r'CCP: CP raw 00 -> ATT error 0x0d',
        r'CCP: notify Call Control Point Originate call 0 -> Invalid Outgoing URI',
        r'CCP: notify Call Control Point Originate call 0 -> Invalid Outgoing URI',
        r'CCP: notify Call State \[1 Dialing flags 0x01\]',
        r'CCP: notify Call Control Point Originate call 1 -> Success',
        r'CCP: notify Call Control Point Accept call 1 -> State Mismatch',
        r'CCP: notify Call Control Point Local Retrieve call 1 -> State Mismatch',
        r'CCP: notify Call State \[1 Alerting flags 0x01\]',
        r'CCP: notify Call State \[1 Active flags 0x01\]',
        r'CCP: notify Call Control Point Local Hold call 1 -> Success',
        r'CCP: notify Call Control Point Local Retrieve call 1 -> Success',
        r'CCP: notify Termination Reason call 1 reason 0x06',
        r'CCP: notify Call Control Point Terminate call 1 -> Success',
        r'CCP: call 1 ended, reason 0x06',
    ],
}
# Lines sink.log must contain anywhere (notifications that race with the steps' own lines)
EXPECT_ANYWHERE = {
    'ccp-incoming-accept': [rf'CCP: GTBS .* ATT MTU {MTU}$'],
    # The list of two calls (80 bytes) cut to ATT_MTU - 3 = 61 bytes
    'ccp-terminate': [rf'CCP: GTBS .* ATT MTU {MTU}$',
                      rf'CCP: notify Bearer List Current Calls \[1 Active flags 0x00 {re.escape(CALLER)}, '
                      r'truncated item\] \(61 bytes\)'],
}
# BlueCaster's side (bluecaster.log)
EXPECT_BlueCaster = {
    'ccp-incoming-accept': [r'LE Audio: CCP: client 0x[0-9a-f]{4} subscribed to Call State, simulation starts',
                            r'LE Audio: CCP: incoming call 1 from',
                            r'LE Audio: CCP: call 1 ended \(reason 0x02\)'],
    'ccp-terminate': [r'LE Audio: CCP: 0x[0-9a-f]{4} changed during a long read'],
}


def expect_in_order(log, patterns, where):
    failures, pos = [], 0
    for p in patterns:
        m = re.compile(p).search(log, pos)
        if not m:
            failures.append(f'{where}: missing (in order) /{p}/')
            # Keep going from the same place: report every missing line
            continue
        pos = m.end()
    return failures


def check(case, ctx):
    if 'ccp' not in case.tags:
        # Without the client nothing reads GTBS, but BlueCaster must offer it
        if 'LE Audio: CCP: GTBS handles' not in ctx['bluecaster_log']:
            return ['BlueCaster did not register GTBS']
        return []
    sink_log = ctx['sink_log']
    failures = expect_in_order(sink_log, COMMON + EXPECT.get(case.name, []) + FINAL, 'sink.log')
    for p in EXPECT_ANYWHERE.get(case.name, []):
        if not re.search(p, sink_log, re.M):
            failures.append(f'sink.log: missing /{p}/')
    for bad in ('CCP: mandatory characteristic missing', 'lacks properties', 'CCP: error', 'CCP: unknown step'):
        for line in sink_log.splitlines():
            if bad in line:
                failures.append(f'sink.log: {line.strip()}')
    failures += expect_in_order(ctx['bluecaster_log'], EXPECT_BlueCaster.get(case.name, []), 'bluecaster.log')
    if 'LE Audio: CCP: simulation' in ctx['bluecaster_log'] and 'failed' in ctx['bluecaster_log']:
        for line in ctx['bluecaster_log'].splitlines():
            if 'LE Audio: CCP: simulation' in line and 'failed' in line:
                failures.append(f'bluecaster.log: {line.strip()}')
    return failures
