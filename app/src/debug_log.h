/*
 * Debug Logging Macros - Timestamped structured logging
 *
 * When debug_mode is off, stderr is redirected to NUL at startup,
 * so all fprintf(stderr, ...) calls become OS-level no-ops.
 * These macros add timestamps and severity levels for structured output.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef DEBUG_LOG_H
#define DEBUG_LOG_H

#include <cstdio>
#include <windows.h>

#define LOG_DEBUG(fmt, ...) do { \
    SYSTEMTIME _st; GetLocalTime(&_st); \
    fprintf(stderr, "[%02d:%02d:%02d.%03d DEBUG] " fmt "\n", \
            _st.wHour, _st.wMinute, _st.wSecond, _st.wMilliseconds, ##__VA_ARGS__); \
} while(0)

#define LOG_INFO(fmt, ...) do { \
    SYSTEMTIME _st; GetLocalTime(&_st); \
    fprintf(stderr, "[%02d:%02d:%02d.%03d INFO ] " fmt "\n", \
            _st.wHour, _st.wMinute, _st.wSecond, _st.wMilliseconds, ##__VA_ARGS__); \
} while(0)

#define LOG_WARN(fmt, ...) do { \
    SYSTEMTIME _st; GetLocalTime(&_st); \
    fprintf(stderr, "[%02d:%02d:%02d.%03d WARN ] " fmt "\n", \
            _st.wHour, _st.wMinute, _st.wSecond, _st.wMilliseconds, ##__VA_ARGS__); \
} while(0)

#define LOG_ERROR(fmt, ...) do { \
    SYSTEMTIME _st; GetLocalTime(&_st); \
    fprintf(stderr, "[%02d:%02d:%02d.%03d ERROR] " fmt "\n", \
            _st.wHour, _st.wMinute, _st.wSecond, _st.wMilliseconds, ##__VA_ARGS__); \
} while(0)

#endif /* DEBUG_LOG_H */
