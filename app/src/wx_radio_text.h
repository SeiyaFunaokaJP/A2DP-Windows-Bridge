/*
 * Radio state as text for the link quality windows
 * SPDX-License-Identifier: MIT
 */

#ifndef WX_RADIO_TEXT_H
#define WX_RADIO_TEXT_H

#include "link_stats.h"
#include "localization.h"

#include <wx/string.h>

/* "RSSI +0 dB vs. golden range, AFH 45/79 ch (avoiding Wi-Fi 6, 11)" */
inline wxString radio_text(bool has_rssi, int rssi, int afh_in_use, const std::string &avoided) {
    wxString t;
    if (has_rssi) t = wxString::Format(wxString::FromUTF8(L("radio.rssi")), rssi);
    if (afh_in_use >= 0) {
        if (!t.empty()) t += wxString::FromUTF8(L("radio.separator"));
        t += wxString::Format(wxString::FromUTF8(L("radio.afh")), afh_in_use);
    }
    if (!avoided.empty())
        t += wxString::Format(wxString::FromUTF8(L("radio.afh_avoid")), wxString::FromUTF8(avoided));
    return t.empty() ? wxString("-") : t;
}

inline wxString radio_text(const LinkRadio &r) {
    return radio_text(r.has_rssi, r.rssi, r.afh_in_use, r.afh_usable >= 0 ? r.afh_avoided : std::string());
}

#endif /* WX_RADIO_TEXT_H */
