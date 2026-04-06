/*
 * Zadig Dialog Implementation (download only)
 * SPDX-License-Identifier: MIT
 */

#include "wx_zadig_dialog.h"
#include "zadig_helper.h"
#include "localization.h"
#include "theme_manager.h"

ZadigDialog::ZadigDialog(wxWindow *parent)
    : wxDialog(parent, wxID_ANY, wxString::FromUTF8(L("zadig.download")),
               wxDefaultPosition, wxDefaultSize, wxDEFAULT_DIALOG_STYLE)
{
    /* Opening the Zadig website is now handled directly from the menu.
     * This dialog is kept for backward compatibility but should not
     * normally be shown. */
    auto *vbox = new wxBoxSizer(wxVERTICAL);
    auto *ok_btn = new wxButton(this, wxID_OK, wxString::FromUTF8(L("modal.ok")));
    vbox->Add(ok_btn, 0, wxALIGN_CENTER | wxALL, 12);
    ok_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { EndModal(wxID_OK); });
    SetBackgroundColour(TM().get(ThemeColor::DialogBg));
    SetForegroundColour(TM().get(ThemeColor::TextPrimary));

    SetSizer(vbox);
    Fit();
    CentreOnParent();
}
