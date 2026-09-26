/*
 * Debug Console Dialog (Debug menu) - connection flow checklist, live
 * debug.log view with filters, and clipboard copies for bug reports
 * SPDX-License-Identifier: MIT
 */

#ifndef WX_DEBUG_CONSOLE_DIALOG_H
#define WX_DEBUG_CONSOLE_DIALOG_H

#include "debug_log_model.h"

#include <wx/wx.h>
#include <wx/listctrl.h>
#include <wx/srchctrl.h>
#include <wx/timer.h>

#include <cstdint>
#include <vector>

class MainFrame;
class DebugLogListCtrl;

/* Modeless; open with DebugConsoleDialog::ShowFor() so only one exists */
class DebugConsoleDialog : public wxDialog {
public:
    static void ShowFor(MainFrame *frame, DebugLogModel *model);

private:
    DebugConsoleDialog(MainFrame *frame, DebugLogModel *model);

    void OnTimer(wxTimerEvent &evt);
    void rebuild_filter();
    void append_new();
    bool matches(const DebugLogModel::Entry &e) const;
    void refresh_flow();
    void update_count();
    void jump_to(uint64_t seq);

    /* Copies */
    wxString format_line(const DebugLogModel::Entry &e) const;
    wxString flow_text() const;
    wxString diagnostics_text() const;
    void copy_selected_lines();
    void copy_shown_lines();
    void copy_to_clipboard(const wxString &text, size_t lines);

    /* Flow row as shown (status may be inferred from the service state) */
    struct FlowRow {
        DebugLogModel::StepStatus status;
        wxString time, elapsed, detail;
    };
    FlowRow flow_row(int step, bool english = false) const;

    MainFrame *frame_;
    DebugLogModel *model_;
    wxTimer timer_;

    wxStaticText     *session_label_ = nullptr;
    wxListCtrl       *flow_list_ = nullptr;
    wxChoice         *level_choice_ = nullptr;
    wxSearchCtrl     *search_ctrl_ = nullptr;
    wxCheckBox       *session_only_ = nullptr;
    wxCheckBox       *autoscroll_ = nullptr;
    wxCheckBox       *mask_addr_ = nullptr;
    wxStaticText     *count_label_ = nullptr;
    wxStaticText     *feedback_label_ = nullptr;
    DebugLogListCtrl *log_list_ = nullptr;

    /* Seqs of the entries shown, ascending */
    std::vector<uint64_t> shown_;
    uint64_t seen_seq_ = 0;       /* last model seq considered */
    uint64_t seen_first_ = 0;     /* model first_seq at the last rebuild */
    uint32_t seen_session_ = 0;
    wxString query_lower_;

    /* Live packet rate for the audio step */
    uint64_t prev_tick_ = 0, prev_packets_ = 0, prev_bytes_ = 0;
    double pps_ = -1.0, kbps_ = 0.0;

    friend class DebugLogListCtrl;
};

#endif /* WX_DEBUG_CONSOLE_DIALOG_H */
