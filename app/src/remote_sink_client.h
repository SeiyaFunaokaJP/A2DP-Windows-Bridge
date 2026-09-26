/*
 * Remote sink client - statistics from a measuring receiver on another PC
 *
 * tools/linux_sink/a2dpwb_sink.py runs on a Linux PC as an ordinary A2DP sink
 * (Bumble on its Bluetooth adapter, the built-in one is fine) and measures
 * what arrives: lost / late packets, jitter, gaps, dropouts of a modelled
 * playout buffer, frame and decode errors, RSSI. It serves one JSON object per line over TCP; this
 * client keeps the connection (reconnecting every few seconds) and holds the
 * latest statistics for the link quality window and the CLI.
 *
 * The receiver also answers a UDP broadcast on the same port, so it can be
 * found on the local network: target "auto", or discover().
 *
 * Process-wide like link_stats(): the connection outlives the window.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef REMOTE_SINK_CLIENT_H
#define REMOTE_SINK_CLIENT_H

#include <condition_variable>
#include <cstdint>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

/* Default TCP (statistics) and UDP (discovery) port of a2dpwb_sink.py */
constexpr uint16_t REMOTE_SINK_DEFAULT_PORT = 51201;
/* Target meaning: find the receiver on the local network */
constexpr const char *REMOTE_SINK_AUTO = "auto";

/* A receiver: from a discovery reply or the hello line of a connection */
struct RemoteSinkInfo {
    std::string ip;               /* address it answered from */
    uint16_t port = REMOTE_SINK_DEFAULT_PORT;
    std::string host;             /* its host name */
    std::string bt_address;       /* its Bluetooth adapter: the device to stream to */
    std::string bt_name;
    std::vector<std::string> codecs;
    uint32_t mtu = 0;             /* L2CAP MTU it offers for AVDTP */
};

/* Statistics of the latest message. Counters are cumulative for the stream
 * the receiver is measuring (stream_id); they restart with a new stream. */
struct RemoteSinkStats {
    uint64_t received_tick = 0;   /* GetTickCount64() when the message arrived */
    bool connected = false;       /* a source is connected to the receiver */
    bool streaming = false;       /* media is flowing right now */
    std::string device;           /* Bluetooth address of that source */
    uint32_t buffer_target_ms = 0;
    uint32_t mtu = 0;             /* L2CAP MTU the receiver offers for new streams */
    std::vector<std::string> codecs;  /* codecs it offers */
    bool has_rssi = false;
    int rssi = 0, tx_power = 0;
    bool has_tx_power = false;
    int afh_channels = -1;        /* channels the link hops over, seen from the receiver */
    std::string rssi_note;        /* why RSSI is missing, if the receiver says */

    bool has_stream = false;      /* fields below are valid */
    uint32_t stream_id = 0;
    std::string codec, config;
    bool decoding = false;        /* the receiver decodes (not only header checks) */
    uint64_t packets = 0, bytes = 0, frames = 0;
    uint64_t lost = 0, late = 0;
    uint64_t ts_errors = 0, frame_errors = 0, decode_errors = 0;
    uint64_t underruns = 0, overflows = 0, pauses = 0;
    bool idle = false;            /* the source sends nothing right now */
    double audio_ms = 0, underrun_ms = 0, overflow_ms = 0, max_gap_ms = 0, jitter_ms = 0;
    double buffer_ms = -1;        /* modelled playout buffer, < 0 = not playing */
    /* Last reporting interval (about one second) */
    uint64_t interval_packets = 0, interval_bytes = 0;
    double interval_max_gap_ms = 0;
};

class RemoteSinkClient {
public:
    enum class Status { Off, Connecting, Connected, Failed };

    static RemoteSinkClient &instance();

    /* Connects to "host[:port]" (default port REMOTE_SINK_DEFAULT_PORT), or
     * with "auto" / "" to the first receiver found on the local network, and
     * keeps reconnecting until stop(). Replaces a previous target. */
    void start(const std::string &target);
    void stop();

    Status status(std::string *detail = nullptr) const;
    std::string target() const;
    /* false while nothing was received from the current connection */
    bool latest(RemoteSinkStats *out) const;
    /* The receiver of the current connection, once it introduced itself */
    bool receiver(RemoteSinkInfo *out) const;

    /* Receiver settings for the next stream (0 = leave unchanged); false if
     * not connected. The receiver confirms them in its statistics. */
    bool configure(uint32_t mtu, uint32_t buffer_ms);

    /* Asks for receivers: a UDP broadcast on every IPv4 network, or only
     * `host` if given. Blocks for up to timeout_ms. */
    static std::vector<RemoteSinkInfo> discover(const std::string &host = std::string(),
                                                uint16_t port = REMOTE_SINK_DEFAULT_PORT,
                                                unsigned timeout_ms = 1500);

    /* Parses one JSON line from a2dpwb_sink.py; false if it is no stats message */
    static bool parse_stats(const std::string &line, RemoteSinkStats *out);
    /* Parses a discovery reply or hello line; false if it is neither */
    static bool parse_info(const std::string &text, RemoteSinkInfo *out);

    ~RemoteSinkClient();

private:
    RemoteSinkClient() = default;
    void run(std::string host, uint16_t port);
    void set_status(Status s, const std::string &detail);

    mutable std::mutex mutex_;
    std::condition_variable wake_;
    std::thread thread_;
    bool stop_ = false;
    uintptr_t socket_ = ~uintptr_t(0);  /* SOCKET of the live connection */
    std::string target_;
    Status status_ = Status::Off;
    std::string detail_;
    bool have_stats_ = false;
    RemoteSinkStats stats_;
    bool have_info_ = false;
    RemoteSinkInfo info_;
};

#endif /* REMOTE_SINK_CLIENT_H */
