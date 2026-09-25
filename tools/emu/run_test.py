"""End-to-end A2DP test of A2DPWB against a virtual sink - no Bluetooth hardware.

    python tools/emu/setup_env.py          (once)
    python tools/emu/run_test.py [--codecs sbc aac ...] [--duration 5]

For each codec: starts emu_sink.py (Bumble virtual link + A2DP sink), runs
A2DPWB.exe --cli with a test tone over H4/TCP, then checks both HCI captures
with a2dpwb_decode:
  - A2DPWB streams the requested codec and a2dpwb_decode finds no problem,
    on the sent side and on the received side
  - the sink received every media packet / frame A2DPWB sent
  - decoded audio (all codecs but LDAC) is identical on both sides and is
    the test tone: 1 kHz left, 1.5 kHz right, amplitude 0.5

Outputs (captures, logs, reports, WAV) go to tools/emu/out/<codec>/.
Exit code 0 when every codec passed. Uses only the Python standard library;
the sink runs in the tools/emu/.venv environment.
"""

import argparse
import math
import os
import queue
import re
import shutil
import struct
import subprocess
import sys
import threading
import time
import wave

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
VENV_PYTHON = os.path.join(HERE, '.venv', 'Scripts', 'python.exe')
SINK_ADDRESS = '00:A2:D0:00:00:02'
BASE_PORT = 9101

CODECS = ['sbc', 'aac', 'aptx', 'aptxhd', 'aptxll', 'ldac']
CONFIG_PATTERN = {  # a2dpwb_decode's "Configuration :" line
    'sbc': r'SBC \d', 'aac': r'AAC ', 'aptx': r'aptX \d', 'aptxhd': r'aptX HD \d',
    'aptxll': r'aptX LL \d', 'ldac': r'LDAC \d',
}
TONES = ((1000.0, 0.5), (1500.0, 0.5))  # (frequency, amplitude) per channel, as --test-tone


def parse_report(text):
    """First stream of an a2dpwb_decode report."""
    def find(pattern, cast=str):
        m = re.search(pattern, text, re.M)
        return cast(m.group(1)) if m else None
    return {
        'config': find(r'Configuration : (.*)$'),
        'packets': find(r'Media packets : (\d+)', int),
        'frames': find(r'Frames\s+: (\d+)', int),
        'audio_s': find(r'Audio\s+: ([\d.]+) s', float),
        'wav': find(r'WAV\s+: (.*)$'),
        'streams': len(re.findall(r'^=== Stream ', text, re.M)),
        'result_ok': re.search(r'^RESULT: OK', text, re.M) is not None,
        'problems': re.findall(r'^    - (.*)$', text, re.M),
    }


def read_wav(path):
    with wave.open(path) as w:
        ch, width, rate, n = w.getnchannels(), w.getsampwidth(), w.getframerate(), w.getnframes()
        raw = w.readframes(n)
    full = float(1 << (8 * width - 1))
    samples = [[] for _ in range(ch)]
    for i in range(n * ch):
        b = raw[i * width:(i + 1) * width]
        if width == 3:
            b += b'\xff' if b[2] & 0x80 else b'\x00'
        samples[i % ch].append(struct.unpack('<h' if width == 2 else '<i', b)[0] / full)
    return rate, samples


def tone_check(path):
    """Returns (ok, description) for the test tone in the middle of the WAV."""
    rate, channels = read_wav(path)
    if len(channels) != 2 or len(channels[0]) < rate:
        return False, f'expected >= 1 s of stereo audio, got {len(channels)} ch'
    start, count = len(channels[0]) // 2 - rate // 4, rate // 2
    notes, ok = [], True
    for xs, (freq, amp) in zip(channels, TONES):
        xs = xs[start:start + count]
        k = 2 * math.cos(2 * math.pi * freq / rate)
        s1 = s2 = 0.0
        for x in xs:
            s1, s2 = x + k * s1 - s2, s1
        tone_amp = 2 * math.sqrt(max(s1 * s1 + s2 * s2 - k * s1 * s2, 0.0)) / count
        mean_sq = sum(x * x for x in xs) / count
        share = (tone_amp * tone_amp / 2) / mean_sq if mean_sq else 0.0
        ok &= share > 0.95 and abs(tone_amp - amp) < 0.05
        notes.append(f'{freq:.0f} Hz {100 * share:.1f}% amp {tone_amp:.3f}')
    return ok, ', '.join(notes)


def start_sink(port, capture, log_path):
    log = open(log_path, 'w', encoding='utf-8')
    proc = subprocess.Popen(
        [VENV_PYTHON, '-u', os.path.join(HERE, 'emu_sink.py'), '--port', str(port), '--capture', capture],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding='utf-8', errors='replace')
    lines = queue.Queue()

    def pump():
        for line in proc.stdout:
            log.write(line)
            log.flush()
            lines.put(line)
        log.close()
    threading.Thread(target=pump, daemon=True).start()
    deadline = time.time() + 30
    while time.time() < deadline:
        try:
            if lines.get(timeout=0.5).startswith('READY'):
                return proc
        except queue.Empty:
            if proc.poll() is not None:
                break
    proc.kill()
    raise RuntimeError(f'virtual sink did not start, see {log_path}')


def run_codec(args, codec, port):
    out = os.path.join(args.out, codec)
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    src_capture = os.path.join(out, 'a2dpwb.pklg')
    sink_capture = os.path.join(out, 'sink.pklg')
    failures = []

    sink = start_sink(port, sink_capture, os.path.join(out, 'sink.log'))
    try:
        env = dict(os.environ, A2DPWB_CONFIG_DIR=os.path.join(out, 'config'))  # isolated link keys
        with open(os.path.join(out, 'a2dpwb.log'), 'w', encoding='utf-8', errors='replace') as log:
            try:
                rc = subprocess.run(
                    [args.a2dpwb, '--cli', '--hci-tcp', f'127.0.0.1:{port}', '-d', SINK_ADDRESS,
                     '-c', codec, '--test-tone', '--duration', str(args.duration),
                     '--hci-capture', src_capture]
                    + (['--max-packet', str(args.max_packet)] if args.max_packet else []),
                    stdout=log, stderr=subprocess.STDOUT, env=env, timeout=args.duration + 90).returncode
            except subprocess.TimeoutExpired:
                rc = 'timeout'
        if rc != 0:
            failures.append(f'A2DPWB exited with {rc} (see a2dpwb.log)')
    finally:
        sink.terminate()
        sink.wait(10)

    reports = {}
    for side, capture, extra in (('sent', src_capture, []), ('received', sink_capture, ['--received'])):
        if not os.path.exists(capture):
            failures.append(f'{side}: no capture')
            continue
        text = subprocess.run([args.decoder, capture, '-o', os.path.join(out, side)] + extra,
                              capture_output=True, text=True, encoding='utf-8', errors='replace').stdout
        with open(os.path.join(out, f'{side}_report.txt'), 'w', encoding='utf-8') as f:
            f.write(text)
        r = reports[side] = parse_report(text)
        if r['streams'] != 1:
            failures.append(f'{side}: {r["streams"]} streams (expected 1)')
        if not re.match(CONFIG_PATTERN[codec], r['config'] or ''):
            failures.append(f'{side}: configuration "{r["config"]}"')
        if not r['result_ok']:
            failures.extend(f'{side}: {p}' for p in r['problems'] or ['a2dpwb_decode reported problems'])

    sent, received = reports.get('sent'), reports.get('received')
    detail = ''
    if sent and received:
        if (sent['packets'], sent['frames']) != (received['packets'], received['frames']):
            failures.append(f'sent {sent["packets"]} packets / {sent["frames"]} frames, '
                            f'received {received["packets"]} / {received["frames"]}')
        if (sent['audio_s'] or 0) < 0.9 * args.duration:
            failures.append(f'only {sent["audio_s"]} s of audio for --duration {args.duration}')
        detail = f'{sent["packets"]} packets, {sent["frames"]} frames, {sent["audio_s"]} s'
        if codec != 'ldac':
            if not (sent['wav'] and received['wav']):
                failures.append('no decoded WAV')
            else:
                with open(sent['wav'], 'rb') as a, open(received['wav'], 'rb') as b:
                    if a.read() != b.read():
                        failures.append('decoded audio differs between sent and received side')
                ok, notes = tone_check(received['wav'])
                detail += f' | {notes}'
                if not ok:
                    failures.append(f'test tone not found: {notes}')
    return failures, detail


def main():
    parser = argparse.ArgumentParser(description='End-to-end A2DP test of A2DPWB against a virtual sink')
    parser.add_argument('--codecs', nargs='+', choices=CODECS, default=CODECS)
    parser.add_argument('--duration', type=int, default=5, help='seconds of streaming per codec')
    parser.add_argument('--max-packet', type=int, help='passed to A2DPWB --max-packet (default: its own)')
    parser.add_argument('--build-dir', default=os.path.join(REPO, 'build'))
    parser.add_argument('--config', default='Release', help='build configuration (Release / Debug)')
    parser.add_argument('--out', default=os.path.join(HERE, 'out'))
    args = parser.parse_args()
    args.a2dpwb = os.path.join(args.build_dir, 'app', args.config, 'A2DPWB.exe')
    args.decoder = os.path.join(args.build_dir, 'tools', 'a2dp_decode', args.config, 'a2dpwb_decode.exe')
    for path, hint in ((VENV_PYTHON, 'run: python tools/emu/setup_env.py'),
                       (args.a2dpwb, 'build A2DPWB'), (args.decoder, 'build a2dpwb_decode')):
        if not os.path.exists(path):
            sys.exit(f'Missing {path} ({hint})')

    results = []
    for index, codec in enumerate(args.codecs):
        print(f'--- {codec} ...', flush=True)
        failures, detail = run_codec(args, codec, BASE_PORT + index)
        results.append((codec, failures))
        print(f'    {"PASS" if not failures else "FAIL"}  {detail}')
        for failure in failures:
            print(f'      - {failure}')

    passed = sum(1 for _, f in results if not f)
    print(f'\n{passed}/{len(results)} codecs passed. Details: {args.out}')
    sys.exit(0 if passed == len(results) else 1)


if __name__ == '__main__':
    main()
