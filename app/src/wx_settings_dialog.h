/*
 * Settings Dialog
 * SPDX-License-Identifier: MIT
 */

#ifndef WX_SETTINGS_DIALOG_H
#define WX_SETTINGS_DIALOG_H

#include "app_settings.h"
#include <wx/wx.h>

class SettingsDialog : public wxDialog {
public:
    SettingsDialog(wxWindow *parent, AppSettings *settings);

private:
    void OnOk(wxCommandEvent &evt);
    AppSettings *settings_;

    wxChoice   *lang_ctrl_ = nullptr;
    wxChoice   *theme_ctrl_ = nullptr;
    wxCheckBox *start_win_ = nullptr;
    wxCheckBox *minimize_tray_ = nullptr;
};

#endif /* WX_SETTINGS_DIALOG_H */
