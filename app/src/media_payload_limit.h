/*
 * Max media packet size - advanced setting shared by the transport, the
 * service, the settings file, the GUI and the CLI.
 *
 * The media payload of each A2DP packet (RTP header excluded) is the remote's
 * media channel MTU, limited to this value.
 *   MIN      679: libldac requires at least this (LDACBT_MTU_REQUIRED, one 2-DH5)
 *   MAX     1679: largest payload BtStackTransport can queue and send
 *                 (HCI_ACL_PAYLOAD_SIZE 1695 - 4-byte L2CAP - 12-byte RTP header)
 *   DEFAULT 1023: recommended. Fits one 3-DH5 baseband packet; larger packets
 *                 span several, so a lost baseband packet loses more audio,
 *                 while saving only 1-2% overhead.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef MEDIA_PAYLOAD_LIMIT_H
#define MEDIA_PAYLOAD_LIMIT_H

#include <cstdint>

constexpr uint16_t MEDIA_PAYLOAD_LIMIT_MIN = 679;
constexpr uint16_t MEDIA_PAYLOAD_LIMIT_MAX = 1679;
constexpr uint16_t MEDIA_PAYLOAD_LIMIT_DEFAULT = 1023;

inline uint16_t clamp_media_payload_limit(uint32_t limit) {
    return limit < MEDIA_PAYLOAD_LIMIT_MIN ? MEDIA_PAYLOAD_LIMIT_MIN
         : limit > MEDIA_PAYLOAD_LIMIT_MAX ? MEDIA_PAYLOAD_LIMIT_MAX
         : static_cast<uint16_t>(limit);
}

#endif /* MEDIA_PAYLOAD_LIMIT_H */
