/*
 * bluecaster_decode --le-audio: checker / decoder for the LE Audio (LC3 over
 * CIS) stream in PacketLogger captures of BlueCaster or the tools/emu LE sink.
 *
 * From the capture it follows:
 *   - the ASE Control Point writes (ATT Write Request / Command with a
 *     well-formed Config Codec or Config QoS operation, ASCS 5.1 / 5.2): the
 *     LC3 configuration and the CIG / CIS of each ASE, per ACL link
 *   - LE Set CIG Parameters + its Command Complete and LE Create CIS (sending
 *     side), or LE CIS Request (receiving side): CIS handle -> CIG / CIS ID
 *     and ACL link
 *   - the HCI ISO data packets of the CIS (sent: host -> controller; with
 *     --received: controller -> host)
 * and checks each CIS: complete SDUs, consecutive sequence numbers, SDU
 * length = channels x octets per frame, packet status. Every LC3 frame is
 * decoded with liblc3; the channels are placed by their audio location
 * (Front Left -> left, Front Right -> right, mono -> both) on a timeline of
 * SDU sequence numbers and written to <prefix>.<n>.lc3.wav (stereo, 16-bit).
 * Several captures (one per sink of a group) are read together; each CIS
 * line of the report names its capture (1, 2, ... in the order given).
 *
 * Like the A2DP part it does not link BTstack, only liblc3 (Apache-2.0).
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

#include <algorithm>
#include <cstdarg>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <map>
#include <memory>
#include <set>
#include <string>
#include <tuple>
#include <vector>

extern "C" {
#include "lc3.h"
}

namespace {

uint16_t le16(const uint8_t *p) { return (uint16_t)(p[0] | (p[1] << 8)); }
uint32_t le24(const uint8_t *p) { return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16); }
uint32_t le32(const uint8_t *p) { return le24(p) | ((uint32_t)p[3] << 24); }
uint32_t be32(const uint8_t *p) {
    return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) | ((uint32_t)p[2] << 8) | p[3];
}

std::string strf(const char *fmt, ...) {
    char buf[512];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    return buf;
}

uint32_t sf_code_to_hz(uint8_t c) {
    static const uint32_t t[] = {0, 8000, 11025, 16000, 22050, 24000, 32000, 44100, 48000, 88200, 96000};
    return c < sizeof(t) / sizeof(t[0]) ? t[c] : 0;
}

/* Keys: capture file index in the upper bits, so handles of several
 * captures (several controllers) do not collide */
uint32_t fkey(int file, uint16_t handle) { return ((uint32_t)file << 16) | handle; }

struct AseConfig {
    bool codec = false;
    uint32_t sample_rate = 0;
    uint16_t frame_us = 10000;
    uint32_t allocation = 0;
    bool has_allocation = false;
    uint16_t octets = 0;
    uint8_t blocks = 1;
    bool qos = false;
    uint8_t cig = 0, cis = 0;
    uint32_t sdu_interval = 0;
    uint16_t max_sdu = 0;
    int channels() const {
        int n = 0;
        for (uint32_t a = allocation; a; a >>= 1) n += (int)(a & 1u);
        return n ? n : 1;
    }
    std::string desc() const {
        return strf("LC3 %u Hz %s ms %u octets", sample_rate, frame_us == 7500 ? "7.5" : "10", octets);
    }
};

struct WavOut {
    std::string path;
    std::vector<int16_t> samples;   /* interleaved stereo */
    void write(uint32_t rate) const {
        FILE *f = fopen(path.c_str(), "wb");
        if (!f) { fprintf(stderr, "Cannot create %s\n", path.c_str()); return; }
        uint32_t data = (uint32_t)(samples.size() * 2);
        uint8_t h[44];
        auto p16 = [&](int o, uint32_t v) { h[o] = (uint8_t)v; h[o + 1] = (uint8_t)(v >> 8); };
        auto p32 = [&](int o, uint32_t v) { p16(o, v); p16(o + 2, v >> 16); };
        memcpy(h, "RIFF", 4); p32(4, 36 + data);
        memcpy(h + 8, "WAVEfmt ", 8); p32(16, 16); p16(20, 1); p16(22, 2);
        p32(24, rate); p32(28, rate * 4); p16(32, 4); p16(34, 16);
        memcpy(h + 36, "data", 4); p32(40, data);
        fwrite(h, 1, 44, f);
        fwrite(samples.data(), 2, samples.size(), f);
        fclose(f);
    }
};

/* One CIS of a stream */
struct CisStream {
    uint32_t key = 0;                /* fkey(file, CIS handle) */
    int file = 0;
    int session = 0;                 /* CIG set-ups seen before in this capture */
    uint64_t order = 0;              /* creation order */
    uint16_t handle = 0, acl = 0;
    uint8_t ase = 0;
    AseConfig cfg;
    uint64_t sdus = 0, frames = 0, gaps = 0, missing = 0, bad_len = 0, status_err = 0, fragments = 0,
             decode_err = 0, ts_bad = 0;
    int first_seq = -1, last_seq = -1;
    uint32_t last_ts = 0;
    bool have_ts = false;
    std::vector<std::vector<uint8_t>> decoder_mem;
    std::vector<lc3_decoder_t> decoders;
    /* decoded PCM per SDU sequence number (unwrapped), per channel */
    std::map<int64_t, std::vector<std::vector<int16_t>>> pcm;
    int64_t seq_base = 0;            /* unwrapping of the 16-bit sequence numbers */
};

class LeAnalyzer {
public:
    LeAnalyzer(const std::string &prefix, bool wav, bool received)
        : prefix_(prefix), wav_(wav), received_(received) {}

    void on_record(int file, double t, uint8_t type, const uint8_t *p, size_t n) {
        if (t0_ < 0) t0_ = t;
        t_ = t - t0_;
        switch (type) {
        case 0x00: on_command(file, p, n); break;
        case 0x01: on_event(file, p, n); break;
        case 0x02: on_acl(file, false, p, n); break;
        case 0x03: on_acl(file, true, p, n); break;
        case 0x0C: if (!received_) on_iso(file, p, n); else iso_other_++; break;
        case 0x0D: if (received_) on_iso(file, p, n); else iso_other_++; break;
        default: break;
        }
    }

    int finish();

private:
    void problem(const std::string &msg) { problems_[msg]++; }

    void on_command(int file, const uint8_t *p, size_t n) {
        if (n < 3) return;
        uint16_t op = le16(p);
        const uint8_t *c = p + 3;
        size_t len = p[2];
        if (op == 0x2062 && len >= 15) {         /* LE Set CIG Parameters */
            /* A new set-up: earlier streams of these handles are done */
            pending_cig_[file] = {c[0], {}};
            uint8_t count = c[14];
            for (uint8_t i = 0; i < count && 15u + 9u * (i + 1u) <= len; i++)
                pending_cig_[file].second.push_back(c[15 + 9 * i]);
        } else if (op == 0x2064 && len >= 1) {   /* LE Create CIS */
            uint8_t count = c[0];
            for (uint8_t i = 0; i < count && 1u + 4u * (i + 1u) <= len; i++) {
                uint16_t cis = le16(c + 1 + 4 * i) & 0x0FFF, acl = le16(c + 3 + 4 * i) & 0x0FFF;
                cis_acl_[fkey(file, cis)] = acl;
            }
        }
    }

    void on_event(int file, const uint8_t *p, size_t n) {
        if (n < 2) return;
        uint8_t code = p[0];
        const uint8_t *e = p + 2;
        size_t len = p[1];
        if (code == 0x0E && len >= 4 && le16(e + 1) == 0x2062) {   /* Command Complete: Set CIG */
            if (e[3] != 0 || len < 6) return;
            uint8_t cig = e[4], count = e[5];
            auto it = pending_cig_.find(file);
            if (it == pending_cig_.end() || it->second.first != cig) return;
            bool retired = false;
            for (uint8_t i = 0; i < count && i < it->second.second.size() && 6u + 2u * (i + 1u) <= len; i++) {
                uint32_t k = fkey(file, le16(e + 6 + 2 * i) & 0x0FFF);
                retired |= retire(k);
                cis_ids_[k] = {cig, it->second.second[i]};
            }
            if (retired) session_[file]++;
        } else if (code == 0x3E && len >= 7 && e[0] == 0x1A) {    /* LE CIS Request */
            uint16_t acl = le16(e + 1) & 0x0FFF, cis = le16(e + 3) & 0x0FFF;
            if (retire(fkey(file, cis))) session_[file]++;
            cis_acl_[fkey(file, cis)] = acl;
            cis_ids_[fkey(file, cis)] = {e[5], e[6]};
        } else if (code == 0x05 && len >= 4) {                    /* Disconnection Complete */
            uint16_t h = le16(e + 1) & 0x0FFF;
            acl_rx_.erase(fkey(file, h) | 0x80000000u);
            acl_rx_.erase(fkey(file, h));
        }
    }

    /* L2CAP reassembly per link and direction */
    void on_acl(int file, bool in, const uint8_t *p, size_t n) {
        if (n < 4) return;
        uint16_t handle = le16(p) & 0x0FFF;
        uint8_t pb = (p[1] >> 4) & 0x03;
        uint16_t len = le16(p + 2);
        if (4u + len > n) return;
        uint32_t k = fkey(file, handle) | (in ? 0x80000000u : 0);
        std::vector<uint8_t> &buf = acl_rx_[k];
        if (pb == 0x01) {
            if (buf.empty()) return;
            buf.insert(buf.end(), p + 4, p + 4 + len);
        } else {
            buf.assign(p + 4, p + 4 + len);
        }
        if (buf.size() < 4) return;
        uint16_t l2len = le16(buf.data());
        if (buf.size() < 4u + l2len) return;
        uint16_t cid = le16(buf.data() + 2);
        if (cid == 0x0004) on_att(file, handle, buf.data() + 4, l2len);
        buf.clear();
    }

    void on_att(int file, uint16_t acl, const uint8_t *a, size_t n) {
        if (n < 4 || (a[0] != 0x12 && a[0] != 0x52)) return;   /* Write Request / Command */
        const uint8_t *v = a + 3;
        size_t len = n - 3;
        if (len < 2) return;
        if (v[0] == 0x01) parse_config_codec(file, acl, v, len);
        else if (v[0] == 0x02) parse_config_qos(file, acl, v, len);
    }

    /* ASCS 5.1, only when the whole value is consistent */
    void parse_config_codec(int file, uint16_t acl, const uint8_t *v, size_t len) {
        uint8_t count = v[1];
        if (count == 0) return;
        std::vector<std::pair<uint8_t, AseConfig>> found;
        size_t pos = 2;
        for (uint8_t i = 0; i < count; i++) {
            if (pos + 9 > len) return;
            uint8_t ase = v[pos];
            const uint8_t *codec_id = v + pos + 3;
            uint8_t cfg_len = v[pos + 8];
            if (pos + 9 + cfg_len > len) return;
            if (codec_id[0] != 0x06) return;                    /* LC3 only */
            AseConfig c;
            const uint8_t *l = v + pos + 9;
            for (size_t o = 0; o < cfg_len;) {
                uint8_t ll = l[o];
                if (ll == 0 || o + 1 + ll > cfg_len) return;
                uint8_t type = l[o + 1];
                const uint8_t *val = l + o + 2;
                if (type == 0x01 && ll >= 2) c.sample_rate = sf_code_to_hz(val[0]);
                else if (type == 0x02 && ll >= 2) c.frame_us = val[0] == 0 ? 7500 : 10000;
                else if (type == 0x03 && ll >= 5) { c.allocation = le32(val); c.has_allocation = true; }
                else if (type == 0x04 && ll >= 3) c.octets = le16(val);
                else if (type == 0x05 && ll >= 2) c.blocks = val[0];
                o += 1u + ll;
            }
            if (!c.sample_rate || !c.octets) return;
            c.codec = true;
            found.push_back({ase, c});
            pos += 9u + cfg_len;
        }
        if (pos != len) return;
        for (auto &f : found) {
            AseConfig &dst = ase_[std::make_tuple(file, acl, f.first)];
            dst = f.second;
            printf("[%9.3f] ACL 0x%03X ASE %u Config Codec: %s, allocation 0x%x\n", t_, acl, f.first,
                   dst.desc().c_str(), dst.allocation);
        }
    }

    /* ASCS 5.2 */
    void parse_config_qos(int file, uint16_t acl, const uint8_t *v, size_t len) {
        uint8_t count = v[1];
        if (count == 0 || len != 2u + 16u * count) return;
        for (uint8_t i = 0; i < count; i++) {
            const uint8_t *q = v + 2 + 16 * i;
            auto it = ase_.find(std::make_tuple(file, acl, q[0]));
            if (it == ase_.end()) continue;
            it->second.qos = true;
            it->second.cig = q[1];
            it->second.cis = q[2];
            it->second.sdu_interval = le24(q + 3);
            it->second.max_sdu = le16(q + 8);
            printf("[%9.3f] ACL 0x%03X ASE %u Config QoS: CIG %u CIS %u, SDU interval %u us, max SDU %u, "
                   "RTN %u, latency %u ms, presentation delay %u us\n", t_, acl, q[0], q[1], q[2],
                   it->second.sdu_interval, it->second.max_sdu, q[10], le16(q + 11), le24(q + 13));
        }
    }

    /* The stream on handle k ended (the handle is set up anew) */
    bool retire(uint32_t k) {
        auto it = streams_.find(k);
        if (it == streams_.end()) return false;
        done_.push_back(std::move(it->second));
        streams_.erase(it);
        return true;
    }

    CisStream *stream_for(int file, uint16_t handle) {
        uint32_t k = fkey(file, handle);
        auto it = streams_.find(k);
        if (it != streams_.end()) return it->second.get();
        auto ids = cis_ids_.find(k);
        auto acl = cis_acl_.find(k);
        if (ids == cis_ids_.end() || acl == cis_acl_.end()) return nullptr;
        for (auto &a : ase_) {
            if (std::get<0>(a.first) != file || std::get<1>(a.first) != acl->second) continue;
            const AseConfig &c = a.second;
            if (!c.codec || !c.qos || c.cig != ids->second.first || c.cis != ids->second.second) continue;
            auto s = std::make_unique<CisStream>();
            s->key = k;
            s->file = file;
            s->session = session_[file];
            s->order = next_order_++;
            s->handle = handle;
            s->acl = acl->second;
            s->ase = std::get<2>(a.first);
            s->cfg = c;
            for (int ch = 0; ch < c.channels(); ch++) {
                s->decoder_mem.emplace_back(lc3_decoder_size(c.frame_us, (int)c.sample_rate));
                s->decoders.push_back(lc3_setup_decoder(c.frame_us, (int)c.sample_rate, 0,
                                                        s->decoder_mem.back().data()));
            }
            printf("[%9.3f] CIS 0x%03X (CIG %u CIS %u) on ACL 0x%03X: ASE %u, %s, %d channel(s)\n", t_, handle,
                   c.cig, c.cis, s->acl, s->ase, c.desc().c_str(), c.channels());
            CisStream *raw = s.get();
            streams_[k] = std::move(s);
            return raw;
        }
        return nullptr;
    }

    void on_iso(int file, const uint8_t *p, size_t n) {
        if (n < 4) return;
        uint16_t handle = le16(p) & 0x0FFF;
        uint8_t pb = (p[1] >> 4) & 0x03;
        bool ts = (p[1] & 0x40) != 0;
        uint16_t len = le16(p + 2) & 0x3FFF;
        if (4u + len > n) { problem("truncated ISO packet"); return; }
        CisStream *s = stream_for(file, handle);
        if (!s) { problem("ISO data on a CIS without a known ASE configuration"); return; }
        if (pb != 0x02) {   /* only complete SDUs are expected (BlueCaster sends them whole) */
            s->fragments++;
            return;
        }
        const uint8_t *d = p + 4;
        size_t dl = len;
        uint32_t timestamp = 0;
        if (ts) {
            if (dl < 4) return;
            timestamp = le32(d);
            d += 4; dl -= 4;
        }
        if (dl < 4) { problem("ISO packet without SDU header"); return; }
        uint16_t seq = le16(d);
        uint16_t sdu_len = le16(d + 2) & 0x0FFF;
        uint8_t status = (uint8_t)(d[3] >> 6);
        d += 4; dl -= 4;
        if (status) s->status_err++;
        if (sdu_len != dl) { s->bad_len++; return; }

        /* Sequence numbers: consecutive, 16-bit wrap */
        if (s->last_seq >= 0) {
            uint16_t expect = (uint16_t)(s->last_seq + 1);
            if (seq != expect) {
                s->gaps++;
                s->missing += (uint16_t)(seq - expect);
            }
            if (seq < (uint16_t)s->last_seq && (uint16_t)s->last_seq - seq > 0x8000) s->seq_base += 0x10000;
        } else {
            s->first_seq = seq;
        }
        if (ts && s->have_ts && s->cfg.sdu_interval) {
            uint32_t d_ts = timestamp - s->last_ts;
            uint32_t d_seq = (uint16_t)(seq - s->last_seq);
            if (d_ts != d_seq * s->cfg.sdu_interval) s->ts_bad++;
        }
        s->last_ts = timestamp;
        s->have_ts = ts;
        s->last_seq = seq;
        s->sdus++;

        int channels = s->cfg.channels();
        size_t expected = (size_t)channels * s->cfg.octets * s->cfg.blocks;
        if (dl != expected) { s->bad_len++; return; }
        int samples = lc3_frame_samples(s->cfg.frame_us, (int)s->cfg.sample_rate);
        std::vector<std::vector<int16_t>> pcm(channels, std::vector<int16_t>(samples * s->cfg.blocks));
        for (int b = 0; b < s->cfg.blocks; b++)
            for (int ch = 0; ch < channels; ch++) {
                const uint8_t *frame = d + ((size_t)b * channels + ch) * s->cfg.octets;
                int r = lc3_decode(s->decoders[ch], frame, s->cfg.octets, LC3_PCM_FORMAT_S16,
                                   pcm[ch].data() + (size_t)b * samples, 1);
                if (r != 0) s->decode_err++;
                s->frames++;
            }
        if (wav_) s->pcm[s->seq_base + seq] = std::move(pcm);
    }

    std::string prefix_;
    bool wav_, received_;
    double t0_ = -1, t_ = 0;
    uint64_t iso_other_ = 0;
    std::map<int, std::pair<uint8_t, std::vector<uint8_t>>> pending_cig_;
    std::map<uint32_t, std::pair<uint8_t, uint8_t>> cis_ids_;   /* fkey(CIS) -> (CIG, CIS ID) */
    std::map<uint32_t, uint16_t> cis_acl_;                       /* fkey(CIS) -> ACL handle */
    std::map<std::tuple<int, uint16_t, uint8_t>, AseConfig> ase_;
    std::map<uint32_t, std::vector<uint8_t>> acl_rx_;
    std::map<uint32_t, std::unique_ptr<CisStream>> streams_;   /* current, by handle */
    std::vector<std::unique_ptr<CisStream>> done_;              /* ended (handle set up again) */
    std::map<int, int> session_;
    uint64_t next_order_ = 0;
    std::map<std::string, uint64_t> problems_;
};

int LeAnalyzer::finish() {
    for (auto &e : streams_) done_.push_back(std::move(e.second));
    streams_.clear();
    std::sort(done_.begin(), done_.end(),
              [](const std::unique_ptr<CisStream> &a, const std::unique_ptr<CisStream> &b) { return a->order < b->order; });
    if (done_.empty()) {
        printf("\nNo LE Audio ISO data with a known ASE configuration found (%s side).\n",
               received_ ? "received" : "sent");
        printf("\nRESULT: PROBLEMS FOUND (see above)\n");
        return 1;
    }
    /* One stream per set-up (session) and LC3 configuration, in the order
     * they started */
    std::vector<std::pair<std::string, std::vector<CisStream *>>> groups;
    for (auto &e : done_) {
        std::string key = strf("%d %s CIG %u", e->session, e->cfg.desc().c_str(), e->cfg.cig);
        auto g = std::find_if(groups.begin(), groups.end(), [&](const auto &x) { return x.first == key; });
        if (g == groups.end()) groups.push_back({key, {e.get()}});
        else g->second.push_back(e.get());
    }

    int index = 0;
    bool any_problem = !problems_.empty();
    for (auto &g : groups) {
        index++;
        const AseConfig &cfg = g.second[0]->cfg;
        int total_channels = 0;
        uint64_t sdus = 0, frames = 0;
        for (CisStream *s : g.second) {
            total_channels += s->cfg.channels();
            sdus += s->sdus;
            frames += s->frames;
        }
        printf("\n=== Stream %d (LE Audio, CIG %u) ===\n", index, cfg.cig);
        printf("  Configuration : %s, %d channel(s) over %u CIS\n", cfg.desc().c_str(), total_channels,
               (unsigned)g.second.size());
        std::vector<std::string> issues;
        int64_t lo = INT64_MAX, hi = INT64_MIN;
        for (CisStream *s : g.second) {
            std::string locs;
            for (int b = 0; b < 32; b++)
                if (s->cfg.allocation & (1u << b)) locs += strf("%s0x%x", locs.empty() ? "" : ",", 1u << b);
            /* capture: 1-based index of the input file (which sink, --received) */
            printf("  CIS 0x%03X     : ASE %u on ACL 0x%03X, location(s) %s, %llu SDUs, sequence %d-%d, "
                   "capture %d\n",
                   s->handle, s->ase, s->acl, locs.empty() ? "mono" : locs.c_str(),
                   (unsigned long long)s->sdus, s->first_seq, s->last_seq, s->file + 1);
            if (s->gaps) issues.push_back(strf("CIS 0x%03X: %llu sequence gap(s), %llu SDU(s) missing", s->handle,
                                               (unsigned long long)s->gaps, (unsigned long long)s->missing));
            if (s->bad_len) issues.push_back(strf("CIS 0x%03X: %llu SDU(s) with a wrong length", s->handle,
                                                  (unsigned long long)s->bad_len));
            if (s->status_err) issues.push_back(strf("CIS 0x%03X: %llu SDU(s) with packet status errors",
                                                     s->handle, (unsigned long long)s->status_err));
            if (s->fragments) issues.push_back(strf("CIS 0x%03X: %llu fragmented ISO packet(s) (not decoded)",
                                                    s->handle, (unsigned long long)s->fragments));
            if (s->decode_err) issues.push_back(strf("CIS 0x%03X: %llu LC3 decoding error(s)", s->handle,
                                                     (unsigned long long)s->decode_err));
            if (s->ts_bad) issues.push_back(strf("CIS 0x%03X: %llu time stamp step(s) differ from the SDU interval",
                                                 s->handle, (unsigned long long)s->ts_bad));
            if (!s->pcm.empty()) {
                lo = std::min<int64_t>(lo, s->pcm.begin()->first);
                hi = std::max<int64_t>(hi, s->pcm.rbegin()->first);
            }
        }
        double audio_s = g.second.empty() || !cfg.sdu_interval ? 0.0
                       : (double)g.second[0]->sdus * cfg.frame_us * cfg.blocks / 1e6;
        printf("  Media packets : %llu (ISO SDUs)\n", (unsigned long long)sdus);
        printf("  Frames        : %llu (LC3 frames, all channels)\n", (unsigned long long)frames);
        printf("  Audio         : %.3f s of audio at %u Hz\n", audio_s, cfg.sample_rate);
        if (wav_ && lo <= hi) {
            int samples = lc3_frame_samples(cfg.frame_us, (int)cfg.sample_rate) * cfg.blocks;
            WavOut w;
            w.path = strf("%s.%d.lc3.wav", prefix_.c_str(), index);
            w.samples.assign((size_t)(hi - lo + 1) * samples * 2, 0);
            bool has_left = false, has_right = false;
            for (CisStream *s : g.second) {
                has_left |= (s->cfg.allocation & 1u) != 0;
                has_right |= (s->cfg.allocation & 2u) != 0;
            }
            for (CisStream *s : g.second) {
                /* channel order in the SDU: ascending location bit */
                std::vector<int> side;
                for (int b = 0; b < 32; b++)
                    if (s->cfg.allocation & (1u << b))
                        side.push_back(b == 0 ? 0 : b == 1 ? 1 : -1);
                if (side.empty()) side.push_back(-1);
                for (auto &e : s->pcm) {
                    size_t base = (size_t)(e.first - lo) * samples * 2;
                    for (size_t ch = 0; ch < e.second.size() && ch < side.size(); ch++) {
                        int sd = side[ch];
                        /* mono (or one side only): both channels */
                        bool both = sd < 0 || !(has_left && has_right);
                        for (int i = 0; i < samples; i++) {
                            int16_t v = e.second[ch][i];
                            if (both || sd == 0) w.samples[base + 2 * i] = v;
                            if (both || sd == 1) w.samples[base + 2 * i + 1] = v;
                        }
                    }
                }
            }
            w.write(cfg.sample_rate);
            printf("  WAV           : %s\n", w.path.c_str());
        }
        if (issues.empty()) {
            printf("  Result        : OK, no problems found\n");
        } else {
            any_problem = true;
            printf("  Result        : %zu kind(s) of problem found\n", issues.size());
            for (const auto &s : issues) printf("    - %s\n", s.c_str());
        }
    }
    if (!problems_.empty()) {
        printf("\nOther problems:\n");
        for (auto &p : problems_) printf("    - %s: %llu time(s)\n", p.first.c_str(), (unsigned long long)p.second);
    }
    if (iso_other_)
        printf("\nnote: %llu ISO packets in the other direction were not analysed\n",
               (unsigned long long)iso_other_);
    printf("\nRESULT: %s\n", any_problem ? "PROBLEMS FOUND (see above)" : "OK");
    return any_problem ? 1 : 0;
}

} /* namespace */

/* Entry from a2dp_decode.cpp main() for --le-audio */
int le_audio_decode_main(const std::vector<std::string> &inputs, const std::string &prefix, bool write_wav,
                         bool received) {
    LeAnalyzer an(prefix, write_wav, received);
    for (size_t file = 0; file < inputs.size(); file++) {
        FILE *f = fopen(inputs[file].c_str(), "rb");
        if (!f) { fprintf(stderr, "Cannot open %s\n", inputs[file].c_str()); return 2; }
        printf("bluecaster_decode --le-audio: %s\n", inputs[file].c_str());
        std::vector<uint8_t> rec;
        for (;;) {
            uint8_t hdr[13];
            size_t got = fread(hdr, 1, sizeof(hdr), f);
            if (got < sizeof(hdr)) break;
            uint32_t len = be32(hdr);
            if (len < 9 || len > (1u << 20)) {
                fprintf(stderr, "error: invalid record length %u - not a PacketLogger file?\n", len);
                fclose(f);
                return 2;
            }
            rec.resize(len - 9);
            if (!rec.empty() && fread(rec.data(), 1, rec.size(), f) != rec.size()) break;
            double t = be32(hdr + 4) + be32(hdr + 8) / 1e6;
            an.on_record((int)file, t, hdr[12], rec.data(), rec.size());
        }
        fclose(f);
    }
    printf("\n");
    return an.finish();
}
