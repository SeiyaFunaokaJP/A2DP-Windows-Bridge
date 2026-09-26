/*
 * Main Frame Implementation
 * SPDX-License-Identifier: MIT
 */

#include "wx_main_frame.h"
#include "wx_profile_dialog.h"
#include "wx_settings_dialog.h"
#include "wx_advanced_dialog.h"
#include "wx_link_quality_dialog.h"
#include "wx_about_dialog.h"
#include "wx_firmware_dialog.h"
#include "wx_zadig_dialog.h"
#include "zadig_helper.h"
#include "localization.h"
#include "theme_manager.h"
#include "system_integration.h"
#include "update_checker.h"
#include "hci_capture.h"

#include <wx/scrolwin.h>
#include <wx/statline.h>
#include <wx/clipbrd.h>
#include <wx/datetime.h>
#include <shellapi.h>

#include <algorithm>

#ifndef APP_VERSION
#define APP_VERSION "1.0.1"
#endif

wxDEFINE_EVENT(wxEVT_STATUS_UPDATE, wxThreadEvent);
wxDEFINE_EVENT(wxEVT_STREAM_INFO, wxThreadEvent);
wxDEFINE_EVENT(wxEVT_SCAN_COMPLETE, wxThreadEvent);
wxDEFINE_EVENT(wxEVT_UPDATE_CHECK_DONE, wxThreadEvent);

/* URLs from the GitHub API are plain ASCII */
static void open_url(const std::string &url) {
    std::wstring wide(url.begin(), url.end());
    ShellExecuteW(nullptr, L"open", wide.c_str(), nullptr, nullptr, SW_SHOWNORMAL);
}

wxBEGIN_EVENT_TABLE(MainFrame, wxFrame)
    EVT_CLOSE(MainFrame::OnClose)
    EVT_ICONIZE(MainFrame::OnIconize)
wxEND_EVENT_TABLE()

/* ======================================================================== */
/* Construction                                                              */
/* ======================================================================== */

MainFrame::MainFrame()
    : wxFrame(nullptr, wxID_ANY, wxString::Format("A2DPWB v%s", APP_VERSION),
              wxDefaultPosition, wxSize(520, 600))
{
    SetMinSize(wxSize(480, 400));
    /* Load settings and data */
    settings_.load();
    Localization::instance().load(settings_.language);
    profile_mgr_.load();
    service_.load_saved_devices();
    service_.set_bt_chip_pid(settings_.bt_chip_pid);
    service_.set_bt_chip_fw_stem(settings_.bt_chip_fw_stem);
    service_.set_debug_mode(settings_.debug_mode);
    service_.set_media_payload_limit(settings_.max_media_payload);
    /* Debug mode changes apply after a restart; the Debug menu follows the boot state */
    debug_active_ = settings_.debug_mode;
    service_.check_firmware_present();
    /* Set icon */
    SetIcon(wxIcon(wxT("APP_ICON"), wxBITMAP_TYPE_ICO_RESOURCE));

    /* Fixed window size — scrolling happens inside the profile list */

    /* Apply frame background */
    SetBackgroundColour(TM().get(ThemeColor::WindowBg));

    /* Build UI */
    create_menu_bar();
    create_ui();

    /* Wire up service callbacks (post events to GUI thread) */
    service_.set_state_callback([this](A2dpService::State state, const std::string &text) {
        auto *evt = new wxThreadEvent(wxEVT_STATUS_UPDATE);
        StatusPayload payload{state, text};
        evt->SetPayload(payload);
        wxQueueEvent(this, evt);
    });

    service_.set_stream_info_callback([this](const A2dpService::StreamInfo &info) {
        auto *evt = new wxThreadEvent(wxEVT_STREAM_INFO);
        evt->SetPayload(info);
        wxQueueEvent(this, evt);
    });

    service_.set_scan_complete_callback([this]() {
        auto *evt = new wxThreadEvent(wxEVT_SCAN_COMPLETE);
        wxQueueEvent(this, evt);
    });

    /* Bind thread events */
    Bind(wxEVT_STATUS_UPDATE, &MainFrame::OnStatusUpdate, this);
    Bind(wxEVT_STREAM_INFO, &MainFrame::OnStreamInfo, this);
    Bind(wxEVT_SCAN_COMPLETE, &MainFrame::OnScanComplete, this);
    Bind(wxEVT_UPDATE_CHECK_DONE, &MainFrame::OnCheckUpdateDone, this);

    /* System tray */
    tray_icon_ = new A2dpTrayIcon(this);
    wxIcon tray_ico = GetIcon();
    if (!tray_ico.IsOk()) {
        HICON hIcon = LoadIconW(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(101));
        if (hIcon)
            tray_ico.CreateFromHICON(hIcon);
    }
    if (!tray_ico.IsOk() || !tray_icon_->SetIcon(tray_ico, wxString::Format("A2DPWB v%s", APP_VERSION))) {
        delete tray_icon_;
        tray_icon_ = nullptr;
    }
    if (tray_icon_)
        tray_icon_->Bind(wxEVT_TASKBAR_BALLOON_CLICK, &MainFrame::OnTrayBalloonClick, this);

    /* Update startup registry */
    if (settings_.start_with_windows)
        RegisterStartup("--minimized");

    update_status_display();
    rebuild_profile_list();

    /* Automatic update check, at most once per day, silent unless outdated */
    if (settings_.check_updates_on_startup) {
        std::string today = wxDateTime::Now().FormatISODate().ToStdString();
        if (settings_.last_update_check != today) {
            settings_.last_update_check = today;
            settings_.save();
            start_update_check(true);
        }
    }
}

MainFrame::~MainFrame() {
    if (update_thread_.joinable())
        update_thread_.join();

    service_.stop_streaming();

    if (tray_icon_) {
        tray_icon_->RemoveIcon();
        delete tray_icon_;
        tray_icon_ = nullptr;
    }
}

/* ======================================================================== */
/* Menu Bar                                                                  */
/* ======================================================================== */

void MainFrame::create_menu_bar() {
    auto *menu_bar = new wxMenuBar();

    /* Profile menu (was File) */
    auto *file_menu = new wxMenu();
    file_menu->Append(ID_NEW_PROFILE, wxString::FromUTF8(L("profile.new")));
    file_menu->Append(ID_OPEN_LINK_QUALITY, wxString::FromUTF8(L("menu.file.link_quality")));
    file_menu->AppendSeparator();
    file_menu->Append(ID_OPEN_FIRMWARE, wxString::FromUTF8(L("firmware.title")));
    file_menu->AppendSeparator();
    file_menu->Append(ID_OPEN_CONFIG, wxString::FromUTF8(L("menu.file.open_config")));
    file_menu->AppendSeparator();
    file_menu->Append(wxID_EXIT, wxString::FromUTF8(L("menu.exit")));
    menu_bar->Append(file_menu, wxString::FromUTF8(L("menu.file")));

    /* Settings menu */
    auto *settings_menu = new wxMenu();

    /* Language submenu */
    auto *lang_menu = new wxMenu();
    auto langs = Localization::instance().get_available_languages();
    for (int i = 0; i < static_cast<int>(langs.size()); i++) {
        int id = wxID_HIGHEST + 200 + i;
        lang_menu->AppendRadioItem(id, wxString::FromUTF8(langs[i].display_name));
        if (langs[i].code == settings_.language)
            lang_menu->Check(id, true);
        Bind(wxEVT_MENU, &MainFrame::OnLanguageChange, this, id);
    }
    settings_menu->AppendSubMenu(lang_menu, wxString::FromUTF8(L("settings.language")));

    /* Theme submenu */
    auto *theme_menu = new wxMenu();
    {
        static const char *theme_keys[] = { "settings.theme_dark", "settings.theme_light", "settings.theme_system" };
        static const ThemeMode theme_modes[] = { ThemeMode::Dark, ThemeMode::Light, ThemeMode::System };
        auto cur_mode = TM().mode();
        for (int i = 0; i < 3; i++) {
            int id = ID_THEME_BASE + i;
            theme_menu->AppendRadioItem(id, wxString::FromUTF8(L(theme_keys[i])));
            if (theme_modes[i] == cur_mode)
                theme_menu->Check(id, true);
            Bind(wxEVT_MENU, &MainFrame::OnThemeChange, this, id);
        }
    }
    settings_menu->AppendSubMenu(theme_menu, wxString::FromUTF8(L("settings.theme")));

    settings_menu->AppendSeparator();
    settings_menu->AppendCheckItem(ID_SETTING_START_WIN, wxString::FromUTF8(L("settings.start_with_windows")));
    settings_menu->Check(ID_SETTING_START_WIN, settings_.start_with_windows);
    settings_menu->AppendCheckItem(ID_SETTING_TRAY, wxString::FromUTF8(L("settings.minimize_to_tray")));
    settings_menu->Check(ID_SETTING_TRAY, settings_.minimize_to_tray);
    settings_menu->AppendCheckItem(ID_SETTING_UPDATE_CHECK, wxString::FromUTF8(L("settings.check_updates_on_startup")));
    settings_menu->Check(ID_SETTING_UPDATE_CHECK, settings_.check_updates_on_startup);

    settings_menu->AppendSeparator();
    settings_menu->AppendCheckItem(ID_SETTING_DEBUG, wxString::FromUTF8(L("settings.debug_mode")));
    settings_menu->Check(ID_SETTING_DEBUG, settings_.debug_mode);
    settings_menu->Append(ID_OPEN_ADVANCED, wxString::FromUTF8(L("settings.advanced")));
    menu_bar->Append(settings_menu, wxString::FromUTF8(L("menu.settings")));

    /* Debug menu: only when the app was started in debug mode */
    if (debug_active_) {
        auto *debug_menu = new wxMenu();
        debug_menu->Append(ID_DEBUG_CAPTURE_START, wxString::FromUTF8(L("debug.capture_start")));
        debug_menu->Append(ID_DEBUG_CAPTURE_STOP, wxString::FromUTF8(L("debug.capture_stop")));
        debug_menu->AppendSeparator();
        debug_menu->Append(ID_DEBUG_OPEN_LOG, wxString::FromUTF8(L("debug.open_log")));
        debug_menu->Append(ID_OPEN_CONFIG, wxString::FromUTF8(L("menu.file.open_config")));
        menu_bar->Append(debug_menu, wxString::FromUTF8(L("menu.debug")));

        Bind(wxEVT_MENU, &MainFrame::OnDebugCaptureStart, this, ID_DEBUG_CAPTURE_START);
        Bind(wxEVT_MENU, &MainFrame::OnDebugCaptureStop, this, ID_DEBUG_CAPTURE_STOP);
        Bind(wxEVT_MENU, &MainFrame::OnDebugOpenLog, this, ID_DEBUG_OPEN_LOG);
        Bind(wxEVT_UPDATE_UI, [](wxUpdateUIEvent &e) { e.Enable(!hci_capture::active()); },
             ID_DEBUG_CAPTURE_START);
        Bind(wxEVT_UPDATE_UI, [](wxUpdateUIEvent &e) { e.Enable(hci_capture::active()); },
             ID_DEBUG_CAPTURE_STOP);
    }

    /* Help menu */
    auto *help_menu = new wxMenu();
    help_menu->Append(ID_OPEN_ABOUT, wxString::FromUTF8(L("help.about")));
    help_menu->AppendSeparator();
    help_menu->Append(ID_OPEN_ZADIG, wxString::FromUTF8(L("zadig.download")));
    help_menu->Append(ID_OPEN_VBCABLE, wxString::FromUTF8(L("vbcable.download")));
    help_menu->AppendSeparator();
    help_menu->Append(ID_CHECK_UPDATE, wxString::FromUTF8(L("update.check")));
    help_menu->Append(ID_REPORT_BUG, wxString::FromUTF8(L("menu.help.report_issue")));
    menu_bar->Append(help_menu, wxString::FromUTF8(L("menu.help")));

    SetMenuBar(menu_bar);

    /* Bind menu events */
    Bind(wxEVT_MENU, &MainFrame::OnNewProfile, this, ID_NEW_PROFILE);
    Bind(wxEVT_MENU, &MainFrame::OnOpenConfig, this, ID_OPEN_CONFIG);
    Bind(wxEVT_MENU, &MainFrame::OnExit, this, wxID_EXIT);
    Bind(wxEVT_MENU, &MainFrame::OnToggleStartWithWindows, this, ID_SETTING_START_WIN);
    Bind(wxEVT_MENU, &MainFrame::OnToggleMinimizeToTray, this, ID_SETTING_TRAY);
    Bind(wxEVT_MENU, &MainFrame::OnToggleUpdateCheck, this, ID_SETTING_UPDATE_CHECK);
    Bind(wxEVT_MENU, &MainFrame::OnToggleDebugMode, this, ID_SETTING_DEBUG);
    Bind(wxEVT_MENU, &MainFrame::OnOpenAdvanced, this, ID_OPEN_ADVANCED);
    Bind(wxEVT_MENU, &MainFrame::OnOpenLinkQuality, this, ID_OPEN_LINK_QUALITY);

    Bind(wxEVT_MENU, &MainFrame::OnOpenFirmware, this, ID_OPEN_FIRMWARE);
    Bind(wxEVT_BUTTON, &MainFrame::OnOpenFirmware, this, ID_OPEN_FIRMWARE);
    Bind(wxEVT_MENU, &MainFrame::OnOpenZadig, this, ID_OPEN_ZADIG);
    Bind(wxEVT_MENU, &MainFrame::OnOpenVBCable, this, ID_OPEN_VBCABLE);
    Bind(wxEVT_MENU, &MainFrame::OnCheckUpdate, this, ID_CHECK_UPDATE);
    Bind(wxEVT_MENU, &MainFrame::OnOpenAbout, this, ID_OPEN_ABOUT);
    Bind(wxEVT_MENU, &MainFrame::OnReportBug, this, ID_REPORT_BUG);
}

/* ======================================================================== */
/* UI Layout                                                                 */
/* ======================================================================== */

void MainFrame::create_ui() {
    main_panel_ = new wxPanel(this);
    main_panel_->SetBackgroundColour(TM().get(ThemeColor::WindowBg));
    auto *vbox = new wxBoxSizer(wxVERTICAL);

    /* ---- Firmware warning bar (hidden if present) ---- */
    firmware_bar_ = new wxPanel(main_panel_);
    firmware_bar_->SetBackgroundColour(TM().get(ThemeColor::FirmwareBarBg));
    auto *fw_sizer = new wxBoxSizer(wxHORIZONTAL);
    auto *fw_text = new wxStaticText(firmware_bar_, wxID_ANY,
        wxString::FromUTF8(L("firmware.missing_short")));
    fw_text->SetForegroundColour(TM().get(ThemeColor::FirmwareBarText));
    fw_sizer->Add(fw_text, 1, wxALIGN_CENTER_VERTICAL | wxLEFT, 8);
    auto *fw_btn = new wxButton(firmware_bar_, ID_OPEN_FIRMWARE,
        wxString::FromUTF8(L("firmware.setup")), wxDefaultPosition, wxDefaultSize, wxBU_EXACTFIT);
    fw_sizer->Add(fw_btn, 0, wxALL, 2);
    firmware_bar_->SetSizer(fw_sizer);
    firmware_bar_->Show(!service_.is_firmware_present());
    vbox->Add(firmware_bar_, 0, wxEXPAND);

    /* ---- Status section ---- */
    status_panel_ = new wxPanel(main_panel_);
    auto *status_panel = status_panel_;
    auto *status_sizer = new wxBoxSizer(wxHORIZONTAL);

    /* Width set by fit_status_label() so a long device name is ellipsized
     * instead of pushing the disconnect button out of the window */
    status_label_ = new wxStaticText(status_panel, wxID_ANY, wxString::FromUTF8(L("status.idle")),
        wxDefaultPosition, wxDefaultSize, wxST_ELLIPSIZE_END | wxST_NO_AUTORESIZE);
    auto font = status_label_->GetFont();
    font.SetPointSize(font.GetPointSize() + 2);
    font.SetWeight(wxFONTWEIGHT_BOLD);
    status_label_->SetFont(font);
    status_sizer->Add(status_label_, 0, wxALIGN_CENTER_VERTICAL | wxLEFT, 8);

    stream_info_label_ = new wxStaticText(status_panel, wxID_ANY, "",
        wxDefaultPosition, wxDefaultSize, wxST_ELLIPSIZE_END);
    stream_info_label_->SetForegroundColour(TM().get(ThemeColor::TextStreamInfo));
    stream_info_label_->SetCursor(wxCursor(wxCURSOR_HAND));
    stream_info_label_->SetMinSize(wxSize(0, -1)); /* ellipsized; never widens the row */
    stream_info_label_->Bind(wxEVT_LEFT_DOWN, [this](wxMouseEvent &) {
        if (current_state_ == A2dpService::State::Error && !current_status_text_.empty()) {
            if (wxTheClipboard->Open()) {
                wxTheClipboard->SetData(new wxTextDataObject(wxString::FromUTF8(current_status_text_)));
                wxTheClipboard->Close();
            }
        } else if (current_state_ == A2dpService::State::Streaming) {
            LinkQualityDialog::ShowFor(this);
        }
    });
    status_sizer->Add(stream_info_label_, 1, wxALIGN_CENTER_VERTICAL | wxLEFT, 12);

    disconnect_btn_ = new wxButton(status_panel, ID_DISCONNECT,
        wxString::FromUTF8(L("connection.disconnect")), wxDefaultPosition, wxDefaultSize, wxBU_EXACTFIT);
    disconnect_btn_->Show(false);
    status_sizer->Add(disconnect_btn_, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    Bind(wxEVT_BUTTON, &MainFrame::OnDisconnect, this, ID_DISCONNECT);

    status_panel->SetSizer(status_sizer);
    status_panel->Bind(wxEVT_SIZE, [this](wxSizeEvent &evt) {
        fit_status_label();
        evt.Skip(); /* default handler lays the row out */
    });
    vbox->Add(status_panel, 0, wxEXPAND | wxTOP | wxBOTTOM, 6);

    /* Separator */
    vbox->Add(new wxStaticLine(main_panel_), 0, wxEXPAND | wxLEFT | wxRIGHT, 4);

    /* ---- Profile list (scrollable) ---- */
    profile_scroll_ = new wxScrolledWindow(main_panel_, wxID_ANY,
        wxDefaultPosition, wxDefaultSize, wxVSCROLL);
    profile_scroll_->SetBackgroundColour(TM().get(ThemeColor::WindowBg));
    profile_scroll_->SetScrollRate(0, 10);
    profile_sizer_ = new wxBoxSizer(wxVERTICAL);
    profile_scroll_->SetSizer(profile_sizer_);
    vbox->Add(profile_scroll_, 1, wxEXPAND | wxALL, 4);

    add_profile_btn_ = new wxButton(main_panel_, ID_NEW_PROFILE,
        wxString::FromUTF8(L("profile.new")));
    vbox->Add(add_profile_btn_, 0, wxEXPAND | wxLEFT | wxRIGHT | wxBOTTOM, 8);
    Bind(wxEVT_BUTTON, &MainFrame::OnNewProfile, this, ID_NEW_PROFILE);

    main_panel_->SetSizer(vbox);
}

/* ======================================================================== */
/* Theme                                                                     */
/* ======================================================================== */

void MainFrame::apply_theme() {
    main_panel_->SetBackgroundColour(TM().get(ThemeColor::WindowBg));
    profile_scroll_->SetBackgroundColour(TM().get(ThemeColor::WindowBg));
    firmware_bar_->SetBackgroundColour(TM().get(ThemeColor::FirmwareBarBg));
    stream_info_label_->SetForegroundColour(TM().get(ThemeColor::TextStreamInfo));
    SetBackgroundColour(TM().get(ThemeColor::WindowBg));
    rebuild_profile_list();
    update_status_display();
    main_panel_->Refresh();
    Refresh();
}

/* ======================================================================== */
/* Profile List                                                              */
/* ======================================================================== */

void MainFrame::rebuild_profile_list() {
    /* Clear existing */
    profile_sizer_->Clear(true);

    auto &profs = profile_mgr_.profiles();
    if (profs.empty()) {
        auto *empty = new wxStaticText(profile_scroll_, wxID_ANY,
            wxString::FromUTF8(L("profile.empty")));
        empty->SetForegroundColour(TM().get(ThemeColor::TextMuted));
        profile_sizer_->Add(empty, 0, wxALL, 12);
    } else {
        for (int i = 0; i < static_cast<int>(profs.size()); i++) {
            auto &p = profs[i];

            auto *row = new wxPanel(profile_scroll_);
            bool selected = (selected_profile_ == i);
            wxColour row_bg = selected ? TM().get(ThemeColor::ProfileBgSelected) : TM().get(ThemeColor::ProfileBgNormal);
            row->SetBackgroundColour(row_bg);

            auto *row_sizer = new wxBoxSizer(wxHORIZONTAL);

            /* Text info */
            auto *text_sizer = new wxBoxSizer(wxVERTICAL);
            const char *dname = p.device_name.empty() ? p.device_address.c_str() : p.device_name.c_str();
            auto *name_text = new wxStaticText(row, wxID_ANY, wxString::FromUTF8(dname),
                wxDefaultPosition, wxDefaultSize, wxST_ELLIPSIZE_END);
            name_text->SetMinSize(wxSize(50, -1));
            auto name_font = name_text->GetFont();
            name_font.SetWeight(wxFONTWEIGHT_BOLD);
            name_text->SetFont(name_font);
            name_text->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
            name_text->SetBackgroundColour(row_bg);

            /* Codec display name */
            const char *codec_str;
            if      (p.codec == "ldac")   codec_str = "LDAC";
            else if (p.codec == "aptxhd") codec_str = "aptX HD";
            else if (p.codec == "aptxll") codec_str = "aptX LL";
            else if (p.codec == "aptx")   codec_str = "aptX";
            else if (p.codec == "sbc")    codec_str = "SBC";
            else if (p.codec == "aac")    codec_str = "AAC";
            else if (p.codec == "auto")   codec_str = L("codec.auto");
            else                          codec_str = p.codec.c_str();

            /* Quality display name with bitrate */
            const char *q_name = (p.quality == "hq") ? L("quality.high") :
                                 (p.quality == "sq") ? L("quality.standard") :
                                 L("quality.mobile");
            int q_idx = (p.quality == "hq") ? 0 : (p.quality == "sq") ? 1 : 2;
            char quality_buf[64];
            bool is_441 = (p.sample_rate == 44100 || p.sample_rate == 88200);
            if (p.codec == "ldac") {
                static const int rates_48[] = {990, 660, 330};
                static const int rates_44[] = {909, 606, 303};
                const int *r = is_441 ? rates_44 : rates_48;
                snprintf(quality_buf, sizeof(quality_buf), "%s(%dkbps)", q_name, r[q_idx]);
            } else if (p.codec == "aac") {
                static const int rates[] = {256, 192, 128};
                snprintf(quality_buf, sizeof(quality_buf), "%s(%dkbps)", q_name, rates[q_idx]);
            } else if (p.codec == "sbc") {
                static const char *rates[] = {"~345", "~249", "~153"};
                snprintf(quality_buf, sizeof(quality_buf), "%s(%skbps)", q_name, rates[q_idx]);
            } else {
                snprintf(quality_buf, sizeof(quality_buf), "%s", q_name);
            }
            const char *quality_str = quality_buf;

            char sr_buf[16], bd_buf[16];
            if (p.sample_rate > 0) snprintf(sr_buf, sizeof(sr_buf), "%ukHz", p.sample_rate / 1000);
            else snprintf(sr_buf, sizeof(sr_buf), "Auto");
            if (p.bit_depth > 0) snprintf(bd_buf, sizeof(bd_buf), "%ubit", p.bit_depth);
            else snprintf(bd_buf, sizeof(bd_buf), "Auto");
            char detail[256];
            snprintf(detail, sizeof(detail), "%s / %s / %s / %s", codec_str, quality_str, sr_buf, bd_buf);

            auto *detail_text = new wxStaticText(row, wxID_ANY, wxString::FromUTF8(detail),
                wxDefaultPosition, wxDefaultSize, wxST_ELLIPSIZE_END);
            detail_text->SetMinSize(wxSize(50, -1));
            detail_text->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
            detail_text->SetBackgroundColour(row_bg);

            text_sizer->Add(name_text, 0, wxEXPAND | wxBOTTOM, 2);
            text_sizer->Add(detail_text, 0, wxEXPAND);
            row_sizer->Add(text_sizer, 1, wxALL | wxALIGN_CENTER_VERTICAL, 8);

            /* Edit button */
            auto *edit_btn = new wxButton(row, wxID_ANY, L"\u270F",
                wxDefaultPosition, wxSize(30, 30));
            auto edit_font = edit_btn->GetFont();
            edit_font.SetPointSize(edit_font.GetPointSize() + 2);
            edit_btn->SetFont(edit_font);
            edit_btn->SetToolTip(wxString::FromUTF8(L("profile.edit")));
            row_sizer->Add(edit_btn, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 4);

            /* Delete button */
            auto *del_btn = new wxButton(row, wxID_ANY, L"\u2716",
                wxDefaultPosition, wxSize(30, 30));
            auto del_font = del_btn->GetFont();
            del_font.SetPointSize(del_font.GetPointSize() + 2);
            del_btn->SetFont(del_font);
            del_btn->SetToolTip(wxString::FromUTF8(L("profile.delete")));
            del_btn->SetForegroundColour(TM().get(ThemeColor::DeleteButton));
            row_sizer->Add(del_btn, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);

            row->SetSizer(row_sizer);

            /* Store profile index in client data */
            int profile_idx = i;

            /* Click on row: connect (or switch if already streaming) */
            row->Bind(wxEVT_LEFT_DOWN, [this, profile_idx](wxMouseEvent &) {
                CallAfter([this, profile_idx]() {
                    auto state = service_.state();
                    if (state == A2dpService::State::Streaming ||
                        state == A2dpService::State::Connecting) {
                        service_.stop_streaming();
                    }
                    selected_profile_ = profile_idx;
                    auto &prof = profile_mgr_.profiles()[profile_idx];
                    settings_.last_profile = prof.name;
                    settings_.save();
                    service_.start_streaming(prof);
                    rebuild_profile_list();
                });
            });
            name_text->Bind(wxEVT_LEFT_DOWN, [this, profile_idx, row](wxMouseEvent &evt) {
                wxPostEvent(row, evt);
            });
            detail_text->Bind(wxEVT_LEFT_DOWN, [this, profile_idx, row](wxMouseEvent &evt) {
                wxPostEvent(row, evt);
            });

            edit_btn->Bind(wxEVT_BUTTON, [this, profile_idx](wxCommandEvent &) {
                ProfileDialog dlg(this, &service_, &profile_mgr_, profile_idx);
                if (dlg.ShowModal() == wxID_OK) {
                    /* Defer to avoid use-after-free (rebuild destroys this button) */
                    CallAfter([this, profile_idx]() {
                        fprintf(stderr, "CallAfter: rebuild_profile_list (edit idx=%d)\n", profile_idx);
                        fflush(stderr);
                        rebuild_profile_list();
                        /* If the edited profile is currently streaming, reconnect */
                        auto state = service_.state();
                        if (profile_idx == selected_profile_ &&
                            (state == A2dpService::State::Streaming ||
                             state == A2dpService::State::Connecting)) {
                            service_.stop_streaming();
                            auto &prof = profile_mgr_.profiles()[profile_idx];
                            service_.start_streaming(prof);
                        }
                    });
                }
            });

            del_btn->Bind(wxEVT_BUTTON, [this, profile_idx](wxCommandEvent &) {
                CallAfter([this, profile_idx]() {
                    if (wxMessageBox(
                            wxString::FromUTF8(L("modal.confirm_delete_profile")),
                            wxString::FromUTF8(L("modal.confirm_title")),
                            wxYES_NO | wxICON_WARNING, this) != wxYES)
                        return;
                    profile_mgr_.remove(profile_idx);
                    if (selected_profile_ >= static_cast<int>(profile_mgr_.profiles().size()))
                        selected_profile_ = static_cast<int>(profile_mgr_.profiles().size()) - 1;
                    rebuild_profile_list();
                });
            });

            profile_sizer_->Add(row, 0, wxEXPAND | wxBOTTOM, 2);
        }
    }

    profile_scroll_->FitInside();
    profile_scroll_->Layout();
}

/* ======================================================================== */
/* Status Display                                                            */
/* ======================================================================== */

void MainFrame::update_status_display() {
    wxColour color;
    const char *label;
    switch (current_state_) {
    case A2dpService::State::Idle:         color = TM().get(ThemeColor::StatusIdle);         label = L("status.idle"); break;
    case A2dpService::State::Connecting:   color = TM().get(ThemeColor::StatusConnecting);   label = L("status.connecting"); break;
    case A2dpService::State::Streaming:    color = TM().get(ThemeColor::StatusStreaming);     label = L("status.streaming"); break;
    case A2dpService::State::Reconnecting: color = TM().get(ThemeColor::StatusReconnecting); label = L("status.reconnecting"); break;
    case A2dpService::State::Error:        color = TM().get(ThemeColor::StatusError);        label = L("status.error"); break;
    }

    wxString display_label = wxString::FromUTF8(label);
    if (current_state_ == A2dpService::State::Streaming &&
        selected_profile_ >= 0 &&
        selected_profile_ < static_cast<int>(profile_mgr_.profiles().size())) {
        const auto &prof = profile_mgr_.profiles()[selected_profile_];
        const std::string &dname = prof.device_name.empty() ? prof.device_address : prof.device_name;
        if (!dname.empty())
            display_label += " (" + wxString::FromUTF8(dname) + ")";
    }
    status_label_->SetLabel(display_label);
    status_label_->SetForegroundColour(color);

    bool show_disconnect = (current_state_ == A2dpService::State::Streaming ||
                            current_state_ == A2dpService::State::Connecting ||
                            current_state_ == A2dpService::State::Reconnecting);
    disconnect_btn_->Show(show_disconnect);
    fit_status_label();

    if (current_state_ == A2dpService::State::Streaming) {
        stream_info_label_->SetLabel("");
        stream_info_label_->UnsetToolTip();
    } else if (current_state_ == A2dpService::State::Connecting ||
               current_state_ == A2dpService::State::Reconnecting) {
        stream_info_label_->SetLabel(wxString::FromUTF8(current_status_text_));
        stream_info_label_->UnsetToolTip();
    } else if (current_state_ == A2dpService::State::Error) {
        wxString err_display = wxString::FromUTF8("- " + current_status_text_);
        stream_info_label_->SetLabel(err_display);
        wxString tip = wxString::FromUTF8(current_status_text_) + "\n"
                     + wxString::FromUTF8(L("error.click_to_copy_hint"));
        stream_info_label_->SetToolTip(tip);
    } else {
        stream_info_label_->SetLabel("");
        stream_info_label_->UnsetToolTip();
    }

    /* Update tray tooltip */
    if (tray_icon_) {
        wxString tooltip = (current_state_ == A2dpService::State::Streaming)
            ? wxString::FromUTF8(L("tray.tooltip_streaming"))
            : wxString::FromUTF8(L("tray.tooltip_idle"));
        if (current_state_ == A2dpService::State::Streaming &&
            selected_profile_ >= 0 &&
            selected_profile_ < static_cast<int>(profile_mgr_.profiles().size())) {
            const auto &prof = profile_mgr_.profiles()[selected_profile_];
            const std::string &dname = prof.device_name.empty() ? prof.device_address : prof.device_name;
            if (!dname.empty())
                tooltip += " (" + wxString::FromUTF8(dname) + ")";
        }
        tray_icon_->SetIcon(GetIcon(), tooltip);
    }

    main_panel_->Layout();
}

/* Give the status label its text width, but no more than the status row
 * leaves beside the disconnect button; the rest is ellipsized. */
void MainFrame::fit_status_label() {
    wxString label = status_label_->GetLabel();
    int want = status_label_->GetTextExtent(label).x + 2;
    int avail = status_panel_->GetClientSize().x - 8 - 12; /* label and stream info margins */
    if (disconnect_btn_->IsShown())
        avail -= disconnect_btn_->GetBestSize().x + 8;
    int width = std::max(0, std::min(want, avail));
    status_label_->SetMinSize(wxSize(width, -1));
    if (width < want)
        status_label_->SetToolTip(label);
    else
        status_label_->UnsetToolTip();
}

/* ======================================================================== */
/* Thread Event Handlers                                                     */
/* ======================================================================== */

void MainFrame::OnStatusUpdate(wxThreadEvent &evt) {
    auto payload = evt.GetPayload<StatusPayload>();
    current_state_ = payload.state;
    current_status_text_ = payload.text;
    update_status_display();
}

void MainFrame::OnStreamInfo(wxThreadEvent &evt) {
    current_stream_info_ = evt.GetPayload<A2dpService::StreamInfo>();
    update_status_display();
}

void MainFrame::OnScanComplete(wxThreadEvent &) {
    /* Dialogs handle their own scan completion refresh */
}

/* ======================================================================== */
/* Window Events                                                             */
/* ======================================================================== */

void MainFrame::OnClose(wxCloseEvent &evt) {
    if (settings_.minimize_to_tray && tray_icon_ && evt.CanVeto()) {
        Hide();
        if (!first_minimize_shown_) {
            tray_icon_->ShowBalloon(
                L"A2DPWB",
                wxString::FromUTF8(L("tray.minimized_hint")),
                15000, wxICON_INFORMATION);
            first_minimize_shown_ = true;
        }
        evt.Veto();
        return;
    }
    if (tray_icon_) {
        tray_icon_->RemoveIcon();
        delete tray_icon_;
        tray_icon_ = nullptr;
    }
    Destroy();
}

void MainFrame::OnIconize(wxIconizeEvent &evt) {
    if (evt.IsIconized() && settings_.minimize_to_tray && tray_icon_) {
        Show(false);
    }
    evt.Skip();
}

/* ======================================================================== */
/* Menu Handlers                                                             */
/* ======================================================================== */

void MainFrame::OnNewProfile(wxCommandEvent &) {
    ProfileDialog dlg(this, &service_, &profile_mgr_, -1);
    if (dlg.ShowModal() == wxID_OK) {
        rebuild_profile_list();
    }
}

void MainFrame::OnDisconnect(wxCommandEvent &) {
    service_.stop_streaming();
}

void MainFrame::OnOpenConfig(wxCommandEvent &) {
    std::string cfg = service_.get_config_dir();
    ShellExecuteA(nullptr, "open", cfg.c_str(), nullptr, nullptr, SW_SHOWNORMAL);
}

void MainFrame::OnExit(wxCommandEvent &) {
    Close(true);
}

void MainFrame::OnOpenSettings(wxCommandEvent &) {
    SettingsDialog dlg(this, &settings_);
    if (dlg.ShowModal() == wxID_OK) {
        apply_theme();
    }
}

void MainFrame::OnToggleStartWithWindows(wxCommandEvent &) {
    settings_.start_with_windows = !settings_.start_with_windows;
    if (settings_.start_with_windows)
        RegisterStartup("--minimized");
    else
        UnregisterStartup();
    settings_.save();
}

void MainFrame::OnToggleMinimizeToTray(wxCommandEvent &) {
    settings_.minimize_to_tray = !settings_.minimize_to_tray;
    settings_.save();
}

void MainFrame::OnToggleUpdateCheck(wxCommandEvent &) {
    settings_.check_updates_on_startup = !settings_.check_updates_on_startup;
    settings_.save();
}

void MainFrame::OnToggleDebugMode(wxCommandEvent &) {
    /* debug.log is opened in A2dpBridgeApp::OnInit and the HCI dump when
     * BTstack is first initialized, so the change applies after a restart. */
    settings_.debug_mode = !settings_.debug_mode;
    settings_.save();
    wxMessageBox(
        wxString::FromUTF8(L(settings_.debug_mode ? "settings.debug_mode_enabled"
                                                  : "settings.debug_mode_disabled")),
        wxString::FromUTF8(L("settings.debug_mode")),
        wxOK | wxICON_INFORMATION, this);
}

void MainFrame::OnOpenFirmware(wxCommandEvent &) {
    FirmwareDialog dlg(this, &service_, &settings_);
    dlg.ShowModal();
    if (dlg.chip_changed() || service_.was_firmware_updated()) {
        /* Force BTstack reinit on next connect to pick up new chip/firmware */
        service_.reset_btstack();
    }
    firmware_bar_->Show(!service_.is_firmware_present());
    main_panel_->Layout();
}

void MainFrame::OnOpenZadig(wxCommandEvent &) {
    ZadigHelper::open_zadig_website();
}

void MainFrame::OnOpenVBCable(wxCommandEvent &) {
    ShellExecuteW(nullptr, L"open", L"https://vb-audio.com/Cable/", nullptr, nullptr, SW_SHOWNORMAL);
}

void MainFrame::OnReportBug(wxCommandEvent &) {
    ShellExecuteW(nullptr, L"open", L"https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/new/choose", nullptr, nullptr, SW_SHOWNORMAL);
}

void MainFrame::OnCheckUpdate(wxCommandEvent &) {
    start_update_check(false);
}

void MainFrame::start_update_check(bool silent) {
    bool expected = false;
    if (!update_check_running_.compare_exchange_strong(expected, true))
        return; /* a check is already in flight */
    if (update_thread_.joinable())
        update_thread_.join(); /* reap the previous, already-finished check */

    update_thread_ = std::thread([this, silent] {
        UpdateCheckResult r = CheckLatestRelease();
        auto *evt = new wxThreadEvent(wxEVT_UPDATE_CHECK_DONE);
        evt->SetPayload(r);
        evt->SetInt(silent ? 1 : 0);
        wxQueueEvent(this, evt);
    });
}

void MainFrame::OnCheckUpdateDone(wxThreadEvent &evt) {
    update_check_running_ = false;
    UpdateCheckResult result = evt.GetPayload<UpdateCheckResult>();
    bool silent = evt.GetInt() != 0;

    bool newer = result.ok &&
                 CompareVersions(result.latest_version, APP_VERSION) > 0;

    if (silent) {
        if (!newer)
            return;
        /* Started hidden in the tray (e.g. via autostart): use a balloon
         * notification instead of popping a modal dialog. */
        if (tray_icon_ && !IsShown()) {
            pending_update_url_ = result.release_url;
            tray_icon_->ShowBalloon(
                wxString::FromUTF8(L("update.available_title")),
                wxString::Format(wxString::FromUTF8(L("update.balloon")),
                                 wxString::FromUTF8(result.latest_version.c_str())),
                15000, wxICON_INFORMATION);
            return;
        }
    }

    if (newer) {
        int answer = wxMessageBox(
            wxString::Format(wxString::FromUTF8(L("update.available")),
                             wxString::FromUTF8(result.latest_version.c_str()),
                             APP_VERSION),
            wxString::FromUTF8(L("update.available_title")),
            wxYES_NO | wxICON_INFORMATION, this);
        if (answer == wxYES)
            open_url(result.release_url);
    } else if (result.ok) {
        wxMessageBox(
            wxString::Format(wxString::FromUTF8(L("update.uptodate")), APP_VERSION),
            wxString::FromUTF8(L("update.check")),
            wxOK | wxICON_INFORMATION, this);
    } else {
        int answer = wxMessageBox(
            wxString::FromUTF8(L("update.error")),
            wxString::FromUTF8(L("update.check")),
            wxYES_NO | wxICON_ERROR, this);
        if (answer == wxYES)
            open_url(kReleasesPageUrl);
    }
}

void MainFrame::OnTrayBalloonClick(wxTaskBarIconEvent &) {
    if (pending_update_url_.empty())
        return;
    open_url(pending_update_url_);
    pending_update_url_.clear();
}

void MainFrame::OnOpenAdvanced(wxCommandEvent &) {
    AdvancedDialog dlg(this, &settings_);
    if (dlg.ShowModal() == wxID_OK)
        service_.set_media_payload_limit(settings_.max_media_payload);
}

void MainFrame::OnOpenLinkQuality(wxCommandEvent &) {
    LinkQualityDialog::ShowFor(this);
}

void MainFrame::update_title() {
    wxString title = wxString::Format("A2DPWB v%s", APP_VERSION);
    if (hci_capture::active())
        title += wxString::FromUTF8(L("debug.capture_title_suffix"));
    SetTitle(title);
}

void MainFrame::OnDebugCaptureStart(wxCommandEvent &) {
    char stamp[32];
    SYSTEMTIME st;
    GetLocalTime(&st);
    snprintf(stamp, sizeof(stamp), "%04u%02u%02u_%02u%02u%02u", st.wYear, st.wMonth, st.wDay,
             st.wHour, st.wMinute, st.wSecond);
    std::string path = service_.get_config_dir() + "\\hci_" + stamp + ".pklg";
    if (!hci_capture::start(path)) {
        wxMessageBox(wxString::Format(wxString::FromUTF8(L("debug.capture_failed")),
                                      wxString::FromUTF8(path.c_str())),
                     wxString::FromUTF8(L("menu.debug")), wxOK | wxICON_ERROR, this);
    }
    update_title();
}

void MainFrame::OnDebugCaptureStop(wxCommandEvent &) {
    hci_capture::stop();
    update_title();
    std::string path = hci_capture::path();
    double mb = hci_capture::bytes_written() / (1024.0 * 1024.0);
    int answer = wxMessageBox(
        wxString::Format(wxString::FromUTF8(L("debug.capture_saved")),
                         wxString::FromUTF8(path.c_str()), mb),
        wxString::FromUTF8(L("menu.debug")), wxYES_NO | wxICON_INFORMATION, this);
    if (answer == wxYES) {
        std::string args = "/select,\"" + path + "\"";
        ShellExecuteA(nullptr, "open", "explorer.exe", args.c_str(), nullptr, SW_SHOWNORMAL);
    }
}

void MainFrame::OnDebugOpenLog(wxCommandEvent &) {
    std::string log = service_.get_config_dir() + "\\debug.log";
    ShellExecuteA(nullptr, "open", log.c_str(), nullptr, nullptr, SW_SHOWNORMAL);
}

void MainFrame::OnOpenAbout(wxCommandEvent &) {
    AboutDialog dlg(this);
    dlg.ShowModal();
}

void MainFrame::OnLanguageChange(wxCommandEvent &evt) {
    int idx = evt.GetId() - (wxID_HIGHEST + 200);
    auto langs = Localization::instance().get_available_languages();
    if (idx >= 0 && idx < static_cast<int>(langs.size())) {
        settings_.language = langs[idx].code;
        Localization::instance().load(settings_.language);
        settings_.save();
        /* Rebuild the entire UI with new language */
        SetMenuBar(nullptr);
        create_menu_bar();
        disconnect_btn_->SetLabel(wxString::FromUTF8(L("connection.disconnect")));
        add_profile_btn_->SetLabel(wxString::FromUTF8(L("profile.new")));
        rebuild_profile_list();
        update_status_display();
        apply_theme();
    }
}

void MainFrame::OnThemeChange(wxCommandEvent &evt) {
    static const ThemeMode modes[] = { ThemeMode::Dark, ThemeMode::Light, ThemeMode::System };
    int idx = evt.GetId() - ID_THEME_BASE;
    if (idx >= 0 && idx <= 2) {
        TM().set_mode(modes[idx]);
        settings_.theme = ThemeManager::mode_to_string(modes[idx]);
        settings_.save();
        apply_theme();
    }
}

/* ======================================================================== */
/* Profile Handlers                                                          */
/* ======================================================================== */

void MainFrame::OnProfileClick(wxMouseEvent &) { /* handled inline in lambdas */ }
void MainFrame::OnProfileEdit(wxCommandEvent &) { /* handled inline in lambdas */ }
void MainFrame::OnProfileDelete(wxCommandEvent &) { /* handled inline in lambdas */ }


/* ======================================================================== */
/* Update Bar Handlers                                                       */
/* ======================================================================== */

/* ======================================================================== */
/* Tray Icon                                                                 */
/* ======================================================================== */

A2dpTrayIcon::A2dpTrayIcon(MainFrame *frame) : frame_(frame) {
    Bind(wxEVT_TASKBAR_LEFT_UP, [this](wxTaskBarIconEvent &) {
        if (!frame_->IsShown())
            frame_->Show(true);
        frame_->Iconize(false);
        frame_->Raise();
        frame_->SetFocus();
    });
}

wxMenu *A2dpTrayIcon::CreatePopupMenu() {
    auto *menu = new wxMenu();

    /* Profile quick-connect */
    auto &profs = frame_->profiles().profiles();
    if (!profs.empty()) {
        for (int i = 0; i < static_cast<int>(profs.size()) && i < 10; i++) {
            int id = ID_TRAY_PROFILE_BASE + i;
            wxString label = wxString::FromUTF8(
                profs[i].device_name.empty() ? profs[i].device_address : profs[i].device_name);
            if (i == frame_->selected_profile())
                label = L"\x25B6 " + label;  /* Play symbol for active */
            menu->Append(id, label);
        }
    }

    menu->AppendSeparator();
    bool streaming = (frame_->service().state() == A2dpService::State::Streaming);
    menu->Append(ID_TRAY_DISCONNECT, wxString::FromUTF8(L("connection.disconnect")))->Enable(streaming);
    menu->AppendSeparator();
    menu->Append(ID_TRAY_EXIT, wxString::FromUTF8(L("tray.exit")));

    /* Bind events */
    menu->Bind(wxEVT_MENU, [this](wxCommandEvent &) {
        frame_->service().stop_streaming();
    }, ID_TRAY_DISCONNECT);

    menu->Bind(wxEVT_MENU, [this](wxCommandEvent &) {
        frame_->CallAfter([this]() { frame_->Close(true); });
    }, ID_TRAY_EXIT);

    for (int i = 0; i < static_cast<int>(profs.size()) && i < 10; i++) {
        int id = ID_TRAY_PROFILE_BASE + i;
        menu->Bind(wxEVT_MENU, [this, i](wxCommandEvent &) {
            auto &p = frame_->profiles().profiles()[i];
            frame_->service().start_streaming(p);
            frame_->Show(true);
            frame_->Raise();
        }, id);
    }

    return menu;
}
