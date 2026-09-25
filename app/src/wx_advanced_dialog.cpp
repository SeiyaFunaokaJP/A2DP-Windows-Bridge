/*
 * Advanced Settings Dialog Implementation
 * SPDX-License-Identifier: MIT
 */

#include "wx_advanced_dialog.h"
#include "localization.h"
#include "media_payload_limit.h"
#include "theme_manager.h"
#include <wx/statline.h>

AdvancedDialog::AdvancedDialog(wxWindow *parent, AppSettings *settings)
    : wxDialog(parent, wxID_ANY, wxString::FromUTF8(L("advanced.title")),
               wxDefaultPosition, wxDefaultSize, wxDEFAULT_DIALOG_STYLE)
    , settings_(settings)
{
    auto *vbox = new wxBoxSizer(wxVERTICAL);

    /* Max media packet size */
    auto *row = new wxBoxSizer(wxHORIZONTAL);
    auto *label = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("advanced.max_packet")));
    label->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    row->Add(label, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    packet_size_ctrl_ = new wxSpinCtrl(this, wxID_ANY, wxEmptyString, wxDefaultPosition,
                                       wxDefaultSize, wxSP_ARROW_KEYS,
                                       MEDIA_PAYLOAD_LIMIT_MIN, MEDIA_PAYLOAD_LIMIT_MAX,
                                       settings->max_media_payload);
    packet_size_ctrl_->SetBackgroundColour(TM().get(ThemeColor::CtrlBg));
    packet_size_ctrl_->SetForegroundColour(TM().get(ThemeColor::CtrlFg));
    row->Add(packet_size_ctrl_, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    auto *reset_btn = new wxButton(this, wxID_ANY,
        wxString::Format(wxString::FromUTF8(L("advanced.max_packet_recommended")),
                         (int)MEDIA_PAYLOAD_LIMIT_DEFAULT));
    row->Add(reset_btn, 0, wxALIGN_CENTER_VERTICAL);
    vbox->Add(row, 0, wxLEFT | wxRIGHT | wxTOP, 16);

    auto *help = new wxStaticText(this, wxID_ANY,
        wxString::Format(wxString::FromUTF8(L("advanced.max_packet_help")),
                         (int)MEDIA_PAYLOAD_LIMIT_MIN, (int)MEDIA_PAYLOAD_LIMIT_MAX,
                         (int)MEDIA_PAYLOAD_LIMIT_DEFAULT));
    help->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
    help->Wrap(440);
    vbox->Add(help, 0, wxALL, 16);

    vbox->Add(new wxStaticLine(this), 0, wxEXPAND | wxLEFT | wxRIGHT, 8);

    auto *btn_sizer = new wxBoxSizer(wxHORIZONTAL);
    btn_sizer->AddStretchSpacer();
    auto *ok_btn = new wxButton(this, wxID_OK, wxString::FromUTF8(L("modal.ok")));
    auto *cancel_btn = new wxButton(this, wxID_CANCEL, wxString::FromUTF8(L("modal.cancel")));
    btn_sizer->Add(ok_btn, 0, wxRIGHT, 8);
    btn_sizer->Add(cancel_btn, 0);
    vbox->Add(btn_sizer, 0, wxEXPAND | wxALL, 12);

    reset_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) {
        packet_size_ctrl_->SetValue(MEDIA_PAYLOAD_LIMIT_DEFAULT);
    });
    ok_btn->Bind(wxEVT_BUTTON, &AdvancedDialog::OnOk, this);
    cancel_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { EndModal(wxID_CANCEL); });

    SetBackgroundColour(TM().get(ThemeColor::DialogBg));
    SetForegroundColour(TM().get(ThemeColor::TextPrimary));

    SetSizer(vbox);
    SetMinSize(wxSize(480, -1));
    Fit();
    CentreOnParent();
}

void AdvancedDialog::OnOk(wxCommandEvent &) {
    settings_->max_media_payload = clamp_media_payload_limit(
        static_cast<uint32_t>(packet_size_ctrl_->GetValue()));
    settings_->save();
    EndModal(wxID_OK);
}
