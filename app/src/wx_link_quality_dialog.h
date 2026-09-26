/*
 * Link Quality Dialog - live bitrate, packet loss and Bluetooth bandwidth use
 * SPDX-License-Identifier: MIT
 */

#ifndef WX_LINK_QUALITY_DIALOG_H
#define WX_LINK_QUALITY_DIALOG_H

#include <wx/wx.h>
#include <wx/gauge.h>
#include <wx/timer.h>

#include <cstdint>

class MainFrame;

/* Modeless; open with LinkQualityDialog::ShowFor() so only one exists */
class LinkQualityDialog : public wxDialog {
public:
    static void ShowFor(MainFrame *frame);

private:
    explicit LinkQualityDialog(MainFrame *frame);

    struct Sample {
        uint64_t tick_ms = 0;
        uint64_t packets = 0, bytes = 0, dropped = 0;
        uint64_t capture_frames = 0, capture_dropped = 0;
    };
    static Sample take_sample();

    void OnTimer(wxTimerEvent &evt);
    void refresh();
    wxStaticText *add_row(wxWindow *parent, wxFlexGridSizer *grid, const char *label_key);
    wxStaticText *add_heading(wxWindow *parent, wxSizer *sizer, const char *key);

    MainFrame *frame_;
    wxTimer timer_;
    Sample baseline_{}, prev_{};
    bool have_prev_ = false;

    wxStaticText *idle_label_ = nullptr;
    wxStaticText *codec_value_ = nullptr;
    wxStaticText *bitrate_value_ = nullptr;
    wxStaticText *packets_value_ = nullptr;
    wxStaticText *queue_value_ = nullptr;
    wxStaticText *radio_value_ = nullptr;
    wxStaticText *theory_value_ = nullptr;
    wxGauge      *theory_gauge_ = nullptr;
    wxStaticText *airtime_value_ = nullptr;
    wxGauge      *airtime_gauge_ = nullptr;
    wxStaticText *sent_value_ = nullptr;
    wxStaticText *dropped_value_ = nullptr;
    wxStaticText *capture_value_ = nullptr;
};

#endif /* WX_LINK_QUALITY_DIALOG_H */
