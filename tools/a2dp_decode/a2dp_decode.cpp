/*
 * a2dpwb_decode - offline checker for the A2DP media stream A2DPWB sends.
 *
 * Reads the PacketLogger HCI capture (.pklg) that A2DPWB writes in debug mode
 * (Debug > Start HCI Capture), follows L2CAP / AVDTP signaling to find the negotiated
 * codec, extracts the outgoing media packets and decodes them:
 *
 *   SBC               Bluedroid SBC decoder (BTstack 3rd-party/bluedroid)
 *   AAC               fdk-aac (LATM, muxConfigPresent=1)
 *   aptX / HD / LL    libopenaptx 0.2.0
 *   LDAC              no open-source decoder: frame headers are validated only
 *
 * Decoded audio is written to WAV files next to the dump. The report on
 * stdout lists each configuration, packet/frame counts and every
 * inconsistency found (RTP, media payload header, frame headers vs the
 * negotiated configuration, decoder errors).
 *
 * Deliberately does not link BTstack: HCI / L2CAP / AVDTP parsing is
 * self-contained, so this tool only carries the codec libraries' licenses
 * (see THIRD_PARTY_LICENSES.md).
 *
 * Usage: a2dpwb_decode <capture.pklg> [-o <output prefix>] [--no-wav] [--received]
 * Exit:  0 = no problems found, 1 = problems found, 2 = usage / file error
 */

#include <cstdarg>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <map>
#include <memory>
#include <string>
#include <tuple>
#include <vector>

extern "C" {
#include "oi_codec_sbc.h"
#include "oi_status.h"
#include "openaptx.h"
}
#include "aacdecoder_lib.h"

/* ------------------------------------------------------------------------ */
/* Byte helpers                                                             */
/* ------------------------------------------------------------------------ */

static uint16_t le16(const uint8_t *p) { return (uint16_t)(p[0] | (p[1] << 8)); }
static uint32_t le32(const uint8_t *p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}
static uint16_t be16(const uint8_t *p) { return (uint16_t)((p[0] << 8) | p[1]); }
static uint32_t be32(const uint8_t *p) {
    return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) | ((uint32_t)p[2] << 8) | p[3];
}

/* ------------------------------------------------------------------------ */
/* Codec configuration (AVDTP Media Codec capability)                       */
/* ------------------------------------------------------------------------ */

enum class Codec { Unknown, SBC, AAC, Aptx, AptxHD, AptxLL, LDAC };

static const char *codec_name(Codec c) {
    switch (c) {
    case Codec::SBC:    return "SBC";
    case Codec::AAC:    return "AAC";
    case Codec::Aptx:   return "aptX";
    case Codec::AptxHD: return "aptX HD";
    case Codec::AptxLL: return "aptX LL";
    case Codec::LDAC:   return "LDAC";
    default:            return "unknown";
    }
}

static const char *codec_file_tag(Codec c) {
    switch (c) {
    case Codec::SBC:    return "sbc";
    case Codec::AAC:    return "aac";
    case Codec::Aptx:   return "aptx";
    case Codec::AptxHD: return "aptxhd";
    case Codec::AptxLL: return "aptxll";
    case Codec::LDAC:   return "ldac";
    default:            return "unknown";
    }
}

struct CodecConfig {
    Codec codec = Codec::Unknown;
    uint32_t sample_rate = 0;
    int channels = 0;
    std::string desc;

    /* SBC (Bluedroid decoder conventions: mode SBC_MONO.., alloc SBC_LOUDNESS/SNR) */
    int sbc_mode = -1, sbc_blocks = 0, sbc_subbands = 0, sbc_alloc = -1;
    int sbc_min_bitpool = 0, sbc_max_bitpool = 0;
    /* LDAC frame header ids: sampling rate id and channel config id */
    int ldac_sr_id = -1, ldac_ch_id = -1;

    /* A2DPWB sends classic aptX and aptX LL without an RTP header */
    bool has_rtp() const { return codec != Codec::Aptx && codec != Codec::AptxLL; }
};

/* Exactly one bit must be set in a configuration (not capability) field. */
static int single_bit_index(uint32_t v, int nbits) {
    int found = -1;
    for (int i = 0; i < nbits; i++) {
        if (v & (1u << i)) {
            if (found >= 0) return -2; /* more than one bit */
            found = i;
        }
    }
    return found;
}

static std::string strf(const char *fmt, ...) {
    char buf[512];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    return buf;
}

/* p points at the Media Codec capability value: [media type][codec type][info...] */
static CodecConfig parse_media_codec(const uint8_t *p, size_t n) {
    CodecConfig c;
    if (n < 2) { c.desc = "truncated media codec capability"; return c; }
    uint8_t codec_type = p[1];
    const uint8_t *info = p + 2;
    size_t len = n - 2;

    if (codec_type == 0x00 && len >= 4) {
        /* A2DP SBC codec information element */
        static const uint32_t freqs[4] = {48000, 44100, 32000, 16000};   /* bit0..3 of high nibble */
        static const int modes[4] = {SBC_JOINT_STEREO, SBC_STEREO, SBC_DUAL_CHANNEL, SBC_MONO};
        static const char *mode_names[4] = {"joint stereo", "stereo", "dual channel", "mono"};
        static const int blocks[4] = {16, 12, 8, 4};
        static const int subbands[2] = {8, 4};
        int fi = single_bit_index(info[0] >> 4, 4);
        int mi = single_bit_index(info[0] & 0x0F, 4);
        int bi = single_bit_index(info[1] >> 4, 4);
        int si = single_bit_index((info[1] >> 2) & 0x03, 2);
        int ai = single_bit_index(info[1] & 0x03, 2);
        c.codec = Codec::SBC;
        c.sample_rate = fi >= 0 ? freqs[fi] : 0;
        c.sbc_mode = mi >= 0 ? modes[mi] : -1;
        c.channels = mi == 3 ? 1 : (mi >= 0 ? 2 : 0);
        c.sbc_blocks = bi >= 0 ? blocks[bi] : 0;
        c.sbc_subbands = si >= 0 ? subbands[si] : 0;
        c.sbc_alloc = ai == 0 ? SBC_LOUDNESS : (ai == 1 ? SBC_SNR : -1);
        c.sbc_min_bitpool = info[2];
        c.sbc_max_bitpool = info[3];
        c.desc = strf("SBC %u Hz, %s, %d blocks, %d subbands, %s, bitpool %d-%d",
            c.sample_rate, mi >= 0 ? mode_names[mi] : "?", c.sbc_blocks, c.sbc_subbands,
            ai == 0 ? "loudness" : (ai == 1 ? "SNR" : "?"), c.sbc_min_bitpool, c.sbc_max_bitpool);
        if (fi < 0 || mi < 0 || bi < 0 || si < 0 || ai < 0)
            c.desc += " (INVALID: not exactly one option per field)";
        return c;
    }

    if (codec_type == 0x02 && len >= 6) {
        /* A2DP MPEG-2/4 AAC codec information element */
        /* 12-bit sampling frequency field: bit 11 = 8000 ... bit 0 = 96000 */
        static const uint32_t by_bit[12] = {96000, 88200, 64000, 48000, 44100, 32000,
                                            24000, 22050, 16000, 12000, 11025, 8000};
        uint32_t fbits = ((uint32_t)info[1] << 4) | (info[2] >> 4);
        int fi = single_bit_index(fbits, 12);
        c.codec = Codec::AAC;
        c.sample_rate = fi >= 0 ? by_bit[fi] : 0;
        c.channels = (info[2] & 0x08) ? 1 : ((info[2] & 0x04) ? 2 : 0);
        uint32_t bitrate = ((uint32_t)(info[3] & 0x7F) << 16) | ((uint32_t)info[4] << 8) | info[5];
        c.desc = strf("AAC object type 0x%02X, %u Hz, %d ch, %s, bitrate %u", info[0],
            c.sample_rate, c.channels, (info[3] & 0x80) ? "VBR" : "CBR", bitrate);
        if (fi < 0 || (info[2] & 0x0C) == 0x0C || c.channels == 0)
            c.desc += " (INVALID: not exactly one option per field)";
        return c;
    }

    if (codec_type == 0xFF && len >= 6) {
        uint32_t vendor = le32(info);
        uint16_t vcodec = le16(info + 4);
        const uint8_t *v = info + 6;
        size_t vlen = len - 6;

        bool aptx = vendor == 0x4F && vcodec == 0x0001;
        bool aptx_hd = vendor == 0xD7 && vcodec == 0x0024;
        bool aptx_ll = (vendor == 0x0A || vendor == 0xD7) && vcodec == 0x0002;
        if ((aptx || aptx_hd || aptx_ll) && vlen >= 1) {
            c.codec = aptx ? Codec::Aptx : (aptx_hd ? Codec::AptxHD : Codec::AptxLL);
            uint8_t f = v[0] & 0xF0;
            c.sample_rate = f == 0x20 ? 44100 : f == 0x10 ? 48000 : f == 0x40 ? 32000 : f == 0x80 ? 16000 : 0;
            uint8_t ch = v[0] & 0x0F;
            c.channels = ch == 0x02 ? 2 : ch == 0x01 ? 1 : 0;
            c.desc = strf("%s %u Hz, %d ch", codec_name(c.codec), c.sample_rate, c.channels);
            if (c.sample_rate == 0 || c.channels == 0)
                c.desc += " (INVALID: not exactly one option per field)";
            return c;
        }
        if (vendor == 0x12D && vcodec == 0x00AA && vlen >= 2) {
            static const uint32_t rates[6] = {44100, 48000, 88200, 96000, 176400, 192000};
            static const uint8_t rate_bits[6] = {0x20, 0x10, 0x08, 0x04, 0x02, 0x01};
            c.codec = Codec::LDAC;
            for (int i = 0; i < 6; i++)
                if (v[0] == rate_bits[i]) { c.sample_rate = rates[i]; c.ldac_sr_id = i; }
            /* A2DP channel mode 0x04 mono / 0x02 dual / 0x01 stereo ->
             * LDAC frame header channel config id 0 mono / 1 dual / 2 stereo */
            if (v[1] == 0x04) { c.channels = 1; c.ldac_ch_id = 0; }
            if (v[1] == 0x02) { c.channels = 2; c.ldac_ch_id = 1; }
            if (v[1] == 0x01) { c.channels = 2; c.ldac_ch_id = 2; }
            c.desc = strf("LDAC %u Hz, %s", c.sample_rate,
                c.ldac_ch_id == 0 ? "mono" : c.ldac_ch_id == 1 ? "dual channel" :
                c.ldac_ch_id == 2 ? "stereo" : "?");
            if (c.ldac_sr_id < 0 || c.ldac_ch_id < 0)
                c.desc += " (INVALID: not exactly one option per field)";
            return c;
        }
        c.desc = strf("vendor codec 0x%08X:0x%04X (not supported by this tool)", vendor, vcodec);
        return c;
    }

    c.desc = strf("codec type 0x%02X (not supported by this tool)", codec_type);
    return c;
}

/* ------------------------------------------------------------------------ */
/* WAV writer (opened lazily on the first write)                            */
/* ------------------------------------------------------------------------ */

class WavWriter {
public:
    ~WavWriter() { close(); }
    void set_path(const std::string &path) { path_ = path; }
    const std::string &path() const { return path_; }

    void write(const void *data, size_t bytes, uint32_t rate, uint16_t channels, uint16_t bits) {
        if (path_.empty() || failed_) return;
        if (!f_) {
            f_ = fopen(path_.c_str(), "wb");
            if (!f_) {
                fprintf(stderr, "Cannot create %s\n", path_.c_str());
                failed_ = true;
                return;
            }
            rate_ = rate; channels_ = channels; bits_ = bits;
            uint8_t hdr[44] = {0};
            fwrite(hdr, 1, sizeof(hdr), f_); /* patched in close() */
        }
        fwrite(data, 1, bytes, f_);
        data_bytes_ += bytes;
    }

    void close() {
        if (!f_) return;
        uint16_t align = (uint16_t)(channels_ * (bits_ / 8));
        uint32_t data = data_bytes_ > 0xFFFFFFF0u - 36 ? 0xFFFFFFF0u - 36 : (uint32_t)data_bytes_;
        uint8_t h[44];
        memcpy(h, "RIFF", 4);  put32(h + 4, 36 + data);
        memcpy(h + 8, "WAVEfmt ", 8);
        put32(h + 16, 16);     put16(h + 20, 1);  /* PCM */
        put16(h + 22, channels_);
        put32(h + 24, rate_);  put32(h + 28, rate_ * align);
        put16(h + 32, align);  put16(h + 34, bits_);
        memcpy(h + 36, "data", 4); put32(h + 40, data);
        fseek(f_, 0, SEEK_SET);
        fwrite(h, 1, sizeof(h), f_);
        fclose(f_);
        f_ = nullptr;
    }

private:
    static void put16(uint8_t *p, uint32_t v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); }
    static void put32(uint8_t *p, uint32_t v) { put16(p, v); put16(p + 2, v >> 16); }

    std::string path_;
    FILE *f_ = nullptr;
    bool failed_ = false;
    uint64_t data_bytes_ = 0;
    uint32_t rate_ = 0;
    uint16_t channels_ = 0, bits_ = 0;
};

/* ------------------------------------------------------------------------ */
/* Stream session: one accepted configuration and the media sent with it    */
/* ------------------------------------------------------------------------ */

class Session {
public:
    Session(int index, uint16_t handle, double start, const CodecConfig &cfg, const std::string &wav_path)
        : index_(index), handle_(handle), start_(start), cfg_(cfg) {
        wav_.set_path(wav_path);
        switch (cfg_.codec) {
        case Codec::SBC:
            if (OI_CODEC_SBC_DecoderReset(&sbc_, sbc_data_, sizeof(sbc_data_), SBC_MAX_CHANNELS,
                                          (OI_UINT8)(cfg_.channels == 1 ? 1 : 2), FALSE) != OI_OK)
                issue("SBC: decoder reset failed");
            break;
        case Codec::AAC:
            aac_ = aacDecoder_Open(TT_MP4_LATM_MCP1, 1);
            if (!aac_) issue("AAC: aacDecoder_Open failed");
            break;
        case Codec::Aptx:
        case Codec::AptxLL:
        case Codec::AptxHD:
            aptx_ = aptx_init(cfg_.codec == Codec::AptxHD ? 1 : 0);
            if (!aptx_) issue("aptX: aptx_init failed");
            break;
        default:
            break;
        }
    }

    ~Session() {
        if (aac_) aacDecoder_Close(aac_);
        if (aptx_) aptx_finish(aptx_);
    }

    void on_packet(double t, const uint8_t *p, size_t n) {
        packets_++;
        if (packets_ == 1) first_t_ = t;
        else if (t - last_t_ > max_gap_) max_gap_ = t - last_t_;
        last_t_ = t;

        const uint8_t *pl = p;
        size_t pn = n;
        if (cfg_.has_rtp()) {
            if (!strip_rtp(p, n, &pl, &pn)) return;
        }
        payload_bytes_ += pn;

        switch (cfg_.codec) {
        case Codec::SBC:    decode_sbc(pl, pn); break;
        case Codec::AAC:    decode_aac(pl, pn); break;
        case Codec::Aptx:
        case Codec::AptxLL:
        case Codec::AptxHD: decode_aptx(pl, pn); break;
        case Codec::LDAC:   check_ldac(pl, pn); break;
        default: break;
        }
    }

    void finish() {
        if (aptx_) {
            size_t left = aptx_decode_sync_finish(aptx_);
            if (left) issue("aptX: undecoded bytes left at end of stream");
        }
        wav_.close();
    }

    bool has_problems() const { return !issues_.empty() || cfg_.codec == Codec::Unknown; }

    void report(double t0) const {
        printf("\n=== Stream %d (ACL handle 0x%03X, configured at %.3f s) ===\n",
               index_, handle_, start_ - t0);
        printf("  Configuration : %s\n", cfg_.desc.c_str());
        if (cfg_.codec == Codec::Unknown) {
            printf("  Not decoded: codec not supported by this tool\n");
            return;
        }
        double wall = packets_ > 1 ? last_t_ - first_t_ : 0.0;
        printf("  Media packets : %llu over %.3f s (largest gap between packets %.1f ms)\n",
               (unsigned long long)packets_, wall, max_gap_ * 1000.0);
        if (packets_ == 0) {
            printf("  No media packets were sent with this configuration.\n");
        } else {
            printf("  RTP header    : %s\n", cfg_.has_rtp() ? "expected, present" : "not used (as in Android / PipeWire)");
            if (cfg_.has_rtp() && have_seq_) {
                double ts_rate = wall > 0 ? (double)(uint32_t)(last_ts_ - first_ts_) / wall : 0.0;
                printf("  RTP           : payload type %u, %llu sequence gaps, timestamp advances ~%.0f /s\n",
                       rtp_pt_, (unsigned long long)seq_gaps_, ts_rate);
            }
            double audio_s = cfg_.sample_rate ? (double)samples_ / cfg_.sample_rate : 0.0;
            printf("  Frames        : %llu (%s)\n", (unsigned long long)frames_,
                   cfg_.codec == Codec::LDAC ? "headers validated; LDAC has no open-source decoder"
                                             : "decoded");
            if (audio_s > 0) {
                printf("  Audio         : %.3f s of audio at %u Hz%s\n", audio_s, cfg_.sample_rate,
                       wall > 0 ? strf(" (%.1f%% of the streaming time)", 100.0 * audio_s / wall).c_str() : "");
                printf("  Bitrate       : %.1f kbps (media payload per second of audio)\n",
                       payload_bytes_ * 8.0 / audio_s / 1000.0);
            }
            if (cfg_.codec == Codec::LDAC && ldac_frame_bytes_ > 0 && frames_ > 0) {
                printf("  LDAC frames   : %llu-%llu bytes incl. 3-byte header, %llu-%llu frames per packet\n",
                       (unsigned long long)ldac_min_frame_, (unsigned long long)ldac_max_frame_,
                       (unsigned long long)ldac_min_fpp_, (unsigned long long)ldac_max_fpp_);
            }
            if (!wav_.path().empty() && samples_ > 0)
                printf("  WAV           : %s\n", wav_.path().c_str());
        }
        if (issues_.empty()) {
            printf("  Result        : OK, no problems found\n");
        } else {
            printf("  Result        : %zu kind(s) of problem found\n", issues_.size());
            for (const auto &it : issues_)
                printf("    - %s: %llu time(s), first in media packet #%llu\n", it.first.c_str(),
                       (unsigned long long)it.second.count, (unsigned long long)it.second.first_packet);
        }
    }

private:
    struct Issue { uint64_t count = 0; uint64_t first_packet = 0; };

    void issue(const std::string &msg) {
        Issue &i = issues_[msg];
        if (i.count++ == 0) i.first_packet = packets_;
    }

    bool strip_rtp(const uint8_t *p, size_t n, const uint8_t **pl, size_t *pn) {
        if (n < 12 || (p[0] >> 6) != 2) { issue("RTP: missing or invalid header (version != 2)"); return false; }
        size_t h = 12 + 4 * (size_t)(p[0] & 0x0F);
        if (p[0] & 0x10) { /* header extension */
            if (n < h + 4) { issue("RTP: truncated header extension"); return false; }
            h += 4 + 4 * (size_t)be16(p + h + 2);
        }
        size_t pad = (p[0] & 0x20) ? p[n - 1] : 0;
        if (n < h + pad) { issue("RTP: truncated packet"); return false; }
        uint16_t seq = be16(p + 2);
        uint32_t ts = be32(p + 4);
        if (!have_seq_) {
            have_seq_ = true;
            first_ts_ = ts;
            rtp_pt_ = p[1] & 0x7F;
        } else {
            if (seq != (uint16_t)(last_seq_ + 1)) { seq_gaps_++; issue("RTP: sequence number gap"); }
            if ((int32_t)(ts - last_ts_) < 0) issue("RTP: timestamp went backwards");
            /* samples_ already includes the previous packet (skipped while a
             * decoder is still producing no audio, e.g. AAC before sync) */
            uint64_t prev_samples = samples_ - samples_at_last_ts_;
            if (prev_samples > 0 && (uint32_t)(ts - last_ts_) != (uint32_t)prev_samples)
                issue("RTP: timestamp step differs from the samples in the previous packet");
            if ((p[1] & 0x7F) != rtp_pt_) issue("RTP: payload type changed");
        }
        last_seq_ = seq;
        last_ts_ = ts;
        samples_at_last_ts_ = samples_;
        *pl = p + h;
        *pn = n - h - pad;
        return true;
    }

    void decode_sbc(const uint8_t *p, size_t n) {
        if (n < 1) { issue("SBC: empty media payload"); return; }
        if (p[0] & 0x80) { issue("SBC: fragmented media payload (not supported by this tool)"); return; }
        int announced = p[0] & 0x0F;
        const OI_BYTE *data = p + 1;
        OI_UINT32 left = (OI_UINT32)(n - 1);
        int got = 0;
        while (left > 0) {
            OI_INT16 pcm[SBC_MAX_CHANNELS * SBC_MAX_BANDS * SBC_MAX_BLOCKS];
            OI_UINT32 pcm_bytes = sizeof(pcm);
            OI_STATUS st = OI_CODEC_SBC_DecodeFrame(&sbc_, &data, &left, pcm, &pcm_bytes);
            if (st != OI_OK) { issue(strf("SBC: decode error (OI_STATUS %d)", (int)st)); break; }
            got++;
            frames_++;
            const OI_CODEC_SBC_FRAME_INFO &fi = sbc_.common.frameInfo;
            if (fi.frequency != cfg_.sample_rate) issue("SBC: frame sampling frequency differs from configuration");
            if (fi.mode != cfg_.sbc_mode) issue("SBC: frame channel mode differs from configuration");
            if (fi.nrof_blocks != cfg_.sbc_blocks) issue("SBC: frame block length differs from configuration");
            if (fi.nrof_subbands != cfg_.sbc_subbands) issue("SBC: frame subbands differ from configuration");
            if (fi.alloc != cfg_.sbc_alloc) issue("SBC: frame allocation method differs from configuration");
            if (fi.bitpool < cfg_.sbc_min_bitpool || fi.bitpool > cfg_.sbc_max_bitpool)
                issue("SBC: frame bitpool outside configured range");
            samples_ += (uint64_t)fi.nrof_blocks * fi.nrof_subbands;
            wav_.write(pcm, pcm_bytes, fi.frequency, fi.nrof_channels, 16);
        }
        if (got != announced) issue("SBC: media payload header frame count differs from frames in packet");
    }

    void decode_aac(const uint8_t *p, size_t n) {
        if (!aac_) return;
        UCHAR *buf = const_cast<UCHAR *>(p);
        UINT size = (UINT)n, valid = (UINT)n;
        if (aacDecoder_Fill(aac_, &buf, &size, &valid) != AAC_DEC_OK || valid != 0) {
            issue("AAC: aacDecoder_Fill could not take the whole packet");
            return;
        }
        int got = 0;
        for (;;) {
            static INT_PCM pcm[8 * 2048];
            AAC_DECODER_ERROR err = aacDecoder_DecodeFrame(aac_, pcm, (INT)(sizeof(pcm) / sizeof(pcm[0])), 0);
            if (err == AAC_DEC_NOT_ENOUGH_BITS) break;
            if (err != AAC_DEC_OK) {
                /* Before the first in-band StreamMuxConfig the decoder cannot
                 * work; that is expected only at the very start. */
                issue(aac_synced_ ? strf("AAC: decode error 0x%04X", (unsigned)err)
                                  : strf("AAC: decode error 0x%04X before the first decoded frame", (unsigned)err));
                break;
            }
            aac_synced_ = true;
            got++;
            frames_++;
            const CStreamInfo *si = aacDecoder_GetStreamInfo(aac_);
            if (!si) break;
            if ((uint32_t)si->sampleRate != cfg_.sample_rate) issue("AAC: stream sample rate differs from configuration");
            if (si->numChannels != cfg_.channels) issue("AAC: stream channel count differs from configuration");
            if (si->aot != AOT_AAC_LC) issue("AAC: audio object type is not AAC-LC");
            samples_ += (uint64_t)si->frameSize;
            wav_.write(pcm, (size_t)si->frameSize * si->numChannels * sizeof(INT_PCM),
                       (uint32_t)si->sampleRate, (uint16_t)si->numChannels, 16);
        }
        if (got == 0 && aac_synced_) issue("AAC: packet produced no audio frame");
        if (got > 1) issue("AAC: more than one access unit in a packet");
    }

    void decode_aptx(const uint8_t *p, size_t n) {
        if (!aptx_) return;
        size_t unit = cfg_.codec == Codec::AptxHD ? 6 : 4;
        if (n % unit) issue(strf("aptX: payload size is not a multiple of %zu bytes", unit));
        std::vector<uint8_t> out((n / unit + 2) * 24);
        size_t written = 0, dropped = 0;
        int synced = 0;
        aptx_decode_sync(aptx_, p, n, out.data(), out.size(), &written, &synced, &dropped);
        if (dropped) issue("aptX: bytes dropped by the decoder (parity check / resync)");
        if (!synced) issue("aptX: decoder not synchronized at end of packet");
        frames_ += n / unit;
        samples_ += (n / unit) * 4;
        wav_.write(out.data(), written, cfg_.sample_rate, 2, 24);
    }

    void check_ldac(const uint8_t *p, size_t n) {
        if (n < 1) { issue("LDAC: empty media payload"); return; }
        uint8_t hdr = p[0];
        size_t off = 1;
        uint64_t got = 0;
        while (off < n) {
            if (n - off < 3) { issue("LDAC: trailing bytes after the last frame"); break; }
            if (p[off] != 0xAA) { issue("LDAC: frame sync word 0xAA not found where a frame should start"); break; }
            int sr_id = p[off + 1] >> 5;
            int ch_id = (p[off + 1] >> 3) & 0x03;
            size_t frame_len = ((size_t)(p[off + 1] & 0x07) << 6 | (p[off + 2] >> 2)) + 1;
            size_t total = 3 + frame_len;
            if (off + total > n) { issue("LDAC: frame runs past the end of the packet"); break; }
            if (sr_id != cfg_.ldac_sr_id) issue("LDAC: frame sampling rate differs from configuration");
            if (ch_id != cfg_.ldac_ch_id) issue("LDAC: frame channel config differs from configuration");
            got++;
            frames_++;
            samples_ += (sr_id >= 2) ? 256 : 128; /* LDAC_2FSLSU / LDAC_1FSLSU */
            ldac_frame_bytes_ += total;
            if (ldac_min_frame_ == 0 || total < ldac_min_frame_) ldac_min_frame_ = total;
            if (total > ldac_max_frame_) ldac_max_frame_ = total;
            off += total;
        }
        if (got) {
            if (ldac_min_fpp_ == 0 || got < ldac_min_fpp_) ldac_min_fpp_ = got;
            if (got > ldac_max_fpp_) ldac_max_fpp_ = got;
        }
        /* AOSP (A2DP_LDAC_HDR_NUM_MSK 0x0F) and PipeWire (struct rtp_payload)
         * carry the frame count in the low 4 bits of this byte. */
        if ((uint64_t)(hdr & 0x0F) != got) {
            if ((uint64_t)(hdr >> 4) == got)
                issue("LDAC: media payload header has the frame count in the HIGH 4 bits "
                      "(AOSP / PipeWire use the low 4 bits)");
            else
                issue("LDAC: media payload header frame count differs from frames in packet");
        }
    }

    int index_;
    uint16_t handle_;
    double start_;
    CodecConfig cfg_;
    WavWriter wav_;
    std::map<std::string, Issue> issues_;

    uint64_t packets_ = 0, payload_bytes_ = 0, frames_ = 0, samples_ = 0;
    double first_t_ = 0, last_t_ = 0, max_gap_ = 0;

    bool have_seq_ = false;
    uint16_t last_seq_ = 0;
    uint32_t first_ts_ = 0, last_ts_ = 0;
    uint64_t samples_at_last_ts_ = 0;
    uint8_t rtp_pt_ = 0;
    uint64_t seq_gaps_ = 0;

    OI_CODEC_SBC_DECODER_CONTEXT sbc_ = {};
    OI_UINT32 sbc_data_[CODEC_DATA_WORDS(SBC_MAX_CHANNELS, SBC_CODEC_FAST_FILTER_BUFFERS)] = {};
    HANDLE_AACDECODER aac_ = nullptr;
    bool aac_synced_ = false;
    struct aptx_context *aptx_ = nullptr;

    uint64_t ldac_frame_bytes_ = 0, ldac_min_frame_ = 0, ldac_max_frame_ = 0;
    uint64_t ldac_min_fpp_ = 0, ldac_max_fpp_ = 0;
};

/* ------------------------------------------------------------------------ */
/* HCI / L2CAP / AVDTP tracking                                             */
/* ------------------------------------------------------------------------ */

enum Dir { OUT = 0, IN = 1 }; /* OUT = host (A2DPWB) -> controller */

static const uint16_t PSM_AVDTP = 0x0019;

struct L2capChannel {
    uint16_t psm, local_cid, remote_cid;
};

struct HandleState {
    std::vector<uint8_t> reasm[2];      /* ACL -> L2CAP reassembly per direction */
    std::vector<L2capChannel> avdtp;    /* in order of establishment: [0] signaling, [1] media */
    uint64_t acl_out = 0, completed = 0, flushes = 0, media_in = 0;
};

struct PendingConn { uint16_t psm, scid; };
struct PendingConfig { CodecConfig cfg; uint8_t signal; };

class Analyzer {
public:
    Analyzer(std::string prefix, bool write_wav, bool received)
        : prefix_(std::move(prefix)), write_wav_(write_wav), media_dir_(received ? IN : OUT) {}

    void on_record(double t, uint8_t type, const uint8_t *p, size_t n) {
        /* Time 0 = first packet. Notes are skipped: a capture started during a
         * connection writes its note before the (older) setup packets. */
        if (!have_t0_ && type != 0xFC) { t0_ = t; have_t0_ = true; }
        now_ = have_t0_ ? t : t0_;
        switch (type) {
        case 0x01: on_event(p, n); break;
        case 0x02: on_acl(OUT, p, n); break;
        case 0x03: on_acl(IN, p, n); break;
        case 0xFC: /* text note; A2DPWB marks where its HCI capture starts / stops */
            if (n >= 7 && memcmp(p, "A2DPWB:", 7) == 0)
                printf("[%9.3f] %.*s\n", rel(), (int)n, (const char *)p);
            break;
        default: break;
        }
    }

    int finish() {
        while (!active_.empty()) finish_session(active_.begin()->first);
        bool problems = false;
        for (const auto &s : sessions_) {
            s->report(t0_);
            problems |= s->has_problems();
        }
        if (sessions_.empty()) {
            printf("\nNo accepted AVDTP SET_CONFIGURATION / RECONFIGURE found in the dump.\n"
                   "Start the capture before connecting, or while connected with an A2DPWB\n"
                   "version that records the connection setup (Debug > Start HCI Capture).\n");
            problems = true;
        }
        printf("\n=== HCI flow control ===\n");
        for (const auto &it : handles_) {
            const HandleState &h = it.second;
            printf("  ACL handle 0x%03X: %llu ACL packets sent, %llu reported completed by the controller, "
                   "%llu flush events\n", it.first, (unsigned long long)h.acl_out,
                   (unsigned long long)h.completed, (unsigned long long)h.flushes);
            if (h.flushes) problems = true;
            if (h.media_in)
                printf("    note: %llu media packets received from the remote device (not analysed)\n",
                       (unsigned long long)h.media_in);
        }
        if (avdtp_fragmented_)
            printf("  note: %llu fragmented AVDTP signaling packets were skipped\n",
                   (unsigned long long)avdtp_fragmented_);
        printf("\nRESULT: %s\n", problems ? "PROBLEMS FOUND (see above)" : "OK");
        return problems ? 1 : 0;
    }

private:
    double rel() const { return now_ - t0_; }

    void on_event(const uint8_t *p, size_t n) {
        if (n < 2) return;
        uint8_t code = p[0];
        const uint8_t *e = p + 2;
        size_t len = n - 2;
        if (code == 0x13 && len >= 1) { /* Number Of Completed Packets */
            size_t num = e[0];
            for (size_t i = 0; i < num && 1 + i * 4 + 4 <= len; i++) {
                uint16_t handle = le16(e + 1 + i * 4) & 0x0FFF;
                handles_[handle].completed += le16(e + 3 + i * 4);
            }
        } else if (code == 0x11 && len >= 2) { /* Flush Occurred */
            handles_[le16(e) & 0x0FFF].flushes++;
        } else if (code == 0x05 && len >= 4 && e[0] == 0) { /* Disconnection Complete */
            uint16_t handle = le16(e + 1) & 0x0FFF;
            printf("[%9.3f] ACL 0x%03X disconnected (reason 0x%02X)\n", rel(), handle, e[3]);
            finish_session(handle);
            handles_[handle].avdtp.clear();
            handles_[handle].reasm[0].clear();
            handles_[handle].reasm[1].clear();
        }
    }

    void on_acl(Dir dir, const uint8_t *p, size_t n) {
        if (n < 4) return;
        uint16_t handle = le16(p) & 0x0FFF;
        uint8_t pb = (p[1] >> 4) & 0x03;
        size_t len = le16(p + 2);
        if (4 + len > n) return;
        HandleState &h = handles_[handle];
        if (dir == OUT) h.acl_out++;
        std::vector<uint8_t> &r = h.reasm[dir];
        if (pb == 0x01) {           /* continuing fragment */
            if (r.empty()) return;
        } else {                    /* first fragment (0b00 or 0b10) */
            r.clear();
        }
        r.insert(r.end(), p + 4, p + 4 + len);
        if (r.size() < 4) return;
        size_t l2len = le16(r.data());
        if (r.size() < 4 + l2len) return;
        on_l2cap(handle, dir, le16(r.data() + 2), r.data() + 4, l2len);
        r.clear();
    }

    void on_l2cap(uint16_t handle, Dir dir, uint16_t cid, const uint8_t *p, size_t n) {
        if (cid == 0x0001) { on_l2cap_signaling(handle, dir, p, n); return; }
        HandleState &h = handles_[handle];
        for (size_t i = 0; i < h.avdtp.size(); i++) {
            const L2capChannel &ch = h.avdtp[i];
            if (cid != (dir == OUT ? ch.remote_cid : ch.local_cid)) continue;
            if (i == 0) {
                on_avdtp_signaling(handle, dir, p, n);
            } else if (dir == media_dir_) {
                auto it = active_.find(handle);
                if (it != active_.end()) it->second->on_packet(now_, p, n);
            } else {
                h.media_in++;
            }
            return;
        }
    }

    void on_l2cap_signaling(uint16_t handle, Dir dir, const uint8_t *p, size_t n) {
        HandleState &h = handles_[handle];
        size_t off = 0;
        while (off + 4 <= n) {
            uint8_t code = p[off], id = p[off + 1];
            size_t len = le16(p + off + 2);
            const uint8_t *d = p + off + 4;
            if (off + 4 + len > n) break;
            if (code == 0x02 && len >= 4) {          /* Connection Request: PSM, SCID */
                pending_conn_[std::make_tuple(handle, (int)dir, id)] = {le16(d), le16(d + 2)};
            } else if (code == 0x03 && len >= 8) {   /* Connection Response: DCID, SCID, result */
                auto key = std::make_tuple(handle, (int)(dir == OUT ? IN : OUT), id);
                auto it = pending_conn_.find(key);
                uint16_t result = le16(d + 4);
                if (it != pending_conn_.end() && result != 1 /* pending */) {
                    if (result == 0 && it->second.psm == PSM_AVDTP) {
                        uint16_t dcid = le16(d), scid = le16(d + 2);
                        bool requested_by_us = std::get<1>(key) == OUT;
                        L2capChannel ch{PSM_AVDTP, requested_by_us ? scid : dcid, requested_by_us ? dcid : scid};
                        h.avdtp.push_back(ch);
                        printf("[%9.3f] ACL 0x%03X AVDTP %s channel open (local CID 0x%04X, remote CID 0x%04X)\n",
                               rel(), handle, h.avdtp.size() == 1 ? "signaling" : "media",
                               ch.local_cid, ch.remote_cid);
                    }
                    pending_conn_.erase(it);
                }
            } else if (code == 0x06 && len >= 4) {   /* Disconnection Request: DCID, SCID */
                uint16_t dcid = le16(d), scid = le16(d + 2);
                uint16_t local = dir == OUT ? scid : dcid;
                for (size_t i = 0; i < h.avdtp.size(); i++) {
                    if (h.avdtp[i].local_cid != local) continue;
                    printf("[%9.3f] ACL 0x%03X AVDTP %s channel closed\n", rel(), handle,
                           i == 0 ? "signaling" : "media");
                    if (i != 0) finish_session(handle);
                    h.avdtp.erase(h.avdtp.begin() + i);
                    break;
                }
            }
            off += 4 + len;
        }
    }

    static const char *signal_name(uint8_t s) {
        static const char *names[] = {"?", "DISCOVER", "GET_CAPABILITIES", "SET_CONFIGURATION",
            "GET_CONFIGURATION", "RECONFIGURE", "OPEN", "START", "CLOSE", "SUSPEND", "ABORT",
            "SECURITY_CONTROL", "GET_ALL_CAPABILITIES", "DELAYREPORT"};
        return s < sizeof(names) / sizeof(names[0]) ? names[s] : "?";
    }

    void on_avdtp_signaling(uint16_t handle, Dir dir, const uint8_t *p, size_t n) {
        if (n < 2) return;
        uint8_t label = p[0] >> 4, ptype = (p[0] >> 2) & 0x03, mtype = p[0] & 0x03;
        if (ptype != 0) { avdtp_fragmented_++; return; }
        uint8_t signal = p[1] & 0x3F;
        const uint8_t *d = p + 2;
        size_t len = n - 2;
        const char *who = dir == OUT ? "sent" : "received";
        auto key = std::make_tuple(handle, (int)dir, label);

        if (mtype == 0) { /* command */
            if (signal == 0x03 || signal == 0x05) {
                size_t caps = signal == 0x03 ? 2 : 1;   /* SET_CONFIGURATION: ACP+INT SEID; RECONFIGURE: ACP SEID */
                PendingConfig pc;
                pc.signal = signal;
                pc.cfg.desc = "no Media Codec capability";
                for (size_t off = caps; off + 2 <= len;) {
                    uint8_t cat = d[off], losc = d[off + 1];
                    if (off + 2 + losc > len) break;
                    if (cat == 0x07) pc.cfg = parse_media_codec(d + off + 2, losc);
                    off += 2 + losc;
                }
                printf("[%9.3f] ACL 0x%03X %s (%s): %s\n", rel(), handle, signal_name(signal), who, pc.cfg.desc.c_str());
                pending_cfg_[key] = pc;
            } else if (signal >= 0x06 && signal <= 0x0A) {
                printf("[%9.3f] ACL 0x%03X %s (%s)\n", rel(), handle, signal_name(signal), who);
            }
            return;
        }

        /* response: match the command sent in the other direction */
        auto ckey = std::make_tuple(handle, (int)(dir == OUT ? IN : OUT), label);
        auto it = pending_cfg_.find(ckey);
        if ((signal == 0x03 || signal == 0x05) && it != pending_cfg_.end() && it->second.signal == signal) {
            if (mtype == 2) {
                printf("[%9.3f] ACL 0x%03X %s accepted\n", rel(), handle, signal_name(signal));
                start_session(handle, it->second.cfg);
            } else {
                printf("[%9.3f] ACL 0x%03X %s REJECTED (error 0x%02X)\n", rel(), handle, signal_name(signal),
                       len >= 2 ? d[len - 1] : 0);
            }
            pending_cfg_.erase(it);
        } else if (signal >= 0x06 && signal <= 0x0A) {
            printf("[%9.3f] ACL 0x%03X %s %s\n", rel(), handle, signal_name(signal),
                   mtype == 2 ? "accepted" : "rejected");
        }
    }

    void start_session(uint16_t handle, const CodecConfig &cfg) {
        finish_session(handle);
        int index = (int)sessions_.size() + 1;
        std::string wav;
        if (write_wav_ && cfg.codec != Codec::Unknown && cfg.codec != Codec::LDAC)
            wav = strf("%s.%d.%s.wav", prefix_.c_str(), index, codec_file_tag(cfg.codec));
        sessions_.push_back(std::make_unique<Session>(index, handle, now_, cfg, wav));
        active_[handle] = sessions_.back().get();
    }

    void finish_session(uint16_t handle) {
        auto it = active_.find(handle);
        if (it == active_.end()) return;
        it->second->finish();
        active_.erase(it);
    }

    std::string prefix_;
    bool write_wav_;
    Dir media_dir_;   /* OUT: media sent by the capturing host (A2DPWB); IN: received (sink) */
    bool have_t0_ = false;
    double t0_ = 0, now_ = 0;
    std::map<uint16_t, HandleState> handles_;
    std::map<std::tuple<uint16_t, int, uint8_t>, PendingConn> pending_conn_;
    std::map<std::tuple<uint16_t, int, uint8_t>, PendingConfig> pending_cfg_;
    std::vector<std::unique_ptr<Session>> sessions_;
    std::map<uint16_t, Session *> active_;
    uint64_t avdtp_fragmented_ = 0;
};

/* ------------------------------------------------------------------------ */

static void usage() {
    fprintf(stderr,
        "Usage: a2dpwb_decode <capture.pklg> [-o <output prefix>] [--no-wav] [--received]\n"
        "\n"
        "Checks and decodes the A2DP media stream recorded in an A2DPWB debug-mode\n"
        "HCI capture. Decoded audio is written to <prefix>.<n>.<codec>.wav\n"
        "(default prefix: the capture path without its extension).\n"
        "--received analyses the media the capturing host received instead of sent\n"
        "(e.g. the capture of the tools/emu test sink).\n");
}

int main(int argc, char **argv) {
    std::string input, prefix;
    bool write_wav = true, received = false;
    for (int i = 1; i < argc; i++) {
        std::string a = argv[i];
        if (a == "-o" && i + 1 < argc) prefix = argv[++i];
        else if (a == "--no-wav") write_wav = false;
        else if (a == "--received") received = true;
        else if (a == "-h" || a == "--help") { usage(); return 2; }
        else if (input.empty() && a[0] != '-') input = a;
        else { usage(); return 2; }
    }
    if (input.empty()) { usage(); return 2; }
    if (prefix.empty()) {
        prefix = input;
        size_t dot = prefix.find_last_of('.');
        size_t sep = prefix.find_last_of("/\\");
        if (dot != std::string::npos && (sep == std::string::npos || dot > sep)) prefix.erase(dot);
    }

    FILE *f = fopen(input.c_str(), "rb");
    if (!f) { fprintf(stderr, "Cannot open %s\n", input.c_str()); return 2; }

    printf("a2dpwb_decode: %s\n\n", input.c_str());
    Analyzer an(prefix, write_wav, received);
    std::vector<uint8_t> rec;
    uint64_t records = 0;
    for (;;) {
        /* PacketLogger record: len(4 BE) = 9 + payload, sec(4 BE), usec(4 BE), type(1), payload */
        uint8_t hdr[13];
        size_t got = fread(hdr, 1, sizeof(hdr), f);
        if (got == 0) break;
        if (got < sizeof(hdr)) { fprintf(stderr, "warning: truncated record at end of file\n"); break; }
        uint32_t len = be32(hdr);
        if (len < 9 || len > (1u << 20)) {
            fprintf(stderr, "error: invalid record length %u after %llu records - not a PacketLogger file?\n",
                    len, (unsigned long long)records);
            fclose(f);
            return 2;
        }
        rec.resize(len - 9);
        if (!rec.empty() && fread(rec.data(), 1, rec.size(), f) != rec.size()) {
            fprintf(stderr, "warning: truncated record at end of file\n");
            break;
        }
        double t = be32(hdr + 4) + be32(hdr + 8) / 1e6;
        an.on_record(t, hdr[12], rec.data(), rec.size());
        records++;
    }
    fclose(f);
    return an.finish();
}
