/*
 * Remote sink client - Implementation
 * SPDX-License-Identifier: MIT
 */

#include "remote_sink_client.h"

#include <winsock2.h>
#include <ws2tcpip.h>
#include <iphlpapi.h>
#include <windows.h>

#include <nlohmann/json.hpp>

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>

using json = nlohmann::json;

namespace {

const auto RECONNECT_DELAY = std::chrono::seconds(3);
const DWORD CONNECT_TIMEOUT_MS = 3000;
/* The receiver sends once per second; silence this long means it is gone */
const DWORD RECEIVE_TIMEOUT_MS = 5000;
const size_t MAX_LINE = 1 << 20;

/* "host", "host:port", "[v6]:port", "auto[:port]" (host = "auto"); false
 * on a bad port */
bool split_target(const std::string &target, std::string *host, uint16_t *port) {
    *port = REMOTE_SINK_DEFAULT_PORT;
    std::string t = target.empty() ? std::string(REMOTE_SINK_AUTO) : target;
    size_t colon = std::string::npos;
    if (!t.empty() && t[0] == '[') {
        size_t close = t.find(']');
        if (close == std::string::npos) return false;
        *host = t.substr(1, close - 1);
        if (close + 1 < t.size()) {
            if (t[close + 1] != ':') return false;
            colon = close + 1;
        }
    } else {
        colon = t.find(':');
        if (colon != std::string::npos && t.find(':', colon + 1) != std::string::npos)
            colon = std::string::npos;  /* bare IPv6 address */
        *host = t.substr(0, colon);
    }
    if (colon != std::string::npos) {
        char *end = nullptr;
        unsigned long p = strtoul(t.c_str() + colon + 1, &end, 10);
        if (!end || *end != '\0' || p == 0 || p > 65535) return false;
        *port = static_cast<uint16_t>(p);
    }
    return !host->empty();
}

/* Connects with a timeout; INVALID_SOCKET and *error on failure */
SOCKET connect_to(const std::string &host, uint16_t port, std::string *error) {
    addrinfo hints = {};
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_protocol = IPPROTO_TCP;
    addrinfo *res = nullptr;
    char port_str[8];
    snprintf(port_str, sizeof(port_str), "%u", port);
    if (getaddrinfo(host.c_str(), port_str, &hints, &res) != 0 || !res) {
        *error = "cannot resolve " + host;
        return INVALID_SOCKET;
    }
    SOCKET s = INVALID_SOCKET;
    *error = "cannot connect";
    for (addrinfo *ai = res; ai; ai = ai->ai_next) {
        s = socket(ai->ai_family, ai->ai_socktype, ai->ai_protocol);
        if (s == INVALID_SOCKET) continue;
        u_long nonblocking = 1;
        ioctlsocket(s, FIONBIO, &nonblocking);
        int rc = connect(s, ai->ai_addr, static_cast<int>(ai->ai_addrlen));
        bool ok = rc == 0;
        if (!ok && WSAGetLastError() == WSAEWOULDBLOCK) {
            fd_set wr, ex;
            FD_ZERO(&wr);
            FD_ZERO(&ex);
            FD_SET(s, &wr);
            FD_SET(s, &ex);
            timeval tv = {static_cast<long>(CONNECT_TIMEOUT_MS / 1000),
                          static_cast<long>((CONNECT_TIMEOUT_MS % 1000) * 1000)};
            ok = select(0, nullptr, &wr, &ex, &tv) == 1 && FD_ISSET(s, &wr);
            if (!ok) *error = FD_ISSET(s, &ex) ? "connection refused" : "connection timed out";
        }
        if (ok) {
            nonblocking = 0;
            ioctlsocket(s, FIONBIO, &nonblocking);
            DWORD timeout = RECEIVE_TIMEOUT_MS;
            setsockopt(s, SOL_SOCKET, SO_RCVTIMEO, reinterpret_cast<const char *>(&timeout), sizeof(timeout));
            break;
        }
        closesocket(s);
        s = INVALID_SOCKET;
    }
    freeaddrinfo(res);
    return s;
}

template <typename T>
T num(const json &j, const char *key, T fallback = T()) {
    auto it = j.find(key);
    if (it == j.end() || !it->is_number()) return fallback;
    return it->get<T>();
}

std::string str(const json &j, const char *key) {
    auto it = j.find(key);
    return (it != j.end() && it->is_string()) ? it->get<std::string>() : std::string();
}

/* Broadcast address of every IPv4 network this PC is on, plus the limited
 * broadcast (which Windows sends on one interface only) */
std::vector<in_addr> broadcast_addresses() {
    std::vector<in_addr> out;
    in_addr all;
    all.s_addr = INADDR_BROADCAST;
    out.push_back(all);
    ULONG size = 16 * 1024;
    std::vector<unsigned char> buf(size);
    ULONG flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    ULONG rc = GetAdaptersAddresses(AF_INET, flags, nullptr,
                                    reinterpret_cast<IP_ADAPTER_ADDRESSES *>(buf.data()), &size);
    if (rc == ERROR_BUFFER_OVERFLOW) {
        buf.resize(size);
        rc = GetAdaptersAddresses(AF_INET, flags, nullptr,
                                  reinterpret_cast<IP_ADAPTER_ADDRESSES *>(buf.data()), &size);
    }
    if (rc != NO_ERROR) return out;
    for (auto *a = reinterpret_cast<IP_ADAPTER_ADDRESSES *>(buf.data()); a; a = a->Next) {
        if (a->OperStatus != IfOperStatusUp || a->IfType == IF_TYPE_SOFTWARE_LOOPBACK) continue;
        for (auto *u = a->FirstUnicastAddress; u; u = u->Next) {
            if (u->Address.lpSockaddr->sa_family != AF_INET) continue;
            ULONG prefix = u->OnLinkPrefixLength;
            if (prefix == 0 || prefix >= 31) continue;
            ULONG ip = ntohl(reinterpret_cast<sockaddr_in *>(u->Address.lpSockaddr)->sin_addr.s_addr);
            ULONG mask = 0xFFFFFFFFu << (32 - prefix);
            in_addr b;
            b.s_addr = htonl(ip | ~mask);
            bool dup = false;
            for (const auto &o : out) dup |= o.s_addr == b.s_addr;
            if (!dup) out.push_back(b);
        }
    }
    return out;
}

} // namespace

RemoteSinkClient &RemoteSinkClient::instance() {
    static RemoteSinkClient client;
    return client;
}

RemoteSinkClient::~RemoteSinkClient() {
    stop();
}

bool RemoteSinkClient::parse_info(const std::string &text, RemoteSinkInfo *out) {
    json j = json::parse(text, nullptr, false);
    if (j.is_discarded() || !j.is_object()) return false;
    std::string type = str(j, "type");
    if ((type != "announce" && type != "hello") || str(j, "tool") != "a2dpwb_sink") return false;
    RemoteSinkInfo info;
    info.port = num<uint16_t>(j, "port", REMOTE_SINK_DEFAULT_PORT);
    info.host = str(j, "host");
    info.bt_address = str(j, "bt_address");
    info.bt_name = str(j, "bt_name");
    info.mtu = num<uint32_t>(j, "mtu");
    auto codecs = j.find("codecs");
    if (codecs != j.end() && codecs->is_array())
        for (const auto &c : *codecs)
            if (c.is_string()) info.codecs.push_back(c.get<std::string>());
    *out = std::move(info);
    return true;
}

std::vector<RemoteSinkInfo> RemoteSinkClient::discover(const std::string &host, uint16_t port,
                                                       unsigned timeout_ms) {
    std::vector<RemoteSinkInfo> found;
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) return found;
    SOCKET s = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (s != INVALID_SOCKET) {
        BOOL on = TRUE;
        setsockopt(s, SOL_SOCKET, SO_BROADCAST, reinterpret_cast<const char *>(&on), sizeof(on));
        std::vector<in_addr> targets;
        if (host.empty()) {
            targets = broadcast_addresses();
        } else {
            addrinfo hints = {}, *res = nullptr;
            hints.ai_family = AF_INET;
            hints.ai_socktype = SOCK_DGRAM;
            if (getaddrinfo(host.c_str(), nullptr, &hints, &res) == 0 && res) {
                targets.push_back(reinterpret_cast<sockaddr_in *>(res->ai_addr)->sin_addr);
                freeaddrinfo(res);
            }
        }
        static const char REQUEST[] = "{\"type\":\"discover\",\"version\":1,\"tool\":\"A2DPWB\"}";
        for (const auto &t : targets) {
            sockaddr_in to = {};
            to.sin_family = AF_INET;
            to.sin_port = htons(port);
            to.sin_addr = t;
            sendto(s, REQUEST, static_cast<int>(sizeof(REQUEST) - 1), 0,
                   reinterpret_cast<const sockaddr *>(&to), sizeof(to));
        }
        ULONGLONG deadline = GetTickCount64() + timeout_ms;
        for (;;) {
            ULONGLONG now = GetTickCount64();
            if (now >= deadline) break;
            ULONGLONG left = deadline - now;
            fd_set rd;
            FD_ZERO(&rd);
            FD_SET(s, &rd);
            timeval tv = {static_cast<long>(left / 1000), static_cast<long>((left % 1000) * 1000)};
            if (select(0, &rd, nullptr, nullptr, &tv) != 1) break;
            char buf[4096];
            sockaddr_in from = {};
            int from_len = sizeof(from);
            int n = recvfrom(s, buf, sizeof(buf) - 1, 0, reinterpret_cast<sockaddr *>(&from), &from_len);
            if (n <= 0) continue;
            RemoteSinkInfo info;
            if (!parse_info(std::string(buf, static_cast<size_t>(n)), &info)) continue;
            char ip[INET_ADDRSTRLEN] = {};
            inet_ntop(AF_INET, &from.sin_addr, ip, sizeof(ip));
            info.ip = ip;
            bool dup = std::any_of(found.begin(), found.end(),
                                   [&](const RemoteSinkInfo &f) { return f.ip == info.ip && f.port == info.port; });
            if (!dup) found.push_back(std::move(info));
            if (!host.empty()) break;  /* the one asked for answered */
        }
        closesocket(s);
    }
    WSACleanup();
    return found;
}

bool RemoteSinkClient::parse_stats(const std::string &line, RemoteSinkStats *out) {
    json j = json::parse(line, nullptr, false);
    if (j.is_discarded() || !j.is_object() || str(j, "type") != "stats") return false;

    RemoteSinkStats s;
    s.received_tick = GetTickCount64();
    s.connected = j.value("connected", false);
    s.streaming = j.value("streaming", false);
    s.device = str(j, "device");
    s.buffer_target_ms = num<uint32_t>(j, "buffer_target_ms");
    s.mtu = num<uint32_t>(j, "mtu");
    auto codecs = j.find("codecs");
    if (codecs != j.end() && codecs->is_array())
        for (const auto &c : *codecs)
            if (c.is_string()) s.codecs.push_back(c.get<std::string>());
    s.has_rssi = j.contains("rssi") && j["rssi"].is_number();
    s.rssi = num<int>(j, "rssi");
    s.has_tx_power = j.contains("tx_power") && j["tx_power"].is_number();
    s.tx_power = num<int>(j, "tx_power");
    s.rssi_note = str(j, "rssi_note");
    s.afh_channels = num<int>(j, "afh_channels", -1);

    auto st = j.find("stream");
    if (st != j.end() && st->is_object()) {
        const json &t = *st;
        s.has_stream = true;
        s.stream_id = num<uint32_t>(t, "id");
        s.codec = str(t, "codec");
        s.config = str(t, "config");
        s.decoding = t.value("decoding", false);
        s.packets = num<uint64_t>(t, "packets");
        s.bytes = num<uint64_t>(t, "bytes");
        s.frames = num<uint64_t>(t, "frames");
        s.lost = num<uint64_t>(t, "lost");
        s.late = num<uint64_t>(t, "late");
        s.ts_errors = num<uint64_t>(t, "ts_errors");
        s.frame_errors = num<uint64_t>(t, "frame_errors");
        s.decode_errors = num<uint64_t>(t, "decode_errors");
        s.underruns = num<uint64_t>(t, "underruns");
        s.overflows = num<uint64_t>(t, "overflows");
        s.pauses = num<uint64_t>(t, "pauses");
        s.idle = t.value("idle", false);
        s.overflow_ms = num<double>(t, "overflow_ms");
        s.audio_ms = num<double>(t, "audio_ms");
        s.underrun_ms = num<double>(t, "underrun_ms");
        s.max_gap_ms = num<double>(t, "max_gap_ms");
        s.jitter_ms = num<double>(t, "jitter_ms");
        s.buffer_ms = num<double>(t, "buffer_ms", -1.0);
        auto iv = t.find("interval");
        if (iv != t.end() && iv->is_object()) {
            s.interval_packets = num<uint64_t>(*iv, "packets");
            s.interval_bytes = num<uint64_t>(*iv, "bytes");
            s.interval_max_gap_ms = num<double>(*iv, "max_gap_ms");
        }
    }
    *out = std::move(s);
    return true;
}

void RemoteSinkClient::start(const std::string &target) {
    stop();
    std::string host;
    uint16_t port = 0;
    std::lock_guard<std::mutex> lock(mutex_);
    target_ = target.empty() ? std::string(REMOTE_SINK_AUTO) : target;
    have_stats_ = false;
    have_info_ = false;
    if (!split_target(target, &host, &port)) {
        status_ = Status::Failed;
        detail_ = "invalid address (host or host:port)";
        return;
    }
    stop_ = false;
    status_ = Status::Connecting;
    detail_.clear();
    thread_ = std::thread(&RemoteSinkClient::run, this, host, port);
}

void RemoteSinkClient::stop() {
    std::thread t;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        stop_ = true;
        if (socket_ != ~uintptr_t(0))
            shutdown(static_cast<SOCKET>(socket_), SD_BOTH);  /* wakes recv() */
        t = std::move(thread_);
    }
    wake_.notify_all();
    if (t.joinable()) t.join();
    std::lock_guard<std::mutex> lock(mutex_);
    status_ = Status::Off;
    detail_.clear();
    have_stats_ = false;
    have_info_ = false;
}

RemoteSinkClient::Status RemoteSinkClient::status(std::string *detail) const {
    std::lock_guard<std::mutex> lock(mutex_);
    if (detail) *detail = detail_;
    return status_;
}

std::string RemoteSinkClient::target() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return target_;
}

bool RemoteSinkClient::receiver(RemoteSinkInfo *out) const {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!have_info_) return false;
    *out = info_;
    return true;
}

bool RemoteSinkClient::configure(uint32_t mtu, uint32_t buffer_ms) {
    json j = {{"type", "configure"}, {"version", 1}};
    if (mtu) j["mtu"] = mtu;
    if (buffer_ms) j["buffer_ms"] = buffer_ms;
    std::string line = j.dump() + "\n";
    std::lock_guard<std::mutex> lock(mutex_);
    if (socket_ == ~uintptr_t(0)) return false;
    return send(static_cast<SOCKET>(socket_), line.data(), static_cast<int>(line.size()), 0) ==
           static_cast<int>(line.size());
}

bool RemoteSinkClient::latest(RemoteSinkStats *out) const {
    std::lock_guard<std::mutex> lock(mutex_);
    if (!have_stats_) return false;
    *out = stats_;
    return true;
}

void RemoteSinkClient::set_status(Status s, const std::string &detail) {
    std::lock_guard<std::mutex> lock(mutex_);
    status_ = s;
    detail_ = detail;
    if (s != Status::Connected) have_stats_ = false;
}

void RemoteSinkClient::run(std::string host, uint16_t port) {
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        set_status(Status::Failed, "WSAStartup failed");
        return;
    }
    const bool automatic = host == REMOTE_SINK_AUTO;
    for (;;) {
        {
            std::lock_guard<std::mutex> lock(mutex_);
            if (stop_) break;
        }
        set_status(Status::Connecting, "");
        std::string error;
        std::string connect_host = host;
        uint16_t connect_port = port;
        SOCKET s = INVALID_SOCKET;
        if (automatic) {
            std::vector<RemoteSinkInfo> found = discover(std::string(), port);
            if (found.empty()) {
                error = "no receiver found on the local network";
            } else {
                connect_host = found[0].ip;
                connect_port = found[0].port;
                std::lock_guard<std::mutex> lock(mutex_);
                info_ = found[0];
                have_info_ = true;
            }
        }
        if (!automatic || connect_host != REMOTE_SINK_AUTO)
            s = connect_to(connect_host, connect_port, &error);
        if (s != INVALID_SOCKET) {
            {
                std::lock_guard<std::mutex> lock(mutex_);
                if (stop_) {
                    closesocket(s);
                    break;
                }
                socket_ = static_cast<uintptr_t>(s);
            }
            set_status(Status::Connected, "");
            std::string buf;
            char chunk[4096];
            for (;;) {
                int n = recv(s, chunk, sizeof(chunk), 0);
                if (n <= 0) {
                    error = n == 0 ? "closed by the receiver"
                                   : (WSAGetLastError() == WSAETIMEDOUT ? "no data from the receiver"
                                                                        : "connection lost");
                    break;
                }
                buf.append(chunk, static_cast<size_t>(n));
                size_t nl;
                while ((nl = buf.find('\n')) != std::string::npos) {
                    std::string line = buf.substr(0, nl);
                    RemoteSinkStats st;
                    RemoteSinkInfo info;
                    if (parse_stats(line, &st)) {
                        std::lock_guard<std::mutex> lock(mutex_);
                        stats_ = std::move(st);
                        have_stats_ = true;
                    } else if (parse_info(line, &info)) {
                        info.ip = connect_host;
                        std::lock_guard<std::mutex> lock(mutex_);
                        info_ = std::move(info);
                        have_info_ = true;
                    }
                    buf.erase(0, nl + 1);
                }
                if (buf.size() > MAX_LINE) {
                    error = "protocol error";
                    break;
                }
            }
            {
                std::lock_guard<std::mutex> lock(mutex_);
                socket_ = ~uintptr_t(0);
            }
            closesocket(s);
        }
        std::unique_lock<std::mutex> lock(mutex_);
        if (stop_) break;
        status_ = Status::Failed;
        detail_ = error;
        have_stats_ = false;
        wake_.wait_for(lock, RECONNECT_DELAY, [this] { return stop_; });
        if (stop_) break;
    }
    WSACleanup();
}
