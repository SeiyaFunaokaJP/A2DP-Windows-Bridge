/*
 * Peer Receiver Test Dialog (Debug menu)
 *
 * Works with tools/linux_sink/a2dpwb_sink.py on another PC: finds it on the
 * local network, streams to its Bluetooth adapter by itself and shows what
 * was sent next to what arrived.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef WX_RECEIVER_DIALOG_H
#define WX_RECEIVER_DIALOG_H

#include <wx/wx.h>
#include <wx/timer.h>

#include <cstdint>

#include "a2dp_service.h"
#include "remote_sink_client.h"

class MainFrame;

/* Modeless; open with ReceiverDialog::ShowFor() so only one exists */
class ReceiverDialog : public wxDialog {
public:
    static void ShowFor(MainFrame *frame);

private:
    explicit ReceiverDialog(MainFrame *frame);

    struct Sample {
        uint64_t tick_ms = 0;
        uint64_t packets = 0, bytes = 0, dropped = 0;
        uint64_t capture_frames = 0, capture_dropped = 0;
    };
    static Sample take_sample();

    void refresh();
    void refresh_steps();
    void refresh_status();
    void refresh_sent(bool streaming, const Sample &now);
    void refresh_received();
    void toggle_connection();
    void start_stop();
    void reset();

    /* One comparison row: label | sent | received */
    void add_row(wxFlexGridSizer *grid, const char *label_key, wxStaticText **sent, wxStaticText **received);
    /* Steps of a measurement, shown as OK / NG, changes go to the log */
    enum Step { StepDiscover, StepStats, StepConfigure, StepLink, StepCodec, StepAudio, STEP_COUNT };
    enum class StepState { Pending, Running, Ok, Failed };
    void set_step(Step step, StepState state, const wxString &detail = wxString());
    void reset_measurement_steps();
    void log(const wxString &line);

    wxStaticText *value_text();
    wxChoice *add_choice(wxSizer *row, const char *label_key, const wxArrayString &items, int selection);
    /* Wraps at `width` pixels anywhere between CJK characters and at spaces
     * elsewhere (wxStaticText::Wrap() only breaks at spaces) */
    wxString wrap(const wxString &text, int width) const;
    void set_value(wxStaticText *t, const wxString &text, bool bad = false);

    MainFrame *frame_;
    wxTimer timer_;

    Sample baseline_{}, prev_{};
    bool have_prev_ = false;
    /* Codec of the last stream; its counts stay shown after it stopped so
     * they can be compared with the receiver's final ones */
    wxString last_codec_;
    uint32_t stream_starts_ = 0;   /* LinkStats::stream_starts baseline_ was taken at */
    A2dpService::State last_state_ = A2dpService::State::Idle;
    /* Receiver counters are relative to this; a new stream at the receiver restarts them */
    RemoteSinkStats remote_base_{};
    bool have_remote_base_ = false;
    bool measuring_ = false;       /* this window started the stream */
    bool service_left_idle_ = false; /* A2DPWB started working on it (Idle before that is not the end) */
    wxString failure_;             /* why the last stream this window started ended */
    wxString status_text_;         /* last text set (Wrap() changes the label) */
    StepState step_state_[STEP_COUNT] = {};
    wxStaticText *step_text_[STEP_COUNT] = {};
    wxTextCtrl   *log_ = nullptr;
    std::string last_service_text_; /* A2DPWB status text last logged */
    uint64_t measure_start_ms_ = 0;
    uint32_t requested_mtu_ = 0, requested_buffer_ms_ = 0;
    uint32_t stream_id_at_start_ = 0;
    bool had_stats_at_start_ = false;
    uint64_t link_ok_ms_ = 0;

    wxTextCtrl   *addr_ = nullptr;
    wxButton     *connect_btn_ = nullptr;
    wxStaticText *status_ = nullptr;
    wxChoice     *codec_ = nullptr;
    wxChoice     *quality_ = nullptr;
    wxChoice     *rate_ = nullptr;
    wxChoice     *depth_ = nullptr;
    wxChoice     *source_ = nullptr;
    wxChoice     *afh_ = nullptr;
    wxTextCtrl   *afh_channels_ = nullptr;
    void apply_afh();
    wxChoice     *mtu_ = nullptr;
    wxChoice     *buffer_ = nullptr;
    wxButton     *start_btn_ = nullptr;

    wxStaticText *codec_s_ = nullptr, *codec_r_ = nullptr;
    wxStaticText *bitrate_s_ = nullptr, *bitrate_r_ = nullptr;
    wxStaticText *total_s_ = nullptr, *total_r_ = nullptr;
    wxStaticText *loss_s_ = nullptr, *loss_r_ = nullptr;
    wxStaticText *timing_s_ = nullptr, *timing_r_ = nullptr;
    wxStaticText *buffer_s_ = nullptr, *buffer_r_ = nullptr;
    wxStaticText *audio_s_ = nullptr, *audio_r_ = nullptr;
    wxStaticText *errors_s_ = nullptr, *errors_r_ = nullptr;
    wxStaticText *signal_s_ = nullptr, *signal_r_ = nullptr;
};

#endif /* WX_RECEIVER_DIALOG_H */
