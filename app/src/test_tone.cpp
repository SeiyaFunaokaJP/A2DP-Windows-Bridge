/*
 * Test tone - Implementation
 * SPDX-License-Identifier: MIT
 */

#include "test_tone.h"

#include <chrono>
#include <cmath>
#include <vector>

void TestTone::start(uint32_t sample_rate, AudioCallback callback) {
    stop();
    running_.store(true);
    thread_ = std::thread([this, sample_rate, callback]() {
        const uint32_t block = sample_rate / 100;  /* 10 ms */
        const double two_pi = 6.283185307179586;
        std::vector<float> buf(block * CHANNELS);
        uint64_t n = 0;
        auto next = std::chrono::steady_clock::now();
        while (running_.load()) {
            for (uint32_t i = 0; i < block; i++, n++) {
                double t = static_cast<double>(n) / sample_rate;
                buf[2 * i]     = static_cast<float>(AMPLITUDE * std::sin(two_pi * LEFT_HZ * t));
                buf[2 * i + 1] = static_cast<float>(AMPLITUDE * std::sin(two_pi * RIGHT_HZ * t));
            }
            callback(reinterpret_cast<const uint8_t *>(buf.data()), block, CHANNELS, sample_rate,
                     BITS_PER_SAMPLE);
            next += std::chrono::milliseconds(10);
            std::this_thread::sleep_until(next);
        }
    });
}

void TestTone::stop() {
    running_.store(false);
    if (thread_.joinable()) thread_.join();
}
