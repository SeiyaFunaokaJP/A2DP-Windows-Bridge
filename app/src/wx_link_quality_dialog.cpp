/*
 * Link Quality Dialog Implementation
 * SPDX-License-Identifier: MIT
 */

#include "wx_link_quality_dialog.h"
#include "wx_main_frame.h"
#include "link_stats.h"
#include "localization.h"
#include "theme_manager.h"

#include <wx/statline.h>

#include <windows.h>

namespace {

const char *DIALOG_NAME = "a2dpwb_link_quality";

/* Bluetooth BR/EDR baseband: 625 us slots. The best ACL packet type on an
 * EDR 3 Mbps link is 3-DH5 (5 slots, up to 1021 bytes); each packet is
 * followed by one slot for the receiver's reply. */
const double SLOT_SECONDS = 625e-6;
const uint32_t L2CAP_HEADER_BYTES = 4;
struct AclPacketType { const char *name; uint32_t max_bytes; uint32_t slots; };
const AclPacketType EDR3_TYPES[] = {
    {"3-DH1", 83, 1},
    {"3-DH3", 552, 3},
    {"3-DH5", 1021, 5},
};
const AclPacketType &LARGEST_TYPE = EDR3_TYPES[2];
/* 1021 bytes every 6 slots: 2178 kbps */
const double THEORETICAL_MAX_KBPS = LARGEST_TYPE.max_bytes * 8.0 / ((LARGEST_TYPE.slots + 1) * SLOT_SECONDS) / 1000.0;

/* Slots (including reply slots) one ACL payload of acl_bytes takes on air,
 * and the packet type used */
uint32_t airtime_slots(uint32_t acl_bytes, const char **type_name) {
    for (const auto &t : EDR3_TYPES) {
        if (acl_bytes <= t.max_bytes) {
            *type_name = t.name;
            return t.slots + 1;
        }
    }
    /* Split over several 3-DH5 packets */
    uint32_t n = (acl_bytes + LARGEST_TYPE.max_bytes - 1) / LARGEST_TYPE.max_bytes;
    *type_name = LARGEST_TYPE.name;
    return n * (LARGEST_TYPE.slots + 1);
}

wxString U(const char *key) { return wxString::FromUTF8(L(key)); }

int gauge_value(double percent) {
    if (percent < 0) return 0;
    if (percent > 100) return 100;
    return static_cast<int>(percent + 0.5);
}

} // namespace

void LinkQualityDialog::ShowFor(MainFrame *frame) {
    if (wxWindow *existing = wxWindow::FindWindowByName(DIALOG_NAME, frame)) {
        existing->Raise();
        return;
    }
    auto *dlg = new LinkQualityDialog(frame);
    dlg->Show();
}

LinkQualityDialog::Sample LinkQualityDialog::take_sample() {
    const LinkStats &s = link_stats();
    Sample r;
    r.tick_ms = GetTickCount64();
    r.packets = s.packets_sent.load(std::memory_order_relaxed);
    r.bytes = s.bytes_sent.load(std::memory_order_relaxed);
    r.dropped = s.queue_drops.load(std::memory_order_relaxed) +
                s.send_errors.load(std::memory_order_relaxed);
    r.capture_frames = s.capture_frames.load(std::memory_order_relaxed);
    r.capture_dropped = s.capture_dropped_frames.load(std::memory_order_relaxed);
    return r;
}

wxStaticText *LinkQualityDialog::add_heading(wxWindow *parent, wxSizer *sizer, const char *key) {
    auto *h = new wxStaticText(parent, wxID_ANY, U(key));
    auto font = h->GetFont();
    font.SetWeight(wxFONTWEIGHT_BOLD);
    h->SetFont(font);
    h->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    sizer->Add(h, 0, wxLEFT | wxRIGHT | wxTOP, 16);
    return h;
}

wxStaticText *LinkQualityDialog::add_row(wxWindow *parent, wxFlexGridSizer *grid, const char *label_key) {
    auto *label = new wxStaticText(parent, wxID_ANY, U(label_key));
    label->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
    grid->Add(label, 0, wxALIGN_CENTER_VERTICAL);
    auto *value = new wxStaticText(parent, wxID_ANY, "-");
    value->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    grid->Add(value, 0, wxALIGN_CENTER_VERTICAL);
    return value;
}

LinkQualityDialog::LinkQualityDialog(MainFrame *frame)
    : wxDialog(frame, wxID_ANY, U("quality.title"), wxDefaultPosition, wxDefaultSize,
               wxDEFAULT_DIALOG_STYLE, DIALOG_NAME)
    , frame_(frame)
    , timer_(this)
{
    auto *vbox = new wxBoxSizer(wxVERTICAL);

    idle_label_ = new wxStaticText(this, wxID_ANY, U("quality.not_streaming"));
    idle_label_->SetForegroundColour(TM().get(ThemeColor::StatusReconnecting));
    vbox->Add(idle_label_, 0, wxLEFT | wxRIGHT | wxTOP, 16);

    auto new_grid = [&]() {
        auto *g = new wxFlexGridSizer(2, 6, 16);
        g->AddGrowableCol(1, 1);
        return g;
    };

    add_heading(this, vbox, "quality.section_stream");
    auto *g_stream = new_grid();
    codec_value_ = add_row(this, g_stream, "quality.codec");
    vbox->Add(g_stream, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 8);

    add_heading(this, vbox, "quality.section_send");
    auto *g_send = new_grid();
    bitrate_value_ = add_row(this, g_send, "quality.bitrate");
    packets_value_ = add_row(this, g_send, "quality.packets");
    queue_value_ = add_row(this, g_send, "quality.queue");
    vbox->Add(g_send, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 8);

    add_heading(this, vbox, "quality.section_band");
    auto *g_band = new_grid();
    theory_value_ = add_row(this, g_band, "quality.theory");
    g_band->AddSpacer(0);
    theory_gauge_ = new wxGauge(this, wxID_ANY, 100, wxDefaultPosition, wxSize(-1, 10));
    g_band->Add(theory_gauge_, 0, wxEXPAND);
    airtime_value_ = add_row(this, g_band, "quality.airtime");
    g_band->AddSpacer(0);
    airtime_gauge_ = new wxGauge(this, wxID_ANY, 100, wxDefaultPosition, wxSize(-1, 10));
    g_band->Add(airtime_gauge_, 0, wxEXPAND);
    vbox->Add(g_band, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 8);
    auto *band_help = new wxStaticText(this, wxID_ANY, U("quality.band_help"));
    band_help->SetForegroundColour(TM().get(ThemeColor::TextMuted));
    band_help->Wrap(460);
    vbox->Add(band_help, 0, wxLEFT | wxRIGHT | wxTOP, 16);

    add_heading(this, vbox, "quality.section_loss");
    auto *g_loss = new_grid();
    sent_value_ = add_row(this, g_loss, "quality.sent_total");
    dropped_value_ = add_row(this, g_loss, "quality.dropped");
    capture_value_ = add_row(this, g_loss, "quality.capture_dropped");
    vbox->Add(g_loss, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 8);
    auto *loss_help = new wxStaticText(this, wxID_ANY, U("quality.loss_help"));
    loss_help->SetForegroundColour(TM().get(ThemeColor::TextMuted));
    loss_help->Wrap(460);
    vbox->Add(loss_help, 0, wxLEFT | wxRIGHT | wxTOP, 16);

    vbox->Add(new wxStaticLine(this), 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 8);
    auto *btn_sizer = new wxBoxSizer(wxHORIZONTAL);
    auto *reset_btn = new wxButton(this, wxID_ANY, U("quality.reset"));
    btn_sizer->Add(reset_btn, 0);
    btn_sizer->AddStretchSpacer();
    auto *close_btn = new wxButton(this, wxID_CLOSE, U("modal.close"));
    btn_sizer->Add(close_btn, 0);
    vbox->Add(btn_sizer, 0, wxEXPAND | wxALL, 12);

    reset_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) {
        baseline_ = take_sample();
        refresh();
    });
    close_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { Close(); });
    Bind(wxEVT_CLOSE_WINDOW, [this](wxCloseEvent &) {
        timer_.Stop();
        Destroy();
    });
    Bind(wxEVT_TIMER, &LinkQualityDialog::OnTimer, this);

    SetBackgroundColour(TM().get(ThemeColor::DialogBg));
    SetForegroundColour(TM().get(ThemeColor::TextPrimary));

    SetSizer(vbox);
    SetMinSize(wxSize(500, -1));

    baseline_ = take_sample();
    prev_ = baseline_;
    refresh();
    Fit();
    CentreOnParent();
    timer_.Start(1000);
}

void LinkQualityDialog::OnTimer(wxTimerEvent &) {
    refresh();
}

void LinkQualityDialog::refresh() {
    Sample now = take_sample();
    const A2dpService::StreamInfo &info = frame_->stream_info();
    bool streaming = frame_->state() == A2dpService::State::Streaming;

    /* ---- Stream ---- */
    if (streaming && !info.codec.empty()) {
        codec_value_->SetLabel(wxString::Format(U("quality.codec_value"),
            wxString::FromUTF8(info.codec), info.sample_rate / 1000.0,
            (unsigned)info.channels, (unsigned)info.bitrate_kbps));
    } else {
        codec_value_->SetLabel("-");
    }

    /* ---- Last second ---- */
    double dt = have_prev_ ? (now.tick_ms - prev_.tick_ms) / 1000.0 : 0.0;
    uint64_t d_packets = now.packets - prev_.packets;
    uint64_t d_bytes = now.bytes - prev_.bytes;
    bool have_rate = streaming && dt > 0.2 && d_packets > 0;
    bool resize = idle_label_->IsShown() != !streaming;
    idle_label_->Show(!streaming);

    if (have_rate) {
        double pps = d_packets / dt;
        double avg_bytes = static_cast<double>(d_bytes) / d_packets;
        double kbps = d_bytes * 8.0 / dt / 1000.0;
        bitrate_value_->SetLabel(wxString::Format(U("quality.bitrate_value"), kbps));
        packets_value_->SetLabel(wxString::Format(U("quality.packets_value"), pps, avg_bytes));

        /* Bandwidth: each L2CAP payload gets a 4-byte L2CAP header in the ACL packet */
        double acl_bytes = avg_bytes + L2CAP_HEADER_BYTES;
        double acl_kbps = pps * acl_bytes * 8.0 / 1000.0;
        double theory_pct = 100.0 * acl_kbps / THEORETICAL_MAX_KBPS;
        theory_value_->SetLabel(wxString::Format(U("quality.theory_value"),
                                                 THEORETICAL_MAX_KBPS, theory_pct));
        theory_gauge_->SetValue(gauge_value(theory_pct));

        const char *type_name = "";
        uint32_t slots = airtime_slots(static_cast<uint32_t>(acl_bytes + 0.5), &type_name);
        double airtime_pct = 100.0 * pps * slots * SLOT_SECONDS;
        airtime_value_->SetLabel(wxString::Format(U("quality.airtime_value"),
                                                  airtime_pct, type_name, (unsigned)slots));
        airtime_gauge_->SetValue(gauge_value(airtime_pct));
        airtime_value_->SetForegroundColour(TM().get(airtime_pct >= 80.0
            ? ThemeColor::StatusError : ThemeColor::TextPrimary));
    } else {
        bitrate_value_->SetLabel("-");
        packets_value_->SetLabel("-");
        theory_value_->SetLabel("-");
        theory_gauge_->SetValue(0);
        airtime_value_->SetLabel("-");
        airtime_value_->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
        airtime_gauge_->SetValue(0);
    }
    queue_value_->SetLabel(streaming
        ? wxString::Format(U("quality.queue_value"),
              (unsigned)link_stats().queue_depth.load(std::memory_order_relaxed),
              (unsigned)MEDIA_QUEUE_PACKETS)
        : wxString("-"));

    /* ---- Losses since opened / reset ---- */
    uint64_t sent = now.packets - baseline_.packets;
    uint64_t dropped = now.dropped - baseline_.dropped;
    uint64_t cap_frames = now.capture_frames - baseline_.capture_frames;
    uint64_t cap_dropped = now.capture_dropped - baseline_.capture_dropped;
    sent_value_->SetLabel(wxString::Format("%llu", (unsigned long long)sent));
    double drop_pct = (sent + dropped) ? 100.0 * dropped / (sent + dropped) : 0.0;
    dropped_value_->SetLabel(wxString::Format("%llu (%.2f%%)", (unsigned long long)dropped, drop_pct));
    dropped_value_->SetForegroundColour(TM().get(dropped ? ThemeColor::StatusError : ThemeColor::TextPrimary));
    double cap_ms = info.source_sample_rate ? 1000.0 * cap_dropped / info.source_sample_rate : 0.0;
    double cap_pct = cap_frames ? 100.0 * cap_dropped / cap_frames : 0.0;
    capture_value_->SetLabel(wxString::Format("%.0f ms (%.2f%%)", cap_ms, cap_pct));
    capture_value_->SetForegroundColour(TM().get(cap_dropped ? ThemeColor::StatusError : ThemeColor::TextPrimary));

    prev_ = now;
    have_prev_ = true;
    if (resize) Fit();
    else Layout();
}
