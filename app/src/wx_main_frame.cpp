/*
 * Main Frame Implementation
 * SPDX-License-Identifier: MIT
 */

#include "wx_main_frame.h"
#include "wx_profile_dialog.h"
#include "wx_settings_dialog.h"
#include "wx_about_dialog.h"
#include "wx_firmware_dialog.h"
#include "wx_zadig_dialog.h"
#include "zadig_helper.h"
#include "localization.h"
#include "theme_manager.h"
#include "system_integration.h"

#include <wx/scrolwin.h>
#include <wx/statline.h>
#include <wx/clipbrd.h>
#include <shellapi.h>

#ifndef APP_VERSION
#define APP_VERSION "1.0.0"
#endif

wxDEFINE_EVENT(wxEVT_STATUS_UPDATE, wxThreadEvent);
wxDEFINE_EVENT(wxEVT_STREAM_INFO, wxThreadEvent);
wxDEFINE_EVENT(wxEVT_SCAN_COMPLETE, wxThreadEvent);

wxBEGIN_EVENT_TABLE(MainFrame, wxFrame)
    EVT_CLOSE(MainFrame::OnClose)
    EVT_ICONIZE(MainFrame::OnIconize)
wxEND_EVENT_TABLE()

/* ======================================================================== */
/* Construction                                                              */
/* ======================================================================== */

MainFrame::MainFrame()
    : wxFrame(nullptr, wxID_ANY, wxString::Format("A2DPWB v%s", APP_VERSION),
              wxDefaultPosition, wxSize(520, 600),
              wxDEFAULT_FRAME_STYLE & ~(wxRESIZE_BORDER | wxMAXIMIZE_BOX))
{
    /* Load settings and data */
    settings_.load();
    Localization::instance().load(settings_.language);
    profile_mgr_.load();
    service_.load_saved_devices();
    service_.set_bt_chip_pid(settings_.bt_chip_pid);
    service_.set_bt_chip_fw_stem(settings_.bt_chip_fw_stem);
    service_.set_debug_mode(settings_.debug_mode);
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

    /* Update startup registry */
    if (settings_.start_with_windows)
        RegisterStartup("--minimized");

    update_status_display();
    rebuild_profile_list();
}

MainFrame::~MainFrame() {
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
    menu_bar->Append(settings_menu, wxString::FromUTF8(L("menu.settings")));

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
    auto *status_panel = new wxPanel(main_panel_);
    auto *status_sizer = new wxBoxSizer(wxHORIZONTAL);

    status_label_ = new wxStaticText(status_panel, wxID_ANY, wxString::FromUTF8(L("status.idle")));
    auto font = status_label_->GetFont();
    font.SetPointSize(font.GetPointSize() + 2);
    font.SetWeight(wxFONTWEIGHT_BOLD);
    status_label_->SetFont(font);
    status_sizer->Add(status_label_, 0, wxALIGN_CENTER_VERTICAL | wxLEFT, 8);

    stream_info_label_ = new wxStaticText(status_panel, wxID_ANY, "",
        wxDefaultPosition, wxDefaultSize, wxST_ELLIPSIZE_END);
    stream_info_label_->SetForegroundColour(TM().get(ThemeColor::TextStreamInfo));
    stream_info_label_->SetCursor(wxCursor(wxCURSOR_HAND));
    stream_info_label_->Bind(wxEVT_LEFT_DOWN, [this](wxMouseEvent &) {
        if (current_state_ == A2dpService::State::Error && !current_status_text_.empty()) {
            if (wxTheClipboard->Open()) {
                wxTheClipboard->SetData(new wxTextDataObject(wxString::FromUTF8(current_status_text_)));
                wxTheClipboard->Close();
            }
        }
    });
    status_sizer->Add(stream_info_label_, 1, wxALIGN_CENTER_VERTICAL | wxLEFT, 12);

    disconnect_btn_ = new wxButton(status_panel, ID_DISCONNECT,
        wxString::FromUTF8(L("connection.disconnect")), wxDefaultPosition, wxDefaultSize, wxBU_EXACTFIT);
    disconnect_btn_->Show(false);
    status_sizer->Add(disconnect_btn_, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    Bind(wxEVT_BUTTON, &MainFrame::OnDisconnect, this, ID_DISCONNECT);

    status_panel->SetSizer(status_sizer);
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
            auto *name_text = new wxStaticText(row, wxID_ANY, wxString::FromUTF8(dname));
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

            auto *detail_text = new wxStaticText(row, wxID_ANY, wxString::FromUTF8(detail));
            detail_text->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
            detail_text->SetBackgroundColour(row_bg);

            text_sizer->Add(name_text, 0, wxBOTTOM, 2);
            text_sizer->Add(detail_text, 0);
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
    ShellExecuteW(nullptr, L"open", L"https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues", nullptr, nullptr, SW_SHOWNORMAL);
}

void MainFrame::OnCheckUpdate(wxCommandEvent &) {
    ShellExecuteW(nullptr, L"open", L"https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases", nullptr, nullptr, SW_SHOWNORMAL);
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
