"""CSIP cases and checks of run_test.py --le-audio.

The earbuds layout with --csip-set: the two earbuds (Front Left / Front
Right, one sink ASE each) are the members of one coordinated set. BlueCaster is
given only the first earbud's address (first_address_only); its CSIP module
reads the SIRK, finds the second earbud by its RSI, connects it and streams
left to one and right to the other. Before the stream is set up the set is
locked in rank order (CSIP Lock Request / Ordered Access) and released, in
reverse order, once it runs.
"""

import re

from .case import MONO_TONES, Case

TWO_CIS = r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 2 CIS'
ONE_CIS = r'LC3 48000 Hz 10 ms 155 octets, 1 channel\(s\) over 1 CIS'

CASES = [
    # Plaintext SIRK: BlueCaster finds and connects the second earbud itself
    Case('csip-set', 'earbuds', TWO_CIS, sink_args=['--csip-set', 'plaintext'],
         tags=('csip',), first_address_only=True),
    # Encrypted SIRK (decrypted with the LTK); the earbud named first has
    # rank 2, so the found one (rank 1) must come first
    Case('csip-encrypted-sirk', 'earbuds', TWO_CIS, sink_args=['--csip-set', 'encrypted', '--csip-ranks', '2,1'],
         tags=('csip',), first_address_only=True),
    # The second earbud never advertises: stream to the first one alone
    # (mono downmix), with a warning
    Case('csip-missing-member', 'earbuds', ONE_CIS, tones=MONO_TONES,
         sink_args=['--csip-set', 'plaintext', '--csip-missing'], tags=('csip',), first_address_only=True),
    # Both earbuds named (-d / --le-device) and a set: no member twice
    Case('csip-both-named', 'earbuds', TWO_CIS, sink_args=['--csip-set', 'plaintext'], tags=('csip',)),
    # The named earbud drops its link while streaming: BlueCaster reconnects it,
    # keeps the other (still connected) and streams to both again
    Case('csip-link-loss', 'earbuds', TWO_CIS, sink_args=['--csip-set', 'plaintext', '--drop-link-after', '5'],
         tags=('csip',), first_address_only=True, streams=2, lossy=True, duration=14),
    # Another coordinator holds the second earbud's lock for the first two
    # requests: Lock Denied -> BlueCaster releases the first earbud, waits, tries
    # again and gets the set on the third attempt
    Case('csip-lock-denied', 'earbuds', TWO_CIS, sink_args=['--csip-set', 'plaintext', '--csip-lock-deny', '2'],
         tags=('csip',), first_address_only=True),
    # The earbuds have no Set Member Lock (optional): ordered access without
    # locking, the stream as usual
    Case('csip-no-lock', 'earbuds', TWO_CIS, sink_args=['--csip-set', 'plaintext', '--csip-no-lock'],
         tags=('csip',), first_address_only=True),
]

LOCK = re.compile(r'(BlueCaster LE \w+): CSIS lock: (.*?)\s*$', re.M)


def _lock_events(sink_log):
    """[(sink name, what)] of the sinks' Set Member Lock, in order"""
    return [(m.group(1), m.group(2)) for m in LOCK.finditer(sink_log)]


def _check_lock(case, log, sink_log):
    failures = []
    events = _lock_events(sink_log)
    if case.name == 'csip-no-lock':
        if 'not every member has a Set Member Lock: ordered access without locking' not in log:
            failures.append('BlueCaster did not report the set without a lock')
        if events:
            failures.append(f'lock requests to members without a lock: {events}')
        return failures
    if case.name == 'csip-link-loss':
        return failures      # the lock of the lost link is released by its disconnect: checked by csip-set
    first, second = ('BlueCaster LE R', 'BlueCaster LE L') if case.name == 'csip-encrypted-sirk' else (
        'BlueCaster LE L', 'BlueCaster LE R')
    locked, released = 'locked by BlueCaster', 'released by BlueCaster'
    if case.name == 'csip-missing-member':
        want = [(first, locked), (first, released)]
    else:
        want = [(first, locked), (second, locked), (second, released), (first, released)]
    if case.name == 'csip-lock-denied':
        denied = (second, 'Lock Denied (another coordinator holds it)')
        want = [(first, locked), denied, (first, released)] * 2 + want
        if log.count('Lock Denied by') != 2:
            failures.append(f'BlueCaster reported {log.count("Lock Denied by")} Lock Denied answers (expected 2)')
    if events != want:
        failures.append(f'lock sequence at the sinks {events}, expected {want}')
    members = 1 if case.name == 'csip-missing-member' else 2
    if f'CSIP: coordinated set 0 locked ({members} member(s), in rank order)' not in log:
        failures.append('BlueCaster did not report the set locked')
    if 'without the lock' in log:
        failures.append('BlueCaster streamed without the lock')
    return failures


def check(case, ctx):
    if 'csip' not in case.tags:
        return []
    log = ctx['bluecaster_log']
    sink_log = ctx['sink_log']
    failures = []
    if 'CSIP: crypto self-check passed' not in log:
        failures.append('CSIP crypto self-check did not pass')
    first, second = ctx['sink_addresses'][0], ctx['sink_addresses'][1]
    sirk_type = 'encrypted' if 'encrypted' in case.sink_args else 'plaintext'
    for addr in (first, second) if case.name != 'csip-missing-member' else (first,):
        if not re.search(re.escape(addr) + r': member of coordinated set 0, rank \d of 2 \(' + sirk_type, log):
            failures.append(f'CSIP: {addr} not read as a member of the set ({sirk_type} SIRK)')
    if f'CSIS SIRK read ({sirk_type})' not in sink_log:
        failures.append(f'the sinks did not serve a {sirk_type} SIRK')
    failures += _check_lock(case, log, sink_log)

    found = re.search(re.escape(second) + r' "[^"]*" resolves to coordinated set 0 \(RSI\)', log)
    if case.first_address_only and case.name != 'csip-missing-member' and not found:
        failures.append(f'CSIP did not find {second} by its RSI')
    if case.name == 'csip-both-named' and found:
        failures.append('CSIP scanned for a member that was named already')

    if case.name == 'csip-missing-member':
        if 'CSIP: only 1 of 2 members of coordinated set 0 found' not in log:
            failures.append('no warning about the missing set member')
        if 'streaming to coordinated set 0, 1 of 2 members' not in log:
            failures.append('CSIP did not stream to the member available')
        if found:
            failures.append('the missing member was found after all')
        return failures

    # The set: both members, ordered by rank
    m = re.findall(r'CSIP: streaming to coordinated set 0, 2 of 2 members: (\S+) rank (\d), (\S+) rank (\d)', log)
    if len(m) != case.streams:
        failures.append(f'CSIP: {len(m)} set-ups with both members (expected {case.streams})')
    for a1, r1, a2, r2 in m:
        if (r1, r2) != ('1', '2') or {a1, a2} != {first, second}:
            failures.append(f'CSIP: members not ordered by rank: {a1} rank {r1}, {a2} rank {r2}')
    if case.name == 'csip-link-loss':
        if 'LE Audio stream restored' not in log:
            failures.append('the stream was not set up again after the link loss')
        if not re.search(re.escape(first) + r' \(rank \d of coordinated set 0\) disconnected', log):
            failures.append('CSIP did not see the member disconnect')
    return failures
