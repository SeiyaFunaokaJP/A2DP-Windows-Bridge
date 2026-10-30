"""TMAP / GMAP cases and checks of run_test.py --le-audio.

Every emulated sink has a TMAS with TMAP Role UMR (le_audio/tmap_gmap.py);
GMAS only with --gmap-role. The cases:

  tmap-role              BlueCaster logs the sink's "TMAP: UMR"; the sink reads
                         BlueCaster's TMAS (UMS) and GMAS (UGG, UGG Features 0x03)
  gmap-low-latency       UGT with the Sink feature + --le-low-latency: GMAP
                         48_3_gr (48 kHz, 7.5 ms, 90 octets) with GMAP's QoS
                         (SDU interval 7500 us, RTN 1, 15 ms, 10 ms
                         presentation delay) and the Game context
  gmap-earbuds           the same for two UGT earbuds (2 CIS on 2 links)
  gmap-off               UGT without --le-low-latency: the media preset
  gmap-not-ugt           GMAS with only Broadcast Game Receiver and
                         --le-low-latency: the media preset
  gmap-media-context     a UGT whose PACS has Media but not Game available:
                         48_3_gr with the Media context (not Unspecified)
"""

import os
import re

from .case import Case

MEDIA = r'LC3 48000 Hz 10 ms 155 octets, 2 channel\(s\) over 1 CIS'
UGT = ['--gmap-role', '0x02', '--gmap-ugt-features', '0x3C']   # UGT: sink, 64 kbps sink, multiplex, multisink

CASES = [
    Case('tmap-role', 'stereo', MEDIA, sink_args=['--tmap-read-peer'], tags=('tmap',)),
    Case('gmap-low-latency', 'stereo', r'LC3 48000 Hz 7\.5 ms 90 octets, 2 channel\(s\) over 1 CIS',
         sink_args=UGT, bluecaster_args=['--le-low-latency'], tags=('gmap', 'gmap_ll')),
    Case('gmap-earbuds', 'earbuds', r'LC3 48000 Hz 7\.5 ms 90 octets, 2 channel\(s\) over 2 CIS',
         sink_args=UGT, bluecaster_args=['--le-low-latency'], tags=('gmap', 'gmap_ll')),
    Case('gmap-off', 'stereo', MEDIA, sink_args=UGT, tags=('gmap',)),
    Case('gmap-not-ugt', 'stereo', MEDIA, sink_args=['--gmap-role', '0x08'],
         bluecaster_args=['--le-low-latency'], tags=('gmap',)),
    Case('gmap-media-context', 'stereo', r'LC3 48000 Hz 7\.5 ms 90 octets, 2 channel\(s\) over 1 CIS',
         sink_args=UGT + ['--gmap-available-contexts', '0x0006'], bluecaster_args=['--le-low-latency'],
         tags=('gmap', 'gmap_ll', 'gmap_media')),
]

# Config QoS of the GMAP 48_3_gr setting (GMAP 1.0 Table 3.16), as
# bluecaster_decode prints it
GMAP_QOS = re.compile(r'Config QoS: CIG \d+ CIS \d+, SDU interval 7500 us, max SDU \d+, RTN 1, latency 15 ms, '
                      r'presentation delay 10000 us')
ANY_QOS = re.compile(r'Config QoS: .*')


def read(path):
    try:
        with open(path, encoding='utf-8', errors='replace') as f:
            return f.read()
    except OSError:
        return ''


def check(case, ctx):
    if not {'tmap', 'gmap'} & set(case.tags):
        return []
    failures = []
    log = ctx['bluecaster_log']
    sink_log = ctx['sink_log']

    # Every sink has TMAS (UMR): BlueCaster reads it on every case of this module
    if 'TMAP: UMR' not in log:
        failures.append('BlueCaster did not log the sink\'s TMAP Role ("TMAP: UMR")')

    if case.name == 'tmap-role':
        if 'peer TMAP Role 0x0004 (UMS)' not in sink_log:
            failures.append('the sink did not read TMAP Role UMS from BlueCaster\'s TMAS')
        if 'peer GMAP Role 0x01 (UGG), UGG Features 0x03' not in sink_log:
            failures.append('the sink did not read GMAP Role UGG / UGG Features 0x03 from BlueCaster\'s GMAS')
        if 'GMAP: low latency off' not in log:
            failures.append('no "GMAP: low latency off" in the log')

    sent_report = read(os.path.join(ctx['out'], 'sent_report.txt'))
    received_report = read(os.path.join(ctx['out'], 'received_report.txt'))
    if 'gmap_ll' in case.tags:
        if 'GMAP: UGT' not in log:
            failures.append('BlueCaster did not log the sink\'s GMAP Role ("GMAP: UGT")')
        if 'GMAP: low latency for' not in log:
            failures.append('the GMAP presets were not put first')
        context = '0x0004' if 'gmap_media' in case.tags else '0x0008'
        if not re.search(r'GMAP: streaming 48_3_gr, contexts ' + context, log):
            failures.append(f'not streaming 48_3_gr with the contexts {context}')
        if 'gmap_media' in case.tags and 'has no Game context available (0x0006): Media context instead' not in log:
            failures.append('BlueCaster did not log the Media context instead of Game')
        for side, text in (('sent', sent_report), ('received', received_report)):
            qos = ANY_QOS.findall(text)
            if not qos:
                failures.append(f'{side}: no Config QoS in the report')
            elif not all(GMAP_QOS.search(q) for q in qos):
                failures.append(f'{side}: Config QoS is not GMAP 48_3_gr: {qos[0]}')
    elif 'gmap' in case.tags:
        # Not low latency: a media (high reliability) QoS
        if any('SDU interval 7500 us' in q or 'RTN 1,' in q for q in ANY_QOS.findall(sent_report)):
            failures.append('a GMAP / low-latency QoS was used although it should not')
        if case.name == 'gmap-off':
            if 'GMAP: UGT' not in log:
                failures.append('BlueCaster did not log the sink\'s GMAP Role ("GMAP: UGT")')
            if 'GMAP: low latency off' not in log:
                failures.append('no "GMAP: low latency off" in the log')
        if case.name == 'gmap-not-ugt':
            if 'GMAP: BGR' not in log:
                failures.append('BlueCaster did not log the sink\'s GMAP Role ("GMAP: BGR")')
            if 'not a Unicast Game Terminal' not in log:
                failures.append('no "not a Unicast Game Terminal" in the log')
    return failures
