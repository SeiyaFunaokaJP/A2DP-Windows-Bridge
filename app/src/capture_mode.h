/*
 * Capture Mode Definitions
 * SPDX-License-Identifier: MIT
 */
#ifndef CAPTURE_MODE_H
#define CAPTURE_MODE_H

enum class CaptureMode {
    SystemLoopback = 0,  /* WASAPI loopback on default device */
    VirtualDevice  = 1,  /* WASAPI loopback on user-selected virtual device */
    Application    = 2,  /* per-process loopback: one app and its child processes */
};

#endif /* CAPTURE_MODE_H */
