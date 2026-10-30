"""BAP unicast cases: the stream set-ups BlueCaster plans from the sink's PACS /
ASCS (app/src/crates/le-audio/src/presets.rs), one per emu_le_audio_sink.py layout."""

import re

from .case import MONO_TONES, Case

CASES = [
    # One headset, one 2-channel ASE: 1 CIS, LC3 48_6_2 (the best media preset)
    Case('stereo', 'stereo', r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 1 CIS'),
    # One headset, two 1-channel ASEs: 2 CIS on one link
    Case('two-ase', 'two-ase', r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 2 CIS'),
    # Two earbuds (Front Left / Front Right), one ASE each: 2 CIS on two links
    Case('earbuds', 'earbuds', r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 2 CIS'),
    # Mono sink with a forced low-latency preset (--le-preset): downmix, 16 kHz
    Case('mono-16k', 'mono', r'LC3 16000 Hz 10 ms 40 octets, 1 channel\(s\) over 1 CIS',
         tones=MONO_TONES, bluecaster_args=['--le-preset', '16_2_1']),
    # The sink drops the LE link 3 s after BlueCaster connected: BlueCaster tears the
    # stream down (CIS, CIG) and sets it up again on a new link (bond reused)
    Case('link-loss', 'stereo', r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 1 CIS',
         sink_args=['--drop-link-after', '3'], streams=2, lossy=True, duration=14),
    # The sink keeps released ASEs in Releasing until the client drops the
    # CIS: BlueCaster disconnects the CIS after Disable, so stop does not wait
    # out the Release timeout
    Case('release-cis', 'two-ase', r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 2 CIS',
         sink_args=['--ascs-release-waits-for-cis'], tags=('release-cis',)),
    # No sink context available until 3 s after the connection: Enable waits
    # for the Available Audio Contexts notification instead of failing
    Case('contexts-later', 'stereo', r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 1 CIS',
         sink_args=['--available-contexts-after', '3'], tags=('contexts-later',)),
    # AFH host channel classification forced (--afh 6,11; by default BlueCaster
    # sends it depending on the PC's Wi-Fi): the controller must answer it
    # and streaming go on as usual
    Case('afh', 'stereo', r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 1 CIS',
         bluecaster_args=['--afh', '6,11']),
    # A controller with LE but no ISO buffers (LE Read Buffer Size v2: 0):
    # BlueCaster must refuse LE Audio with an error that names ISO
    Case('no-iso-buffers', 'stereo', r'.*', sink_args=['--no-iso-buffers'], expect_error=r'ISO'),
]

# The emulator's PACS Sink Audio Locations per sink, in capture order
# (le_audio/bap.py logs them)
SINK_LOCATIONS = re.compile(r'LE Audio sink, locations 0x([0-9a-fA-F]+)')
# bluecaster_decode --le-audio: a CIS of a stream, its Audio_Channel_Allocation
# (Config Codec) and the capture (sink) it is in
CIS_LINE = re.compile(r'^  CIS 0x([0-9A-F]+) +: ASE (\d+) on ACL 0x[0-9A-F]+, location\(s\) (\S+), '
                      r'.*, capture (\d+)$', re.M)


def check_locations(ctx):
    """Each CIS's Audio_Channel_Allocation is within the PACS Sink Audio
    Locations of the sink it goes to (a sink with locations gets some of
    them, one without gets none): e.g. the left earbud Front Left, the right
    one Front Right. That the audio of each allocation is the right channel
    (1 kHz left, 1.5 kHz right) is the tone check's part: the decoder places
    every CIS's channels by their allocation."""
    received = ctx['received']
    if not received:
        return []
    pacs = [int(x, 16) for x in SINK_LOCATIONS.findall(ctx['sink_log'])]
    cis = CIS_LINE.findall(received['text'])
    if not cis:
        return ['no CIS lines (with their capture) in the received report']
    failures = []
    for handle, ase, locs, capture in cis:
        allocation = 0 if locs == 'mono' else sum(int(x, 16) for x in locs.split(','))
        index = int(capture) - 1
        if index >= len(pacs):
            failures.append(f'sink {index + 1}: no PACS locations in sink.log')
            continue
        where = f'sink {index + 1} ({ctx["sink_addresses"][index]}, PACS locations 0x{pacs[index]:x})'
        if pacs[index] == 0 and allocation:
            failures.append(f'{where}: CIS 0x{handle} ASE {ase} got allocation 0x{allocation:x}, expected none')
        elif pacs[index] and (not allocation or allocation & ~pacs[index]):
            failures.append(f'{where}: CIS 0x{handle} ASE {ase} got allocation '
                            f'{"none" if not allocation else f"0x{allocation:x}"}, not within its locations')
    return failures


def check(case, ctx):
    failures = []
    log = ctx['bluecaster_log']
    failures += check_locations(ctx)
    # Bumble's complaints about a command the emulated controller did not
    # answer (BTstack then waits for ever)
    for complaint in ('Async command handlers should return None', 'Sync command handlers should return'):
        if complaint in ctx['sink_log']:
            failures.append(f'emulated controller: "{complaint}" (a command was not answered)')
    if case.name == 'afh' and 'AFH host classification: avoid Wi-Fi' not in log:
        failures.append('BlueCaster did not send the AFH host channel classification (--afh 6,11)')
    if 'Streaming LE Audio:' not in log:
        failures.append('BlueCaster did not report "Streaming LE Audio"')
    # The stream must be torn down in order: Disable / Release / Remove CIG
    for step in ('unicast: Disable', 'unicast: Release', 'CIG 1 removed'):
        if step not in log:
            failures.append(f'teardown step missing from the log: {step}')
    # (the underrun / silence counters are checked by run_test.py)
    m = re.search(r'LE Audio: (\d+) SDU sets sent .*?(\d+) underruns', log)
    if not m:
        failures.append('no LE Audio statistics line')
    if case.name == 'link-loss' and 'LE Audio stream restored' not in log:
        failures.append('the stream was not set up again after the link loss')
    for timeout in ('Disable timed out', 'Release timed out', 'CIG removal timed out'):
        if timeout in log and not case.lossy:
            failures.append(f'teardown: {timeout}')
    if 'release-cis' in case.tags and 'disconnecting CIS' not in log:
        failures.append('BlueCaster did not disconnect the CIS before Release')
    if 'contexts-later' in case.tags and 'no audio context available for Enable' not in log:
        failures.append('BlueCaster did not wait for an available audio context')
    return failures
