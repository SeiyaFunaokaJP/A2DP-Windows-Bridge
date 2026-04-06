/*
 * Profile Dialog Implementation
 * SPDX-License-Identifier: MIT
 */

#include "wx_profile_dialog.h"
#include "localization.h"
#include "theme_manager.h"
#include <wx/statline.h>
#include <cctype>
#include <cstring>

ProfileDialog::ProfileDialog(wxWindow *parent, A2dpService *service,
                             ProfileManager *mgr, int edit_index)
    : wxDialog(parent, wxID_ANY,
               edit_index < 0 ? wxString::FromUTF8(L("profile.new_title"))
                              : wxString::FromUTF8(L("profile.edit_title")),
               wxDefaultPosition, wxDefaultSize,
               wxDEFAULT_DIALOG_STYLE)
    , service_(service), mgr_(mgr), edit_index_(edit_index)
{
    create_ui();

    if (edit_index >= 0 && edit_index < static_cast<int>(mgr->profiles().size())) {
        populate_from_profile(mgr->profiles()[edit_index]);
    }

    update_codec_dependent();
    SetMinSize(wxSize(620, 500));
    Fit();
    CentreOnParent();
}

void ProfileDialog::create_ui() {
    auto *vbox = new wxBoxSizer(wxVERTICAL);

    /* Device scanning */
    auto *scan_row = new wxBoxSizer(wxHORIZONTAL);
    scan_btn_ = new wxButton(this, wxID_ANY, wxString::FromUTF8(L("connection.scan")));
    scan_btn_->Bind(wxEVT_BUTTON, &ProfileDialog::OnScan, this);
    scan_row->Add(scan_btn_, 0);
    scan_row->AddStretchSpacer();
    dev_edit_btn_ = new wxButton(this, wxID_ANY, wxString::FromUTF8(L("device.edit_name")),
        wxDefaultPosition, wxDefaultSize, wxBU_EXACTFIT);
    dev_edit_btn_->Bind(wxEVT_BUTTON, &ProfileDialog::OnDeviceEditName, this);
    dev_edit_btn_->Enable(false);
    scan_row->Add(dev_edit_btn_, 0, wxRIGHT, 4);
    dev_del_btn_ = new wxButton(this, wxID_ANY, wxString::FromUTF8(L("profile.delete")),
        wxDefaultPosition, wxDefaultSize, wxBU_EXACTFIT);
    dev_del_btn_->Bind(wxEVT_BUTTON, &ProfileDialog::OnDeviceDelete, this);
    dev_del_btn_->Enable(false);
    scan_row->Add(dev_del_btn_, 0);
    vbox->Add(scan_row, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 12);

    device_list_ = new wxListBox(this, wxID_ANY, wxDefaultPosition, wxSize(-1, 100));
    device_list_->Bind(wxEVT_LISTBOX, &ProfileDialog::OnDeviceSelect, this);
    vbox->Add(device_list_, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 12);

    /* Populate saved devices */
    auto devices = service_->get_devices();
    for (auto &d : devices) {
        wxString label = wxString::Format("[%s] %s (%s)",
            d.saved ? wxString::FromUTF8(L("device.tag_saved")) : wxString::FromUTF8(L("device.tag_audio")),
            d.name.empty() ? wxString::FromUTF8(L("device.name_unknown")) : wxString::FromUTF8(d.name),
            wxString::FromUTF8(d.addr_str));
        device_list_->Append(label);
    }

    /* Address and device name */
    auto *grid2 = new wxFlexGridSizer(2, 8, 8);
    grid2->AddGrowableCol(1, 1);

    grid2->Add(new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("connection.device_addr"))),
               0, wxALIGN_CENTER_VERTICAL);
    addr_ctrl_ = new wxTextCtrl(this, wxID_ANY, "", wxDefaultPosition, wxSize(300, -1), wxTE_RICH2);
    addr_ctrl_->SetHint(wxString::FromUTF8(L("connection.addr_hint")));
    addr_ctrl_->Bind(wxEVT_TEXT, &ProfileDialog::OnAddrChange, this);
    grid2->Add(addr_ctrl_, 1, wxEXPAND);

    grid2->Add(new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("profile.device_name"))),
               0, wxALIGN_CENTER_VERTICAL);
    devname_ctrl_ = new wxTextCtrl(this, wxID_ANY, "", wxDefaultPosition, wxSize(300, -1), wxTE_RICH2);
    grid2->Add(devname_ctrl_, 1, wxEXPAND);

    vbox->Add(grid2, 0, wxEXPAND | wxALL, 12);

    /* Codec settings */
    auto *codec_grid = new wxFlexGridSizer(2, 8, 8);
    codec_grid->AddGrowableCol(1, 1);

    /* Codec */
    auto *codec_label = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("connection.codec")));
    codec_label->SetToolTip(wxString::FromUTF8(L("tooltip.codec")));
    codec_grid->Add(codec_label, 0, wxALIGN_CENTER_VERTICAL);
    codec_ctrl_ = new wxChoice(this, wxID_ANY);
    codec_ctrl_->Append(wxString::FromUTF8(L("codec.auto")));
    codec_ctrl_->Append(wxString::FromUTF8(L("codec.ldac")));
    codec_ctrl_->Append(wxString::FromUTF8(L("codec.aptx_hd")));
    codec_ctrl_->Append(wxString::FromUTF8(L("codec.aptx_ll")));
    codec_ctrl_->Append(wxString::FromUTF8(L("codec.sbc")));
    codec_ctrl_->Append(wxString::FromUTF8(L("codec.aac")));
    codec_ctrl_->SetSelection(0);
    codec_ctrl_->Bind(wxEVT_CHOICE, &ProfileDialog::OnCodecChange, this);
    codec_grid->Add(codec_ctrl_, 1, wxEXPAND);

    /* Quality */
    auto *quality_label = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("connection.quality")));
    quality_label->SetToolTip(wxString::FromUTF8(L("tooltip.quality")));
    codec_grid->Add(quality_label, 0, wxALIGN_CENTER_VERTICAL);
    quality_ctrl_ = new wxChoice(this, wxID_ANY);
    quality_ctrl_->Append(wxString::FromUTF8(L("quality.high")));
    quality_ctrl_->Append(wxString::FromUTF8(L("quality.standard")));
    quality_ctrl_->Append(wxString::FromUTF8(L("quality.mobile")));
    quality_ctrl_->SetSelection(0);
    codec_grid->Add(quality_ctrl_, 1, wxEXPAND);

    /* ABR */
    codec_grid->Add(new wxStaticText(this, wxID_ANY, ""), 0);
    abr_ctrl_ = new wxCheckBox(this, wxID_ANY, wxString::FromUTF8(L("connection.abr")));
    abr_ctrl_->SetToolTip(wxString::FromUTF8(L("tooltip.abr")));
    codec_grid->Add(abr_ctrl_, 0);

    /* Sample rate */
    auto *sr_label = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("connection.sample_rate")));
    sr_label->SetToolTip(wxString::FromUTF8(L("tooltip.sample_rate")));
    codec_grid->Add(sr_label, 0, wxALIGN_CENTER_VERTICAL);
    sample_rate_ctrl_ = new wxChoice(this, wxID_ANY);
    codec_grid->Add(sample_rate_ctrl_, 1, wxEXPAND);

    /* Bit depth */
    auto *bd_label = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("connection.bit_depth")));
    bd_label->SetToolTip(wxString::FromUTF8(L("tooltip.bit_depth")));
    codec_grid->Add(bd_label, 0, wxALIGN_CENTER_VERTICAL);
    bit_depth_ctrl_ = new wxChoice(this, wxID_ANY);
    codec_grid->Add(bit_depth_ctrl_, 1, wxEXPAND);

    /* Capture mode */
    auto *capture_label = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("capture.mode")));
    capture_label->SetToolTip(wxString::FromUTF8(L("tooltip.capture_mode")));
    codec_grid->Add(capture_label, 0, wxALIGN_CENTER_VERTICAL);
    capture_ctrl_ = new wxChoice(this, wxID_ANY);
    capture_ctrl_->Append(wxString::FromUTF8(L("capture.loopback")));
    capture_ctrl_->Append(wxString::FromUTF8(L("capture.virtual")));
    capture_ctrl_->SetSelection(0);
    capture_ctrl_->Bind(wxEVT_CHOICE, &ProfileDialog::OnCaptureChange, this);
    codec_grid->Add(capture_ctrl_, 1, wxEXPAND);

    /* Audio device (virtual mode) */
    audio_dev_label_ = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("capture.audio_device")));
    audio_dev_label_->SetToolTip(wxString::FromUTF8(L("tooltip.audio_device")));
    codec_grid->Add(audio_dev_label_, 0, wxALIGN_CENTER_VERTICAL);
    audio_dev_ctrl_ = new wxChoice(this, wxID_ANY);
    codec_grid->Add(audio_dev_ctrl_, 1, wxEXPAND);

    auto_switch_label_ = new wxStaticText(this, wxID_ANY, "");
    codec_grid->Add(auto_switch_label_, 0);
    auto_switch_ctrl_ = new wxCheckBox(this, wxID_ANY, wxString::FromUTF8(L("capture.auto_switch")));
    auto_switch_ctrl_->SetToolTip(wxString::FromUTF8(L("tooltip.auto_switch")));
    auto_switch_ctrl_->SetValue(true);
    codec_grid->Add(auto_switch_ctrl_, 0);

    vbox->Add(codec_grid, 0, wxEXPAND | wxLEFT | wxRIGHT, 12);

    /* Audio device format info (above separator) */
    format_info_ = new wxStaticText(this, wxID_ANY, "",
        wxDefaultPosition, wxDefaultSize, wxST_NO_AUTORESIZE);
    format_info_->SetFont(format_info_->GetFont().Smaller());
    vbox->Add(format_info_, 0, wxEXPAND | wxLEFT | wxRIGHT | wxTOP, 12);

    /* Bind events for format info updates */
    sample_rate_ctrl_->Bind(wxEVT_CHOICE, [this](wxCommandEvent &) { update_format_info(); });
    bit_depth_ctrl_->Bind(wxEVT_CHOICE, [this](wxCommandEvent &) { update_format_info(); });
    audio_dev_ctrl_->Bind(wxEVT_CHOICE, [this](wxCommandEvent &) { update_format_info(); });

    /* Separator + buttons */
    vbox->Add(new wxStaticLine(this), 0, wxEXPAND | wxALL, 8);

    auto *btn_sizer = new wxBoxSizer(wxHORIZONTAL);
    btn_sizer->AddStretchSpacer();
    auto *save_btn = new wxButton(this, wxID_OK, wxString::FromUTF8(L("modal.save")));
    auto *cancel_btn = new wxButton(this, wxID_CANCEL, wxString::FromUTF8(L("modal.cancel")));
    btn_sizer->Add(save_btn, 0, wxRIGHT, 8);
    btn_sizer->Add(cancel_btn, 0);
    vbox->Add(btn_sizer, 0, wxEXPAND | wxALL, 12);

    save_btn->Bind(wxEVT_BUTTON, &ProfileDialog::OnSave, this);
    cancel_btn->Bind(wxEVT_BUTTON, &ProfileDialog::OnCancel, this);

    SetBackgroundColour(TM().get(ThemeColor::DialogBg));
    SetForegroundColour(TM().get(ThemeColor::TextPrimary));

    /* Ensure all child controls inherit theme colors (Windows wxWidgets quirk) */
    wxColour dlg_bg = TM().get(ThemeColor::DialogBg);
    for (auto *child : GetChildren()) {
        if (dynamic_cast<wxStaticText *>(child) || dynamic_cast<wxCheckBox *>(child)) {
            child->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
            child->SetBackgroundColour(dlg_bg);
        } else if (dynamic_cast<wxChoice *>(child)) {
            child->SetBackgroundColour(TM().get(ThemeColor::CtrlBg));
            child->SetForegroundColour(TM().get(ThemeColor::CtrlFg));
        }
        /* Buttons: no custom colors — use native rendering for proper hover */
    }

    /* Style input controls for theme */
    device_list_->SetBackgroundColour(TM().get(ThemeColor::CtrlBg));
    device_list_->SetForegroundColour(TM().get(ThemeColor::CtrlFg));
    addr_ctrl_->SetBackgroundColour(TM().get(ThemeColor::CtrlBg));
    addr_ctrl_->SetForegroundColour(TM().get(ThemeColor::CtrlFg));
    devname_ctrl_->SetBackgroundColour(TM().get(ThemeColor::CtrlBg));
    devname_ctrl_->SetForegroundColour(TM().get(ThemeColor::CtrlFg));

    SetSizer(vbox);

    /* Initially hide virtual device controls */
    audio_dev_label_->Show(false);
    audio_dev_ctrl_->Show(false);
    auto_switch_label_->Show(false);
    auto_switch_ctrl_->Show(false);
}

void ProfileDialog::populate_from_profile(const ConnectionProfile &p) {
    addr_ctrl_->SetValue(wxString::FromUTF8(p.device_address));
    devname_ctrl_->SetValue(wxString::FromUTF8(p.device_name));
    codec_ctrl_->SetSelection(ProfileManager::codec_to_index(p.codec));
    quality_ctrl_->SetSelection(ProfileManager::quality_to_index(p.quality));
    abr_ctrl_->SetValue(p.abr);

    int sr_idx = (p.sample_rate == 44100) ? 1 :
                 (p.sample_rate == 48000) ? 2 :
                 (p.sample_rate == 88200) ? 3 :
                 (p.sample_rate == 96000) ? 4 : 0;
    /* sample_rate_ctrl_ populated by update_codec_dependent */

    int bd_idx = ProfileManager::bit_depth_to_index(p.bit_depth);
    capture_ctrl_->SetSelection(ProfileManager::capture_mode_to_index(p.capture_mode));

    update_codec_dependent();

    /* Set auto_switch AFTER update_codec_dependent() makes the control visible.
     * wxCheckBox::SetValue on a hidden control can be unreliable on Windows. */
    auto_switch_ctrl_->SetValue(p.auto_switch_device);

    /* Fix sample rate selection after choices are populated */
    if (sr_idx < sample_rate_ctrl_->GetCount())
        sample_rate_ctrl_->SetSelection(sr_idx);

    /* Fix bit depth selection after choices are populated */
    if (bd_idx >= 0 && bd_idx < bit_depth_ctrl_->GetCount())
        bit_depth_ctrl_->SetSelection(bd_idx);

    /* Fix audio device selection to match saved device */
    if (!p.audio_device_id.empty()) {
        std::wstring saved_wid;
        int wlen = MultiByteToWideChar(CP_UTF8, 0, p.audio_device_id.c_str(),
            (int)p.audio_device_id.size(), nullptr, 0);
        if (wlen > 0) {
            saved_wid.resize(wlen);
            MultiByteToWideChar(CP_UTF8, 0, p.audio_device_id.c_str(),
                (int)p.audio_device_id.size(), &saved_wid[0], wlen);
        }
        for (size_t i = 0; i < audio_devices_.size(); i++) {
            if (audio_devices_[i].id == saved_wid) {
                audio_dev_ctrl_->SetSelection(static_cast<int>(i));
                break;
            }
        }
    }

    update_format_info();
}

void ProfileDialog::update_codec_dependent() {
    int codec_idx = codec_ctrl_->GetSelection();
    bool is_ldac = (codec_idx == 1);
    bool quality_enabled = (codec_idx == 0 || codec_idx == 1 || codec_idx == 4 || codec_idx == 5);
    bool abr_enabled = (codec_idx == 0 || codec_idx == 1);

    /* Rebuild quality items with codec-specific bitrate info */
    int cur_q = quality_ctrl_->GetSelection();
    quality_ctrl_->Clear();
    /* Bitrates per quality: [High, Standard, Mobile] in kbps */
    switch (codec_idx) {
    case 1: /* LDAC */
        quality_ctrl_->Append(wxString::Format("%s (990kbps)", wxString::FromUTF8(L("quality.high"))));
        quality_ctrl_->Append(wxString::Format("%s (660kbps)", wxString::FromUTF8(L("quality.standard"))));
        quality_ctrl_->Append(wxString::Format("%s (330kbps)", wxString::FromUTF8(L("quality.mobile"))));
        break;
    case 5: /* AAC */
        quality_ctrl_->Append(wxString::Format("%s (256kbps)", wxString::FromUTF8(L("quality.high"))));
        quality_ctrl_->Append(wxString::Format("%s (192kbps)", wxString::FromUTF8(L("quality.standard"))));
        quality_ctrl_->Append(wxString::Format("%s (128kbps)", wxString::FromUTF8(L("quality.mobile"))));
        break;
    case 4: /* SBC */
        quality_ctrl_->Append(wxString::Format("%s (~345kbps)", wxString::FromUTF8(L("quality.high"))));
        quality_ctrl_->Append(wxString::Format("%s (~240kbps)", wxString::FromUTF8(L("quality.standard"))));
        quality_ctrl_->Append(wxString::Format("%s (~150kbps)", wxString::FromUTF8(L("quality.mobile"))));
        break;
    default: /* Auto or codecs without quality control */
        quality_ctrl_->Append(wxString::FromUTF8(L("quality.high")));
        quality_ctrl_->Append(wxString::FromUTF8(L("quality.standard")));
        quality_ctrl_->Append(wxString::FromUTF8(L("quality.mobile")));
        break;
    }
    if (cur_q >= 0 && cur_q < quality_ctrl_->GetCount())
        quality_ctrl_->SetSelection(cur_q);
    else
        quality_ctrl_->SetSelection(0);

    quality_ctrl_->Enable(quality_enabled);
    abr_ctrl_->Enable(abr_enabled);
    abr_ctrl_->SetForegroundColour(TM().get(abr_enabled ? ThemeColor::TextPrimary : ThemeColor::TextMuted));
    abr_ctrl_->Refresh();
    if (!abr_enabled) abr_ctrl_->SetValue(false);

    /* Sample rate options */
    int cur_sr = sample_rate_ctrl_->GetSelection();
    sample_rate_ctrl_->Clear();
    sample_rate_ctrl_->Append(wxString::FromUTF8(L("codec.auto")));
    sample_rate_ctrl_->Append("44100 Hz");
    sample_rate_ctrl_->Append("48000 Hz");
    if (is_ldac) {
        sample_rate_ctrl_->Append("88200 Hz");
        sample_rate_ctrl_->Append("96000 Hz");
    }
    if (cur_sr >= 0 && cur_sr < sample_rate_ctrl_->GetCount())
        sample_rate_ctrl_->SetSelection(cur_sr);
    else
        sample_rate_ctrl_->SetSelection(0);

    /* Bit depth */
    bool bd_enabled = (codec_idx == 1);
    int cur_bd = bit_depth_ctrl_->GetSelection();
    bit_depth_ctrl_->Clear();
    if (bd_enabled) {
        bit_depth_ctrl_->Append(wxString::FromUTF8(L("codec.auto")));
        bit_depth_ctrl_->Append(wxString::FromUTF8(L("bitdepth.16")));
        bit_depth_ctrl_->Append(wxString::FromUTF8(L("bitdepth.24")));
    } else {
        bit_depth_ctrl_->Append(wxString::FromUTF8(L("bitdepth.16")));
    }
    bit_depth_ctrl_->Enable(bd_enabled);
    if (cur_bd >= 0 && cur_bd < bit_depth_ctrl_->GetCount())
        bit_depth_ctrl_->SetSelection(cur_bd);
    else
        bit_depth_ctrl_->SetSelection(0);

    /* Virtual device visibility */
    bool virtual_mode = (capture_ctrl_->GetSelection() == 1);
    audio_dev_label_->Show(virtual_mode);
    audio_dev_ctrl_->Show(virtual_mode);
    auto_switch_label_->Show(virtual_mode);
    auto_switch_ctrl_->Show(virtual_mode);

    if (virtual_mode && audio_dev_ctrl_->GetCount() == 0) {
        AudioDeviceEnumerator enumerator;
        if (enumerator.init()) {
            audio_devices_ = enumerator.get_virtual_devices();
            enumerator.shutdown();
        }
        for (auto &d : audio_devices_)
            audio_dev_ctrl_->Append(wxString::FromUTF8(d.display_name));
        if (!audio_devices_.empty())
            audio_dev_ctrl_->SetSelection(0);
    }

    update_format_info();

    Layout();
    Fit();
}

void ProfileDialog::update_format_info() {
    /* Determine which device to query */
    bool virtual_mode = (capture_ctrl_->GetSelection() == 1);
    AudioDeviceFormat dev_fmt;

    if (virtual_mode) {
        int dev_sel = audio_dev_ctrl_->GetSelection();
        if (dev_sel >= 0 && dev_sel < static_cast<int>(audio_devices_.size()))
            dev_fmt = AudioDeviceEnumerator::get_device_format(audio_devices_[dev_sel].id);
    } else {
        dev_fmt = AudioDeviceEnumerator::get_device_format(); /* default device */
    }

    if (!dev_fmt.valid) {
        format_info_->SetLabel("");
        return;
    }

    /* Get currently selected profile values */
    static const uint32_t rate_values[] = { 0, 44100, 48000, 88200, 96000 };
    int sr_sel = sample_rate_ctrl_->GetSelection();
    uint32_t profile_sr = (sr_sel >= 0 && sr_sel <= 4) ? rate_values[sr_sel] : 0;

    static const uint32_t bd_values[] = { 0, 16, 24 };
    int bd_sel = bit_depth_ctrl_->GetSelection();
    uint32_t profile_bd = (bd_sel >= 0 && bd_sel <= 2) ? bd_values[bd_sel] : 0;

    /* Check mismatch (auto=0 never mismatches) */
    bool sr_mismatch = (profile_sr != 0 && profile_sr != dev_fmt.sample_rate);
    bool bd_mismatch = (profile_bd != 0 && profile_bd != dev_fmt.bits_per_sample);
    bool mismatch = sr_mismatch || bd_mismatch;

    wxString label = wxString::Format(
        wxString::FromUTF8(L("capture.device_format")),
        dev_fmt.sample_rate, dev_fmt.bits_per_sample);

    format_info_->SetLabel(label);
    format_info_->SetForegroundColour(
        mismatch ? wxColour(220, 50, 50) : TM().get(ThemeColor::TextMuted));
    format_info_->SetBackgroundColour(TM().get(ThemeColor::DialogBg));
    format_info_->Refresh();
}

void ProfileDialog::OnCodecChange(wxCommandEvent &) {
    update_codec_dependent();
}

void ProfileDialog::OnCaptureChange(wxCommandEvent &) {
    update_codec_dependent();
}

void ProfileDialog::OnDeviceSelect(wxCommandEvent &) {
    int sel = device_list_->GetSelection();
    if (sel == wxNOT_FOUND) {
        dev_edit_btn_->Enable(false);
        dev_del_btn_->Enable(false);
        return;
    }

    auto devices = service_->get_devices();
    if (sel >= 0 && sel < static_cast<int>(devices.size())) {
        auto &d = devices[sel];
        addr_ctrl_->SetValue(wxString::FromUTF8(d.addr_str));
        devname_ctrl_->SetValue(wxString::FromUTF8(d.name));
        /* Enable edit/delete only for saved (paired) devices */
        dev_edit_btn_->Enable(d.saved);
        dev_del_btn_->Enable(d.saved);
    }
}

void ProfileDialog::OnDeviceEditName(wxCommandEvent &) {
    int sel = device_list_->GetSelection();
    if (sel == wxNOT_FOUND) return;

    auto devices = service_->get_devices();
    if (sel < 0 || sel >= static_cast<int>(devices.size()) || !devices[sel].saved) return;

    wxTextEntryDialog dlg(this,
        wxString::FromUTF8(L("device.enter_name")),
        wxString::FromUTF8(L("device.edit_name_title")),
        wxString::FromUTF8(devices[sel].name));

    /* Apply dark mode theme to the text entry dialog */
    dlg.SetBackgroundColour(TM().get(ThemeColor::DialogBg));
    dlg.SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    for (auto *child : dlg.GetChildren()) {
        if (dynamic_cast<wxStaticText *>(child)) {
            child->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
            child->SetBackgroundColour(TM().get(ThemeColor::DialogBg));
        } else if (dynamic_cast<wxTextCtrl *>(child)) {
            child->SetBackgroundColour(TM().get(ThemeColor::CtrlBg));
            child->SetForegroundColour(TM().get(ThemeColor::CtrlFg));
        }
    }

    if (dlg.ShowModal() == wxID_OK) {
        std::string new_name = dlg.GetValue().utf8_string();
        service_->save_device(devices[sel].addr_str.c_str(), new_name);
        devname_ctrl_->SetValue(dlg.GetValue());
        /* Refresh device list */
        on_scan_complete();
    }
}

void ProfileDialog::OnDeviceDelete(wxCommandEvent &) {
    int sel = device_list_->GetSelection();
    if (sel == wxNOT_FOUND) return;

    auto devices = service_->get_devices();
    if (sel < 0 || sel >= static_cast<int>(devices.size()) || !devices[sel].saved) return;

    service_->delete_saved_device(sel);
    /* Refresh device list */
    on_scan_complete();
    dev_edit_btn_->Enable(false);
    dev_del_btn_->Enable(false);
}

void ProfileDialog::OnScan(wxCommandEvent &) {
    scan_btn_->SetLabel(wxString::FromUTF8(L("connection.scanning")));
    scan_btn_->Enable(false);

    /* Register callback for scan completion */
    service_->set_scan_complete_callback([this]() {
        CallAfter([this]() { on_scan_complete(); });
    });

    service_->start_scan();
}

void ProfileDialog::on_scan_complete() {
    scan_btn_->SetLabel(wxString::FromUTF8(L("connection.scan")));
    scan_btn_->Enable(true);

    /* Refresh device list */
    device_list_->Clear();
    auto devices = service_->get_devices();
    for (auto &d : devices) {
        wxString label = wxString::Format("[%s] %s (%s)",
            d.saved ? wxString::FromUTF8(L("device.tag_saved")) : wxString::FromUTF8(L("device.tag_audio")),
            d.name.empty() ? wxString::FromUTF8(L("device.name_unknown")) : wxString::FromUTF8(d.name),
            wxString::FromUTF8(d.addr_str));
        device_list_->Append(label);
    }
}

bool ProfileDialog::validate_address(const wxString &addr) {
    if (addr.IsEmpty()) return true; /* empty is not invalid, just incomplete */
    std::string s = addr.utf8_string();
    if (s.length() != 17) return false;
    for (int i = 0; i < 17; i++) {
        if (i % 3 == 2) {
            if (s[i] != ':') return false;
        } else {
            if (!std::isxdigit(static_cast<unsigned char>(s[i]))) return false;
        }
    }
    return true;
}

void ProfileDialog::update_addr_visual() {
    wxString addr = addr_ctrl_->GetValue();
    bool valid = validate_address(addr);
    bool show_error = !valid && !addr.IsEmpty();
    addr_ctrl_->SetBackgroundColour(show_error ? wxColour(80, 20, 20) : TM().get(ThemeColor::CtrlBg));
    addr_ctrl_->SetForegroundColour(show_error ? wxColour(255, 100, 100) : TM().get(ThemeColor::CtrlFg));
    addr_ctrl_->Refresh();
}

void ProfileDialog::OnAddrChange(wxCommandEvent &) {
    update_addr_visual();
}

void ProfileDialog::OnSave(wxCommandEvent &) {
    wxString addr = addr_ctrl_->GetValue();
    if (addr.IsEmpty()) return;

    if (!validate_address(addr)) {
        wxMessageBox(wxString::FromUTF8(L("error.invalid_address_format")),
                     wxString::FromUTF8(L("error.invalid_address")),
                     wxOK | wxICON_WARNING, this);
        addr_ctrl_->SetFocus();
        return;
    }

    ConnectionProfile p;
    p.device_address = addr.utf8_string();
    p.device_name = devname_ctrl_->GetValue().utf8_string();

    /* Auto-generate internal name from device + codec */
    static const char *codec_short[] = { "Auto", "LDAC", "aptX HD", "aptX LL", "SBC", "AAC" };
    std::string auto_name = p.device_name.empty() ? p.device_address : p.device_name;
    int ci = codec_ctrl_->GetSelection();
    if (ci >= 0 && ci <= 5) { auto_name += " "; auto_name += codec_short[ci]; }
    p.name = auto_name;
    p.codec = ProfileManager::index_to_codec(codec_ctrl_->GetSelection());
    p.quality = ProfileManager::index_to_quality(quality_ctrl_->GetSelection());
    p.abr = abr_ctrl_->GetValue();

    static const uint32_t rate_values[] = { 0, 44100, 48000, 88200, 96000 };
    int sr_sel = sample_rate_ctrl_->GetSelection();
    p.sample_rate = (sr_sel >= 0 && sr_sel <= 4) ? rate_values[sr_sel] : 0;

    static const uint32_t bd_values[] = { 0, 16, 24 };
    int bd_sel = bit_depth_ctrl_->GetSelection();
    p.bit_depth = (bd_sel >= 0 && bd_sel <= 2) ? bd_values[bd_sel] : 0;

    p.capture_mode = ProfileManager::index_to_capture_mode(capture_ctrl_->GetSelection());

    if (capture_ctrl_->GetSelection() == 1) {
        int dev_sel = audio_dev_ctrl_->GetSelection();
        if (dev_sel >= 0 && dev_sel < static_cast<int>(audio_devices_.size())) {
            auto &dev = audio_devices_[dev_sel];
            int needed = WideCharToMultiByte(CP_UTF8, 0, dev.id.c_str(),
                (int)dev.id.size(), nullptr, 0, nullptr, nullptr);
            if (needed > 0) {
                std::string utf8_id(needed, '\0');
                WideCharToMultiByte(CP_UTF8, 0, dev.id.c_str(),
                    (int)dev.id.size(), &utf8_id[0], needed, nullptr, nullptr);
                p.audio_device_id = utf8_id;
            }
            p.audio_device_name = dev.display_name;
        }
    }
    p.auto_switch_device = auto_switch_ctrl_->GetValue();

    fprintf(stderr, "ProfileDialog::OnSave: name='%s' codec=%s quality=%s\n",
            p.name.c_str(), p.codec.c_str(), p.quality.c_str());
    fprintf(stderr, "  bit_depth=%u (bd_sel=%d) sample_rate=%u (sr_sel=%d)\n",
            p.bit_depth, bd_sel, p.sample_rate, sr_sel);
    fprintf(stderr, "  capture_mode=%s audio_device_id='%s' audio_device_name='%s'\n",
            p.capture_mode.c_str(), p.audio_device_id.c_str(), p.audio_device_name.c_str());
    fprintf(stderr, "  edit_index=%d\n", edit_index_);
    fflush(stderr);

    if (edit_index_ >= 0) {
        bool ok = mgr_->update(edit_index_, p);
        fprintf(stderr, "  mgr_->update(%d) returned %d\n", edit_index_, ok);
    } else {
        size_t idx = mgr_->add(p);
        fprintf(stderr, "  mgr_->add() returned %zu\n", idx);
    }

    /* Verify saved data */
    if (edit_index_ >= 0 && edit_index_ < (int)mgr_->profiles().size()) {
        auto &saved = mgr_->profiles()[edit_index_];
        fprintf(stderr, "  verify: bit_depth=%u sample_rate=%u capture_mode=%s\n",
                saved.bit_depth, saved.sample_rate, saved.capture_mode.c_str());
    }
    fflush(stderr);

    EndModal(wxID_OK);
}

void ProfileDialog::OnCancel(wxCommandEvent &) {
    EndModal(wxID_CANCEL);
}
