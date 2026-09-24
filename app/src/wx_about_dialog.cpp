/*
 * About Dialog Implementation
 * SPDX-License-Identifier: MIT
 */

#include "wx_about_dialog.h"
#include "localization.h"
#include "theme_manager.h"
#include <wx/hyperlink.h>

#ifndef APP_VERSION
#define APP_VERSION "1.0.1"
#endif

AboutDialog::AboutDialog(wxWindow *parent)
    : wxDialog(parent, wxID_ANY, wxString::FromUTF8(L("help.about_title")),
               wxDefaultPosition, wxDefaultSize, wxDEFAULT_DIALOG_STYLE)
{
    auto *vbox = new wxBoxSizer(wxVERTICAL);

    auto *title = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("help.full_name")));
    auto font = title->GetFont();
    font.SetPointSize(font.GetPointSize() + 4);
    font.SetWeight(wxFONTWEIGHT_BOLD);
    title->SetFont(font);
    title->SetForegroundColour(TM().get(ThemeColor::AboutTitle));
    vbox->Add(title, 0, wxALL, 16);

    wxString version = wxString::Format("%s: %s",
        wxString::FromUTF8(L("help.version")), APP_VERSION);
    auto *ver_text = new wxStaticText(this, wxID_ANY, version);
    ver_text->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    vbox->Add(ver_text, 0, wxLEFT | wxRIGHT, 16);

    vbox->AddSpacer(8);
    auto *author = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("help.author")));
    author->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    vbox->Add(author, 0, wxLEFT | wxRIGHT, 16);

    vbox->AddSpacer(8);
    auto *github_link = new wxHyperlinkCtrl(this, wxID_ANY,
        wxString::FromUTF8(L("help.github")),
        "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge");
    github_link->SetNormalColour(TM().get(ThemeColor::AboutTitle));
    github_link->SetHoverColour(TM().get(ThemeColor::AboutTitle));
    github_link->SetVisitedColour(TM().get(ThemeColor::AboutTitle));
    vbox->Add(github_link, 0, wxLEFT | wxRIGHT, 16);

    vbox->AddSpacer(4);
    auto *sponsor_link = new wxHyperlinkCtrl(this, wxID_ANY,
        wxString::FromUTF8(L("help.sponsor")),
        "https://github.com/sponsors/SeiyaFunaokaJP");
    sponsor_link->SetNormalColour(TM().get(ThemeColor::AboutTitle));
    sponsor_link->SetHoverColour(TM().get(ThemeColor::AboutTitle));
    sponsor_link->SetVisitedColour(TM().get(ThemeColor::AboutTitle));
    vbox->Add(sponsor_link, 0, wxLEFT | wxRIGHT, 16);

    vbox->AddSpacer(8);
    auto *license = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("help.license")));
    license->SetForegroundColour(TM().get(ThemeColor::AboutLicense));
    vbox->Add(license, 0, wxLEFT | wxRIGHT, 16);

    /* The binary links BTstack (non-commercial clause) and LGPL/Apache/FDK
     * code whose licenses require these notices to be shown/referenced. */
    vbox->AddSpacer(4);
    auto *binary_license = new wxStaticText(this, wxID_ANY,
        wxString::FromUTF8(L("help.binary_license")));
    binary_license->SetForegroundColour(TM().get(ThemeColor::AboutLicense));
    binary_license->Wrap(FromDIP(360));
    vbox->Add(binary_license, 0, wxLEFT | wxRIGHT, 16);

    vbox->AddSpacer(4);
    auto *third_party = new wxStaticText(this, wxID_ANY,
        wxString::FromUTF8(L("help.third_party")));
    third_party->SetForegroundColour(TM().get(ThemeColor::AboutLicense));
    third_party->Wrap(FromDIP(360));
    vbox->Add(third_party, 0, wxLEFT | wxRIGHT, 16);

    vbox->AddSpacer(4);
    auto *licenses_link = new wxHyperlinkCtrl(this, wxID_ANY,
        wxString::FromUTF8(L("help.third_party_link")),
        "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md");
    licenses_link->SetNormalColour(TM().get(ThemeColor::AboutTitle));
    licenses_link->SetHoverColour(TM().get(ThemeColor::AboutTitle));
    licenses_link->SetVisitedColour(TM().get(ThemeColor::AboutTitle));
    vbox->Add(licenses_link, 0, wxLEFT | wxRIGHT, 16);

    vbox->AddSpacer(12);
    auto *ok_btn = new wxButton(this, wxID_OK, wxString::FromUTF8(L("modal.ok")));
    vbox->Add(ok_btn, 0, wxALIGN_CENTER | wxBOTTOM, 16);
    ok_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { EndModal(wxID_OK); });

    SetBackgroundColour(TM().get(ThemeColor::DialogBg));
    SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    SetSizer(vbox);
    SetMinSize(wxSize(380, -1));
    Fit();
    CentreOnParent();
}
