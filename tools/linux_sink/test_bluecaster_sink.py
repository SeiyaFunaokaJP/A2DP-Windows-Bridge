"""Tests of bluecaster_sink's codec handling, without Bluetooth hardware.

    .venv/bin/python -m unittest test_bluecaster_sink   (or tools/emu's environment)

Packets are built the way BlueCaster sends them (app/src/crates/codec). The
decoder tests run only where the library is installed (libopus0 for Opus).
"""

import ctypes
import math
import struct
import unittest

import bluecaster_sink as s


def sbc_frame(rate_idx, mode, bitpool, blocks=16, subbands=8):
    """An SBC frame of the right length (header valid, contents zero).
    rate_idx: 0 16k, 1 32k, 2 44.1k, 3 48k; mode: 0 mono 1 dual 2 stereo 3 joint"""
    b1 = (rate_idx << 6) | ((blocks // 4 - 1) << 4) | (mode << 2) | (1 if subbands == 8 else 0)
    hdr = bytes([0x9C, b1, bitpool])
    length = s.sbc_frame_length(hdr)[0]
    return hdr + bytes(length - 3)


def rtp(seq, ts, payload):
    return struct.pack('>BBHII', 0x80, 0x60, seq, ts, 0x1234) + payload


def stats(codec, config):
    return s.StreamStats(1, s.CodecConfig(codec, config), 200, None)


class Endpoints(unittest.TestCase):
    def test_sbc_xq_shares_the_sbc_endpoint(self):
        self.assertEqual(s.endpoints([s.LDAC, s.SBC_XQ, s.SBC]), [s.LDAC, s.SBC])
        self.assertEqual(s.endpoints([s.SBC_XQ]), [s.SBC])
        self.assertEqual(s.endpoints([s.SBC]), [s.SBC])

    def test_dual_channel_only_with_sbc_xq(self):
        self.assertEqual(s.endpoint_capabilities(s.SBC, 53, True)[1], bytes([0xFF, 0xFF, 2, 53]))
        no_xq = s.endpoint_capabilities(s.SBC, 53, False)[1]
        self.assertEqual(no_xq[0] & 0x04, 0)
        self.assertEqual(no_xq[0] & 0x0B, 0x0B)

    def test_sbc_xq_needs_bitpool_43(self):
        self.assertIn(s.SBC_XQ, s.offered_codecs(s.ALL_CODECS, 53))
        self.assertIn(s.SBC_XQ, s.offered_codecs(s.ALL_CODECS, 43))
        self.assertNotIn(s.SBC_XQ, s.offered_codecs(s.ALL_CODECS, 42))

    def test_vendor_capabilities(self):
        # as app/src/crates/codec/src/caps.rs parses them
        t, caps = s.endpoint_capabilities(s.OPUS, 53)
        self.assertEqual((t, struct.unpack_from('<IH', caps)), (0xFF, (0x000000E0, 0x0001)))
        self.assertEqual(caps[6], 0x80 | 0x18 | 0x03)  # 48 kHz, 10 / 20 ms, mono / stereo
        t, caps = s.endpoint_capabilities(s.FASTSTREAM, 53)
        self.assertEqual((t, struct.unpack_from('<IH', caps)), (0xFF, (0x0000000A, 0x0001)))
        self.assertEqual(caps[6:], bytes([0x01, 0x03]))  # music direction, 44.1 / 48 kHz

    def test_names_match_the_receiver_test(self):
        # app/src/ui/src/screens/ReceiverTest.tsx: lower case, no spaces = codec id
        ids = {s.CODEC_NAMES[c].lower().replace(' ', '') for c in (s.SBC_XQ, s.OPUS, s.FASTSTREAM)}
        self.assertEqual(ids, {'sbcxq', 'opus', 'faststream'})


class Configs(unittest.TestCase):
    def test_dual_channel_sbc_is_sbc_xq(self):
        # 48 kHz, dual channel, 16 blocks, 8 subbands, loudness, bitpool 43
        cfg = s.CodecConfig(s.SBC, bytes([0x14, 0x15, 2, 43]))
        self.assertEqual((cfg.codec, cfg.sample_rate, cfg.channels), (s.SBC_XQ, 48000, 2))
        self.assertTrue(cfg.has_rtp)
        self.assertTrue(cfg.desc.startswith('SBC XQ 48000 Hz, dual channel'))
        self.assertEqual(s.CodecConfig(s.SBC, bytes([0x21, 0x15, 2, 53])).codec, s.SBC)  # joint stereo

    def test_faststream(self):
        cfg = s.CodecConfig(s.FASTSTREAM, s.vendor_caps(0x0A, 0x0001, [0x01, 0x02]))
        self.assertEqual((cfg.sample_rate, cfg.channels, cfg.has_rtp), (44100, 2, False))
        self.assertEqual(cfg.desc, 'FastStream 44100 Hz, music')
        self.assertEqual(s.CodecConfig(s.FASTSTREAM, s.vendor_caps(0x0A, 1, [0x01, 0x01])).sample_rate, 48000)

    def test_opus(self):
        cfg = s.CodecConfig(s.OPUS, s.vendor_caps(0xE0, 0x0001, [0x92]))
        self.assertEqual((cfg.sample_rate, cfg.channels, cfg.opus_frame_ms, cfg.has_rtp), (48000, 2, 20, True))
        self.assertEqual(cfg.desc, 'Opus 48000 Hz, stereo, 20 ms frames')
        mono = s.CodecConfig(s.OPUS, s.vendor_caps(0xE0, 0x0001, [0x89]))
        self.assertEqual((mono.channels, mono.opus_frame_ms), (1, 10))


class Streams(unittest.TestCase):
    def test_sbc_xq_packets(self):
        st = stats(s.SBC, bytes([0x14, 0x15, 2, 43]))
        st.decoding = False
        frame = sbc_frame(3, 1, 43)
        for i in range(5):
            st.on_packet(i * 0.01, rtp(i, i * 4 * 128, bytes([4]) + frame * 4))
        self.assertEqual((st.frames, st.samples, st.frame_errors, st.lost, st.ts_errors), (20, 2560, 0, 0, 0))
        self.assertEqual(st.issues, {})
        snap = st.snapshot(1.0)
        self.assertEqual(snap['codec'], 'SBC XQ')

    def test_sbc_xq_flags_joint_stereo_frames(self):
        st = stats(s.SBC, bytes([0x14, 0x15, 2, 43]))
        st.decoding = False
        st.on_packet(0.0, rtp(0, 0, bytes([1]) + sbc_frame(3, 3, 43)))
        self.assertIn('SBC XQ: frame is not dual channel', st.issues)

    def test_faststream_packets(self):
        st = stats(s.FASTSTREAM, s.vendor_caps(0x0A, 0x0001, [0x01, 0x02]))
        st.decoding = False
        frame = sbc_frame(2, 3, 29)
        self.assertEqual(len(frame), 71)
        packet = (frame + b'\0') * 3   # no RTP, no media payload header, padded to 72
        self.assertEqual(len(packet), 216)
        for i in range(10):
            st.on_packet(i * 0.008, packet)
        self.assertEqual((st.frames, st.samples, st.frame_errors), (30, 3840, 0))
        self.assertEqual(st.issues, {})
        self.assertEqual(st.snapshot(1.0)['rtp'], False)

    def test_faststream_rejects_unpadded_frames_with_a_frame_error(self):
        st = stats(s.FASTSTREAM, s.vendor_caps(0x0A, 0x0001, [0x01, 0x02]))
        st.decoding = False
        frame = sbc_frame(2, 3, 29)
        st.on_packet(0.0, frame * 2 + b'\0')   # the second frame starts one byte early
        self.assertEqual(st.frame_errors, 1)

    def test_opus_packet_samples(self):
        self.assertEqual(s.opus_packet_samples(bytes([0xFC, 0])), 960)    # CELT FB 20 ms, code 0
        self.assertEqual(s.opus_packet_samples(bytes([0xF4, 0])), 480)    # CELT FB 10 ms
        self.assertEqual(s.opus_packet_samples(bytes([0xFD, 0])), 1920)   # two 20 ms frames
        self.assertEqual(s.opus_packet_samples(bytes([0xFF, 0x03])), 2880)
        self.assertEqual(s.opus_packet_samples(bytes([0x08, 0])), 960)    # SILK NB 20 ms
        self.assertIsNone(s.opus_packet_samples(bytes([0xFF, 0x07])))     # 140 ms: too long
        self.assertIsNone(s.opus_packet_samples(b''))

    def test_opus_packets(self):
        st = stats(s.OPUS, s.vendor_caps(0xE0, 0x0001, [0x92]))
        if st.decoder:
            st.decoder.close()
        st.decoder, st.decoding = None, False
        for i in range(5):
            st.on_packet(i * 0.02, rtp(i, i * 960, bytes([1, 0xFC]) + bytes(100)))
        self.assertEqual((st.frames, st.samples, st.frame_errors, st.ts_errors), (5, 4800, 0, 0))
        self.assertEqual(st.issues, {})
        st.on_packet(0.1, rtp(5, 4800, bytes([2, 0xF4]) + bytes(50)))   # count 2, 10 ms
        self.assertEqual(st.frame_errors, 1)
        self.assertIn('Opus: packet duration differs from the configured frame duration', st.issues)


@unittest.skipUnless(s.OpusDecoder.available(), 'libopus not installed')
class OpusDecoding(unittest.TestCase):
    def test_decodes_what_libopus_encodes(self):
        lib = s.OpusDecoder.lib
        lib.opus_encoder_create.restype = ctypes.c_void_p
        lib.opus_encoder_create.argtypes = [ctypes.c_int32, ctypes.c_int, ctypes.c_int, ctypes.POINTER(ctypes.c_int)]
        lib.opus_encode.restype = ctypes.c_int32
        lib.opus_encode.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_int32]
        lib.opus_encoder_destroy.argtypes = [ctypes.c_void_p]
        err = ctypes.c_int(0)
        enc = lib.opus_encoder_create(48000, 2, 2049, ctypes.byref(err))  # OPUS_APPLICATION_AUDIO
        self.assertEqual(err.value, 0)
        st = stats(s.OPUS, s.vendor_caps(0xE0, 0x0001, [0x92]))
        self.assertTrue(st.decoding)
        out = ctypes.create_string_buffer(1275)
        for i in range(50):
            pcm = (ctypes.c_int16 * 1920)(*[int(8000 * math.sin(2 * math.pi * 1000 * (i * 960 + n // 2) / 48000))
                                           for n in range(1920)])
            n = lib.opus_encode(enc, pcm, 960, out, len(out))
            self.assertGreater(n, 0)
            st.on_packet(i * 0.02, rtp(i, i * 960, bytes([1]) + out.raw[:n]))
        lib.opus_encoder_destroy(enc)
        self.assertEqual((st.frames, st.samples, st.decode_errors, st.frame_errors), (50, 48000, 0, 0))
        self.assertEqual(st.issues, {})
        st.close()


if __name__ == '__main__':
    unittest.main()
