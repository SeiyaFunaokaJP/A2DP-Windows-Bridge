/*
 * Debug Console Dialog Implementation
 * SPDX-License-Identifier: MIT
 */

#include "wx_debug_console_dialog.h"
#include "wx_main_frame.h"
#include "hci_capture.h"
#include "link_stats.h"
#include "localization.h"
#include "theme_manager.h"

#include <wx/clipbrd.h>
#include <wx/datetime.h>
#include <wx/statline.h>
#include <wx/utils.h>

#include <algorithm>

#include <shellapi.h>
#include <windows.h>

#ifndef APP_VERSION
#define APP_VERSION "1.0.1"
#endif

namespace {

const char *DIALOG_NAME = "a2dpwb_debug_console";
const int REFRESH_MS = 500;
/* Diagnostics: how much of the current connection attempt's log to include */
const size_t DIAG_ISSUES = 40;
const size_t DIAG_LINES = 200;

enum LogColumn { COL_TIME, COL_LEVEL, COL_MESSAGE };
enum FlowColumn { FCOL_STEP, FCOL_STATUS, FCOL_TIME, FCOL_ELAPSED, FCOL_DETAIL };

using Level = DebugLogModel::Level;
using StepStatus = DebugLogModel::StepStatus;

wxString U(const char *key) { return wxString::FromUTF8(L(key)); }

/* Level choice index -> lowest level shown */
const Level CHOICE_LEVELS[] = {Level::Debug, Level::Info, Level::Warn, Level::Error};

const char *status_key(StepStatus s) {
    switch (s) {
    case StepStatus::Pending: return "dbgc.status.pending";
    case StepStatus::Running: return "dbgc.status.running";
    case StepStatus::Done:    return "dbgc.status.done";
    case StepStatus::Warning: return "dbgc.status.warning";
    case StepStatus::Failed:  return "dbgc.status.failed";
    }
    return "";
}

/* Plain-text marker for copied flows (no localization, fixed width) */
const char *status_tag(StepStatus s) {
    switch (s) {
    case StepStatus::Pending: return "[ -- ]";
    case StepStatus::Running: return "[ .. ]";
    case StepStatus::Done:    return "[ OK ]";
    case StepStatus::Warning: return "[WARN]";
    case StepStatus::Failed:  return "[FAIL]";
    }
    return "";
}

wxColour status_colour(StepStatus s) {
    switch (s) {
    case StepStatus::Pending: return TM().get(ThemeColor::TextMuted);
    case StepStatus::Running: return TM().get(ThemeColor::StatusConnecting);
    case StepStatus::Done:    return TM().get(ThemeColor::StatusStreaming);
    case StepStatus::Warning: return TM().get(ThemeColor::StatusReconnecting);
    case StepStatus::Failed:  return TM().get(ThemeColor::StatusError);
    }
    return TM().get(ThemeColor::CtrlFg);
}

/* "hh:mm:ss.mmm" -> ms since midnight, -1 if not a time */
long parse_ms(const wxString &t) {
    unsigned h, m, s, ms;
    if (t.length() != 12 || sscanf(t.utf8_str().data(), "%u:%u:%u.%u", &h, &m, &s, &ms) != 4)
        return -1;
    return static_cast<long>(((h * 60 + m) * 60 + s) * 1000 + ms);
}

wxString elapsed_text(const wxString &from, const wxString &to) {
    long a = parse_ms(from), b = parse_ms(to);
    if (a < 0 || b < 0) return wxString();
    const long day = 24L * 3600 * 1000;
    long d = b - a;
    if (d < -day / 2) d += day; /* across midnight */
    /* Plain stderr lines are stamped when read (up to one poll late), so a
     * LOG_* line can look slightly older than the line before it */
    if (d < 0 && d > -1000) d = 0;
    if (d < 0) return wxString();
    return wxString::Format("+%.3f s", d / 1000.0);
}

const char *state_name(int s) {
    switch (s) {
    case 0: return "Idle";
    case 1: return "Connecting";
    case 2: return "Streaming";
    case 3: return "Reconnecting";
    case 4: return "Error";
    }
    return "?";
}

void set_cell(wxListCtrl *list, long row, int col, const wxString &text) {
    if (list->GetItemText(row, col) != text) list->SetItem(row, col, text);
}

long next_selected(const wxListCtrl *list, long after) {
    return list->GetNextItem(after, wxLIST_NEXT_ALL, wxLIST_STATE_SELECTED);
}

/* "AA:BB:CC:DD:EE:FF" -> "AA:BB:CC:xx:xx:xx": keeps the vendor part (OUI) */
void mask_bd_addresses(wxString &text) {
    const size_t len = 17;
    auto is_addr_at = [&](size_t i) {
        if (i + len > text.length()) return false;
        for (size_t k = 0; k < len; k++) {
            wxUniChar c = text[i + k];
            if (k % 3 == 2) {
                if (c != ':') return false;
            } else if (!wxIsxdigit(c)) {
                return false;
            }
        }
        /* Not part of a longer hex / colon run */
        auto part = [&](size_t j) { wxUniChar c = text[j]; return c == ':' || wxIsxdigit(c); };
        if (i > 0 && part(i - 1)) return false;
        if (i + len < text.length() && part(i + len)) return false;
        return true;
    };
    for (size_t i = 0; i + len <= text.length(); i++) {
        if (!is_addr_at(i)) continue;
        text.replace(i + 9, 8, "xx:xx:xx");
        i += len - 1;
    }
}

} // namespace

/* ======================================================================== */
/* Virtual log list                                                          */
/* ======================================================================== */

class DebugLogListCtrl : public wxListCtrl {
public:
    DebugLogListCtrl(wxWindow *parent, DebugConsoleDialog *dlg)
        : wxListCtrl(parent, wxID_ANY, wxDefaultPosition, wxDefaultSize,
                     wxLC_REPORT | wxLC_VIRTUAL)
        , dlg_(dlg)
    {
        warn_attr_.SetTextColour(TM().get(ThemeColor::StatusReconnecting));
        error_attr_.SetTextColour(TM().get(ThemeColor::StatusError));
        debug_attr_.SetTextColour(TM().get(ThemeColor::TextMuted));
    }

    const DebugLogModel::Entry *entry(long item) const {
        if (item < 0 || static_cast<size_t>(item) >= dlg_->shown_.size()) return nullptr;
        return dlg_->model_->find(dlg_->shown_[item]);
    }

    wxString OnGetItemText(long item, long column) const override {
        const DebugLogModel::Entry *e = entry(item);
        if (!e) return wxString();
        switch (column) {
        case COL_TIME:    return e->time;
        case COL_LEVEL:   return DebugLogModel::level_name(e->level);
        case COL_MESSAGE: return e->text;
        }
        return wxString();
    }

    wxListItemAttr *OnGetItemAttr(long item) const override {
        const DebugLogModel::Entry *e = entry(item);
        if (!e) return nullptr;
        switch (e->level) {
        case Level::Error: return &error_attr_;
        case Level::Warn:  return &warn_attr_;
        case Level::Debug: return &debug_attr_;
        case Level::Info:  break;
        }
        return nullptr;
    }

private:
    DebugConsoleDialog *dlg_;
    mutable wxListItemAttr warn_attr_, error_attr_, debug_attr_;
};

/* ======================================================================== */
/* Construction                                                              */
/* ======================================================================== */

void DebugConsoleDialog::ShowFor(MainFrame *frame, DebugLogModel *model) {
    if (!model) return;
    if (wxWindow *existing = wxWindow::FindWindowByName(DIALOG_NAME, frame)) {
        existing->Raise();
        return;
    }
    auto *dlg = new DebugConsoleDialog(frame, model);
    dlg->Show();
}

DebugConsoleDialog::DebugConsoleDialog(MainFrame *frame, DebugLogModel *model)
    : wxDialog(frame, wxID_ANY, U("dbgc.title"), wxDefaultPosition, wxSize(980, 720),
               wxDEFAULT_DIALOG_STYLE | wxRESIZE_BORDER | wxMAXIMIZE_BOX | wxMINIMIZE_BOX,
               DIALOG_NAME)
    , frame_(frame)
    , model_(model)
    , timer_(this)
{
    const wxColour ctrl_bg = TM().get(ThemeColor::CtrlBg);
    const wxColour ctrl_fg = TM().get(ThemeColor::CtrlFg);
    wxFont mono(wxFontInfo(9).Family(wxFONTFAMILY_TELETYPE).FaceName("Consolas"));

    auto *vbox = new wxBoxSizer(wxVERTICAL);

    auto heading = [&](const char *key) {
        auto *h = new wxStaticText(this, wxID_ANY, U(key));
        auto font = h->GetFont();
        font.SetWeight(wxFONTWEIGHT_BOLD);
        h->SetFont(font);
        h->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
        return h;
    };

    /* ---- Connection flow ---- */
    auto *flow_head = new wxBoxSizer(wxHORIZONTAL);
    flow_head->Add(heading("dbgc.section_flow"), 0, wxALIGN_CENTER_VERTICAL);
    session_label_ = new wxStaticText(this, wxID_ANY, wxString(), wxDefaultPosition,
                                      wxDefaultSize, wxST_ELLIPSIZE_END);
    session_label_->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
    flow_head->Add(session_label_, 1, wxALIGN_CENTER_VERTICAL | wxLEFT, 12);
    vbox->Add(flow_head, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 12);

    flow_list_ = new wxListCtrl(this, wxID_ANY, wxDefaultPosition, wxDefaultSize,
                                wxLC_REPORT | wxLC_SINGLE_SEL);
    flow_list_->SetBackgroundColour(ctrl_bg);
    flow_list_->SetTextColour(ctrl_fg);
    flow_list_->AppendColumn(U("dbgc.col_step"), wxLIST_FORMAT_LEFT, FromDIP(230));
    flow_list_->AppendColumn(U("dbgc.col_status"), wxLIST_FORMAT_LEFT, FromDIP(70));
    flow_list_->AppendColumn(U("dbgc.col_time"), wxLIST_FORMAT_LEFT, FromDIP(95));
    flow_list_->AppendColumn(U("dbgc.col_elapsed"), wxLIST_FORMAT_RIGHT, FromDIP(80));
    flow_list_->AppendColumn(U("dbgc.col_detail"), wxLIST_FORMAT_LEFT, FromDIP(480));
    for (int s = 0; s < DebugLogModel::STEP_COUNT; s++)
        flow_list_->InsertItem(s, U(DebugLogModel::step_label_key(static_cast<DebugLogModel::Step>(s))));
    {
        /* Tall enough for the header and every step without a vertical
         * scrollbar (plus room for a horizontal one on narrow windows) */
        wxRect r;
        flow_list_->GetItemRect(0, r);
        int row_h = r.height > 0 ? r.height : FromDIP(18);
        flow_list_->SetMinSize(wxSize(-1, row_h * (DebugLogModel::STEP_COUNT + 1) +
                                              wxSystemSettings::GetMetric(wxSYS_HSCROLL_Y, this) +
                                              FromDIP(8)));
    }
    /* The detail column takes the remaining width */
    flow_list_->Bind(wxEVT_SIZE, [this](wxSizeEvent &e) {
        int used = 0;
        for (int c = 0; c < FCOL_DETAIL; c++) used += flow_list_->GetColumnWidth(c);
        int avail = flow_list_->GetClientSize().x - used;
        flow_list_->SetColumnWidth(FCOL_DETAIL, std::max(avail, FromDIP(200)));
        e.Skip();
    });
    vbox->Add(flow_list_, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 8);
    auto *flow_hint = new wxStaticText(this, wxID_ANY, U("dbgc.flow_hint"));
    flow_hint->SetForegroundColour(TM().get(ThemeColor::TextMuted));
    vbox->Add(flow_hint, 0, wxLEFT | wxRIGHT | wxTOP, 12);

    /* ---- Log ---- */
    auto *log_head = new wxBoxSizer(wxHORIZONTAL);
    log_head->Add(heading("dbgc.section_log"), 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 12);
    level_choice_ = new wxChoice(this, wxID_ANY);
    level_choice_->Append(U("dbgc.level_all"));
    level_choice_->Append(U("dbgc.level_info"));
    level_choice_->Append(U("dbgc.level_warn"));
    level_choice_->Append(U("dbgc.level_error"));
    level_choice_->SetSelection(0);
    log_head->Add(level_choice_, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    search_ctrl_ = new wxSearchCtrl(this, wxID_ANY, wxString(), wxDefaultPosition,
                                    wxSize(FromDIP(200), -1));
    search_ctrl_->SetDescriptiveText(U("dbgc.search_hint"));
    search_ctrl_->ShowCancelButton(true);
    log_head->Add(search_ctrl_, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    session_only_ = new wxCheckBox(this, wxID_ANY, U("dbgc.session_only"));
    log_head->Add(session_only_, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    autoscroll_ = new wxCheckBox(this, wxID_ANY, U("dbgc.autoscroll"));
    autoscroll_->SetValue(true);
    log_head->Add(autoscroll_, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    log_head->AddStretchSpacer();
    count_label_ = new wxStaticText(this, wxID_ANY, wxString());
    count_label_->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
    log_head->Add(count_label_, 0, wxALIGN_CENTER_VERTICAL);
    vbox->Add(log_head, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 12);

    log_list_ = new DebugLogListCtrl(this, this);
    log_list_->SetBackgroundColour(ctrl_bg);
    log_list_->SetTextColour(ctrl_fg);
    log_list_->SetFont(mono);
    log_list_->AppendColumn(U("dbgc.col_time"), wxLIST_FORMAT_LEFT, FromDIP(95));
    log_list_->AppendColumn(U("dbgc.col_level"), wxLIST_FORMAT_LEFT, FromDIP(55));
    log_list_->AppendColumn(U("dbgc.col_message"), wxLIST_FORMAT_LEFT, FromDIP(1400));
    vbox->Add(log_list_, 1, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 8);

    /* ---- Buttons ---- */
    vbox->Add(new wxStaticLine(this), 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 8);
    auto *btn_sizer = new wxBoxSizer(wxHORIZONTAL);
    auto *diag_btn = new wxButton(this, wxID_ANY, U("dbgc.copy_diag"));
    auto *flow_btn = new wxButton(this, wxID_ANY, U("dbgc.copy_flow"));
    auto *log_btn = new wxButton(this, wxID_ANY, U("dbgc.copy_log"));
    btn_sizer->Add(diag_btn, 0, wxRIGHT, 6);
    btn_sizer->Add(flow_btn, 0, wxRIGHT, 6);
    btn_sizer->Add(log_btn, 0, wxRIGHT, 12);
    mask_addr_ = new wxCheckBox(this, wxID_ANY, U("dbgc.mask"));
    mask_addr_->SetValue(true);
    mask_addr_->SetToolTip(U("dbgc.mask_tip"));
    btn_sizer->Add(mask_addr_, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 12);
    feedback_label_ = new wxStaticText(this, wxID_ANY, wxString());
    feedback_label_->SetForegroundColour(TM().get(ThemeColor::StatusStreaming));
    btn_sizer->Add(feedback_label_, 1, wxALIGN_CENTER_VERTICAL);
    auto *open_btn = new wxButton(this, wxID_ANY, U("debug.open_log"));
    btn_sizer->Add(open_btn, 0, wxRIGHT, 6);
    auto *close_btn = new wxButton(this, wxID_CLOSE, U("modal.close"));
    btn_sizer->Add(close_btn, 0);
    vbox->Add(btn_sizer, 0, wxEXPAND | wxALL, 12);

    /* ---- Events ---- */
    diag_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) {
        wxString text = diagnostics_text();
        copy_to_clipboard(text, text.Freq('\n'));
    });
    flow_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) {
        wxString text = flow_text();
        copy_to_clipboard(text, text.Freq('\n'));
    });
    log_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { copy_shown_lines(); });
    open_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) {
        ShellExecuteA(nullptr, "open", model_->path().c_str(), nullptr, nullptr, SW_SHOWNORMAL);
    });
    close_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { Close(); });

    level_choice_->Bind(wxEVT_CHOICE, [this](wxCommandEvent &) { rebuild_filter(); });
    session_only_->Bind(wxEVT_CHECKBOX, [this](wxCommandEvent &) { rebuild_filter(); });
    search_ctrl_->Bind(wxEVT_TEXT, [this](wxCommandEvent &) { rebuild_filter(); });
    search_ctrl_->Bind(wxEVT_SEARCHCTRL_CANCEL_BTN, [this](wxCommandEvent &) {
        search_ctrl_->Clear();
    });

    log_list_->Bind(wxEVT_KEY_DOWN, [this](wxKeyEvent &e) {
        if (e.ControlDown() && e.GetKeyCode() == 'C') {
            copy_selected_lines();
        } else if (e.ControlDown() && e.GetKeyCode() == 'A') {
            log_list_->SetItemState(-1, wxLIST_STATE_SELECTED, wxLIST_STATE_SELECTED);
        } else {
            e.Skip();
        }
    });
    log_list_->Bind(wxEVT_LIST_ITEM_RIGHT_CLICK, [this](wxListEvent &) {
        wxMenu menu;
        menu.Append(1, U("dbgc.copy_selected"));
        menu.Append(2, U("dbgc.copy_log"));
        menu.Bind(wxEVT_MENU, [this](wxCommandEvent &) { copy_selected_lines(); }, 1);
        menu.Bind(wxEVT_MENU, [this](wxCommandEvent &) { copy_shown_lines(); }, 2);
        PopupMenu(&menu);
    });
    /* Scrolling up stops following new lines */
    log_list_->Bind(wxEVT_LIST_ITEM_SELECTED, [this](wxListEvent &e) {
        if (e.GetIndex() + 1 < static_cast<long>(shown_.size())) autoscroll_->SetValue(false);
        e.Skip();
    });

    flow_list_->Bind(wxEVT_LIST_ITEM_ACTIVATED, [this](wxListEvent &e) {
        long step = e.GetIndex();
        if (step >= 0 && step < DebugLogModel::STEP_COUNT)
            jump_to(model_->session().steps[step].seq);
    });
    flow_list_->Bind(wxEVT_KEY_DOWN, [this](wxKeyEvent &e) {
        if (e.ControlDown() && e.GetKeyCode() == 'C') {
            wxString text = flow_text();
            copy_to_clipboard(text, text.Freq('\n'));
        } else {
            e.Skip();
        }
    });

    Bind(wxEVT_CLOSE_WINDOW, [this](wxCloseEvent &) {
        timer_.Stop();
        Destroy();
    });
    Bind(wxEVT_TIMER, &DebugConsoleDialog::OnTimer, this);

    SetBackgroundColour(TM().get(ThemeColor::DialogBg));
    SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    for (wxWindow *w : {static_cast<wxWindow *>(session_only_), static_cast<wxWindow *>(autoscroll_),
                        static_cast<wxWindow *>(mask_addr_)})
        w->SetForegroundColour(TM().get(ThemeColor::TextPrimary));

    SetSizer(vbox);
    SetMinSize(FromDIP(wxSize(760, 560)));
    SetSize(FromDIP(wxSize(980, 720)));

    prev_tick_ = GetTickCount64();
    prev_packets_ = link_stats().packets_sent.load(std::memory_order_relaxed);
    prev_bytes_ = link_stats().bytes_sent.load(std::memory_order_relaxed);

    model_->poll();
    rebuild_filter();
    refresh_flow();
    CentreOnParent();
    timer_.Start(REFRESH_MS);
}

/* ======================================================================== */
/* Log view                                                                  */
/* ======================================================================== */

bool DebugConsoleDialog::matches(const DebugLogModel::Entry &e) const {
    int sel = level_choice_->GetSelection();
    if (sel > 0 && e.level < CHOICE_LEVELS[sel]) return false;
    if (session_only_->GetValue() && e.session != model_->session().id) return false;
    if (!query_lower_.empty() && !e.text.Lower().Contains(query_lower_)) return false;
    return true;
}

void DebugConsoleDialog::rebuild_filter() {
    query_lower_ = search_ctrl_->GetValue().Lower();
    shown_.clear();
    for (const auto &e : model_->entries())
        if (matches(e)) shown_.push_back(e.seq);
    seen_seq_ = model_->last_seq();
    seen_first_ = model_->first_seq();
    seen_session_ = model_->session().id;

    log_list_->SetItemCount(static_cast<long>(shown_.size()));
    log_list_->Refresh();
    if (autoscroll_->GetValue() && !shown_.empty())
        log_list_->EnsureVisible(static_cast<long>(shown_.size()) - 1);
    update_count();
}

void DebugConsoleDialog::append_new() {
    /* Old lines trimmed, or a new attempt while only the current one is shown */
    if (model_->first_seq() != seen_first_ ||
        (session_only_->GetValue() && model_->session().id != seen_session_)) {
        rebuild_filter();
        return;
    }
    uint64_t last = model_->last_seq();
    if (last == seen_seq_) return;
    size_t before = shown_.size();
    for (uint64_t seq = seen_seq_ + 1; seq <= last; seq++) {
        const DebugLogModel::Entry *e = model_->find(seq);
        if (e && matches(*e)) shown_.push_back(seq);
    }
    seen_seq_ = last;
    seen_session_ = model_->session().id;
    if (shown_.size() != before) {
        log_list_->SetItemCount(static_cast<long>(shown_.size()));
        log_list_->RefreshItems(static_cast<long>(before), static_cast<long>(shown_.size()) - 1);
        if (autoscroll_->GetValue())
            log_list_->EnsureVisible(static_cast<long>(shown_.size()) - 1);
    }
    update_count();
}

void DebugConsoleDialog::update_count() {
    count_label_->SetLabel(wxString::Format(U("dbgc.count"), (unsigned)shown_.size(),
                                            (unsigned)model_->entries().size()));
    count_label_->GetParent()->Layout();
}

void DebugConsoleDialog::jump_to(uint64_t seq) {
    if (seq == 0 || !model_->find(seq)) return;
    auto it = std::lower_bound(shown_.begin(), shown_.end(), seq);
    if (it == shown_.end() || *it != seq) {
        /* Hidden by the filters: show everything */
        level_choice_->SetSelection(0);
        search_ctrl_->ChangeValue(wxString());
        rebuild_filter();
        it = std::lower_bound(shown_.begin(), shown_.end(), seq);
        if (it == shown_.end() || *it != seq) return;
    }
    autoscroll_->SetValue(false);
    long row = static_cast<long>(it - shown_.begin());
    for (long sel = next_selected(log_list_, -1); sel != -1; sel = next_selected(log_list_, sel))
        log_list_->SetItemState(sel, 0, wxLIST_STATE_SELECTED);
    log_list_->SetItemState(row, wxLIST_STATE_SELECTED | wxLIST_STATE_FOCUSED,
                            wxLIST_STATE_SELECTED | wxLIST_STATE_FOCUSED);
    /* Scroll so a little context above the line is visible */
    log_list_->EnsureVisible(std::min<long>(row + 8, static_cast<long>(shown_.size()) - 1));
    log_list_->EnsureVisible(std::max<long>(row - 3, 0));
    log_list_->SetFocus();
}

/* ======================================================================== */
/* Flow                                                                      */
/* ======================================================================== */

DebugConsoleDialog::FlowRow DebugConsoleDialog::flow_row(int step, bool english) const {
    /* Copied text is English (bug reports); the window follows the UI language */
    auto T = [english](const char *key, const char *en) { return english ? wxString(en) : U(key); };
    const DebugLogModel::Session &sess = model_->session();
    const DebugLogModel::FlowStep &st = sess.steps[step];
    FlowRow r{st.status, st.time, wxString(), st.detail};
    if (!st.time.empty()) r.elapsed = elapsed_text(sess.start_time, st.time);

    bool active = sess.id != 0 && sess.stop_time.empty();
    A2dpService::State state = frame_->state();

    /* The step being worked on: first unfinished one while connecting */
    if (active && r.status == StepStatus::Pending && state == A2dpService::State::Connecting) {
        bool first_open = true;
        for (int s = 0; s < step; s++) {
            StepStatus p = sess.steps[s].status;
            if (p != StepStatus::Done && p != StepStatus::Warning) { first_open = false; break; }
        }
        if (first_open) r.status = StepStatus::Running;
    }

    if (step == DebugLogModel::STEP_AUDIO && active &&
        (r.status == StepStatus::Done || r.status == StepStatus::Warning) &&
        state == A2dpService::State::Streaming && pps_ >= 0) {
        if (pps_ < 1.0) {
            r.status = StepStatus::Warning;
            r.detail = T("dbgc.audio_stalled", "No packets sent in the last second");
        } else {
            r.detail = wxString::Format(T("dbgc.audio_live", "%.0f packets/s, %.0f kbps"), pps_, kbps_);
        }
    }
    if (step == DebugLogModel::STEP_LINK && sess.id != 0) {
        if (sess.reconnects > 0) {
            wxString count = wxString::Format(T("dbgc.link_reconnects", "%d reconnect(s)"), sess.reconnects);
            r.detail = r.detail.empty() ? count : count + " - " + r.detail;
        } else if (r.status == StepStatus::Done) {
            r.detail = T("dbgc.link_stable", "No disconnects");
        }
    }
    return r;
}

void DebugConsoleDialog::refresh_flow() {
    const DebugLogModel::Session &sess = model_->session();

    wxString label;
    if (sess.id == 0) {
        label = U("dbgc.no_session");
    } else {
        label = wxString::Format(U("dbgc.session"), (int)sess.id, sess.start_time);
        if (!sess.stop_time.empty())
            label += wxString::Format(U("dbgc.session_stopped"), sess.stop_time);
    }
    if (!model_->last_state_text().empty())
        label += "   |   " + model_->last_state_text();
    if (session_label_->GetLabel() != label) session_label_->SetLabel(label);

    for (int s = 0; s < DebugLogModel::STEP_COUNT; s++) {
        FlowRow r = flow_row(s);
        set_cell(flow_list_, s, FCOL_STATUS, U(status_key(r.status)));
        set_cell(flow_list_, s, FCOL_TIME, r.time);
        set_cell(flow_list_, s, FCOL_ELAPSED, r.elapsed);
        set_cell(flow_list_, s, FCOL_DETAIL, r.detail);
        wxColour c = status_colour(r.status);
        if (flow_list_->GetItemTextColour(s) != c) flow_list_->SetItemTextColour(s, c);
    }
}

void DebugConsoleDialog::OnTimer(wxTimerEvent &) {
    /* Packet rate over the last period */
    uint64_t tick = GetTickCount64();
    uint64_t packets = link_stats().packets_sent.load(std::memory_order_relaxed);
    uint64_t bytes = link_stats().bytes_sent.load(std::memory_order_relaxed);
    double dt = (tick - prev_tick_) / 1000.0;
    if (dt >= 0.9) {
        pps_ = (packets - prev_packets_) / dt;
        kbps_ = (bytes - prev_bytes_) * 8.0 / dt / 1000.0;
        prev_tick_ = tick;
        prev_packets_ = packets;
        prev_bytes_ = bytes;
    }

    model_->poll();
    append_new();
    refresh_flow();
}

/* ======================================================================== */
/* Copies                                                                    */
/* ======================================================================== */

wxString DebugConsoleDialog::format_line(const DebugLogModel::Entry &e) const {
    return wxString::Format("%-12s %-5s %s", e.time, DebugLogModel::level_name(e.level), e.text);
}

wxString DebugConsoleDialog::flow_text() const {
    const DebugLogModel::Session &sess = model_->session();
    wxString out;
    if (sess.id == 0) {
        out << "Connection flow: no connection attempt yet\n";
        return out;
    }
    out << wxString::Format("Connection flow: attempt #%u, started %s", (unsigned)sess.id, sess.start_time);
    if (!sess.stop_time.empty()) out << ", stopped " << sess.stop_time;
    if (sess.reconnects > 0) out << wxString::Format(", %d reconnect(s)", sess.reconnects);
    out << "\n";
    for (int s = 0; s < DebugLogModel::STEP_COUNT; s++) {
        FlowRow r = flow_row(s, true);
        out << wxString::Format("  %s %-32s %-12s %-10s %s\n", status_tag(r.status),
                                DebugLogModel::step_name(static_cast<DebugLogModel::Step>(s)),
                                r.time, r.elapsed, r.detail);
    }
    return out;
}

wxString DebugConsoleDialog::diagnostics_text() const {
    wxString out;
    out << "```text\n";
    out << "A2DPWB diagnostics - " << wxDateTime::Now().Format("%Y-%m-%d %H:%M:%S") << "\n";
    out << "Version      : " << APP_VERSION << "\n";
    out << "OS           : " << wxGetOsDescription() << "\n";
    const AppSettings &set = frame_->settings();
    out << "Language     : " << wxString::FromUTF8(set.language) << ", theme "
        << wxString::FromUTF8(set.theme) << "\n";
    out << wxString::Format("State        : %s", state_name(static_cast<int>(frame_->state())));
    if (!model_->last_state_text().empty()) out << " (" << model_->last_state_text() << ")";
    out << "\n";

    out << "\n[Adapter]\n";
    out << wxString::Format("  Setting: chip PID 0x%04X%s%s\n", (unsigned)set.bt_chip_pid,
                            set.bt_chip_pid ? "" : " (auto)",
                            set.bt_chip_fw_stem.empty() ? wxString()
                                : wxString(", firmware stem ") + wxString::FromUTF8(set.bt_chip_fw_stem));
    A2dpService &svc = frame_->service();
    out << "  Firmware files: " << (svc.is_firmware_present() ? "present" : "missing");
    std::string fw = svc.firmware_status();
    if (!fw.empty()) out << " (" << wxString::FromUTF8(fw) << ")";
    out << "\n";
    for (const wxString &l : model_->adapter_lines()) out << "  " << l << "\n";

    out << "\n[Profile]\n";
    int idx = frame_->selected_profile();
    const auto &profs = frame_->profiles().profiles();
    if (idx >= 0 && idx < static_cast<int>(profs.size())) {
        const ConnectionProfile &p = profs[idx];
        out << "  Name: " << wxString::FromUTF8(p.name) << "\n";
        out << "  Device: " << wxString::FromUTF8(p.device_name) << " [" << p.device_address << "]\n";
        out << wxString::Format("  Codec: %s, quality %s, ABR %s, sample rate %s, bit depth %s\n",
                                p.codec, p.quality, p.abr ? "on" : "off",
                                p.sample_rate ? wxString::Format("%u", p.sample_rate) : wxString("auto"),
                                p.bit_depth ? wxString::Format("%u", p.bit_depth) : wxString("auto"));
        out << "  Capture: " << p.capture_mode;
        if (!p.audio_device_name.empty()) out << " '" << wxString::FromUTF8(p.audio_device_name) << "'";
        out << wxString::Format(", auto switch %s, max media packet %u\n",
                                p.auto_switch_device ? "on" : "off", (unsigned)p.max_media_payload);
    } else {
        out << "  (none selected - receiver measurement or not connected)\n";
    }

    out << "\n[Stream]\n";
    const A2dpService::StreamInfo &info = frame_->stream_info();
    if (frame_->state() == A2dpService::State::Streaming && !info.codec.empty()) {
        out << wxString::Format("  %s %u Hz %u ch, %u kbps; source %u Hz %u ch %u bit\n",
                                wxString::FromUTF8(info.codec), info.sample_rate, info.channels,
                                info.bitrate_kbps, info.source_sample_rate, info.source_channels,
                                info.source_bit_depth);
    } else {
        out << "  not streaming\n";
    }
    const LinkStats &ls = link_stats();
    out << wxString::Format("  Media packets sent %llu (%llu bytes), queue drops %llu, send errors %llu\n",
                            (unsigned long long)ls.packets_sent.load(),
                            (unsigned long long)ls.bytes_sent.load(),
                            (unsigned long long)ls.queue_drops.load(),
                            (unsigned long long)ls.send_errors.load());
    out << wxString::Format("  Capture frames %llu, dropped %llu\n",
                            (unsigned long long)ls.capture_frames.load(),
                            (unsigned long long)ls.capture_dropped_frames.load());
    if (hci_capture::active()) {
        out << wxString::Format("  HCI capture running: %s (%.1f MB)\n",
                                wxString::FromUTF8(hci_capture::path()),
                                hci_capture::bytes_written() / (1024.0 * 1024.0));
    }

    out << "\n[Flow]\n" << flow_text();

    /* Lines of the current attempt (everything if none yet) */
    uint32_t sid = model_->session().id;
    std::vector<const DebugLogModel::Entry *> lines, issues;
    for (const auto &e : model_->entries()) {
        if (e.session != sid) continue;
        lines.push_back(&e);
        if (e.level >= Level::Warn) issues.push_back(&e);
    }

    out << "\n[Warnings and errors]\n";
    if (issues.empty()) out << "  none\n";
    size_t skip = issues.size() > DIAG_ISSUES ? issues.size() - DIAG_ISSUES : 0;
    if (skip) out << wxString::Format("  (%u earlier omitted)\n", (unsigned)skip);
    for (size_t i = skip; i < issues.size(); i++) out << "  " << format_line(*issues[i]) << "\n";

    out << "\n[Log]\n";
    skip = lines.size() > DIAG_LINES ? lines.size() - DIAG_LINES : 0;
    if (skip) out << wxString::Format("(%u earlier lines omitted - see debug.log)\n", (unsigned)skip);
    for (size_t i = skip; i < lines.size(); i++) out << format_line(*lines[i]) << "\n";
    out << "```\n";
    return out;
}

void DebugConsoleDialog::copy_selected_lines() {
    wxString text;
    size_t n = 0;
    for (long row = next_selected(log_list_, -1); row != -1; row = next_selected(log_list_, row)) {
        if (static_cast<size_t>(row) >= shown_.size()) break;
        if (const DebugLogModel::Entry *e = model_->find(shown_[row])) {
            text << format_line(*e) << "\n";
            n++;
        }
    }
    if (n) copy_to_clipboard(text, n);
}

void DebugConsoleDialog::copy_shown_lines() {
    wxString text;
    size_t n = 0;
    for (uint64_t seq : shown_) {
        if (const DebugLogModel::Entry *e = model_->find(seq)) {
            text << format_line(*e) << "\n";
            n++;
        }
    }
    copy_to_clipboard(text, n);
}

void DebugConsoleDialog::copy_to_clipboard(const wxString &text_in, size_t lines) {
    wxString text = text_in;
    if (mask_addr_->GetValue()) mask_bd_addresses(text);
    bool ok = false;
    if (wxTheClipboard->Open()) {
        ok = wxTheClipboard->SetData(new wxTextDataObject(text));
        wxTheClipboard->Close();
    }
    feedback_label_->SetForegroundColour(TM().get(ok ? ThemeColor::StatusStreaming : ThemeColor::StatusError));
    feedback_label_->SetLabel(ok ? wxString::Format(U("dbgc.copied"), (unsigned)lines)
                                 : U("dbgc.copy_failed"));
    Layout();
}
