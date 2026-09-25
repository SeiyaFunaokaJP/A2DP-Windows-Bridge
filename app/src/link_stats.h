/*
 * Link statistics - cumulative media counters for the link quality window
 *
 * Process-wide so the counters outlive BtStackTransport instances (BTstack
 * can be re-created while the window is open). Writers: the BTstack thread
 * (packets sent), the encode thread (queue drops) and the WASAPI callback
 * (capture drops). The GUI samples them once per second and works with
 * deltas, so nothing is ever reset.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef LINK_STATS_H
#define LINK_STATS_H

#include <atomic>
#include <cstdint>

/* Capacity of BtStackTransport's media send queue, in packets */
constexpr uint32_t MEDIA_QUEUE_PACKETS = 64;

struct LinkStats {
    /* Media packets handed to L2CAP and their L2CAP payload bytes (RTP header,
     * media payload header and codec data) */
    std::atomic<uint64_t> packets_sent{0};
    std::atomic<uint64_t> bytes_sent{0};
    /* Packets lost before the radio: send queue full / L2CAP refused the packet */
    std::atomic<uint64_t> queue_drops{0};
    std::atomic<uint64_t> send_errors{0};
    /* PCM frames captured from Windows, and those dropped because the encoder
     * fell behind (ring buffer full) */
    std::atomic<uint64_t> capture_frames{0};
    std::atomic<uint64_t> capture_dropped_frames{0};
    /* Instantaneous send queue fill (packets) */
    std::atomic<uint32_t> queue_depth{0};
};

inline LinkStats &link_stats() {
    static LinkStats stats;
    return stats;
}

#endif /* LINK_STATS_H */
