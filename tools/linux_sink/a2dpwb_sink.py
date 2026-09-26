#!/usr/bin/env python3
"""a2dpwb_sink - A2DP sink on Linux that measures the stream A2DPWB sends.

Runs on a Linux PC with an ordinary Bluetooth adapter (built-in is fine).
While it runs, the adapter is taken from BlueZ and driven directly through
the kernel's HCI user channel by Bumble, which acts as an A2DP sink offering
SBC, AAC, aptX, aptX HD, aptX LL and LDAC with an L2CAP MTU of choice
(BlueZ always uses 672 bytes, too small for LDAC). A2DPWB streams to it like
to headphones. Nothing is played; every media packet is measured:

  - received packets / bytes / bitrate, RTP sequence gaps (lost packets),
    late or duplicated packets, RTP timestamp errors
  - arrival jitter (RFC 3550 style, against the audio carried) and the
    largest gap between packets
  - a playout buffer model: how often a sink holding --buffer ms of audio
    would have run dry (audible dropouts) and for how long, and how often a
    burst would have overflowed it (audio skipped). A source that sends
    nothing for more than 3 s (A2DPWB capturing a silent PC) is counted as a
    pause, not as a dropout
  - frame checks (SBC / LDAC headers, aptX sync) and, where a decoder library
    is installed, decode errors: libsbc (SBC), libfreeaptx (aptX / aptX HD /
    aptX LL), libfdk-aac (AAC); LDAC has no open-source decoder
  - AFH: how many of the 79 channels the link hops over (Read AFH Channel Map)
  - RSSI of the link, read from the controller. For BR/EDR this is not an
    absolute level: 0 means within the controller's golden receive range
    (roughly -60 to -40 dBm), negative below it, positive above it

Statistics are printed once per second and served to A2DPWB over TCP (one
JSON object per line, default port 51201); A2DPWB shows them in its link
quality window next to what it sent. A2DPWB finds the receiver by itself on
the local network (UDP broadcast on the same port). Optional CSV log, WAV of
the decoded audio and HCI capture for a2dpwb_decode --received.

Requirements: Python 3.11+, Bumble (see requirements.txt), root for the HCI
user channel; optional libsbc1, libfreeaptx0, libfdk-aac2. See README.md.

    sudo .venv/bin/python a2dpwb_sink.py [--adapter hci0] [--mtu 1005]

Stop with Ctrl+C; the adapter then returns to BlueZ.
"""

import argparse
import asyncio
import ctypes
import ctypes.util
import datetime
import errno
import fcntl
import json
import logging
import os
import signal
import socket
import struct
import sys
import threading
import time
import wave
import warnings

try:
    from bumble import hci
    from bumble.a2dp import make_audio_sink_service_sdp_records
    from bumble.avdtp import (
        AVDTP_AUDIO_MEDIA_TYPE,
        AVDTP_MEDIA_CODEC_SERVICE_CATEGORY,
        AVDTP_PSM,
        Listener,
        MediaCodecCapabilities,
    )
    from bumble.device import Device, DeviceConfiguration
    from bumble.l2cap import ClassicChannelSpec
    from bumble.snoop import Snooper
    from bumble.transport import open_transport
except ImportError as e:  # pragma: no cover - depends on the environment
    sys.exit(f'{e}: run with the Python of the virtual environment holding requirements.txt '
             '(see README.md)')

VERSION = 1
DEFAULT_PORT = 51201
DEFAULT_MTU = 1005  # L2CAP MTU offered for AVDTP, as many headphones
# HCI Change Connection Packet Type: basic rate DM/DH 1/3/5 allowed, the
# EDR "shall not be used" bits (2-DH1/3-DH1/2-DH3/3-DH3/2-DH5/3-DH5) set
PACKET_TYPES_BR_ONLY = 0xCC18 | 0x3306

SBC, AAC, APTX, APTX_HD, APTX_LL, LDAC = 'sbc', 'aac', 'aptx', 'aptxhd', 'aptxll', 'ldac'
CODEC_NAMES = {SBC: 'SBC', AAC: 'AAC', APTX: 'aptX', APTX_HD: 'aptX HD',
               APTX_LL: 'aptX LL', LDAC: 'LDAC'}
ALL_CODECS = [LDAC, APTX_HD, APTX_LL, APTX, AAC, SBC]
# libldac encoders refuse an L2CAP MTU below 679 bytes
LDAC_MIN_MTU = 679


def vendor_caps(vendor_id, codec_id, value):
    return struct.pack('<IH', vendor_id, codec_id) + bytes(value)


def endpoint_capabilities(codec, sbc_max_bitpool):
    """(A2DP codec type, capabilities) this sink offers: every sample rate and
    channel mode a typical high-end headset offers (as tools/emu/emu_sink.py)."""
    aptx_freq_ch = 0x32  # 44.1 kHz (0x20) | 48 kHz (0x10), stereo (0x02)
    return {
        # 16/32/44.1/48 kHz, all channel modes, blocks, subbands and allocations
        SBC: (0x00, bytes([0xFF, 0xFF, 2, sbc_max_bitpool])),
        # MPEG-2/4 AAC LC, 44.1 kHz | 48 kHz, 1 | 2 channels, VBR, 320 kbps
        AAC: (0x02, bytes([0xC0, 0x01, 0x8C, 0x84, 0xE2, 0x00])),
        APTX: (0xFF, vendor_caps(0x0000004F, 0x0001, [aptx_freq_ch])),
        APTX_HD: (0xFF, vendor_caps(0x000000D7, 0x0024, [aptx_freq_ch, 0, 0, 0, 0])),
        APTX_LL: (0xFF, vendor_caps(0x0000000A, 0x0002, [aptx_freq_ch, 0x00])),
        # 44.1 / 48 / 88.2 / 96 kHz, mono / dual / stereo
        LDAC: (0xFF, vendor_caps(0x0000012D, 0x00AA, [0x3C, 0x07])),
    }[codec]


def log(msg):
    print(f'{datetime.datetime.now():%H:%M:%S} {msg}', flush=True)


# ---------------------------------------------------------------------------
# Codec configuration
# ---------------------------------------------------------------------------

def single_bit(v, nbits):
    """Index of the one bit set in v (bit 0 = LSB), or None."""
    bits = [i for i in range(nbits) if v & (1 << i)]
    return bits[0] if len(bits) == 1 else None


class CodecConfig:
    """A negotiated configuration (the codec information of SET_CONFIGURATION)."""

    def __init__(self, codec, config):
        self.codec = codec
        self.raw = bytes(config)
        self.sample_rate = 0
        self.channels = 0
        self.desc = ''
        self.ldac_sr_id = self.ldac_ch_id = None
        c = self.raw
        if codec == SBC and len(c) >= 4:
            fi, mi = single_bit(c[0] >> 4, 4), single_bit(c[0] & 0x0F, 4)
            bi, si, ai = single_bit(c[1] >> 4, 4), single_bit((c[1] >> 2) & 3, 2), single_bit(c[1] & 3, 2)
            self.sample_rate = [48000, 44100, 32000, 16000][fi] if fi is not None else 0
            self.channels = 1 if mi == 3 else 2
            mode = ['joint stereo', 'stereo', 'dual channel', 'mono'][mi] if mi is not None else '?'
            blocks = [16, 12, 8, 4][bi] if bi is not None else 0
            subbands = [8, 4][si] if si is not None else 0
            alloc = ['loudness', 'SNR'][ai] if ai is not None else '?'
            self.sbc_min_bitpool, self.sbc_max_bitpool = c[2], c[3]
            self.desc = (f'SBC {self.sample_rate} Hz, {mode}, {blocks} blocks, {subbands} subbands, '
                         f'{alloc}, bitpool {c[2]}-{c[3]}')
        elif codec == AAC and len(c) >= 6:
            by_bit = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000]
            fi = single_bit((c[1] << 4) | (c[2] >> 4), 12)
            self.sample_rate = by_bit[fi] if fi is not None else 0
            self.channels = 1 if c[2] & 0x08 else 2
            bitrate = ((c[3] & 0x7F) << 16) | (c[4] << 8) | c[5]
            self.desc = (f'AAC object type 0x{c[0]:02X}, {self.sample_rate} Hz, {self.channels} ch, '
                         f'{"VBR" if c[3] & 0x80 else "CBR"}, bitrate {bitrate}')
        elif codec in (APTX, APTX_HD, APTX_LL) and len(c) >= 7:
            f = c[6] & 0xF0
            self.sample_rate = {0x20: 44100, 0x10: 48000, 0x40: 32000, 0x80: 16000}.get(f, 0)
            self.channels = {0x02: 2, 0x01: 1}.get(c[6] & 0x0F, 0)
            self.desc = f'{CODEC_NAMES[codec]} {self.sample_rate} Hz, {self.channels} ch'
        elif codec == LDAC and len(c) >= 8:
            rates = [44100, 48000, 88200, 96000, 176400, 192000]
            rate_bits = [0x20, 0x10, 0x08, 0x04, 0x02, 0x01]
            if c[6] in rate_bits:
                self.ldac_sr_id = rate_bits.index(c[6])
                self.sample_rate = rates[self.ldac_sr_id]
            self.ldac_ch_id = {0x04: 0, 0x02: 1, 0x01: 2}.get(c[7])
            self.channels = 1 if self.ldac_ch_id == 0 else 2
            mode = {0: 'mono', 1: 'dual channel', 2: 'stereo'}.get(self.ldac_ch_id, '?')
            self.desc = f'LDAC {self.sample_rate} Hz, {mode}'
        if not self.desc:
            self.desc = f'{CODEC_NAMES.get(codec, codec)} (unparsed configuration {c.hex()})'

    @property
    def has_rtp(self):
        # Classic aptX and aptX LL are sent without an RTP header (as by
        # Android, PipeWire and A2DPWB)
        return self.codec not in (APTX, APTX_LL)


# ---------------------------------------------------------------------------
# Optional decoders (ctypes; absent libraries only disable decoding)
# ---------------------------------------------------------------------------

def load_library(names):
    for name in names:
        try:
            return ctypes.CDLL(name)
        except OSError:
            continue
    return None


class SbcDecoder:
    class Struct(ctypes.Structure):
        _fields_ = [('flags', ctypes.c_ulong), ('frequency', ctypes.c_uint8),
                    ('blocks', ctypes.c_uint8), ('subbands', ctypes.c_uint8),
                    ('mode', ctypes.c_uint8), ('allocation', ctypes.c_uint8),
                    ('bitpool', ctypes.c_uint8), ('endian', ctypes.c_uint8),
                    ('priv', ctypes.c_void_p), ('priv_alloc_base', ctypes.c_void_p)]

    lib = None

    @classmethod
    def available(cls):
        if cls.lib is None:
            cls.lib = load_library(['libsbc.so.1', ctypes.util.find_library('sbc') or 'libsbc.so']) or False
            if cls.lib:
                cls.lib.sbc_decode.restype = ctypes.c_ssize_t
                cls.lib.sbc_decode.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t,
                                               ctypes.c_void_p, ctypes.c_size_t,
                                               ctypes.POINTER(ctypes.c_size_t)]
        return bool(cls.lib)

    def __init__(self):
        self.sbc = self.Struct()
        if self.lib.sbc_init(ctypes.byref(self.sbc), ctypes.c_ulong(0)) < 0:
            raise RuntimeError('sbc_init failed')
        self.out = ctypes.create_string_buffer(4096)

    def decode_frame(self, frame):
        """PCM (s16le interleaved) of one SBC frame, or None on error."""
        written = ctypes.c_size_t(0)
        n = self.lib.sbc_decode(ctypes.byref(self.sbc), frame, len(frame), self.out,
                                len(self.out), ctypes.byref(written))
        if n <= 0:
            return None
        return self.out.raw[:written.value]

    def close(self):
        self.lib.sbc_finish(ctypes.byref(self.sbc))


class AptxDecoder:
    lib = None

    @classmethod
    def available(cls):
        if cls.lib is None:
            cls.lib = load_library(['libfreeaptx.so.0', 'libopenaptx.so.0',
                                    ctypes.util.find_library('freeaptx') or 'libfreeaptx.so']) or False
            if cls.lib:
                cls.lib.aptx_init.restype = ctypes.c_void_p
                cls.lib.aptx_init.argtypes = [ctypes.c_int]
                cls.lib.aptx_decode_sync.restype = ctypes.c_size_t
                cls.lib.aptx_decode_sync.argtypes = [
                    ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t,
                    ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_int),
                    ctypes.POINTER(ctypes.c_size_t)]
                cls.lib.aptx_finish.argtypes = [ctypes.c_void_p]
        return bool(cls.lib)

    def __init__(self, hd):
        self.ctx = self.lib.aptx_init(1 if hd else 0)
        if not self.ctx:
            raise RuntimeError('aptx_init failed')
        self.out = ctypes.create_string_buffer(8192)

    def decode(self, data):
        """(PCM s24le interleaved, synced, dropped bytes)"""
        need = (len(data) // 4 + 2) * 24
        if need > len(self.out):
            self.out = ctypes.create_string_buffer(need)
        written, synced, dropped = ctypes.c_size_t(0), ctypes.c_int(0), ctypes.c_size_t(0)
        self.lib.aptx_decode_sync(self.ctx, data, len(data), self.out, len(self.out),
                                  ctypes.byref(written), ctypes.byref(synced), ctypes.byref(dropped))
        return self.out.raw[:written.value], bool(synced.value), dropped.value

    def close(self):
        self.lib.aptx_finish(self.ctx)


class AacDecoder:
    TT_MP4_LATM_MCP1 = 6
    AAC_DEC_OK, AAC_DEC_NOT_ENOUGH_BITS = 0, 0x1002

    class StreamInfo(ctypes.Structure):  # leading fields of CStreamInfo
        _fields_ = [('sampleRate', ctypes.c_int), ('frameSize', ctypes.c_int),
                    ('numChannels', ctypes.c_int)]

    lib = None

    @classmethod
    def available(cls):
        if cls.lib is None:
            cls.lib = load_library(['libfdk-aac.so.2', 'libfdk-aac.so.1',
                                    ctypes.util.find_library('fdk-aac') or 'libfdk-aac.so']) or False
            if cls.lib:
                cls.lib.aacDecoder_Open.restype = ctypes.c_void_p
                cls.lib.aacDecoder_Open.argtypes = [ctypes.c_int, ctypes.c_uint]
                cls.lib.aacDecoder_Fill.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_char_p),
                                                    ctypes.POINTER(ctypes.c_uint), ctypes.POINTER(ctypes.c_uint)]
                cls.lib.aacDecoder_DecodeFrame.argtypes = [ctypes.c_void_p, ctypes.c_void_p,
                                                           ctypes.c_int, ctypes.c_uint]
                cls.lib.aacDecoder_GetStreamInfo.restype = ctypes.POINTER(cls.StreamInfo)
                cls.lib.aacDecoder_GetStreamInfo.argtypes = [ctypes.c_void_p]
                cls.lib.aacDecoder_Close.argtypes = [ctypes.c_void_p]
        return bool(cls.lib)

    def __init__(self):
        self.handle = self.lib.aacDecoder_Open(self.TT_MP4_LATM_MCP1, 1)
        if not self.handle:
            raise RuntimeError('aacDecoder_Open failed')
        self.pcm = (ctypes.c_int16 * (8 * 2048))()

    def decode(self, data):
        """[(PCM s16le, sample_rate, channels, frame_size)], error codes"""
        buf = ctypes.c_char_p(bytes(data))
        size, valid = ctypes.c_uint(len(data)), ctypes.c_uint(len(data))
        frames, errors = [], []
        if self.lib.aacDecoder_Fill(self.handle, ctypes.byref(buf), ctypes.byref(size), ctypes.byref(valid)) != 0:
            return frames, ['fill']
        while True:
            err = self.lib.aacDecoder_DecodeFrame(self.handle, self.pcm, len(self.pcm), 0)
            if err == self.AAC_DEC_NOT_ENOUGH_BITS:
                break
            if err != self.AAC_DEC_OK:
                errors.append(err)
                break
            si = self.lib.aacDecoder_GetStreamInfo(self.handle).contents
            n = si.frameSize * si.numChannels
            frames.append((bytes(memoryview(self.pcm).cast('B')[:n * 2]), si.sampleRate,
                           si.numChannels, si.frameSize))
        return frames, errors

    def close(self):
        self.lib.aacDecoder_Close(self.handle)


def decoder_status():
    return {SBC: SbcDecoder.available(), AAC: AacDecoder.available(),
            APTX: AptxDecoder.available(), APTX_HD: AptxDecoder.available(),
            APTX_LL: AptxDecoder.available(), LDAC: False}


# ---------------------------------------------------------------------------
# Stream analysis
# ---------------------------------------------------------------------------

def sbc_frame_length(hdr):
    """(length, samples, sample rate, channels) of the SBC frame starting with hdr, or None."""
    if len(hdr) < 3 or hdr[0] != 0x9C:
        return None
    b1, bitpool = hdr[1], hdr[2]
    rate = [16000, 32000, 44100, 48000][b1 >> 6]
    blocks = 4 * (((b1 >> 4) & 3) + 1)
    mode = (b1 >> 2) & 3  # 0 mono, 1 dual, 2 stereo, 3 joint
    subbands = 8 if b1 & 1 else 4
    channels = 1 if mode == 0 else 2
    length = 4 + (4 * subbands * channels) // 8
    if mode in (0, 1):
        length += (blocks * channels * bitpool + 7) // 8
    elif mode == 2:
        length += (blocks * bitpool + 7) // 8
    else:
        length += (subbands + blocks * bitpool + 7) // 8
    return length, blocks * subbands, rate, channels


class WavOut:
    def __init__(self, path):
        self.path, self.wav, self.fmt = path, None, None

    def write(self, pcm, rate, channels, width):
        if not pcm:
            return
        if self.wav is None:
            self.wav = wave.open(self.path, 'wb')
            self.wav.setnchannels(channels)
            self.wav.setsampwidth(width)
            self.wav.setframerate(rate)
            self.fmt = (rate, channels, width)
        if (rate, channels, width) == self.fmt:
            self.wav.writeframesraw(pcm)

    def close(self):
        if self.wav:
            self.wav.close()
            self.wav = None


class StreamStats:
    """Everything measured for one configured stream (one SET_CONFIGURATION).

    Counters accumulate over every START / SUSPEND cycle; sequence tracking
    and the playout model restart with each START.
    """

    # The playout clock follows the sender's audio clock within +-500 ppm:
    # enough for any crystal, too little to hide lost audio (aptX / aptX LL
    # carry no sequence numbers, so lost packets only show as missing audio)
    MAX_RATE_DEVIATION = 0.0005

    # The modelled sink keeps its latency bounded: audio beyond this many
    # times the target buffer (a burst after a stall) is skipped
    MAX_BUFFER_FACTOR = 2.0
    # No media for longer than this: the source paused (it sends nothing
    # while the PC is silent), not a radio problem; the playout model
    # starts over when media comes back
    PAUSE_S = 3.0

    def __init__(self, stream_id, cfg, buffer_ms, wav_path):
        self.lock = threading.Lock()
        self.id = stream_id
        self.cfg = cfg
        self.buffer_s = buffer_ms / 1000.0
        self.issues = {}
        self.packets = self.bytes = self.frames = self.samples = 0
        self.lost = self.late = self.ts_errors = self.frame_errors = self.decode_errors = 0
        self.underruns = 0
        self.underrun_s = 0.0
        self.overflows = 0
        self.overflow_s = 0.0
        self.pauses = 0
        self.pause_s = 0.0
        self.max_gap_s = 0.0
        self.jitter_s = 0.0
        self.starts = 0
        self.decoding = False
        self.decoder = None
        self.wav = WavOut(wav_path) if wav_path else None
        self.mtu = 0
        self.started_t = time.monotonic()
        # per-report interval
        self.iv_packets = self.iv_bytes = 0
        self.iv_max_gap_s = 0.0
        self._new_decoder()
        self._reset_timing()

    def _new_decoder(self):
        c = self.cfg.codec
        try:
            if c == SBC and SbcDecoder.available():
                self.decoder = SbcDecoder()
            elif c in (APTX, APTX_HD, APTX_LL) and AptxDecoder.available():
                self.decoder = AptxDecoder(c == APTX_HD)
            elif c == AAC and AacDecoder.available():
                self.decoder = AacDecoder()
        except RuntimeError as e:
            self.issue(f'decoder: {e}')
        self.decoding = self.decoder is not None
        self.aac_synced = False

    def _reset_timing(self):
        self.have_seq = False
        self.last_seq = self.last_ts = 0
        self.samples_at_last_ts = 0
        self.first_t = self.last_t = None
        self._restart_playout()

    def _restart_playout(self):
        self.start_samples = self.samples
        self.transit0 = None
        self.play_end = None       # time at which the buffered audio runs out
        self.rate = 1.0            # sender audio clock / our clock
        self.rate_ref = None       # (arrival time, audio received) the rate is measured from
        self.overflowing = False
        self.audio_rx = 0.0        # audio received since START, seconds

    def on_start(self, mtu):
        with self.lock:
            self.starts += 1
            self.mtu = mtu
            self._reset_timing()
            self.started_t = time.monotonic()

    def issue(self, msg):
        self.issues[msg] = self.issues.get(msg, 0) + 1

    # -- per packet --

    def on_packet(self, t, data):
        with self.lock:
            self._on_packet(t, data)

    def _on_packet(self, t, data):
        self.packets += 1
        self.iv_packets += 1
        self.bytes += len(data)
        self.iv_bytes += len(data)
        if self.last_t is not None:
            gap = t - self.last_t
            if gap > self.PAUSE_S:
                self.pauses += 1
                self.pause_s += gap
                self.issue(f'source paused (no media for more than {self.PAUSE_S:.0f} s)')
                self._restart_playout()
                self.first_t = t
            else:
                self.max_gap_s = max(self.max_gap_s, gap)
                self.iv_max_gap_s = max(self.iv_max_gap_s, gap)
        else:
            self.first_t = t
        self.last_t = t

        payload = data
        if self.cfg.has_rtp:
            payload = self._strip_rtp(data)
            if payload is None:
                return
        before = self.samples
        codec = self.cfg.codec
        if codec == SBC:
            self._sbc(payload)
        elif codec == AAC:
            self._aac(payload)
        elif codec in (APTX, APTX_HD, APTX_LL):
            self._aptx(payload)
        elif codec == LDAC:
            self._ldac(payload)
        audio_s = (self.samples - before) / self.cfg.sample_rate if self.cfg.sample_rate else 0.0
        self._timing(t, audio_s)

    def _strip_rtp(self, p):
        if len(p) < 12 or (p[0] >> 6) != 2:
            self.issue('RTP: missing or invalid header')
            return None
        h = 12 + 4 * (p[0] & 0x0F)
        if p[0] & 0x10:
            if len(p) < h + 4:
                self.issue('RTP: truncated header extension')
                return None
            h += 4 + 4 * struct.unpack_from('>H', p, h + 2)[0]
        pad = p[-1] if p[0] & 0x20 else 0
        if len(p) < h + pad:
            self.issue('RTP: truncated packet')
            return None
        seq, ts = struct.unpack_from('>HI', p, 2)
        if self.have_seq:
            diff = (seq - self.last_seq - 1) & 0xFFFF
            if diff == 0:
                prev = self.samples - self.samples_at_last_ts
                if prev > 0 and ((ts - self.last_ts) & 0xFFFFFFFF) != prev:
                    self.ts_errors += 1
                    self.issue('RTP: timestamp step differs from the audio in the previous packet')
            elif diff < 0x8000:
                self.lost += diff
                self.issue('RTP: sequence gap (packets lost)')
            else:
                self.late += 1
                self.issue('RTP: late or duplicated packet')
                return p[h:len(p) - pad]  # counted, but does not move the sequence
        self.have_seq = True
        self.last_seq, self.last_ts = seq, ts
        self.samples_at_last_ts = self.samples
        return p[h:len(p) - pad]

    def _sbc(self, p):
        if not p:
            self.issue('SBC: empty media payload')
            return
        if p[0] & 0x80:
            self.issue('SBC: fragmented media payload (not analysed)')
            return
        announced, off, got = p[0] & 0x0F, 1, 0
        while off < len(p):
            info = sbc_frame_length(p[off:off + 3])
            if info is None:
                self.frame_errors += 1
                self.issue('SBC: frame sync 0x9C not found where a frame should start')
                break
            length, samples, rate, channels = info
            if off + length > len(p):
                self.frame_errors += 1
                self.issue('SBC: frame runs past the end of the packet')
                break
            if rate != self.cfg.sample_rate:
                self.issue('SBC: frame sampling frequency differs from configuration')
            frame = p[off:off + length]
            if self.decoding:
                pcm = self.decoder.decode_frame(frame)
                if pcm is None:
                    self.decode_errors += 1
                    self.issue('SBC: decode error')
                elif self.wav:
                    self.wav.write(pcm, rate, channels, 2)
            got += 1
            self.frames += 1
            self.samples += samples
            off += length
        if got != announced:
            self.issue('SBC: media payload header frame count differs from frames in packet')

    def _aac(self, p):
        self.frames += 1
        if not self.decoding:
            self.samples += 1024  # one AAC-LC access unit per packet
            return
        frames, errors = self.decoder.decode(p)
        for pcm, rate, channels, frame_size in frames:
            self.aac_synced = True
            self.samples += frame_size
            if self.wav:
                self.wav.write(pcm, rate, channels, 2)
        if not frames:
            self.samples += 1024
        for err in errors:
            if self.aac_synced:
                self.decode_errors += 1
                self.issue(f'AAC: decode error 0x{err:04X}' if isinstance(err, int) else 'AAC: decoder fill failed')

    def _aptx(self, p):
        unit = 6 if self.cfg.codec == APTX_HD else 4
        if len(p) % unit:
            self.frame_errors += 1
            self.issue(f'aptX: payload size is not a multiple of {unit} bytes')
        self.frames += len(p) // unit
        self.samples += (len(p) // unit) * 4
        if self.decoding:
            pcm, synced, dropped = self.decoder.decode(p)
            if dropped:
                self.decode_errors += 1
                self.issue('aptX: bytes dropped by the decoder (parity / resync)')
            if not synced:
                self.issue('aptX: decoder not synchronized at end of packet')
            if self.wav:
                self.wav.write(pcm, self.cfg.sample_rate, 2, 3)

    def _ldac(self, p):
        if not p:
            self.issue('LDAC: empty media payload')
            return
        hdr, off, got = p[0], 1, 0
        while off < len(p):
            if len(p) - off < 3:
                self.frame_errors += 1
                self.issue('LDAC: trailing bytes after the last frame')
                break
            if p[off] != 0xAA:
                self.frame_errors += 1
                self.issue('LDAC: frame sync 0xAA not found where a frame should start')
                break
            sr_id, ch_id = p[off + 1] >> 5, (p[off + 1] >> 3) & 3
            total = 3 + (((p[off + 1] & 7) << 6) | (p[off + 2] >> 2)) + 1
            if off + total > len(p):
                self.frame_errors += 1
                self.issue('LDAC: frame runs past the end of the packet')
                break
            if sr_id != self.cfg.ldac_sr_id:
                self.issue('LDAC: frame sampling rate differs from configuration')
            if ch_id != self.cfg.ldac_ch_id:
                self.issue('LDAC: frame channel config differs from configuration')
            got += 1
            self.frames += 1
            self.samples += 256 if sr_id >= 2 else 128
            off += total
        if (hdr & 0x0F) != got:
            self.issue('LDAC: media payload header frame count differs from frames in packet')

    def _timing(self, t, audio_s):
        """Jitter and the playout buffer model; t is the arrival time."""
        if not self.cfg.sample_rate:
            return
        # RFC 3550 interarrival jitter against the audio carried so far
        media_t = (self.samples - self.start_samples) / self.cfg.sample_rate
        transit = t - media_t
        if self.transit0 is not None:
            d = abs(transit - self.transit0)
            self.jitter_s += (d - self.jitter_s) / 16.0
        self.transit0 = transit
        # Playout: a sink starts playing once it holds buffer_s of audio and
        # plays it at the sender's rate, so a small difference between the two
        # clocks does not count as dropouts. The rate is measured from 2 s
        # into the stream (after the start-up burst) and used after 10 s more.
        # Lost packets are not concealed: they shorten the audio.
        self.audio_rx += audio_s
        if self.rate_ref is None:
            if self.first_t is not None and t - self.first_t >= 2.0:
                self.rate_ref = (t, self.audio_rx)
        elif t - self.rate_ref[0] >= 10.0:
            r = (self.audio_rx - self.rate_ref[1]) / (t - self.rate_ref[0])
            self.rate = min(max(r, 1 - self.MAX_RATE_DEVIATION), 1 + self.MAX_RATE_DEVIATION)
        if self.play_end is None:
            self.play_end = t + self.buffer_s + audio_s / self.rate
        elif t > self.play_end:
            # ran dry at play_end: silent until this packet plus the rebuffering
            self.underruns += 1
            self.underrun_s += (t - self.play_end) + self.buffer_s
            self.issue('playout: buffer ran dry (audible dropout)')
            self.play_end = t + self.buffer_s + audio_s / self.rate
        else:
            self.play_end += audio_s / self.rate
            excess = (self.play_end - t) - self.MAX_BUFFER_FACTOR * self.buffer_s
            if excess > 0:
                # a burst overfilled the buffer: the sink skips audio (one
                # event per burst, however many packets it spans)
                if not self.overflowing:
                    self.overflows += 1
                    self.issue('playout: buffer overflowed (audio skipped)')
                self.overflow_s += excess
                self.play_end -= excess
            self.overflowing = excess > 0

    # -- reporting (main loop) --

    def snapshot(self, now):
        with self.lock:
            buffer_ms = None
            idle = self.last_t is not None and now - self.last_t > self.PAUSE_S
            if self.play_end is not None and not idle:
                buffer_ms = max(0.0, (self.play_end - now) * 1000.0)
                if now > self.play_end:
                    # dry right now; counted when the next packet arrives
                    buffer_ms = 0.0
            snap = {
                'id': self.id,
                'codec': CODEC_NAMES[self.cfg.codec],
                'config': self.cfg.desc,
                'sample_rate': self.cfg.sample_rate,
                'channels': self.cfg.channels,
                'rtp': self.cfg.has_rtp,
                'decoding': self.decoding,
                'mtu': self.mtu,
                'starts': self.starts,
                'packets': self.packets,
                'bytes': self.bytes,
                'frames': self.frames,
                'audio_ms': round(1000.0 * self.samples / self.cfg.sample_rate, 1) if self.cfg.sample_rate else 0,
                'lost': self.lost,
                'late': self.late,
                'ts_errors': self.ts_errors,
                'frame_errors': self.frame_errors,
                'decode_errors': self.decode_errors,
                'underruns': self.underruns,
                'underrun_ms': round(self.underrun_s * 1000.0, 1),
                'overflows': self.overflows,
                'overflow_ms': round(self.overflow_s * 1000.0, 1),
                'pauses': self.pauses,
                'pause_ms': round(self.pause_s * 1000.0, 1),
                'idle': idle,
                'max_gap_ms': round(self.max_gap_s * 1000.0, 1),
                'jitter_ms': round(self.jitter_s * 1000.0, 2),
                'buffer_ms': None if buffer_ms is None else round(buffer_ms, 1),
                'interval': {
                    'packets': self.iv_packets,
                    'bytes': self.iv_bytes,
                    'max_gap_ms': round(self.iv_max_gap_s * 1000.0, 1),
                },
                'issues': dict(self.issues),
            }
            self.iv_packets = self.iv_bytes = 0
            self.iv_max_gap_s = 0.0
            return snap

    def close(self):
        with self.lock:
            if self.decoder:
                self.decoder.close()
                self.decoder = None
            self.decoding = False
            if self.wav:
                self.wav.close()


# ---------------------------------------------------------------------------
# Stats server (TCP, JSON lines)
# ---------------------------------------------------------------------------

class StatsServer:
    """Sends the statistics to every client; lines a client sends (JSON
    objects) go to on_message(obj) in the event loop."""

    def __init__(self, bind, port, on_message=None):
        self.on_message = on_message
        self.loop = asyncio.get_running_loop()
        self.clients = []
        self.lock = threading.Lock()
        self.sock = socket.socket(socket.AF_INET6 if ':' in bind else socket.AF_INET)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind((bind, port))
        self.sock.listen(4)
        self.hello = b''
        threading.Thread(target=self._accept, daemon=True).start()

    def _accept(self):
        while True:
            try:
                conn, addr = self.sock.accept()
            except OSError:
                return
            conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            conn.settimeout(2.0)
            log(f'stats client connected: {addr[0]}:{addr[1]}')
            try:
                conn.sendall(self.hello)
            except OSError:
                conn.close()
                continue
            with self.lock:
                self.clients.append((conn, addr))
            threading.Thread(target=self._read, args=(conn,), daemon=True).start()

    def _read(self, conn):
        buf = b''
        while True:
            try:
                data = conn.recv(4096)
            except socket.timeout:
                continue
            except OSError:
                return
            if not data:
                return
            buf += data
            while b'\n' in buf:
                line, buf = buf.split(b'\n', 1)
                try:
                    obj = json.loads(line.decode('utf-8'))
                except (UnicodeDecodeError, ValueError):
                    continue
                if isinstance(obj, dict) and self.on_message:
                    self.loop.call_soon_threadsafe(self.on_message, obj)
            if len(buf) > 65536:
                return

    def send(self, obj):
        line = (json.dumps(obj, separators=(',', ':')) + '\n').encode()
        with self.lock:
            alive = []
            for conn, addr in self.clients:
                try:
                    conn.sendall(line)
                    alive.append((conn, addr))
                except OSError:
                    log(f'stats client disconnected: {addr[0]}:{addr[1]}')
                    conn.close()
            self.clients = alive

    def close(self):
        self.sock.close()
        with self.lock:
            for conn, _ in self.clients:
                conn.close()
            self.clients = []


# ---------------------------------------------------------------------------
# Discovery (UDP): A2DPWB broadcasts {"type":"discover"}, we answer
# ---------------------------------------------------------------------------

class DiscoveryResponder(asyncio.DatagramProtocol):
    def __init__(self, sink):
        self.sink = sink
        self.transport = None

    def connection_made(self, transport):
        self.transport = transport

    def datagram_received(self, data, addr):
        try:
            msg = json.loads(data.decode('utf-8'))
        except (UnicodeDecodeError, ValueError):
            return
        if not isinstance(msg, dict) or msg.get('type') != 'discover':
            return
        reply = json.dumps(self.sink.announcement(), separators=(',', ':')).encode()
        self.transport.sendto(reply, addr)
        if addr[0] not in self.sink.discovered_by:
            self.sink.discovered_by.add(addr[0])
            log(f'discovered by {addr[0]}')


# ---------------------------------------------------------------------------
# HCI
# ---------------------------------------------------------------------------

class PacketLoggerSnooper(Snooper):
    """Writes HCI packets in PacketLogger format (as A2DPWB's own capture),
    for a2dpwb_decode --received."""

    def __init__(self, path):
        self.file = open(path, 'wb', buffering=0)  # unbuffered: survives being killed

    def snoop(self, hci_packet, direction):
        h4_type, payload = hci_packet[0], hci_packet[1:]
        to_controller = direction == Snooper.Direction.HOST_TO_CONTROLLER
        if h4_type == hci.HCI_COMMAND_PACKET:
            pl_type = 0x00
        elif h4_type == hci.HCI_EVENT_PACKET:
            pl_type = 0x01
        elif h4_type == hci.HCI_ACL_DATA_PACKET:
            pl_type = 0x02 if to_controller else 0x03
        else:
            return
        now = time.time()
        sec, usec = int(now), int((now - int(now)) * 1e6)
        self.file.write(struct.pack('>IIIB', 9 + len(payload), sec, usec, pl_type) + payload)


HCIDEVDOWN = 0x400448CA  # _IOW('H', 202, int)


def adapter_index(name):
    if name is None:
        adapters = sorted(os.listdir('/sys/class/bluetooth')) if os.path.isdir('/sys/class/bluetooth') else []
        adapters = [a for a in adapters if a.startswith('hci') and a[3:].isdigit()]
        if not adapters:
            sys.exit('No Bluetooth adapter found (/sys/class/bluetooth is empty)')
        name = adapters[0]
    if not (name.startswith('hci') and name[3:].isdigit()):
        sys.exit(f'--adapter: expected hciN, got {name}')
    return int(name[3:])


async def open_adapter(index):
    """Takes the adapter from BlueZ and opens its HCI user channel. The kernel
    only grants the user channel to an adapter that is down; bluetoothd sees
    it disappear and gets it back when the channel is closed."""
    if os.geteuid() != 0:
        sys.exit('The HCI user channel needs root: run with sudo')
    for attempt in range(10):
        try:
            with socket.socket(socket.AF_BLUETOOTH, socket.SOCK_RAW, socket.BTPROTO_HCI) as s:
                fcntl.ioctl(s.fileno(), HCIDEVDOWN, index)
        except OSError as e:
            if e.errno == errno.ENODEV:
                sys.exit(f'hci{index} does not exist')
            if e.errno != errno.EALREADY:
                log(f'warning: could not take hci{index} down: {e.strerror}')
        try:
            return await open_transport(f'hci-socket:{index}')
        except Exception as e:  # bumble wraps the bind error
            last = e
            await asyncio.sleep(0.3)  # bluetoothd may have powered it up again
    sys.exit(f'Cannot open the HCI user channel of hci{index}: {last}\n'
             'Is another program using it exclusively (btmon is fine)?')


# ---------------------------------------------------------------------------
# The sink
# ---------------------------------------------------------------------------

class Sink:
    def __init__(self, args):
        self.args = args
        self.device = None
        self.stream_seq = 0
        self.current = None         # StreamStats of the configured stream
        self.streaming = False
        self.idle_reported = False
        self.connections = {}       # handle -> peer address
        self.rssi = None
        self.rssi_note = None
        self.afh_channels = None
        self.discovered_by = set()
        self.server = None
        self.csv = None
        self.adapter_name = 'hci?'
        self.address = ''

    # -- setup --

    async def start(self):
        a = self.args
        if a.transport:
            transport = await open_transport(a.transport)
            self.adapter_name = a.transport
        else:
            index = adapter_index(a.adapter)
            self.adapter_name = f'hci{index}'
            transport = await open_adapter(index)
        self.hci_transport = transport

        config = DeviceConfiguration(
            name=a.name or f'A2DPWB Sink ({socket.gethostname()})',
            class_of_device=0x240404,  # Audio / Video, wearable headset
            classic_enabled=True,
            le_enabled=False,
            keystore=f'JsonKeyStore:{a.keys}',
        )
        device = Device.from_config_with_hci(config, transport.source, transport.sink)
        self.device = device
        if a.capture:
            device.host.snooper = PacketLoggerSnooper(a.capture)
        device.sdp_service_records = {0x00010001: make_audio_sink_service_sdp_records(0x00010001)}
        await device.power_on()
        self.address = str(device.public_address).replace('/P', '')
        await device.set_discoverable(not a.no_discoverable)
        await device.set_connectable(True)

        # AVDTP with the L2CAP MTU we choose (Listener.for_device() uses the default)
        listener = Listener(version=(1, 3))
        l2cap_server = device.create_l2cap_server(spec=ClassicChannelSpec(psm=AVDTP_PSM, mtu=a.mtu))
        self.l2cap_server = l2cap_server
        l2cap_server.on(l2cap_server.EVENT_CONNECTION, listener.on_l2cap_connection)
        listener.on(listener.EVENT_CONNECTION, self.on_avdtp_connection)
        device.on(device.EVENT_CONNECTION, self.on_connection)

        decoders = decoder_status()
        dec = ', '.join(f'{CODEC_NAMES[c]} {"decoded" if decoders[c] else "headers only"}' for c in a.codecs)
        log(f'adapter {self.adapter_name} {self.address} "{config.name}" (taken from BlueZ until exit)')
        log(f'A2DP sink: {", ".join(CODEC_NAMES[c] for c in a.codecs)}; L2CAP MTU {a.mtu}')
        log(f'analysis: {dec}')
        if LDAC in a.codecs and a.mtu < LDAC_MIN_MTU:
            log(f'warning: LDAC needs an MTU of at least {LDAC_MIN_MTU}; A2DPWB cannot stream LDAC with {a.mtu}')

        self.server = StatsServer(a.bind, a.port, self.on_message)
        self.server.hello = self.hello_line()
        loop = asyncio.get_running_loop()
        await loop.create_datagram_endpoint(lambda: DiscoveryResponder(self), local_addr=(a.bind, a.port),
                                            allow_broadcast=True)
        log(f'statistics: TCP {a.bind}:{a.port}, discovery: UDP {a.port} '
            f'(A2DPWB finds this PC on the local network)')
        if a.csv:
            self.open_csv(a.csv)

    def on_message(self, msg):
        """Settings from A2DPWB, applied from the next connection on:
        {"type":"configure","mtu":1005,"codecs":["SBC",...],"buffer_ms":200}"""
        if msg.get('type') != 'configure':
            return
        a, changes = self.args, []
        mtu = msg.get('mtu')
        if isinstance(mtu, int) and 48 <= mtu <= 65535 and mtu != a.mtu:
            a.mtu = mtu
            self.l2cap_server.spec.mtu = mtu
            changes.append(f'L2CAP MTU {mtu}')
            if LDAC in a.codecs and mtu < LDAC_MIN_MTU:
                log(f'warning: LDAC needs an MTU of at least {LDAC_MIN_MTU}')
        codecs = msg.get('codecs')
        if isinstance(codecs, list) and codecs:
            by_name = {n.lower(): c for c, n in CODEC_NAMES.items()} | {c: c for c in CODEC_NAMES}
            chosen = [by_name[str(n).lower()] for n in codecs if str(n).lower() in by_name]
            chosen = [c for c in ALL_CODECS if c in chosen]
            if chosen and chosen != a.codecs:
                a.codecs = chosen
                changes.append('codecs ' + ', '.join(CODEC_NAMES[c] for c in chosen))
        buffer_ms = msg.get('buffer_ms')
        if isinstance(buffer_ms, int) and 10 <= buffer_ms <= 5000 and buffer_ms != a.buffer:
            a.buffer = buffer_ms
            changes.append(f'buffer {buffer_ms} ms')
        if changes:
            log('configured by A2DPWB (from the next connection): ' + '; '.join(changes))
            self.server.hello = self.hello_line()

    def announcement(self):
        return {'type': 'announce', 'version': VERSION, 'tool': 'a2dpwb_sink',
                'host': socket.gethostname(), 'port': self.args.port,
                'bt_address': self.address, 'bt_name': self.device.name if self.device else '',
                'codecs': [CODEC_NAMES[c] for c in self.args.codecs], 'mtu': self.args.mtu,
                'buffer_ms': self.args.buffer}

    def hello_line(self):
        hello = dict(self.announcement(), type='hello', adapter=self.adapter_name, address=self.address)
        return (json.dumps(hello, separators=(',', ':')) + '\n').encode()

    async def stop(self):
        if self.current and self.streaming:
            self.print_summary(self.current)
        if self.server:
            self.server.close()
        if self.csv:
            self.csv.close()
        if self.current:
            self.current.close()
        await self.hci_transport.close()

    # -- Bluetooth events --

    def on_connection(self, connection):
        addr = str(connection.peer_address).replace('/P', '')
        self.connections[connection.handle] = addr
        log(f'connected: {addr}')

        def on_disconnection(reason):
            self.connections.pop(connection.handle, None)
            self.rssi = None
            log(f'disconnected: {addr} (reason 0x{reason:02X})')
            self.end_stream()
        connection.on(connection.EVENT_DISCONNECTION, on_disconnection)
        if self.args.packet_types == 'br':
            asyncio.get_running_loop().create_task(self.restrict_packet_types(connection))

    async def restrict_packet_types(self, connection):
        """Basic rate only on this link (--packet-types br), a measurement
        condition: EDR packets are never sent to us or by us."""
        try:
            await self.device.host.send_async_command(hci.HCI_Change_Connection_Packet_Type_Command(
                connection_handle=connection.handle, packet_type=PACKET_TYPES_BR_ONLY))
            log('packet types: basic rate only (no EDR)')
        except Exception as e:  # HCI_Error from the controller
            log(f'warning: could not restrict packet types: {e}')

    def on_avdtp_connection(self, protocol):
        codec_caps = {c: endpoint_capabilities(c, self.args.sbc_max_bitpool) for c in self.args.codecs}
        for codec, (codec_type, caps) in codec_caps.items():
            sink = protocol.add_sink(MediaCodecCapabilities(
                media_type=AVDTP_AUDIO_MEDIA_TYPE, media_codec_type=codec_type,
                media_codec_information=caps))
            sink.on(sink.EVENT_CONFIGURATION, lambda s=sink, c=codec: self.on_configured(s, c))
            sink.on(sink.EVENT_START, self.on_start)
            sink.on(sink.EVENT_SUSPEND, self.on_suspend)
            sink.on(sink.EVENT_CLOSE, self.end_stream)
            sink.on(sink.EVENT_ABORT, self.end_stream)
            sink.on(sink.EVENT_RTP_CHANNEL_CLOSE, self.end_stream)
            # Raw media: Bumble would parse every packet as RTP, but classic
            # aptX / aptX LL carry no RTP header
            sink.on_avdtp_packet = self.on_media

    def on_configured(self, sink, codec):
        config = b''
        for cap in sink.configuration:
            if cap.service_category == AVDTP_MEDIA_CODEC_SERVICE_CATEGORY:
                config = bytes(cap.media_codec_information)
        cfg = CodecConfig(codec, config)
        self.end_stream()
        self.stream_seq += 1
        wav = f'{self.args.wav}_{self.stream_seq}_{codec}.wav' if self.args.wav and codec != LDAC else None
        self.current = StreamStats(self.stream_seq, cfg, self.args.buffer, wav)
        log(f'configured: {cfg.desc}'
            + ('' if self.current.decoding or codec == LDAC else ' (no decoder library: headers only)'))

    def on_start(self):
        if self.current:
            self.current.on_start(self.args.mtu)
            self.streaming = True
            self.idle_reported = False
            log(f'streaming {self.current.cfg.desc}')

    def on_suspend(self):
        if self.streaming:
            log('stream suspended')
        self.streaming = False

    def end_stream(self):
        if self.current and self.streaming:
            log('stream closed')
            self.print_summary(self.current)
        self.streaming = False

    def on_media(self, packet):
        if self.current and self.streaming:
            if self.idle_reported:
                log('media again')
                self.idle_reported = False
            self.current.on_packet(time.monotonic(), bytes(packet))

    # -- periodic --

    async def run_ticks(self):
        while True:
            await asyncio.sleep(1.0)
            await self.read_rssi()
            self.tick()

    async def read_rssi(self):
        if not self.connections:
            return
        handle = next(iter(self.connections))
        try:
            rp = await self.device.host.send_sync_command(hci.HCI_Read_RSSI_Command(handle=handle))
            self.rssi, self.rssi_note = rp.rssi, None
        except Exception as e:  # HCI_Error, timeout
            self.rssi, self.rssi_note = None, f'Read RSSI failed: {e}'[:120]
        # HCI Read AFH Channel Map (no command class in Bumble): status,
        # handle, mode, 10-byte map of the channels the link hops over
        try:
            command = hci.HCI_Command.from_bytes(bytes([hci.HCI_COMMAND_PACKET]) +
                                                 struct.pack('<HBH', 0x1406, 2, handle))
            response = await self.device.host.send_command(command)
            rp = response.return_parameters
            data = bytes(getattr(rp, 'data', rp)) if not isinstance(rp, (bytes, bytearray)) else bytes(rp)
            if len(data) >= 14 and data[0] == 0:
                self.afh_channels = (sum(bin(b).count('1') for b in data[4:14]) - (data[13] >> 7)
                                     if data[3] == 1 else 79)
            else:
                self.afh_channels = None
        except Exception:
            self.afh_channels = None

    # The stream stays open while the source sends nothing (a silent PC);
    # this is only reported, once per pause
    IDLE_REPORT_S = 10.0

    def tick(self):
        now = time.time()
        if self.streaming and self.current and not self.idle_reported:
            last = self.current.last_t or self.current.started_t
            if time.monotonic() - last > self.IDLE_REPORT_S:
                log(f'no media for {self.IDLE_REPORT_S:.0f} s: the source sends nothing (stream still open)')
                self.idle_reported = True
        snap = self.current.snapshot(time.monotonic()) if self.current else None
        if snap and not self.streaming:
            snap['buffer_ms'] = None
        device = next(iter(self.connections.values()), None)
        rssi = self.rssi if device else None
        afh = self.afh_channels if device else None
        msg = {
            'type': 'stats', 'version': VERSION, 'time': round(now, 3),
            'adapter': self.adapter_name, 'address': self.address,
            'connected': device is not None, 'device': device,
            'streaming': self.streaming, 'buffer_target_ms': self.args.buffer, 'mtu': self.args.mtu,
            'codecs': [CODEC_NAMES[c] for c in self.args.codecs],
            'rssi': rssi, 'tx_power': None, 'rssi_note': self.rssi_note if device else None,
            'afh_channels': afh,
            'stream': snap,
        }
        self.server.send(msg)
        if self.streaming and snap and not snap['idle']:
            self.print_line(snap, rssi)
            self.write_csv(now, snap, rssi)

    def print_line(self, s, rssi):
        iv = s['interval']
        kbps = iv['bytes'] * 8 / 1000.0
        buf = '-' if s['buffer_ms'] is None else f'{s["buffer_ms"]:.0f}'
        line = (f'{s["codec"]:<7} {kbps:7.1f} kbps {iv["packets"]:4d} pkt/s  '
                f'lost {s["lost"]}  late {s["late"]}  jitter {s["jitter_ms"]:.1f} ms  '
                f'gap {iv["max_gap_ms"]:.0f} ms  buffer {buf} ms  dropouts {s["underruns"]}')
        if s['overflows']:
            line += f'  skips {s["overflows"]}'
        errors = s['frame_errors'] + s['decode_errors'] + s['ts_errors']
        if errors:
            line += f'  errors {errors}'
        if rssi is not None:
            line += f'  RSSI {rssi:+d} dB vs. golden range'
        if self.afh_channels is not None:
            line += f'  AFH {self.afh_channels} ch'
        log(line)

    def print_summary(self, s):
        snap = s.snapshot(time.monotonic())
        log(f'summary of stream {snap["id"]}: {snap["config"]}')
        log(f'  {snap["packets"]} packets, {snap["audio_ms"] / 1000:.1f} s of audio, '
            f'lost {snap["lost"]}, late {snap["late"]}, max gap {snap["max_gap_ms"]:.0f} ms, '
            f'dropouts {snap["underruns"]} ({snap["underrun_ms"]:.0f} ms), '
            f'skips {snap["overflows"]} ({snap["overflow_ms"]:.0f} ms), '
            f'pauses {snap["pauses"]} ({snap["pause_ms"] / 1000:.1f} s)')
        for msg, n in sorted(snap['issues'].items()):
            log(f'  - {msg}: {n}')

    CSV_FIELDS = ['time', 'codec', 'kbps', 'packets', 'lost', 'late', 'ts_errors', 'frame_errors',
                  'decode_errors', 'jitter_ms', 'max_gap_ms', 'buffer_ms', 'underruns', 'underrun_ms',
                  'overflows', 'overflow_ms', 'rssi']

    def open_csv(self, path):
        new = not os.path.exists(path) or os.path.getsize(path) == 0
        self.csv = open(path, 'a', encoding='utf-8', buffering=1)
        if new:
            self.csv.write(','.join(self.CSV_FIELDS) + '\n')

    def write_csv(self, now, s, rssi):
        if not self.csv:
            return
        iv = s['interval']
        row = [datetime.datetime.fromtimestamp(now).isoformat(timespec='seconds'), s['codec'],
               f'{iv["bytes"] * 8 / 1000.0:.1f}', iv['packets'], s['lost'], s['late'], s['ts_errors'],
               s['frame_errors'], s['decode_errors'], s['jitter_ms'], iv['max_gap_ms'],
               '' if s['buffer_ms'] is None else s['buffer_ms'], s['underruns'], s['underrun_ms'],
               s['overflows'], s['overflow_ms'], '' if rssi is None else rssi]
        self.csv.write(','.join(str(v) for v in row) + '\n')


async def run(args):
    sink = Sink(args)
    await sink.start()
    log('waiting for A2DPWB to connect... (Ctrl+C to stop)')
    stop = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, stop.set)
    ticks = asyncio.create_task(sink.run_ticks())
    await stop.wait()
    ticks.cancel()
    await sink.stop()
    log(f'{sink.adapter_name} returned to BlueZ')


def main():
    parser = argparse.ArgumentParser(
        description='A2DP sink on Linux that measures the stream A2DPWB sends (Bumble, HCI user channel).',
        epilog='Needs root. The adapter is not available to BlueZ while this runs. See README.md.')
    parser.add_argument('--adapter', help='adapter to use, e.g. hci0 (default: the first one)')
    parser.add_argument('--transport', help='Bumble transport instead of an adapter (testing), '
                                            'e.g. tcp-client:127.0.0.1:9002')
    parser.add_argument('--codecs', nargs='+', choices=ALL_CODECS, default=ALL_CODECS,
                        help='codecs to offer (default: all)')
    parser.add_argument('--mtu', type=int, default=DEFAULT_MTU,
                        help=f'L2CAP MTU offered for AVDTP (default {DEFAULT_MTU}; BlueZ uses 672, '
                             f'LDAC needs {LDAC_MIN_MTU})')
    parser.add_argument('--packet-types', choices=['all', 'br'], default='all',
                        help='ACL packet types allowed on the link: all (default) or br = basic '
                             'rate only, no EDR (1 Mbps instead of 2 / 3 Mbps)')
    parser.add_argument('--sbc-max-bitpool', type=int, default=53,
                        help='highest SBC bitpool offered (default 53, as most headphones)')
    parser.add_argument('--buffer', type=int, default=200,
                        help='playout buffer of the modelled sink in ms (default 200); it '
                             'skips audio beyond twice this')
    parser.add_argument('--bind', default='0.0.0.0', help='address the statistics / discovery sockets use')
    parser.add_argument('--port', type=int, default=DEFAULT_PORT,
                        help=f'TCP statistics and UDP discovery port (default {DEFAULT_PORT})')
    parser.add_argument('--csv', help='append one row of statistics per second to this CSV file')
    parser.add_argument('--wav', metavar='PREFIX',
                        help='write the decoded audio to PREFIX_<stream>_<codec>.wav')
    parser.add_argument('--capture', metavar='FILE.pklg',
                        help='HCI capture of the whole run, for a2dpwb_decode --received')
    parser.add_argument('--name', help='Bluetooth name (default "A2DPWB Sink (<host name>)")')
    parser.add_argument('--keys', default=os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                                       'a2dpwb_sink_keys.json'),
                        help='where pairing keys are kept (default: next to this script)')
    parser.add_argument('--verbose', action='store_true', help='show Bumble warnings and debug logs')
    parser.add_argument('--no-discoverable', action='store_true',
                        help='connectable but not discoverable (A2DPWB must know the address)')
    args = parser.parse_args()
    if not 2 <= args.sbc_max_bitpool <= 250:
        parser.error('--sbc-max-bitpool must be 2-250')
    if not 48 <= args.mtu <= 65535:
        parser.error('--mtu must be 48-65535')
    logging.basicConfig(level=logging.DEBUG if args.verbose else logging.WARNING,
                        format='%(asctime)s %(name)s %(levelname)s: %(message)s')
    if not args.verbose:
        # Bumble warns about sources that close the signaling channel right
        # after SUSPEND (as A2DPWB does when stopping); the stream end is
        # detected anyway
        logging.getLogger('bumble').setLevel(logging.ERROR)
        warnings.filterwarnings('ignore', category=RuntimeWarning, message='coroutine .* was never awaited')
    asyncio.run(run(args))


if __name__ == '__main__':
    main()
