/*
 * Theme Manager Implementation
 * SPDX-License-Identifier: MIT
 */

#include "theme_manager.h"
#include <wx/settings.h>
#include <windows.h>

static const int kColorCount = static_cast<int>(ThemeColor::COUNT);

/* Dark palette */
static const wxColour dark_palette[] = {
    /* WindowBg          */ wxColour(30, 30, 30),
    /* PanelBg           */ wxColour(35, 35, 40),
    /* ProfileBgNormal   */ wxColour(30, 30, 35),
    /* ProfileBgSelected */ wxColour(50, 50, 70),
    /* TextPrimary       */ wxColour(255, 255, 255),
    /* TextSecondary     */ wxColour(180, 180, 180),
    /* TextMuted         */ wxColour(128, 128, 128),
    /* TextStreamInfo    */ wxColour(160, 160, 160),
    /* StatusIdle        */ wxColour(128, 128, 128),
    /* StatusConnecting  */ wxColour(255, 200, 0),
    /* StatusStreaming   */ wxColour(0, 255, 100),
    /* StatusReconnecting*/ wxColour(255, 128, 0),
    /* StatusError       */ wxColour(255, 50, 50),
    /* UpdateBarBg       */ wxColour(25, 76, 153),
    /* UpdateBarText     */ wxColour(255, 255, 255),
    /* FirmwareBarBg     */ wxColour(80, 60, 0),
    /* FirmwareBarText   */ wxColour(255, 217, 0),
    /* FirmwareOk        */ wxColour(0, 255, 100),
    /* FirmwareWarning   */ wxColour(255, 217, 0),
    /* FirmwareHint      */ wxColour(180, 180, 180),
    /* FirmwareSource    */ wxColour(140, 140, 140),
    /* AboutTitle        */ wxColour(100, 200, 255),
    /* AboutLicense      */ wxColour(128, 128, 128),
    /* DeleteButton      */ wxColour(230, 100, 100),
    /* ErrorText         */ wxColour(255, 80, 80),
    /* CtrlBg            */ wxColour(45, 45, 50),
    /* CtrlFg            */ wxColour(230, 230, 230),
    /* DialogBg          */ wxColour(30, 30, 30),
};

/* Light palette */
static const wxColour light_palette[] = {
    /* WindowBg          */ wxColour(245, 245, 245),
    /* PanelBg           */ wxColour(240, 240, 240),
    /* ProfileBgNormal   */ wxColour(235, 235, 240),
    /* ProfileBgSelected */ wxColour(200, 210, 240),
    /* TextPrimary       */ wxColour(30, 30, 30),
    /* TextSecondary     */ wxColour(90, 90, 90),
    /* TextMuted         */ wxColour(150, 150, 150),
    /* TextStreamInfo    */ wxColour(100, 100, 100),
    /* StatusIdle        */ wxColour(128, 128, 128),
    /* StatusConnecting  */ wxColour(200, 160, 0),
    /* StatusStreaming   */ wxColour(0, 160, 60),
    /* StatusReconnecting*/ wxColour(220, 110, 0),
    /* StatusError       */ wxColour(210, 40, 40),
    /* UpdateBarBg       */ wxColour(200, 220, 255),
    /* UpdateBarText     */ wxColour(20, 50, 130),
    /* FirmwareBarBg     */ wxColour(255, 245, 200),
    /* FirmwareBarText   */ wxColour(150, 110, 0),
    /* FirmwareOk        */ wxColour(0, 150, 55),
    /* FirmwareWarning   */ wxColour(180, 140, 0),
    /* FirmwareHint      */ wxColour(100, 100, 100),
    /* FirmwareSource    */ wxColour(140, 140, 140),
    /* AboutTitle        */ wxColour(30, 120, 200),
    /* AboutLicense      */ wxColour(128, 128, 128),
    /* DeleteButton      */ wxColour(200, 60, 60),
    /* ErrorText         */ wxColour(200, 50, 50),
    /* CtrlBg            */ wxColour(255, 255, 255),
    /* CtrlFg            */ wxColour(30, 30, 30),
    /* DialogBg          */ wxColour(245, 245, 245),
};

static_assert(sizeof(dark_palette) / sizeof(dark_palette[0]) == kColorCount,
              "dark_palette size mismatch");
static_assert(sizeof(light_palette) / sizeof(light_palette[0]) == kColorCount,
              "light_palette size mismatch");

ThemeManager &ThemeManager::instance() {
    static ThemeManager inst;
    return inst;
}

ThemeManager::ThemeManager() {
    resolve_effective();
}

void ThemeManager::set_mode(ThemeMode mode) {
    mode_ = mode;
    resolve_effective();
}

void ThemeManager::resolve_effective() {
    switch (mode_) {
    case ThemeMode::Dark:   effective_dark_ = true; break;
    case ThemeMode::Light:  effective_dark_ = false; break;
    case ThemeMode::System: effective_dark_ = system_prefers_dark(); break;
    }
}

wxColour ThemeManager::get(ThemeColor role) const {
    int idx = static_cast<int>(role);
    if (idx < 0 || idx >= kColorCount)
        return *wxBLACK;
    return effective_dark_ ? dark_palette[idx] : light_palette[idx];
}

ThemeMode ThemeManager::mode_from_string(const std::string &s) {
    if (s == "light")  return ThemeMode::Light;
    if (s == "system") return ThemeMode::System;
    return ThemeMode::Dark;
}

std::string ThemeManager::mode_to_string(ThemeMode m) {
    switch (m) {
    case ThemeMode::Light:  return "light";
    case ThemeMode::System: return "system";
    default:                return "dark";
    }
}

bool ThemeManager::system_prefers_dark() {
    DWORD value = 1;  /* default to light if key missing */
    DWORD size = sizeof(value);
    HKEY hkey;
    if (RegOpenKeyExA(HKEY_CURRENT_USER,
            "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
            0, KEY_READ, &hkey) == ERROR_SUCCESS) {
        RegQueryValueExA(hkey, "AppsUseLightTheme", nullptr, nullptr,
                         reinterpret_cast<LPBYTE>(&value), &size);
        RegCloseKey(hkey);
    }
    return value == 0;  /* 0 means dark mode */
}
