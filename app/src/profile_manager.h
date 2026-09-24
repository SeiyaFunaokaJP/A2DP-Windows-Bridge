/*
 * Profile Manager - Connection profile persistence
 *
 * Stores device + codec + quality presets as named profiles
 * for one-click Bluetooth connection.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef PROFILE_MANAGER_H
#define PROFILE_MANAGER_H

#include <string>
#include <vector>
#include <cstdint>

struct ConnectionProfile {
    std::string name;
    std::string device_address;   /* XX:XX:XX:XX:XX:XX */
    std::string device_name;
    std::string codec;            /* auto, ldac, aptxhd, aptxll, aptx, sbc, aac */
    std::string quality;          /* hq, sq, mq */
    bool abr = false;
    uint32_t sample_rate = 0;     /* 0=auto, 44100, 48000, 88200, 96000 */
    uint32_t bit_depth = 0;       /* 0=auto, 16, 24 */
    std::string capture_mode;     /* "loopback", "virtual" */
    std::string audio_device_id;  /* WASAPI device ID for virtual mode */
    std::string audio_device_name; /* display name */
    bool auto_switch_device = true;  /* auto-switch default for Virtual Device mode */
};

class ProfileManager {
public:
    /* Load profiles from %APPDATA%\A2DPWB\profiles.json */
    void load();

    /* Save all profiles to disk */
    void save() const;

    /* Get all profiles */
    const std::vector<ConnectionProfile> &profiles() const { return profiles_; }

    /* Add a new profile. Returns index. */
    size_t add(const ConnectionProfile &profile);

    /* Update existing profile at index */
    bool update(size_t index, const ConnectionProfile &profile);

    /* Remove profile at index */
    bool remove(size_t index);

    /* Find profile index by name. Returns -1 if not found. */
    int find_by_name(const std::string &name) const;

    /* Convert codec string to combo index (0=Auto,1=LDAC,2=aptXHD,3=aptXLL,4=aptX,5=SBC,6=AAC) */
    static int codec_to_index(const std::string &codec);

    /* Convert combo index to codec string */
    static std::string index_to_codec(int index);

    /* Convert quality string to combo index (0=HQ,1=SQ,2=MQ) */
    static int quality_to_index(const std::string &quality);

    /* Convert combo index to quality string */
    static std::string index_to_quality(int index);

    /* Convert bit depth value to combo index (0=Auto,1=16,2=24) */
    static int bit_depth_to_index(uint32_t bit_depth);

    /* Convert combo index to bit depth value */
    static uint32_t index_to_bit_depth(int index);

    /* Convert capture mode string to combo index (0=loopback, 1=virtual) */
    static int capture_mode_to_index(const std::string &mode);

    /* Convert combo index to capture mode string */
    static std::string index_to_capture_mode(int index);

private:
    std::vector<ConnectionProfile> profiles_;
    std::string get_profiles_path() const;
};

#endif /* PROFILE_MANAGER_H */
