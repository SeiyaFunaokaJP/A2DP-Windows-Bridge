/*
 * Peer Receiver Test Dialog Implementation
 * SPDX-License-Identifier: MIT
 */

#include "wx_receiver_dialog.h"
#include "wx_main_frame.h"
#include "link_stats.h"
#include "localization.h"
#include "profile_manager.h"
#include "theme_manager.h"
#include "btstack_transport.h"
#include "wx_radio_text.h"

#include <wx/clipbrd.h>
#include <wx/datetime.h>
#include <wx/statline.h>

#include <windows.h>

namespace {

const char *DIALOG_NAME = "a2dpwb_receiver";

/* The receiver reports every second; older data means it stopped */
const uint64_t STATS_STALE_MS = 3000;

wxString U(const char *key) { return wxString::FromUTF8(L(key)); }

/* Codec choice order = ProfileManager::index_to_codec() */
const char *CODEC_LABELS[] = {nullptr /* codec.auto */, "LDAC", "aptX HD", "aptX LL", "aptX", "SBC", "AAC"};
const char *QUALITY_KEYS[] = {"quality.high", "quality.standard", "quality.mobile"};
/* Choice index -> ConnectionProfile values (0 = automatic) */
const uint32_t SAMPLE_RATES[] = {0, 44100, 48000, 88200, 96000};
const uint32_t BIT_DEPTHS[] = {0, 16, 24};
/* Receiver L2CAP MTU: 0 = as the receiver was started; 672 = BlueZ default,
 * 679 = smallest LDAC accepts, 895 / 1005 = common in headphones */
const uint32_t MTUS[] = {0, 672, 679, 895, 1005, 2048};
const uint32_t BUFFERS_MS[] = {0, 100, 200, 300, 500};

bool is_cjk(wxUniChar c) {
    unsigned v = c.GetValue();
    return (v >= 0x2E80 && v <= 0x9FFF) || (v >= 0xF900 && v <= 0xFAFF) || (v >= 0xFF00 && v <= 0xFFEF);
}

/* Characters a line should not start / end with (simple kinsoku) */
bool no_line_start(wxUniChar c) {
    static const wxString chars = wxString::FromUTF8("、。，．）」』】〕！？ー…・：；%)]}.,!?:;");
    return chars.Find(c) != wxNOT_FOUND;
}
bool no_line_end(wxUniChar c) {
    static const wxString chars = wxString::FromUTF8("（「『【〔([{");
    return chars.Find(c) != wxNOT_FOUND;
}

double pct(uint64_t part, uint64_t whole) { return whole ? 100.0 * part / whole : 0.0; }

const char *STEP_KEYS[] = {"receiver.step_discover", "receiver.step_stats", "receiver.step_configure",
                           "receiver.step_link", "receiver.step_codec", "receiver.step_audio"};
/* A step not confirmed within this long after it started is NG */
const uint64_t CONFIGURE_TIMEOUT_MS = 5000;
const uint64_t AUDIO_TIMEOUT_MS = 8000;
const size_t LOG_MAX_CHARS = 64 * 1024;

} // namespace

void ReceiverDialog::ShowFor(MainFrame *frame) {
    if (wxWindow *existing = wxWindow::FindWindowByName(DIALOG_NAME, frame)) {
        existing->Raise();
        return;
    }
    auto *dlg = new ReceiverDialog(frame);
    dlg->Show();
}

ReceiverDialog::Sample ReceiverDialog::take_sample() {
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

wxString ReceiverDialog::wrap(const wxString &text, int width) const {
    wxString out;
    wxArrayString paragraphs = wxSplit(text, '\n', '\0');
    for (size_t p = 0; p < paragraphs.size(); p++) {
        if (p) out += '\n';
        const wxString &para = paragraphs[p];
        wxString line;
        int break_at = -1;  /* index in line after which it may break */
        for (size_t i = 0; i < para.length(); i++) {
            wxUniChar c = para[i];
            if (!line.empty()) {
                wxUniChar prev = line.Last();
                bool opportunity = prev == ' ' ||
                    ((is_cjk(prev) || is_cjk(c)) && !no_line_start(c) && !no_line_end(prev) && c != ' ');
                if (opportunity) break_at = static_cast<int>(line.length());
            }
            line += c;
            if (GetTextExtent(line).GetWidth() > width && break_at > 0) {
                wxString head = line.Left(break_at);
                head.Trim();
                out += head + '\n';
                line = line.Mid(break_at);
                line.Trim(false);
                break_at = -1;
            }
        }
        out += line;
    }
    return out;
}

wxChoice *ReceiverDialog::add_choice(wxSizer *grid, const char *label_key, const wxArrayString &items,
                                     int selection) {
    auto *label = new wxStaticText(this, wxID_ANY, U(label_key));
    label->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
    grid->Add(label, 0, wxALIGN_CENTER_VERTICAL | wxALIGN_RIGHT | wxLEFT, 8);
    auto *choice = new wxChoice(this, wxID_ANY, wxDefaultPosition, wxDefaultSize, items);
    choice->SetSelection(selection);
    grid->Add(choice, 0, wxALIGN_CENTER_VERTICAL | wxEXPAND);
    return choice;
}

wxStaticText *ReceiverDialog::value_text() {
    /* Fixed width, long values end in "..." (full text in the tooltip), so
     * the window never grows sideways */
    auto *t = new wxStaticText(this, wxID_ANY, "-", wxDefaultPosition, wxSize(FromDIP(250), -1),
                               wxST_NO_AUTORESIZE | wxST_ELLIPSIZE_END);
    t->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    return t;
}

void ReceiverDialog::add_row(wxFlexGridSizer *grid, const char *label_key,
                             wxStaticText **sent, wxStaticText **received) {
    auto *label = new wxStaticText(this, wxID_ANY, U(label_key));
    label->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
    grid->Add(label, 0, wxALIGN_CENTER_VERTICAL);
    *sent = value_text();
    grid->Add(*sent, 0, wxALIGN_CENTER_VERTICAL);
    *received = value_text();
    grid->Add(*received, 0, wxALIGN_CENTER_VERTICAL);
}

void ReceiverDialog::set_value(wxStaticText *t, const wxString &text, bool bad) {
    if (t->GetLabel() != text) {
        t->SetLabel(text);
        t->SetToolTip(text);
    }
    t->SetForegroundColour(TM().get(bad ? ThemeColor::StatusError : ThemeColor::TextPrimary));
}

ReceiverDialog::ReceiverDialog(MainFrame *frame)
    : wxDialog(frame, wxID_ANY, U("receiver.title"), wxDefaultPosition, wxDefaultSize,
               wxDEFAULT_DIALOG_STYLE, DIALOG_NAME)
    , frame_(frame)
    , timer_(this)
{
    const int wrap_width = FromDIP(640);
    auto *vbox = new wxBoxSizer(wxVERTICAL);

    auto *intro = new wxStaticText(this, wxID_ANY, wrap(U("receiver.intro"), wrap_width));
    intro->SetForegroundColour(TM().get(ThemeColor::TextMuted));
    vbox->Add(intro, 0, wxLEFT | wxRIGHT | wxTOP, 16);

    /* ---- Receiver: address (empty = find on the local network) ---- */
    auto *addr_row = new wxBoxSizer(wxHORIZONTAL);
    auto *addr_label = new wxStaticText(this, wxID_ANY, U("receiver.address"));
    addr_label->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
    addr_row->Add(addr_label, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    std::string saved = frame_->settings().remote_sink;
    addr_ = new wxTextCtrl(this, wxID_ANY, saved == REMOTE_SINK_AUTO ? "" : wxString::FromUTF8(saved),
                           wxDefaultPosition, wxDefaultSize, wxTE_PROCESS_ENTER);
    addr_->SetHint(U("receiver.address_hint"));
    addr_->SetToolTip(U("receiver.address_tooltip"));
    addr_row->Add(addr_, 1, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    connect_btn_ = new wxButton(this, wxID_ANY, U("receiver.connect"));
    addr_row->Add(connect_btn_, 0, wxALIGN_CENTER_VERTICAL);
    vbox->Add(addr_row, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 16);

    status_ = new wxStaticText(this, wxID_ANY, "");
    status_->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    vbox->Add(status_, 0, wxLEFT | wxRIGHT | wxTOP, 12);

    /* ---- Measurement settings: what A2DPWB sends, how the receiver takes it ---- */
    wxArrayString codecs, qualities, rates, depths, mtus, buffers;
    for (const char *c : CODEC_LABELS) codecs.Add(c ? wxString(c) : U("codec.auto"));
    for (const char *k : QUALITY_KEYS) qualities.Add(U(k));
    for (uint32_t r : SAMPLE_RATES) rates.Add(r ? wxString::Format("%.1f kHz", r / 1000.0) : U("codec.auto"));
    for (uint32_t d : BIT_DEPTHS) depths.Add(d ? wxString::Format("%u bit", d) : U("codec.auto"));
    for (uint32_t m : MTUS) mtus.Add(m ? wxString::Format("%u", m) : U("receiver.as_started"));
    for (uint32_t b : BUFFERS_MS) buffers.Add(b ? wxString::Format("%u ms", b) : U("receiver.as_started"));

    /* Two aligned rows: head | label choice | label choice | ... */
    auto *settings = new wxFlexGridSizer(9, 8, 6);
    auto add_head = [&](const char *key) {
        auto *h = new wxStaticText(this, wxID_ANY, U(key));
        auto font = h->GetFont();
        font.SetWeight(wxFONTWEIGHT_BOLD);
        h->SetFont(font);
        h->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
        settings->Add(h, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 6);
    };
    add_head("receiver.settings_sent");
    codec_ = add_choice(settings, "receiver.codec", codecs, 0);
    quality_ = add_choice(settings, "receiver.quality", qualities, 0);
    rate_ = add_choice(settings, "receiver.sample_rate", rates, 0);
    depth_ = add_choice(settings, "receiver.bit_depth", depths, 0);
    /* Audio source under the codec: the test tone keeps the stream steady */
    settings->AddSpacer(0);
    wxArrayString sources;
    sources.Add(U("receiver.source_tone"));
    sources.Add(U("receiver.source_system"));
    source_ = add_choice(settings, "receiver.source", sources, 0);
    source_->SetToolTip(U("receiver.source_tooltip"));
    /* AFH: auto (Wi-Fi heard by this PC), off, or Wi-Fi channels to avoid */
    wxArrayString afh_modes;
    afh_modes.Add(U("receiver.afh_auto"));
    afh_modes.Add(U("receiver.afh_off"));
    afh_modes.Add(U("receiver.afh_manual"));
    std::string afh_setting = frame_->settings().afh;
    int afh_sel = afh_setting == "auto" ? 0 : (afh_setting == "off" || afh_setting.empty()) ? 1 : 2;
    afh_ = add_choice(settings, "receiver.afh", afh_modes, afh_sel);
    afh_->SetToolTip(U("receiver.afh_tooltip"));
    auto *afh_ch_label = new wxStaticText(this, wxID_ANY, U("receiver.afh_channels"));
    afh_ch_label->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
    settings->Add(afh_ch_label, 0, wxALIGN_CENTER_VERTICAL | wxALIGN_RIGHT | wxLEFT, 8);
    afh_channels_ = new wxTextCtrl(this, wxID_ANY, afh_sel == 2 ? wxString::FromUTF8(afh_setting) : wxString(),
                                   wxDefaultPosition, wxSize(FromDIP(70), -1));
    afh_channels_->SetHint("6,11");
    afh_channels_->Enable(afh_sel == 2);
    settings->Add(afh_channels_, 0, wxALIGN_CENTER_VERTICAL);
    for (int i = 0; i < 2; i++) settings->AddSpacer(0);
    afh_->Bind(wxEVT_CHOICE, [this](wxCommandEvent &) {
        afh_channels_->Enable(afh_->GetSelection() == 2);
        apply_afh();
    });
    afh_channels_->Bind(wxEVT_TEXT, [this](wxCommandEvent &) { apply_afh(); });
    add_head("receiver.settings_received");
    mtu_ = add_choice(settings, "receiver.mtu", mtus, 0);
    mtu_->SetToolTip(U("receiver.mtu_tooltip"));
    buffer_ = add_choice(settings, "receiver.buffer", buffers, 0);
    vbox->Add(settings, 0, wxLEFT | wxRIGHT | wxTOP, 16);

    /* ---- Steps: OK / NG at a glance, then Start ---- */
    auto *start_row = new wxBoxSizer(wxHORIZONTAL);
    auto *steps = new wxFlexGridSizer(3, 4, 18);
    for (int i = 0; i < STEP_COUNT; i++) {
        step_text_[i] = new wxStaticText(this, wxID_ANY, "");
        steps->Add(step_text_[i], 0, wxALIGN_CENTER_VERTICAL);
        set_step(static_cast<Step>(i), StepState::Pending);
    }
    start_row->Add(steps, 0, wxALIGN_CENTER_VERTICAL);
    start_row->AddStretchSpacer();
    start_btn_ = new wxButton(this, wxID_ANY, U("receiver.start"));
    start_row->Add(start_btn_, 0, wxALIGN_CENTER_VERTICAL);
    vbox->Add(start_row, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 12);

    /* ---- Comparison: sent | received ---- */
    vbox->Add(new wxStaticLine(this), 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 16);
    auto *grid = new wxFlexGridSizer(3, 6, 16);
    grid->AddSpacer(0);
    for (const char *key : {"receiver.col_sent", "receiver.col_received"}) {
        auto *h = new wxStaticText(this, wxID_ANY, U(key));
        auto font = h->GetFont();
        font.SetWeight(wxFONTWEIGHT_BOLD);
        h->SetFont(font);
        h->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
        /* What the numbers mean, on hover: keeps the window within the screen */
        h->SetToolTip(U("receiver.help"));
        grid->Add(h, 0);
    }
    add_row(grid, "receiver.row_codec", &codec_s_, &codec_r_);
    add_row(grid, "receiver.row_bitrate", &bitrate_s_, &bitrate_r_);
    add_row(grid, "receiver.row_total", &total_s_, &total_r_);
    add_row(grid, "receiver.row_loss", &loss_s_, &loss_r_);
    add_row(grid, "receiver.row_timing", &timing_s_, &timing_r_);
    add_row(grid, "receiver.row_buffer", &buffer_s_, &buffer_r_);
    add_row(grid, "receiver.row_audio", &audio_s_, &audio_r_);
    add_row(grid, "receiver.row_errors", &errors_s_, &errors_r_);
    add_row(grid, "receiver.row_signal", &signal_s_, &signal_r_);
    vbox->Add(grid, 0, wxLEFT | wxRIGHT | wxTOP, 16);

    /* ---- Log of what happened ---- */
    auto *log_head = new wxStaticText(this, wxID_ANY, U("receiver.log"));
    log_head->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
    vbox->Add(log_head, 0, wxLEFT | wxRIGHT | wxTOP, 16);
    log_ = new wxTextCtrl(this, wxID_ANY, "", wxDefaultPosition, wxSize(wrap_width, FromDIP(96)),
                          wxTE_MULTILINE | wxTE_READONLY | wxTE_RICH2);
    log_->SetBackgroundColour(TM().get(ThemeColor::PanelBg));
    log_->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    vbox->Add(log_, 0, wxLEFT | wxRIGHT | wxTOP, 16);


    vbox->Add(new wxStaticLine(this), 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 12);
    auto *btn_sizer = new wxBoxSizer(wxHORIZONTAL);
    auto *reset_btn = new wxButton(this, wxID_ANY, U("quality.reset"));
    btn_sizer->Add(reset_btn, 0, wxRIGHT, 8);
    auto *copy_btn = new wxButton(this, wxID_ANY, U("receiver.copy_log"));
    btn_sizer->Add(copy_btn, 0);
    btn_sizer->AddStretchSpacer();
    auto *close_btn = new wxButton(this, wxID_CLOSE, U("modal.close"));
    btn_sizer->Add(close_btn, 0);
    vbox->Add(btn_sizer, 0, wxEXPAND | wxALL, 12);

    connect_btn_->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { toggle_connection(); });
    addr_->Bind(wxEVT_TEXT_ENTER, [this](wxCommandEvent &) {
        RemoteSinkClient::instance().stop();
        toggle_connection();
    });
    start_btn_->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { start_stop(); });
    reset_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { reset(); });
    copy_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) {
        if (wxTheClipboard->Open()) {
            wxTheClipboard->SetData(new wxTextDataObject(log_->GetValue()));
            wxTheClipboard->Close();
        }
    });
    close_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { Close(); });
    Bind(wxEVT_CLOSE_WINDOW, [this](wxCloseEvent &) {
        timer_.Stop();
        Destroy();
    });
    Bind(wxEVT_TIMER, [this](wxTimerEvent &) { refresh(); });

    SetBackgroundColour(TM().get(ThemeColor::DialogBg));
    SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    SetSizer(vbox);

    /* Find the receiver right away (address saved last time, else the local network) */
    if (RemoteSinkClient::instance().status() == RemoteSinkClient::Status::Off)
        RemoteSinkClient::instance().start(saved.empty() ? std::string(REMOTE_SINK_AUTO) : saved);

    last_state_ = frame_->state();
    baseline_ = take_sample();
    prev_ = baseline_;
    refresh();
    Fit();
    CentreOnParent();
    timer_.Start(1000);
}

void ReceiverDialog::apply_afh() {
    std::string policy = afh_->GetSelection() == 0 ? "auto" : afh_->GetSelection() == 1 ? "off"
                       : afh_channels_->GetValue().Trim().Trim(false).utf8_string();
    if (policy.empty()) policy = "off";
    BtStackTransport::set_afh_policy(policy);
    if (frame_->settings().afh != policy) {
        frame_->settings().afh = policy;
        frame_->settings().save();
    }
}

void ReceiverDialog::toggle_connection() {
    RemoteSinkClient &client = RemoteSinkClient::instance();
    if (client.status() != RemoteSinkClient::Status::Off) {
        client.stop();
    } else {
        std::string target = addr_->GetValue().Trim().Trim(false).utf8_string();
        if (target.empty()) target = REMOTE_SINK_AUTO;
        client.start(target);
        have_remote_base_ = false;
        if (frame_->settings().remote_sink != target) {
            frame_->settings().remote_sink = target;
            frame_->settings().save();
        }
    }
    refresh();
}

void ReceiverDialog::start_stop() {
    if (measuring_) {
        log(U("receiver.log_stopped"));
        frame_->service().stop_streaming();
        measuring_ = false;
        refresh();
        return;
    }
    RemoteSinkInfo info;
    if (!RemoteSinkClient::instance().receiver(&info) || info.bt_address.empty()) return;
    ConnectionProfile p;
    p.name = L("receiver.profile_name");
    p.device_address = info.bt_address;
    p.device_name = info.bt_name;
    p.codec = ProfileManager::index_to_codec(codec_->GetSelection());
    p.quality = ProfileManager::index_to_quality(quality_->GetSelection());
    p.sample_rate = SAMPLE_RATES[rate_->GetSelection()];
    p.bit_depth = BIT_DEPTHS[depth_->GetSelection()];
    p.capture_mode = "loopback";
    p.test_tone = source_->GetSelection() == 0;
    /* Receiver side first: it applies the settings to the connection that follows */
    reset_measurement_steps();
    requested_mtu_ = MTUS[mtu_->GetSelection()];
    requested_buffer_ms_ = BUFFERS_MS[buffer_->GetSelection()];
    RemoteSinkStats s;
    had_stats_at_start_ = RemoteSinkClient::instance().latest(&s);
    stream_id_at_start_ = had_stats_at_start_ && s.has_stream ? s.stream_id : 0;
    measure_start_ms_ = GetTickCount64();
    log(wxString::Format(U("receiver.log_start"), codec_->GetStringSelection(), quality_->GetStringSelection(),
                         rate_->GetStringSelection(), depth_->GetStringSelection(), source_->GetStringSelection(),
                         wxString::FromUTF8(info.bt_address), mtu_->GetStringSelection(),
                         buffer_->GetStringSelection()));
    set_step(StepConfigure, StepState::Running);
    RemoteSinkClient::instance().configure(requested_mtu_, requested_buffer_ms_);
    set_step(StepLink, StepState::Running);
    service_left_idle_ = false;
    frame_->start_stream(p);
    measuring_ = true;
    failure_.clear();
    refresh();
}

void ReceiverDialog::reset() {
    baseline_ = take_sample();
    have_remote_base_ = RemoteSinkClient::instance().latest(&remote_base_) && remote_base_.has_stream;
    refresh();
}

void ReceiverDialog::refresh() {
    Sample now = take_sample();
    A2dpService::State state = frame_->state();
    bool streaming = state == A2dpService::State::Streaming;

    /* A new stream starts the counts from zero on both sides */
    if (streaming && last_state_ != A2dpService::State::Streaming) {
        baseline_ = now;
        prev_ = now;
        have_prev_ = false;
    }
    if (measuring_ && state != A2dpService::State::Idle)
        service_left_idle_ = true;
    if (measuring_ && (state == A2dpService::State::Error ||
                       (state == A2dpService::State::Idle && service_left_idle_))) {
        measuring_ = false;
        if (state == A2dpService::State::Error)
            failure_ = wxString::FromUTF8(frame_->status_text());
    }
    last_state_ = state;

    refresh_steps();
    refresh_status();
    refresh_sent(streaming, now);
    refresh_received();

    prev_ = now;
    have_prev_ = true;
    Layout();
    Fit();
}

void ReceiverDialog::log(const wxString &line) {
    wxString text = wxDateTime::Now().Format("%H:%M:%S ") + line + "\n";
    if (log_->GetLastPosition() > static_cast<long>(LOG_MAX_CHARS))
        log_->Remove(0, log_->GetLastPosition() / 2);
    log_->AppendText(text);
}

void ReceiverDialog::set_step(Step step, StepState state, const wxString &detail) {
    static const wchar_t *MARKS[] = {L"\u25CB", L"\u2026", L"\u2714", L"\u2716"};  /* ○ … ✔ ✖ */
    static const ThemeColor COLORS[] = {ThemeColor::TextMuted, ThemeColor::StatusConnecting,
                                        ThemeColor::StatusStreaming, ThemeColor::StatusError};
    bool changed = step_state_[step] != state;
    step_state_[step] = state;
    int i = static_cast<int>(state);
    step_text_[step]->SetLabel(wxString(MARKS[i]) + " " + U(STEP_KEYS[step]));
    step_text_[step]->SetForegroundColour(TM().get(COLORS[i]));
    step_text_[step]->SetToolTip(detail);
    if (changed && log_ && (state == StepState::Ok || state == StepState::Failed))
        log(wxString::Format("%s %s%s", state == StepState::Ok ? "OK" : "NG", U(STEP_KEYS[step]),
                             detail.empty() ? wxString() : ": " + detail));
}

void ReceiverDialog::reset_measurement_steps() {
    for (Step s : {StepConfigure, StepLink, StepCodec, StepAudio})
        set_step(s, StepState::Pending);
    link_ok_ms_ = 0;
    last_service_text_.clear();
}

void ReceiverDialog::refresh_steps() {
    RemoteSinkClient &client = RemoteSinkClient::instance();
    std::string detail;
    RemoteSinkClient::Status status = client.status(&detail);
    RemoteSinkInfo info;
    bool have_info = client.receiver(&info);
    RemoteSinkStats s;
    bool have_stats = client.latest(&s) && GetTickCount64() - s.received_tick < STATS_STALE_MS;
    bool automatic = client.target() == REMOTE_SINK_AUTO;

    /* 1-2: finding the receiver and its statistics */
    switch (status) {
    case RemoteSinkClient::Status::Off:
        set_step(StepDiscover, StepState::Pending);
        set_step(StepStats, StepState::Pending);
        break;
    case RemoteSinkClient::Status::Connecting:
        if (!have_info) set_step(StepDiscover, automatic ? StepState::Running : StepState::Pending);
        set_step(StepStats, StepState::Running);
        break;
    case RemoteSinkClient::Status::Failed:
        if (!have_info && automatic)
            set_step(StepDiscover, StepState::Failed, wxString::FromUTF8(detail));
        else
            set_step(StepStats, StepState::Failed, wxString::FromUTF8(detail));
        break;
    case RemoteSinkClient::Status::Connected:
        if (have_info)
            set_step(StepDiscover, StepState::Ok, wxString::Format("%s (%s), Bluetooth %s",
                     wxString::FromUTF8(info.host), wxString::FromUTF8(info.ip), wxString::FromUTF8(info.bt_address)));
        if (have_stats) set_step(StepStats, StepState::Ok);
        break;
    }

    if (measure_start_ms_ == 0) return;
    uint64_t now = GetTickCount64();

    /* 3: the receiver took the settings (it reports them in its statistics) */
    if (step_state_[StepConfigure] == StepState::Running) {
        bool mtu_ok = requested_mtu_ == 0 || (have_stats && s.mtu == requested_mtu_);
        bool buffer_ok = requested_buffer_ms_ == 0 || (have_stats && s.buffer_target_ms == requested_buffer_ms_);
        if (have_stats && mtu_ok && buffer_ok)
            set_step(StepConfigure, StepState::Ok, wxString::Format("MTU %u, %u ms", s.mtu, s.buffer_target_ms));
        else if (now - measure_start_ms_ > CONFIGURE_TIMEOUT_MS)
            set_step(StepConfigure, StepState::Failed, U("receiver.log_configure_failed"));
    }

    /* 4: A2DPWB's own connection, with its status messages in the log */
    A2dpService::State state = frame_->state();
    const std::string &service_text = frame_->status_text();
    if (measuring_ && service_text != last_service_text_) {
        last_service_text_ = service_text;
        log("A2DPWB: " + wxString::FromUTF8(service_text));
    }
    if (step_state_[StepLink] == StepState::Running) {
        if (state == A2dpService::State::Streaming) {
            set_step(StepLink, StepState::Ok);
            link_ok_ms_ = now;
            set_step(StepCodec, StepState::Running);
            set_step(StepAudio, StepState::Running);
        } else if (state == A2dpService::State::Error ||
                   (!measuring_ && service_left_idle_ && state == A2dpService::State::Idle)) {
            set_step(StepLink, StepState::Failed, wxString::FromUTF8(service_text));
        }
    } else if (step_state_[StepLink] == StepState::Ok && state == A2dpService::State::Reconnecting) {
        set_step(StepLink, StepState::Running);
        log(U("receiver.log_link_lost"));
    }

    /* 5-6: what the receiver sees of it */
    bool new_stream = have_stats && s.has_stream && (s.stream_id != stream_id_at_start_ || !had_stats_at_start_);
    if (step_state_[StepCodec] == StepState::Running && new_stream)
        set_step(StepCodec, StepState::Ok, wxString::FromUTF8(s.config));
    if (step_state_[StepAudio] == StepState::Running) {
        if (new_stream && s.streaming && s.interval_packets > 0)
            set_step(StepAudio, StepState::Ok, wxString::Format(U("receiver.bitrate_value"),
                     s.interval_bytes * 8.0 / 1000.0, (double)s.interval_packets));
        else if (link_ok_ms_ && now - link_ok_ms_ > AUDIO_TIMEOUT_MS)
            set_step(StepAudio, StepState::Failed, U("receiver.log_no_audio"));
    }
    if (step_state_[StepCodec] == StepState::Running && link_ok_ms_ && now - link_ok_ms_ > AUDIO_TIMEOUT_MS)
        set_step(StepCodec, StepState::Failed, U("receiver.log_no_audio"));
}

void ReceiverDialog::refresh_status() {
    RemoteSinkClient &client = RemoteSinkClient::instance();
    std::string detail;
    RemoteSinkClient::Status status = client.status(&detail);
    bool off = status == RemoteSinkClient::Status::Off;
    connect_btn_->SetLabel(U(off ? "receiver.connect" : "receiver.disconnect"));
    addr_->Enable(off);

    RemoteSinkInfo info;
    bool have_info = !off && client.receiver(&info);
    RemoteSinkStats s;
    bool have_stats = client.latest(&s) && GetTickCount64() - s.received_tick < STATS_STALE_MS;

    wxString text;
    bool bad = false;
    if (have_info)
        text = wxString::Format(U("receiver.found"), wxString::FromUTF8(info.host), wxString::FromUTF8(info.ip),
                                wxString::FromUTF8(info.bt_address), (unsigned)info.mtu) + "\n";
    switch (status) {
    case RemoteSinkClient::Status::Off:
        text += U("receiver.off");
        break;
    case RemoteSinkClient::Status::Connecting:
        text += U(client.target() == REMOTE_SINK_AUTO ? "receiver.searching" : "receiver.connecting");
        break;
    case RemoteSinkClient::Status::Failed:
        text += wxString::Format(U("receiver.failed"), wxString::FromUTF8(detail));
        bad = true;
        break;
    case RemoteSinkClient::Status::Connected: {
        A2dpService::State state = frame_->state();
        if (have_stats && s.streaming)
            text += U("receiver.state_receiving");
        else if (measuring_ && (state == A2dpService::State::Connecting ||
                                state == A2dpService::State::Reconnecting))
            text += U("receiver.state_connecting");
        else if (measuring_)
            text += U("receiver.state_waiting");
        else if (!failure_.empty()) {
            text += wxString::Format(U("receiver.state_failed"), failure_);
            bad = true;
        } else
            text += U("receiver.state_ready");
        break;
    }
    }
    if (text != status_text_) {
        status_text_ = text;
        status_->SetLabel(wrap(text, FromDIP(640)));
    }
    status_->SetForegroundColour(TM().get(bad ? ThemeColor::StatusError : ThemeColor::TextPrimary));

    start_btn_->SetLabel(U(measuring_ ? "receiver.stop" : "receiver.start"));
    start_btn_->Enable(measuring_ || (have_info && !info.bt_address.empty()));
    for (wxChoice *c : {codec_, quality_, rate_, depth_, source_, mtu_, buffer_})
        c->Enable(!measuring_);
}

void ReceiverDialog::refresh_sent(bool streaming, const Sample &now) {
    const A2dpService::StreamInfo &info = frame_->stream_info();
    if (!streaming || info.codec.empty()) {
        for (auto *t : {codec_s_, bitrate_s_, total_s_, loss_s_, timing_s_, buffer_s_, audio_s_,
                        errors_s_, signal_s_})
            set_value(t, "-");
        return;
    }
    set_value(codec_s_, wxString::Format("%s, %.1f kHz", wxString::FromUTF8(info.codec),
                                         info.sample_rate / 1000.0));
    double dt = have_prev_ ? (now.tick_ms - prev_.tick_ms) / 1000.0 : 0.0;
    uint64_t d_packets = now.packets - prev_.packets;
    if (dt > 0.2 && d_packets > 0)
        set_value(bitrate_s_, wxString::Format(U("receiver.bitrate_value"),
                                               (now.bytes - prev_.bytes) * 8.0 / dt / 1000.0, d_packets / dt));
    else
        set_value(bitrate_s_, "-");
    uint64_t sent = now.packets - baseline_.packets;
    uint64_t dropped = now.dropped - baseline_.dropped;
    set_value(total_s_, wxString::Format("%llu", (unsigned long long)sent));
    set_value(loss_s_, wxString::Format(U("receiver.dropped_value"), (unsigned long long)dropped,
                                        pct(dropped, sent + dropped)), dropped > 0);
    set_value(timing_s_, "-");
    set_value(buffer_s_, wxString::Format(U("receiver.queue_value"),
        (unsigned)link_stats().queue_depth.load(std::memory_order_relaxed), (unsigned)MEDIA_QUEUE_PACKETS));
    uint64_t cap_dropped = now.capture_dropped - baseline_.capture_dropped;
    double cap_ms = info.source_sample_rate ? 1000.0 * cap_dropped / info.source_sample_rate : 0.0;
    set_value(audio_s_, wxString::Format(U("receiver.capture_value"), cap_ms), cap_dropped > 0);
    set_value(errors_s_, "-");
    set_value(signal_s_, radio_text(link_radio()));
}

void ReceiverDialog::refresh_received() {
    RemoteSinkStats s;
    bool have = RemoteSinkClient::instance().latest(&s) && GetTickCount64() - s.received_tick < STATS_STALE_MS;
    if (!have || !s.has_stream) {
        for (auto *t : {codec_r_, bitrate_r_, total_r_, loss_r_, timing_r_, buffer_r_, audio_r_, errors_r_})
            set_value(t, "-");
        set_value(signal_r_, have && s.connected && s.has_rssi
                                 ? wxString::Format(U("receiver.rssi_value"), s.rssi) : wxString("-"));
        return;
    }
    if (!have_remote_base_ || remote_base_.stream_id != s.stream_id) {
        remote_base_ = RemoteSinkStats{};
        remote_base_.stream_id = s.stream_id;
        have_remote_base_ = true;
    }
    const RemoteSinkStats &b = remote_base_;
    uint64_t packets = s.packets - b.packets;
    uint64_t lost = s.lost - b.lost;
    uint64_t late = s.late - b.late;
    uint64_t underruns = s.underruns - b.underruns;
    uint64_t overflows = s.overflows - b.overflows;
    uint64_t pauses = s.pauses - b.pauses;
    uint64_t errors = (s.frame_errors - b.frame_errors) + (s.decode_errors - b.decode_errors) +
                      (s.ts_errors - b.ts_errors);

    set_value(codec_r_, wxString::FromUTF8(s.config));
    if (s.streaming && s.interval_packets > 0) {
        set_value(bitrate_r_, wxString::Format(U("receiver.bitrate_value"), s.interval_bytes * 8.0 / 1000.0,
                                               (double)s.interval_packets));
        set_value(timing_r_, wxString::Format(U("receiver.timing_value"), s.jitter_ms, s.interval_max_gap_ms));
    } else {
        set_value(bitrate_r_, "-");
        set_value(timing_r_, "-");
    }
    set_value(total_r_, wxString::Format("%llu", (unsigned long long)packets));
    set_value(loss_r_, wxString::Format(U("receiver.lost_value"), (unsigned long long)lost,
                                        pct(lost, packets + lost), (unsigned long long)late), lost || late);
    set_value(buffer_r_, s.buffer_ms < 0 ? wxString("-")
                                         : wxString::Format(U("receiver.buffer_value"), s.buffer_ms,
                                                            (unsigned)s.buffer_target_ms));
    wxString audio = wxString::Format(U("receiver.dropouts_value"), (unsigned long long)underruns,
                                      s.underrun_ms - b.underrun_ms, (unsigned long long)overflows);
    if (pauses) audio += wxString::Format(U("receiver.pauses_value"), (unsigned long long)pauses);
    if (s.idle) audio = U("receiver.idle_value") + audio;
    set_value(audio_r_, audio, underruns || overflows);
    set_value(errors_r_, wxString::Format(U(s.decoding ? "receiver.errors_value" : "receiver.errors_value_headers"),
                                          (unsigned long long)errors), errors > 0);
    set_value(signal_r_, s.has_rssi || s.afh_channels >= 0
                             ? radio_text(s.has_rssi, s.rssi, s.afh_channels, std::string())
                             : wxString::FromUTF8(s.rssi_note.empty() ? "-" : s.rssi_note));
}
