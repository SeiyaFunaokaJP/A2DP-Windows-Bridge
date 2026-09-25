/*
 * HCI capture - on-demand HCI packet log (PacketLogger .pklg) for debug mode
 *
 * Installed as BTstack's hci_dump implementation. While no capture is running
 * it keeps only the packets needed to understand a connection later: L2CAP
 * signaling (channel setup) and AVDTP signaling except DELAYREPORT. When a
 * capture starts in the middle of a connection, those packets are written
 * first (with their original timestamps), so Wireshark and a2dpwb_decode
 * still see the codec negotiation and which L2CAP channel carries the media.
 *
 * start() / stop() / status may be called from any thread; the hci_dump
 * callbacks run on the BTstack thread.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef HCI_CAPTURE_H
#define HCI_CAPTURE_H

#include <cstdint>
#include <string>

extern "C" {
#include "hci_dump.h"
}

namespace hci_capture {

/* hci_dump implementation to pass to hci_dump_init() (BTstack thread) */
const hci_dump_t *instance();

/* Forget remembered connection setup packets (call when BTstack (re)starts) */
void reset_tracking();

/* Start writing to path (created / overwritten). Returns false if the file
 * cannot be created or a capture is already running. */
bool start(const std::string &path);

/* Stop the running capture and close the file (no-op if none) */
void stop();

bool active();
std::string path();
uint64_t bytes_written();

} // namespace hci_capture

#endif /* HCI_CAPTURE_H */
