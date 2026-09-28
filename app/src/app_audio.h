/*
 * Application audio capture helpers
 *
 * Lists the apps that currently have an audio session, finds a running app
 * by its executable name, and checks whether Windows supports per-process
 * loopback capture (Application capture mode).
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef APP_AUDIO_H
#define APP_AUDIO_H

#include <cstdint>
#include <string>
#include <vector>
#include <windows.h>

/* Per-process loopback (AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK).
 * Microsoft documents build 20348, but it works from Windows 10 2004
 * (build 19041), which covers 20H2-22H2 (19042-19045). OBS's Application
 * Audio Capture uses the same threshold. */
static const uint32_t APP_CAPTURE_MIN_BUILD = 19041;

struct AudioAppInfo {
    std::string exe_name;      /* UTF-8, e.g. "player.exe" */
    std::string display_name;  /* UTF-8, e.g. "Music Player (player.exe)" */
};

namespace app_audio {

/* Windows build number (RtlGetVersion, not subject to manifest lies) */
uint32_t windows_build();

/* True when this Windows supports Application capture mode */
bool capture_supported();

/* Apps with an audio session on any active output device, one entry per
 * executable name, sorted by display name. Leaves out A2DPWB itself and the
 * system sounds session. */
std::vector<AudioAppInfo> list_audio_apps();

/* Process ID of a running app with this executable name (case-insensitive),
 * or 0. With several processes of that name (browsers), returns the top one
 * of its process tree, since capture includes the child processes. Prefers
 * an instance that has an audio session when several are running. */
DWORD find_process(const std::string &exe_name);

} /* namespace app_audio */

#endif /* APP_AUDIO_H */
