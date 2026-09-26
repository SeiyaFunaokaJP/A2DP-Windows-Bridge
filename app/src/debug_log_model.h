/*
 * Debug Log Model - follows debug.log and derives the connection flow
 *
 * In debug mode every module writes to stderr, which A2dpBridgeApp redirects
 * to debug.log. This model tails that file (no change to the logging path, so
 * nothing can block a writer), keeps the lines in memory and recognises the
 * milestones of a connection attempt (BTstack init, ACL / AVDTP, codec, WASAPI,
 * encoder, stream start) from the lines the modules already print. The debug
 * console window shows both.
 *
 * GUI thread only.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef DEBUG_LOG_MODEL_H
#define DEBUG_LOG_MODEL_H

#include <wx/event.h>
#include <wx/string.h>
#include <wx/timer.h>

#include <array>
#include <cstdint>
#include <deque>
#include <string>
#include <vector>

class DebugLogModel : public wxEvtHandler {
public:
    enum class Level { Debug, Info, Warn, Error };

    struct Entry {
        uint64_t seq = 0;     /* 1-based, never reused */
        wxString time;        /* "hh:mm:ss.mmm" */
        Level    level = Level::Info;
        wxString text;        /* message without the "[time LEVEL]" prefix */
        uint32_t session = 0; /* connection attempt the line belongs to (0 = before any) */
    };

    enum Step {
        STEP_START,     /* start_streaming (profile) */
        STEP_BTSTACK,   /* adapter / HCI power on */
        STEP_CONNECT,   /* ACL + AVDTP signaling */
        STEP_CAPS,      /* capability discovery */
        STEP_CODEC,     /* codec selection */
        STEP_CAPTURE,   /* WASAPI capture init */
        STEP_CONFIG,    /* SET_CONFIGURATION / OPEN */
        STEP_ENCODER,   /* encoder init */
        STEP_STREAM,    /* AVDTP START */
        STEP_AUDIO,     /* WASAPI capture running, packets flowing */
        STEP_LINK,      /* disconnects / reconnects while streaming */
        STEP_COUNT
    };

    enum class StepStatus { Pending, Running, Done, Warning, Failed };

    struct FlowStep {
        StepStatus status = StepStatus::Pending;
        wxString   time;     /* when the status last changed */
        wxString   detail;
        uint64_t   seq = 0;  /* log line that set the status (0 = none) */
    };

    struct Session {
        uint32_t id = 0;          /* 0 = no connection attempt yet */
        uint64_t first_seq = 0;
        wxString start_time;
        wxString stop_time;       /* empty while running */
        int      reconnects = 0;
        std::array<FlowStep, STEP_COUNT> steps{};
    };

    explicit DebugLogModel(const std::string &path);
    ~DebugLogModel() override;

    /* Read what was appended to the file since the last call (also runs on
     * an internal timer) */
    void poll();

    const std::deque<Entry> &entries() const { return entries_; }
    /* Entry by seq, nullptr if trimmed or not yet read */
    const Entry *find(uint64_t seq) const;
    uint64_t last_seq() const { return next_seq_ - 1; }
    /* Seq of the oldest entry still kept (grows when old lines are trimmed) */
    uint64_t first_seq() const { return entries_.empty() ? next_seq_ : entries_.front().seq; }

    const Session &session() const { return session_; }
    const std::string &path() const { return path_; }

    /* Last "A2dpService: state=N text=..." seen */
    int last_state() const { return last_state_; }
    const wxString &last_state_text() const { return last_state_text_; }
    /* Adapter enumeration and controller lines, for the diagnostics summary */
    const std::vector<wxString> &adapter_lines() const { return adapter_lines_; }

    /* English step name (copied text) and localization key (window) */
    static const char *step_name(Step s);
    static const char *step_label_key(Step s);
    static const char *level_name(Level l);

private:
    void OnTimer(wxTimerEvent &evt);
    void add_line(const std::string &raw, const wxString &arrival_time, bool live);
    void track(const Entry &e);
    void set_step(Step s, StepStatus status, const Entry &e, const wxString &detail);
    void set_detail(Step s, const wxString &detail);
    void begin_session(const Entry &e);

    std::string path_;
    void *file_ = nullptr;        /* HANDLE, shared read */
    uint64_t offset_ = 0;
    bool first_read_ = true;
    std::string partial_;         /* incomplete last line */
    wxTimer timer_;

    std::deque<Entry> entries_;
    uint64_t next_seq_ = 1;
    wxString last_time_;          /* for lines without their own timestamp */

    Session session_;
    int last_state_ = 0;
    wxString last_state_text_;
    std::vector<wxString> adapter_lines_;
    bool in_adapter_block_ = false;
};

#endif /* DEBUG_LOG_MODEL_H */
