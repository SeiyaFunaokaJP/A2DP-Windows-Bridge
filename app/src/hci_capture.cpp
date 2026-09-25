/*
 * HCI capture - on-demand HCI packet log (PacketLogger .pklg) for debug mode
 * SPDX-License-Identifier: MIT
 */

#include "hci_capture.h"

#include <windows.h>

#include <atomic>
#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <iterator>
#include <map>
#include <mutex>
#include <tuple>
#include <vector>

extern "C" {
#include "bluetooth.h"
#include "btstack_defines.h"
}

namespace hci_capture {
namespace {

const uint16_t PSM_AVDTP = 0x0019;
const uint8_t AVDTP_SIGNAL_DELAYREPORT = 0x0D;
/* Upper bound on remembered setup packets (a few dozen per connection) */
const size_t MAX_SETUP_RECORDS = 1000;

struct Record {
    uint16_t handle;           /* ACL handle the packet belongs to */
    std::vector<uint8_t> data; /* complete PacketLogger record */
};

struct AvdtpChannel { uint16_t local_cid, remote_cid; };

std::mutex g_mutex;
HANDLE g_file = INVALID_HANDLE_VALUE;
std::string g_path;
std::atomic<bool> g_active{false};
std::atomic<uint64_t> g_bytes{0};

/* Connection setup tracking (BTstack thread, under g_mutex) */
std::vector<Record> g_setup;
std::map<uint16_t, std::vector<AvdtpChannel>> g_avdtp; /* [0] = signaling */
std::map<std::tuple<uint16_t, int, uint8_t>, std::pair<uint16_t, uint16_t>> g_pending_conn;

uint16_t le16(const uint8_t *p) { return (uint16_t)(p[0] | (p[1] << 8)); }

uint8_t packetlogger_type(uint8_t packet_type, uint8_t in) {
    switch (packet_type) {
    case HCI_COMMAND_DATA_PACKET: return 0x00;
    case HCI_EVENT_PACKET:        return 0x01;
    case HCI_ACL_DATA_PACKET:     return in ? 0x03 : 0x02;
    case HCI_SCO_DATA_PACKET:     return in ? 0x09 : 0x08;
    case HCI_ISO_DATA_PACKET:     return in ? 0x0d : 0x0c;
    case LOG_MESSAGE_PACKET:      return 0xfc;
    default:                      return 0xff;
    }
}

/* PacketLogger record: len(4 BE) = 9 + payload, sec(4 BE), usec(4 BE), type(1), payload */
std::vector<uint8_t> make_record(uint8_t pl_type, const uint8_t *packet, uint16_t len) {
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    uint64_t t100ns = ((uint64_t)ft.dwHighDateTime << 32) | ft.dwLowDateTime;
    uint64_t us = t100ns / 10 - 11644473600ULL * 1000000ULL; /* since 1970 */
    uint32_t sec = (uint32_t)(us / 1000000), usec = (uint32_t)(us % 1000000);
    uint32_t rec_len = 9u + len;

    std::vector<uint8_t> r(13 + len);
    const uint32_t words[3] = {rec_len, sec, usec};
    for (int w = 0; w < 3; w++)
        for (int b = 0; b < 4; b++)
            r[w * 4 + b] = (uint8_t)(words[w] >> (24 - 8 * b));
    r[12] = pl_type;
    if (len) memcpy(r.data() + 13, packet, len);
    return r;
}

void write_locked(const std::vector<uint8_t> &rec) {
    if (g_file == INVALID_HANDLE_VALUE) return;
    DWORD written = 0;
    WriteFile(g_file, rec.data(), (DWORD)rec.size(), &written, nullptr);
    g_bytes.fetch_add(written);
}

void write_note_locked(const std::string &text) {
    write_locked(make_record(0xfc, (const uint8_t *)text.data(), (uint16_t)text.size()));
}

void forget_handle_locked(uint16_t handle) {
    g_avdtp.erase(handle);
    for (auto it = g_pending_conn.begin(); it != g_pending_conn.end();)
        it = std::get<0>(it->first) == handle ? g_pending_conn.erase(it) : std::next(it);
    std::vector<Record> kept;
    for (auto &r : g_setup)
        if (r.handle != handle) kept.push_back(std::move(r));
    g_setup.swap(kept);
}

/* Returns true if this packet is part of connection setup worth remembering */
bool track_locked(uint8_t packet_type, uint8_t in, const uint8_t *p, uint16_t len, uint16_t *handle_out) {
    if (packet_type == HCI_EVENT_PACKET) {
        /* Disconnection Complete: status, handle, reason */
        if (len >= 6 && p[0] == 0x05 && p[2] == 0x00)
            forget_handle_locked(le16(p + 3) & 0x0FFF);
        return false;
    }
    if (packet_type != HCI_ACL_DATA_PACKET || len < 8) return false;

    uint16_t handle = le16(p) & 0x0FFF;
    uint8_t pb = (p[1] >> 4) & 0x03;
    uint16_t acl_len = le16(p + 2);
    if (pb == 0x01 || 4u + acl_len > len) return false;    /* continuation fragment */
    uint16_t l2_len = le16(p + 4), cid = le16(p + 6);
    if (8u + l2_len > len) return false;                    /* fragmented L2CAP frame */
    const uint8_t *d = p + 8;
    *handle_out = handle;
    int dir = in ? 1 : 0;

    if (cid == 0x0001) {
        for (size_t off = 0; off + 4 <= l2_len;) {
            uint8_t code = d[off], id = d[off + 1];
            uint16_t clen = le16(d + off + 2);
            const uint8_t *c = d + off + 4;
            if (off + 4 + clen > l2_len) break;
            if (code == 0x02 && clen >= 4) {          /* Connection Request: PSM, SCID */
                g_pending_conn[std::make_tuple(handle, dir, id)] = {le16(c), le16(c + 2)};
            } else if (code == 0x03 && clen >= 8) {   /* Connection Response: DCID, SCID, result */
                auto it = g_pending_conn.find(std::make_tuple(handle, 1 - dir, id));
                uint16_t result = le16(c + 4);
                if (it != g_pending_conn.end() && result != 1 /* pending */) {
                    if (result == 0 && it->second.first == PSM_AVDTP) {
                        bool requested_by_us = std::get<1>(it->first) == 0;
                        uint16_t dcid = le16(c), scid = le16(c + 2);
                        g_avdtp[handle].push_back({requested_by_us ? scid : dcid,
                                                   requested_by_us ? dcid : scid});
                    }
                    g_pending_conn.erase(it);
                }
            } else if (code == 0x06 && clen >= 4) {   /* Disconnection Request: DCID, SCID */
                uint16_t local = in ? le16(c) : le16(c + 2);
                auto &chans = g_avdtp[handle];
                for (size_t i = 0; i < chans.size(); i++)
                    if (chans[i].local_cid == local) { chans.erase(chans.begin() + i); break; }
            }
            off += 4 + clen;
        }
        return true;
    }

    auto it = g_avdtp.find(handle);
    if (it == g_avdtp.end() || it->second.empty()) return false;
    const AvdtpChannel &sig = it->second[0];
    if (cid != (in ? sig.local_cid : sig.remote_cid)) return false;
    /* AVDTP signaling; skip the periodic DELAYREPORT */
    if (l2_len >= 2 && ((d[0] >> 2) & 0x03) == 0 && (d[1] & 0x3F) == AVDTP_SIGNAL_DELAYREPORT)
        return false;
    return true;
}

void dump_reset(void) {}

void dump_log_packet(uint8_t packet_type, uint8_t in, uint8_t *packet, uint16_t len) {
    uint8_t pl_type = packetlogger_type(packet_type, in);
    if (pl_type == 0xff) return;
    std::lock_guard<std::mutex> lock(g_mutex);
    uint16_t handle = 0;
    bool setup = track_locked(packet_type, in, packet, len, &handle);
    if (!setup && g_file == INVALID_HANDLE_VALUE) return;
    std::vector<uint8_t> rec = make_record(pl_type, packet, len);
    write_locked(rec);
    if (setup && g_setup.size() < MAX_SETUP_RECORDS)
        g_setup.push_back({handle, std::move(rec)});
}

void dump_log_message(int log_level, const char *format, va_list argptr) {
    (void)log_level;
    if (!g_active.load()) return;
    char buf[256];
    int n = vsnprintf(buf, sizeof(buf), format, argptr);
    if (n < 0) return;
    if (n >= (int)sizeof(buf)) n = (int)sizeof(buf) - 1;
    std::lock_guard<std::mutex> lock(g_mutex);
    write_locked(make_record(0xfc, (const uint8_t *)buf, (uint16_t)n));
}

} // namespace

const hci_dump_t *instance() {
    static const hci_dump_t dump = {&dump_reset, &dump_log_packet, &dump_log_message};
    return &dump;
}

void reset_tracking() {
    std::lock_guard<std::mutex> lock(g_mutex);
    g_setup.clear();
    g_avdtp.clear();
    g_pending_conn.clear();
}

bool start(const std::string &path) {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (g_file != INVALID_HANDLE_VALUE) return false;
    HANDLE f = CreateFileA(path.c_str(), GENERIC_WRITE, FILE_SHARE_READ, nullptr,
                           CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (f == INVALID_HANDLE_VALUE) {
        fprintf(stderr, "HCI capture: cannot create %s (error %lu)\n", path.c_str(), GetLastError());
        return false;
    }
    g_file = f;
    g_path = path;
    g_bytes.store(0);
    if (!g_setup.empty()) {
        write_note_locked("A2DPWB: capture started during a connection; " +
                          std::to_string(g_setup.size()) +
                          " connection setup packets recorded earlier follow");
        for (const auto &r : g_setup) write_locked(r.data);
    }
    write_note_locked("A2DPWB: live capture starts");
    g_active.store(true);
    fprintf(stderr, "HCI capture: started -> %s\n", path.c_str());
    return true;
}

void stop() {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (g_file == INVALID_HANDLE_VALUE) return;
    write_note_locked("A2DPWB: capture stopped");
    CloseHandle(g_file);
    g_file = INVALID_HANDLE_VALUE;
    g_active.store(false);
    fprintf(stderr, "HCI capture: stopped (%llu bytes) -> %s\n",
            (unsigned long long)g_bytes.load(), g_path.c_str());
}

bool active() { return g_active.load(); }

std::string path() {
    std::lock_guard<std::mutex> lock(g_mutex);
    return g_path;
}

uint64_t bytes_written() { return g_bytes.load(); }

} // namespace hci_capture
