/*
 * Test tone - a known signal instead of captured audio
 *
 * 1 kHz on the left, 1.5 kHz on the right, amplitude 0.5, delivered like
 * WASAPI delivers captured audio: float32 stereo in 10 ms blocks, paced in
 * real time. Used by the CLI (--test-tone, tools/emu) and the peer receiver
 * test, where a steady source keeps the measurement independent of what the
 * PC happens to play (loopback capture delivers nothing while it is silent).
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef TEST_TONE_H
#define TEST_TONE_H

#include "wasapi_capture.h"

#include <atomic>
#include <cstdint>
#include <thread>

class TestTone {
public:
    static constexpr double LEFT_HZ = 1000.0;
    static constexpr double RIGHT_HZ = 1500.0;
    static constexpr double AMPLITUDE = 0.5;
    static constexpr uint32_t CHANNELS = 2;
    static constexpr uint32_t BITS_PER_SAMPLE = 32;  /* float32 */

    ~TestTone() { stop(); }

    void start(uint32_t sample_rate, AudioCallback callback);
    void stop();

private:
    std::atomic<bool> running_{false};
    std::thread thread_;
};

#endif /* TEST_TONE_H */
