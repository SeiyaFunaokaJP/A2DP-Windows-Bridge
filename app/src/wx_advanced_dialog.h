/*
 * Advanced Settings Dialog
 * SPDX-License-Identifier: MIT
 */

#ifndef WX_ADVANCED_DIALOG_H
#define WX_ADVANCED_DIALOG_H

#include "app_settings.h"
#include <wx/wx.h>
#include <wx/spinctrl.h>

class AdvancedDialog : public wxDialog {
public:
    AdvancedDialog(wxWindow *parent, AppSettings *settings);

private:
    void OnOk(wxCommandEvent &evt);
    AppSettings *settings_;

    wxSpinCtrl *packet_size_ctrl_ = nullptr;
};

#endif /* WX_ADVANCED_DIALOG_H */
