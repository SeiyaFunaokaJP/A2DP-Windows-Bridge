/*
 * Debug Log Model Implementation
 * SPDX-License-Identifier: MIT
 */

#include "debug_log_model.h"

#include <wx/datetime.h>

#include <algorithm>
#include <cstring>

#include <windows.h>

namespace {

/* Lines kept in memory (debug.log itself keeps everything) */
const size_t MAX_ENTRIES = 100000;
/* On the first read of a large file, start this far from its end */
const uint64_t MAX_INITIAL_BYTES = 16ull * 1024 * 1024;
/* Plain stderr lines get the time they are read: poll often */
const int POLL_MS = 100;

enum class Action { Done, Running, Warning, Failed, Detail };
enum class DetailFrom { None, After, Whole };

struct Rule {
    DebugLogModel::Step step;
    const char *needle;
    Action action;
    DetailFrom detail;
};

/* Milestones, recognised from the lines the modules already print. Keep in
 * sync with the messages in a2dp_service.cpp / btstack_transport.cpp /
 * wasapi_capture.cpp. Failures that only show up as an error state are
 * caught by the "state=4" handling in track(). */
const Rule RULES[] = {
    {DebugLogModel::STEP_START,   "A2dpService:   profile=",                       Action::Detail,  DetailFrom::After},
    {DebugLogModel::STEP_BTSTACK, "BTstack: thread started",                       Action::Running, DetailFrom::None},
    {DebugLogModel::STEP_BTSTACK, "BTstack: Controller ",                          Action::Detail,  DetailFrom::After},
    {DebugLogModel::STEP_BTSTACK, "BTstack: WARNING",                              Action::Warning, DetailFrom::Whole},
    {DebugLogModel::STEP_BTSTACK, "BTstack: HCI initialization failed",            Action::Failed,  DetailFrom::Whole},
    {DebugLogModel::STEP_BTSTACK, "BTstack: HCI initialization timed out",         Action::Failed,  DetailFrom::Whole},
    {DebugLogModel::STEP_BTSTACK, "A2dpService: BTstack initialized successfully", Action::Done,    DetailFrom::None},
    {DebugLogModel::STEP_CONNECT, "BTstack: Connecting to ",                       Action::Running, DetailFrom::After},
    {DebugLogModel::STEP_CONNECT, "BTstack: HCI ACL connection established",       Action::Detail,  DetailFrom::Whole},
    {DebugLogModel::STEP_CONNECT, "BTstack: Signaling connection established",     Action::Done,    DetailFrom::Whole},
    {DebugLogModel::STEP_CONNECT, "BTstack: Connection timed out",                 Action::Failed,  DetailFrom::Whole},
    {DebugLogModel::STEP_CONNECT, "a2dp_source_establish_stream failed",           Action::Failed,  DetailFrom::Whole},
    {DebugLogModel::STEP_CAPS,    "BTstack: Capability discovery complete",        Action::Done,    DetailFrom::After},
    {DebugLogModel::STEP_CODEC,   "A2dpService: codec selected: ",                 Action::Done,    DetailFrom::After},
    {DebugLogModel::STEP_CAPTURE, "WasapiCapture: Format: ",                       Action::Detail,  DetailFrom::After},
    {DebugLogModel::STEP_CAPTURE, "A2dpService: WASAPI capture init OK",           Action::Done,    DetailFrom::After},
    {DebugLogModel::STEP_CONFIG,  "BTstack: Codec configured",                     Action::Running, DetailFrom::After},
    {DebugLogModel::STEP_CONFIG,  "BTstack: Stream established",                   Action::Done,    DetailFrom::After},
    {DebugLogModel::STEP_CONFIG,  "BTstack: Stream configuration timed out",       Action::Failed,  DetailFrom::Whole},
    {DebugLogModel::STEP_CONFIG,  "BTstack: set_config_other failed",              Action::Failed,  DetailFrom::Whole},
    {DebugLogModel::STEP_ENCODER, "A2dpService: encoder initialized, ",            Action::Done,    DetailFrom::After},
    {DebugLogModel::STEP_STREAM,  "BTstack: Streaming started",                    Action::Done,    DetailFrom::None},
    {DebugLogModel::STEP_AUDIO,   "WasapiCapture: Capture started",                Action::Done,    DetailFrom::None},
    {DebugLogModel::STEP_LINK,    "A2dpService: connection lost",                  Action::Running, DetailFrom::Whole},
    {DebugLogModel::STEP_LINK,    "BTstack: Reconnecting to ",                     Action::Detail,  DetailFrom::Whole},
    {DebugLogModel::STEP_LINK,    "A2dpService: reconnect attempt ",               Action::Detail,  DetailFrom::Whole},
    {DebugLogModel::STEP_LINK,    "A2dpService: reconnected on attempt ",          Action::Warning, DetailFrom::Whole},
};

wxString now_time() {
    return wxDateTime::UNow().Format("%H:%M:%S.%l");
}

/* "Prefix: message" -> "message" */
wxString strip_module(const wxString &t) {
    int pos = t.Find(": ");
    return pos == wxNOT_FOUND ? t : t.Mid(pos + 2);
}

wxString after_needle(const wxString &t, const char *needle) {
    int pos = t.Find(needle);
    wxString s = pos == wxNOT_FOUND ? wxString() : t.Mid(pos + strlen(needle));
    s.Trim(true).Trim(false);
    if (s.EndsWith("...")) s.RemoveLast(3);
    if (s.StartsWith("(") && s.EndsWith(")")) s = s.Mid(1, s.length() - 2);
    return s;
}

DebugLogModel::Level classify(const wxString &text, const wxString &prefix_level) {
    using Level = DebugLogModel::Level;
    if (prefix_level == "ERROR") return Level::Error;
    if (prefix_level == "WARN") return Level::Warn;
    if (prefix_level == "DEBUG") return Level::Debug;
    if (!prefix_level.empty()) return Level::Info;

    /* Plain fprintf(stderr) lines carry no level: go by wording */
    if (text.Contains("state=4 ")) return Level::Error;
    if (text.Contains("state=3 ")) return Level::Warn;
    wxString lower = text.Lower();
    if (lower.Contains("warning") || lower.Contains("discontinuity") ||
        lower.Contains("not loaded") || lower.Contains("connection lost"))
        return Level::Warn;
    if (lower.Contains("error") || lower.Contains("failed") || lower.Contains("timed out") ||
        lower.Contains("exception"))
        return Level::Error;
    return Level::Info;
}

bool finished(DebugLogModel::StepStatus s) {
    return s == DebugLogModel::StepStatus::Done || s == DebugLogModel::StepStatus::Warning;
}

} // namespace

const char *DebugLogModel::step_name(Step s) {
    static const char *names[STEP_COUNT] = {
        "Start (profile)", "Bluetooth adapter (BTstack)", "Connect (ACL / AVDTP signaling)",
        "Capability discovery", "Codec selection", "Audio capture (WASAPI)",
        "Stream configuration", "Encoder", "Stream start (AVDTP START)",
        "Audio sending", "Connection upkeep",
    };
    return (s >= 0 && s < STEP_COUNT) ? names[s] : "";
}

const char *DebugLogModel::step_label_key(Step s) {
    static const char *keys[STEP_COUNT] = {
        "dbgc.step.start", "dbgc.step.btstack", "dbgc.step.connect", "dbgc.step.caps",
        "dbgc.step.codec", "dbgc.step.capture", "dbgc.step.config", "dbgc.step.encoder",
        "dbgc.step.stream", "dbgc.step.audio", "dbgc.step.link",
    };
    return (s >= 0 && s < STEP_COUNT) ? keys[s] : "";
}

const char *DebugLogModel::level_name(Level l) {
    switch (l) {
    case Level::Debug: return "DEBUG";
    case Level::Info:  return "INFO";
    case Level::Warn:  return "WARN";
    case Level::Error: return "ERROR";
    }
    return "";
}

DebugLogModel::DebugLogModel(const std::string &path)
    : path_(path)
    , timer_(this)
{
    Bind(wxEVT_TIMER, &DebugLogModel::OnTimer, this);
    poll();
    timer_.Start(POLL_MS);
}

DebugLogModel::~DebugLogModel() {
    timer_.Stop();
    if (file_) CloseHandle(static_cast<HANDLE>(file_));
}

void DebugLogModel::OnTimer(wxTimerEvent &) {
    poll();
}

const DebugLogModel::Entry *DebugLogModel::find(uint64_t seq) const {
    uint64_t first = first_seq();
    if (seq < first || seq >= next_seq_) return nullptr;
    return &entries_[static_cast<size_t>(seq - first)];
}

void DebugLogModel::poll() {
    if (!file_) {
        /* The CRT opened debug.log for writing with full sharing */
        HANDLE h = CreateFileA(path_.c_str(), GENERIC_READ,
                               FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                               nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
        if (h == INVALID_HANDLE_VALUE) return;
        file_ = h;
    }
    HANDLE h = static_cast<HANDLE>(file_);

    LARGE_INTEGER size{};
    if (!GetFileSizeEx(h, &size)) return;
    uint64_t file_size = static_cast<uint64_t>(size.QuadPart);
    if (file_size < offset_) { /* truncated */
        offset_ = 0;
        partial_.clear();
    }
    bool skip_first_line = false;
    if (first_read_ && file_size > MAX_INITIAL_BYTES) {
        offset_ = file_size - MAX_INITIAL_BYTES;
        skip_first_line = true;
    }
    if (file_size == offset_) {
        first_read_ = false;
        return;
    }

    bool live = !first_read_;
    wxString arrival = now_time();
    std::string buf;
    buf.resize(64 * 1024);
    while (offset_ < file_size) {
        LARGE_INTEGER pos{};
        pos.QuadPart = static_cast<LONGLONG>(offset_);
        if (!SetFilePointerEx(h, pos, nullptr, FILE_BEGIN)) break;
        DWORD want = static_cast<DWORD>(std::min<uint64_t>(buf.size(), file_size - offset_));
        DWORD got = 0;
        if (!ReadFile(h, &buf[0], want, &got, nullptr) || got == 0) break;
        offset_ += got;

        size_t start = 0;
        for (size_t i = 0; i < got; i++) {
            if (buf[i] != '\n') continue;
            partial_.append(buf, start, i - start);
            start = i + 1;
            if (skip_first_line) {
                skip_first_line = false;
            } else {
                add_line(partial_, arrival, live);
            }
            partial_.clear();
        }
        partial_.append(buf, start, got - start);
    }
    first_read_ = false;
}

void DebugLogModel::add_line(const std::string &raw_in, const wxString &arrival_time, bool live) {
    std::string raw = raw_in;
    while (!raw.empty() && (raw.back() == '\r' || raw.back() == '\n')) raw.pop_back();
    if (raw.empty()) return;

    wxString line = wxString::FromUTF8(raw.c_str());
    if (line.empty()) line = wxString(raw.c_str(), wxConvLocal);

    /* LOG_* prefix: "[hh:mm:ss.mmm LEVEL] " */
    Entry e;
    wxString prefix_level;
    if (line.length() > 21 && line[0] == '[' && line[3] == ':' && line[19] == ']') {
        e.time = line.Mid(1, 12);
        prefix_level = line.Mid(14, 5).Trim();
        line = line.Mid(21);
    } else {
        e.time = live ? arrival_time : last_time_;
    }
    last_time_ = e.time;
    e.seq = next_seq_++;
    e.level = classify(line, prefix_level);
    e.text = line;

    track(e);
    e.session = session_.id;
    entries_.push_back(std::move(e));
    while (entries_.size() > MAX_ENTRIES) entries_.pop_front();
}

void DebugLogModel::begin_session(const Entry &e) {
    uint32_t id = session_.id + 1;
    session_ = Session{};
    session_.id = id;
    session_.first_seq = e.seq;
    session_.start_time = e.time;
}

void DebugLogModel::set_step(Step s, StepStatus status, const Entry &e, const wxString &detail) {
    FlowStep &st = session_.steps[s];
    switch (status) {
    case StepStatus::Failed:
        if (st.status == StepStatus::Failed) return; /* keep the first cause */
        break;
    case StepStatus::Warning:
        if (st.status == StepStatus::Failed) return;
        break;
    case StepStatus::Done:
        if (st.status == StepStatus::Failed) return;
        if (st.status == StepStatus::Warning) { /* completed, but keep the warning */
            if (!detail.empty()) st.detail = detail;
            return;
        }
        break;
    case StepStatus::Running:
        /* Only the connection upkeep step goes back from finished to running */
        if (st.status == StepStatus::Failed) return;
        if (finished(st.status) && s != STEP_LINK) return;
        break;
    case StepStatus::Pending:
        break;
    }
    st.status = status;
    st.time = e.time;
    st.seq = e.seq;
    if (!detail.empty()) st.detail = detail;
}

void DebugLogModel::set_detail(Step s, const wxString &detail) {
    if (!detail.empty()) session_.steps[s].detail = detail;
}

void DebugLogModel::track(const Entry &e) {
    const wxString &t = e.text;

    /* ---- Adapter information (independent of connection attempts) ---- */
    if (t.StartsWith("BtAdapterEnumerator: found")) {
        adapter_lines_.erase(std::remove_if(adapter_lines_.begin(), adapter_lines_.end(),
            [](const wxString &l) { return l.StartsWith("BtAdapterEnumerator") || l.StartsWith("  "); }),
            adapter_lines_.end());
        adapter_lines_.insert(adapter_lines_.begin(), t);
        in_adapter_block_ = true;
    } else if (in_adapter_block_ && t.StartsWith("  ")) {
        /* Keep the enumeration together, ahead of the lines below */
        auto it = std::find_if(adapter_lines_.begin(), adapter_lines_.end(),
            [](const wxString &l) { return !l.StartsWith("BtAdapterEnumerator") && !l.StartsWith("  "); });
        adapter_lines_.insert(it, t);
    } else {
        in_adapter_block_ = false;
        if (t.StartsWith("BTstack: Controller ") || t.StartsWith("A2dpService: Using chip") ||
            t.StartsWith("BTstack: Realtek adapter PID") || t.StartsWith("BTstack: H4 over TCP")) {
            wxString key = t.BeforeFirst(' ') + " " + t.AfterFirst(' ').BeforeFirst(' ');
            adapter_lines_.erase(std::remove_if(adapter_lines_.begin(), adapter_lines_.end(),
                [&](const wxString &l) { return l.StartsWith(key); }), adapter_lines_.end());
            adapter_lines_.push_back(t);
        }
    }

    /* ---- Service state ---- */
    int state = -1;
    if (t.StartsWith("A2dpService: state=")) {
        long v = -1;
        t.Mid(19).BeforeFirst(' ').ToLong(&v);
        state = static_cast<int>(v);
        last_state_ = state;
        last_state_text_ = t.AfterFirst(' ').AfterFirst(' ');
        if (last_state_text_.StartsWith("text=")) last_state_text_ = last_state_text_.Mid(5);
    }

    /* ---- Connection attempts ---- */
    if (t.StartsWith("A2dpService: start_streaming called")) {
        begin_session(e);
        set_step(STEP_START, StepStatus::Done, e, wxString());
        return;
    }
    if (session_.id == 0 || !session_.stop_time.empty()) return;

    if (t.StartsWith("A2dpService: stop_streaming called")) {
        session_.stop_time = e.time;
        return;
    }

    if (state == 4) {
        /* Error state: blame the first step that did not complete */
        for (int s = 0; s < STEP_COUNT; s++) {
            if (!finished(session_.steps[s].status) || s == STEP_LINK) {
                set_step(static_cast<Step>(s), StepStatus::Failed, e, last_state_text_);
                break;
            }
        }
        return;
    }

    for (const Rule &r : RULES) {
        if (!t.Contains(r.needle)) continue;
        wxString detail;
        if (r.detail == DetailFrom::After) detail = after_needle(t, r.needle);
        else if (r.detail == DetailFrom::Whole) detail = strip_module(t);

        switch (r.action) {
        case Action::Done:    set_step(r.step, StepStatus::Done, e, detail); break;
        case Action::Running: set_step(r.step, StepStatus::Running, e, detail); break;
        case Action::Warning: set_step(r.step, StepStatus::Warning, e, detail); break;
        case Action::Failed:  set_step(r.step, StepStatus::Failed, e, detail); break;
        case Action::Detail:  set_detail(r.step, detail); break;
        }

        /* Follow-ups */
        if (r.step == STEP_BTSTACK && r.action == Action::Done &&
            session_.steps[STEP_BTSTACK].detail.empty()) {
            /* BTstack already up from an earlier attempt: show the controller */
            for (const wxString &l : adapter_lines_)
                if (l.StartsWith("BTstack: Controller "))
                    set_detail(STEP_BTSTACK, after_needle(l, "BTstack: Controller "));
        }
        if (r.step == STEP_AUDIO && r.action == Action::Done &&
            session_.steps[STEP_LINK].status == StepStatus::Pending) {
            set_step(STEP_LINK, StepStatus::Done, e, wxString());
        }
        if (r.step == STEP_LINK && r.action == Action::Running) {
            session_.reconnects++;
        }
        break;
    }
}
