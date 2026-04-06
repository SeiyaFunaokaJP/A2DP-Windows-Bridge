/*
 * Theme Manager - Dark/Light/System theme support
 * SPDX-License-Identifier: MIT
 */

#ifndef THEME_MANAGER_H
#define THEME_MANAGER_H

#include <wx/colour.h>
#include <string>

/* Semantic color roles used throughout the application */
enum class ThemeColor {
    /* Window / panel backgrounds */
    WindowBg,
    PanelBg,

    /* Profile list */
    ProfileBgNormal,
    ProfileBgSelected,

    /* Text */
    TextPrimary,
    TextSecondary,
    TextMuted,
    TextStreamInfo,

    /* Status indicator colors */
    StatusIdle,
    StatusConnecting,
    StatusStreaming,
    StatusReconnecting,
    StatusError,

    /* Notification bars */
    UpdateBarBg,
    UpdateBarText,
    FirmwareBarBg,
    FirmwareBarText,

    /* Firmware dialog */
    FirmwareOk,
    FirmwareWarning,
    FirmwareHint,
    FirmwareSource,

    /* About dialog */
    AboutTitle,
    AboutLicense,

    /* Buttons */
    DeleteButton,

    /* Error */
    ErrorText,

    /* Input controls */
    CtrlBg,        /* TextCtrl / ListBox / Choice background */
    CtrlFg,        /* TextCtrl / ListBox / Choice text */
    DialogBg,      /* Dialog background (same as WindowBg but explicit) */

    COUNT  /* must be last */
};

enum class ThemeMode {
    Dark,
    Light,
    System,
};

class ThemeManager {
public:
    static ThemeManager &instance();

    /* Set theme mode; resolves System to actual Dark/Light */
    void set_mode(ThemeMode mode);
    ThemeMode mode() const { return mode_; }

    /* True if the resolved (effective) theme is dark */
    bool is_dark() const { return effective_dark_; }

    /* Get a color for the current theme */
    wxColour get(ThemeColor role) const;

    /* Convert to/from string for JSON persistence */
    static ThemeMode mode_from_string(const std::string &s);
    static std::string mode_to_string(ThemeMode m);

    /* Detect Windows system theme (reads registry) */
    static bool system_prefers_dark();

private:
    ThemeManager();
    void resolve_effective();

    ThemeMode mode_ = ThemeMode::Dark;
    bool effective_dark_ = true;
};

/* Convenience alias */
inline ThemeManager &TM() { return ThemeManager::instance(); }

#endif /* THEME_MANAGER_H */
