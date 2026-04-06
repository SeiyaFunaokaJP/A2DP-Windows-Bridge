/*
 * Settings Dialog Implementation
 * SPDX-License-Identifier: MIT
 */

#include "wx_settings_dialog.h"
#include "localization.h"
#include "theme_manager.h"
#include "system_integration.h"
#include <wx/statline.h>

SettingsDialog::SettingsDialog(wxWindow *parent, AppSettings *settings)
    : wxDialog(parent, wxID_ANY, wxString::FromUTF8(L("settings.title")),
               wxDefaultPosition, wxDefaultSize, wxDEFAULT_DIALOG_STYLE)
    , settings_(settings)
{
    auto *vbox = new wxBoxSizer(wxVERTICAL);
    auto *grid = new wxFlexGridSizer(2, 8, 8);
    grid->AddGrowableCol(1, 1);

    /* Language */
    auto *lang_label = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("settings.language")));
    lang_label->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    grid->Add(lang_label, 0, wxALIGN_CENTER_VERTICAL);
    lang_ctrl_ = new wxChoice(this, wxID_ANY);
    lang_ctrl_->SetBackgroundColour(TM().get(ThemeColor::CtrlBg));
    lang_ctrl_->SetForegroundColour(TM().get(ThemeColor::CtrlFg));
    auto langs = Localization::instance().get_available_languages();
    int sel = 0;
    for (int i = 0; i < static_cast<int>(langs.size()); i++) {
        lang_ctrl_->Append(wxString::FromUTF8(langs[i].display_name));
        if (langs[i].code == settings->language) sel = i;
    }
    lang_ctrl_->SetSelection(sel);
    grid->Add(lang_ctrl_, 1, wxEXPAND);

    /* Theme */
    auto *theme_label = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("settings.theme")));
    theme_label->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    grid->Add(theme_label, 0, wxALIGN_CENTER_VERTICAL);
    theme_ctrl_ = new wxChoice(this, wxID_ANY);
    theme_ctrl_->SetBackgroundColour(TM().get(ThemeColor::CtrlBg));
    theme_ctrl_->SetForegroundColour(TM().get(ThemeColor::CtrlFg));
    theme_ctrl_->Append(wxString::FromUTF8(L("settings.theme_dark")));
    theme_ctrl_->Append(wxString::FromUTF8(L("settings.theme_light")));
    theme_ctrl_->Append(wxString::FromUTF8(L("settings.theme_system")));
    {
        auto cur = ThemeManager::instance().mode();
        int theme_sel = (cur == ThemeMode::Dark) ? 0 :
                        (cur == ThemeMode::Light) ? 1 : 2;
        theme_ctrl_->SetSelection(theme_sel);
    }
    grid->Add(theme_ctrl_, 1, wxEXPAND);

    vbox->Add(grid, 0, wxEXPAND | wxALL, 16);

    /* Checkboxes */
    start_win_ = new wxCheckBox(this, wxID_ANY, wxString::FromUTF8(L("settings.start_with_windows")));
    start_win_->SetValue(settings->start_with_windows);
    start_win_->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    vbox->Add(start_win_, 0, wxLEFT | wxRIGHT, 16);

    minimize_tray_ = new wxCheckBox(this, wxID_ANY, wxString::FromUTF8(L("settings.minimize_to_tray")));
    minimize_tray_->SetValue(settings->minimize_to_tray);
    minimize_tray_->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    vbox->Add(minimize_tray_, 0, wxLEFT | wxRIGHT | wxTOP, 16);

    vbox->AddSpacer(16);
    vbox->Add(new wxStaticLine(this), 0, wxEXPAND | wxLEFT | wxRIGHT, 8);

    auto *btn_sizer = new wxBoxSizer(wxHORIZONTAL);
    btn_sizer->AddStretchSpacer();
    auto *ok_btn = new wxButton(this, wxID_OK, wxString::FromUTF8(L("modal.ok")));
    auto *cancel_btn = new wxButton(this, wxID_CANCEL, wxString::FromUTF8(L("modal.cancel")));
    btn_sizer->Add(ok_btn, 0, wxRIGHT, 8);
    btn_sizer->Add(cancel_btn, 0);
    vbox->Add(btn_sizer, 0, wxEXPAND | wxALL, 12);

    ok_btn->Bind(wxEVT_BUTTON, &SettingsDialog::OnOk, this);
    cancel_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { EndModal(wxID_CANCEL); });

    SetBackgroundColour(TM().get(ThemeColor::DialogBg));
    SetForegroundColour(TM().get(ThemeColor::TextPrimary));

    SetSizer(vbox);
    SetMinSize(wxSize(400, -1));
    Fit();
    CentreOnParent();
}

void SettingsDialog::OnOk(wxCommandEvent &) {
    auto langs = Localization::instance().get_available_languages();
    int lang_sel = lang_ctrl_->GetSelection();
    if (lang_sel >= 0 && lang_sel < static_cast<int>(langs.size())) {
        settings_->language = langs[lang_sel].code;
        Localization::instance().load(settings_->language);
    }

    settings_->start_with_windows = start_win_->GetValue();
    if (settings_->start_with_windows)
        RegisterStartup("--minimized");
    else
        UnregisterStartup();

    settings_->minimize_to_tray = minimize_tray_->GetValue();
    static const char *theme_strings[] = { "dark", "light", "system" };
    int theme_sel = theme_ctrl_->GetSelection();
    if (theme_sel >= 0 && theme_sel <= 2) {
        settings_->theme = theme_strings[theme_sel];
        ThemeManager::instance().set_mode(ThemeManager::mode_from_string(settings_->theme));
    }

    settings_->save();

    EndModal(wxID_OK);
}
