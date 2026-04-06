/*
 * Main Frame - Primary window with menu, status, profile list
 * SPDX-License-Identifier: MIT
 */

#ifndef WX_MAIN_FRAME_H
#define WX_MAIN_FRAME_H

#include "a2dp_service.h"
#include "app_settings.h"
#include "profile_manager.h"
#include "zadig_helper.h"

#include <wx/wx.h>
#include <wx/taskbar.h>

/* Custom event IDs */
enum {
    ID_STATUS_UPDATE = wxID_HIGHEST + 1,
    ID_STREAM_INFO,
    ID_SCAN_COMPLETE,
    ID_DISCONNECT,
    ID_NEW_PROFILE,
    ID_OPEN_SETTINGS,
    ID_OPEN_ABOUT,
    ID_OPEN_FIRMWARE,
    ID_OPEN_ZADIG,
    ID_OPEN_VBCABLE,
    ID_CHECK_UPDATE,
    ID_OPEN_CONFIG,
    ID_SETTING_START_WIN,
    ID_SETTING_TRAY,

    ID_REPORT_BUG,
    ID_TRAY_DISCONNECT,
    ID_TRAY_EXIT,
    ID_TRAY_PROFILE_BASE = wxID_HIGHEST + 100,
    ID_THEME_BASE = wxID_HIGHEST + 300,
};

/* Payload for state updates posted from worker threads */
struct StatusPayload {
    A2dpService::State state;
    std::string text;
};

/* wxTaskBarIcon subclass */
class A2dpTrayIcon : public wxTaskBarIcon {
public:
    A2dpTrayIcon(class MainFrame *frame);
    wxMenu *CreatePopupMenu() override;
private:
    MainFrame *frame_;
};

/* Main application window */
class MainFrame : public wxFrame {
public:
    MainFrame();
    ~MainFrame() override;

    /* Public accessors for tray icon */
    A2dpService &service() { return service_; }
    ProfileManager &profiles() { return profile_mgr_; }
    AppSettings &settings() { return settings_; }
    int selected_profile() const { return selected_profile_; }

private:
    /* ---- Initialization ---- */
    void create_menu_bar();
    void create_ui();
    void apply_theme();
    void rebuild_profile_list();
    void update_status_display();

    /* ---- Event handlers ---- */
    void OnStatusUpdate(wxThreadEvent &evt);
    void OnStreamInfo(wxThreadEvent &evt);
    void OnScanComplete(wxThreadEvent &evt);
    void OnClose(wxCloseEvent &evt);
    void OnIconize(wxIconizeEvent &evt);

    /* Menu handlers */
    void OnNewProfile(wxCommandEvent &evt);
    void OnDisconnect(wxCommandEvent &evt);
    void OnOpenConfig(wxCommandEvent &evt);
    void OnExit(wxCommandEvent &evt);
    void OnOpenSettings(wxCommandEvent &evt);
    void OnToggleStartWithWindows(wxCommandEvent &evt);
    void OnToggleMinimizeToTray(wxCommandEvent &evt);

    void OnOpenFirmware(wxCommandEvent &evt);
    void OnOpenZadig(wxCommandEvent &evt);
    void OnOpenVBCable(wxCommandEvent &evt);
    void OnCheckUpdate(wxCommandEvent &evt);
    void OnOpenAbout(wxCommandEvent &evt);
    void OnReportBug(wxCommandEvent &evt);
    void OnLanguageChange(wxCommandEvent &evt);
    void OnThemeChange(wxCommandEvent &evt);

    /* Profile list handlers */
    void OnProfileClick(wxMouseEvent &evt);
    void OnProfileEdit(wxCommandEvent &evt);
    void OnProfileDelete(wxCommandEvent &evt);

    /* ---- Backend ---- */
    A2dpService service_;
    ProfileManager profile_mgr_;
    AppSettings settings_;
    int selected_profile_ = -1;

    /* ---- Tray ---- */
    A2dpTrayIcon *tray_icon_ = nullptr;

    /* ---- UI elements ---- */
    wxPanel       *main_panel_ = nullptr;
    wxStaticText  *status_label_ = nullptr;
    wxStaticText  *stream_info_label_ = nullptr;
    wxButton      *disconnect_btn_ = nullptr;
    wxButton      *add_profile_btn_ = nullptr;
    wxPanel       *firmware_bar_ = nullptr;
    wxScrolledWindow *profile_scroll_ = nullptr;
    wxBoxSizer    *profile_sizer_ = nullptr;

    /* ---- Tray state ---- */
    bool first_minimize_shown_ = false;

    /* ---- Cached state for display ---- */
    A2dpService::State current_state_ = A2dpService::State::Idle;
    std::string current_status_text_ = "Ready";
    A2dpService::StreamInfo current_stream_info_{};

    wxDECLARE_EVENT_TABLE();
};

#endif /* WX_MAIN_FRAME_H */
