/*
 * Firmware Dialog Implementation
 *
 * Consolidated firmware management: detected adapter display, file-based
 * chip selection (scans config folder for *_fw.bin + *_config.bin pairs),
 * firmware status, and manual placement instructions.
 *
 * SPDX-License-Identifier: MIT
 */

#include "wx_firmware_dialog.h"
#include "bt_adapter_enum.h"
#include "localization.h"
#include "theme_manager.h"
#include <wx/statline.h>
#include <wx/hyperlink.h>
#include <shellapi.h>

FirmwareDialog::FirmwareDialog(wxWindow *parent, A2dpService *service, AppSettings *settings)
    : wxDialog(parent, wxID_ANY, wxString::FromUTF8(L("firmware.title")),
               wxDefaultPosition, wxDefaultSize, wxDEFAULT_DIALOG_STYLE)
    , service_(service), settings_(settings)
{
    auto *vbox = new wxBoxSizer(wxVERTICAL);

    /* ---- Detected Bluetooth adapters ---- */
    auto *adapter_label = new wxStaticText(this, wxID_ANY,
        wxString::FromUTF8(L("firmware.detected_adapters")));
    adapter_label->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    vbox->Add(adapter_label, 0, wxLEFT | wxRIGHT | wxTOP, 16);

    adapter_info_ = new wxStaticText(this, wxID_ANY, "", wxDefaultPosition, wxSize(480, -1));
    adapter_info_->Wrap(480);
    adapter_info_->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
    vbox->Add(adapter_info_, 0, wxLEFT | wxRIGHT | wxTOP, 8);

    /* Enumerate USB Bluetooth adapters */
    auto adapters = BtAdapterEnumerator::enumerate();
    uint16_t detected_realtek_pid = 0;
    if (adapters.empty()) {
        adapter_info_->SetLabel(wxString::FromUTF8(L("firmware.no_adapters")));
    } else {
        std::string info_text;
        for (const auto &a : adapters) {
            if (!info_text.empty()) info_text += "\n";
            info_text += a.display_name;
            /* Remember first detected Realtek PID for auto-selection */
            if (a.realtek_pid != 0 && detected_realtek_pid == 0) {
                detected_realtek_pid = a.realtek_pid;
            }
        }
        adapter_info_->SetLabel(wxString::FromUTF8(info_text));
        adapter_info_->Wrap(480);
    }

    vbox->Add(new wxStaticLine(this), 0, wxEXPAND | wxALL, 8);

    /* ---- Chip type selector (populated from firmware files) ---- */
    auto *chip_row = new wxBoxSizer(wxHORIZONTAL);
    auto *chip_label = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("settings.bt_chip")));
    chip_label->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    chip_row->Add(chip_label, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    chip_ctrl_ = new wxChoice(this, wxID_ANY);
    chip_ctrl_->SetBackgroundColour(TM().get(ThemeColor::CtrlBg));
    chip_ctrl_->SetForegroundColour(TM().get(ThemeColor::CtrlFg));

    /* Scan config directory for firmware file pairs */
    fw_entries_ = BtAdapterEnumerator::scan_firmware_files(service->get_config_dir());

    int chip_sel = -1;
    if (fw_entries_.empty()) {
        /* No firmware files found — show hint text (Custom still available) */
        chip_ctrl_->Append(wxString::FromUTF8(L("firmware.no_firmware_files_short")));
    } else {
        for (int i = 0; i < static_cast<int>(fw_entries_.size()); i++) {
            chip_ctrl_->Append(wxString::FromUTF8(fw_entries_[i].display_name));
            if (fw_entries_[i].pid != 0 && fw_entries_[i].pid == settings->bt_chip_pid)
                chip_sel = i;
            else if (fw_entries_[i].pid == 0 && fw_entries_[i].stem == settings->bt_chip_fw_stem)
                chip_sel = i;
        }
    }
    /* Always append Custom as last real entry */
    chip_ctrl_->Append(wxString::FromUTF8("Custom"));

    /* Auto-select based on detected adapter (if firmware files exist) */
    if (detected_realtek_pid != 0 && chip_sel < 0) {
        for (int i = 0; i < static_cast<int>(fw_entries_.size()); i++) {
            if (fw_entries_[i].pid == detected_realtek_pid) {
                chip_sel = i;
                /* Apply auto-selection to settings */
                settings->bt_chip_pid = fw_entries_[i].pid;
                settings->bt_chip_fw_stem = fw_entries_[i].stem;
                settings->save();
                service->set_bt_chip_pid(fw_entries_[i].pid);
                service->set_bt_chip_fw_stem(fw_entries_[i].stem);
                service->check_firmware_present();
                chip_changed_ = true;
                break;
            }
        }
    }

    if (chip_sel >= 0) {
        chip_ctrl_->SetSelection(chip_sel);
    } else if (!fw_entries_.empty()) {
        /* Default to first firmware entry if nothing matched */
        chip_ctrl_->SetSelection(0);
        auto &first = fw_entries_[0];
        if (first.pid != settings->bt_chip_pid || first.stem != settings->bt_chip_fw_stem) {
            settings->bt_chip_pid = first.pid;
            settings->bt_chip_fw_stem = first.stem;
            settings->save();
            service->set_bt_chip_pid(first.pid);
            service->set_bt_chip_fw_stem(first.stem);
            service->check_firmware_present();
            chip_changed_ = true;
        }
    } else {
        chip_ctrl_->SetSelection(0);  /* placeholder or Custom */
    }

    chip_ctrl_->Bind(wxEVT_CHOICE, &FirmwareDialog::OnChipChanged, this);
    chip_row->Add(chip_ctrl_, 1, wxEXPAND);
    vbox->Add(chip_row, 0, wxEXPAND | wxALL, 16);

    vbox->Add(new wxStaticLine(this), 0, wxEXPAND | wxLEFT | wxRIGHT, 8);

    /* ---- Firmware status ---- */
    status_text_ = new wxStaticText(this, wxID_ANY, "", wxDefaultPosition, wxSize(480, -1));
    status_text_->Wrap(480);
    vbox->Add(status_text_, 0, wxLEFT | wxRIGHT | wxTOP, 16);

    /* ---- Required files hint ---- */
    manual_hint_ = new wxStaticText(this, wxID_ANY, "", wxDefaultPosition, wxSize(480, -1));
    manual_hint_->Wrap(480);
    manual_hint_->SetForegroundColour(TM().get(ThemeColor::FirmwareHint));
    vbox->Add(manual_hint_, 0, wxLEFT | wxRIGHT | wxTOP, 8);

    /* ---- Button row: Open Download Page + Open Folder ---- */
    auto *btn_row = new wxBoxSizer(wxHORIZONTAL);
    download_btn_ = new wxButton(this, wxID_ANY, wxString::FromUTF8(L("firmware.open_download_page")));
    download_btn_->Bind(wxEVT_BUTTON, &FirmwareDialog::OnDownload, this);
    btn_row->Add(download_btn_, 0, wxRIGHT, 8);

    auto *open_folder_btn = new wxButton(this, wxID_ANY,
        wxString::FromUTF8(L("firmware.open_folder")));
    open_folder_btn->Bind(wxEVT_BUTTON, &FirmwareDialog::OnOpenFolder, this);
    btn_row->Add(open_folder_btn, 0);
    vbox->Add(btn_row, 0, wxLEFT | wxRIGHT | wxTOP, 16);

    /* ---- Manual placement instructions ---- */
    auto *instructions = new wxStaticText(this, wxID_ANY,
        wxString::FromUTF8(L("firmware.manual_instructions")),
        wxDefaultPosition, wxSize(480, -1));
    instructions->Wrap(480);
    instructions->SetForegroundColour(TM().get(ThemeColor::TextSecondary));
    vbox->Add(instructions, 0, wxALL, 16);

    vbox->Add(new wxStaticLine(this), 0, wxEXPAND | wxLEFT | wxRIGHT, 8);

    auto *ok_btn = new wxButton(this, wxID_OK, wxString::FromUTF8(L("modal.ok")));
    vbox->Add(ok_btn, 0, wxALIGN_CENTER | wxALL, 12);
    ok_btn->Bind(wxEVT_BUTTON, [this](wxCommandEvent &) { EndModal(wxID_OK); });

    /* Rescan firmware presence when dialog regains focus (e.g. after
       user places files via Open Config Folder and switches back). */
    Bind(wxEVT_ACTIVATE, [this](wxActivateEvent &evt) {
        if (evt.GetActive()) {
            service_->check_firmware_present();
            UpdateFirmwareStatus();
            Fit();
        }
        evt.Skip();
    });

    SetBackgroundColour(TM().get(ThemeColor::DialogBg));
    SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    SetSizer(vbox);
    UpdateFirmwareStatus();
    Fit();
    CentreOnParent();
}

void FirmwareDialog::OnChipChanged(wxCommandEvent &) {
    int sel = chip_ctrl_->GetSelection();
    if (sel < 0) return;

    uint16_t new_pid = 0;
    std::string new_stem;

    /* Last item is always Custom */
    int custom_idx = static_cast<int>(fw_entries_.size());
    if (fw_entries_.empty()) custom_idx = 1;  /* placeholder + Custom */

    if (sel == custom_idx || (fw_entries_.empty() && sel == 1)) {
        new_pid = CUSTOM_CHIP_PID;
        new_stem = "custom";
    } else if (!fw_entries_.empty() && sel < static_cast<int>(fw_entries_.size())) {
        new_pid = fw_entries_[sel].pid;
        new_stem = fw_entries_[sel].stem;
    } else {
        return;
    }

    bool changed = (new_pid != settings_->bt_chip_pid) ||
                   (new_stem != settings_->bt_chip_fw_stem);
    if (changed) {
        settings_->bt_chip_pid = new_pid;
        settings_->bt_chip_fw_stem = new_stem;
        settings_->save();
        service_->set_bt_chip_pid(new_pid);
        service_->set_bt_chip_fw_stem(new_stem);
        service_->check_firmware_present();
        chip_changed_ = true;
    }
    UpdateFirmwareStatus();
    Fit();
}

void FirmwareDialog::UpdateFirmwareStatus() {
    uint16_t pid = settings_->bt_chip_pid;
    bool is_custom = (pid == CUSTOM_CHIP_PID);

    /* Determine firmware/config filenames */
    std::string fw_name, cfg_name;
    const char *fw = BtAdapterEnumerator::realtek_fw_name(pid);
    const char *cfg = BtAdapterEnumerator::realtek_cfg_name(pid);
    if (fw && cfg) {
        fw_name = fw;
        cfg_name = cfg;
    } else if (!settings_->bt_chip_fw_stem.empty()) {
        fw_name = settings_->bt_chip_fw_stem + "_fw";
        cfg_name = settings_->bt_chip_fw_stem + "_config";
    }

    if (fw_name.empty()) {
        status_text_->SetLabel(wxString::FromUTF8(L("firmware.select_chip")));
        status_text_->SetForegroundColour(TM().get(ThemeColor::FirmwareWarning));
        download_btn_->Enable(false);
        manual_hint_->SetLabel("");
    } else if (service_->is_firmware_present()) {
        status_text_->SetLabel(wxString::FromUTF8(L("firmware.present")));
        status_text_->SetForegroundColour(TM().get(ThemeColor::FirmwareOk));
        download_btn_->Enable(true);
        std::string hint = fw_name + ".bin, " + cfg_name + ".bin";
        manual_hint_->SetLabel(wxString::FromUTF8(hint));
        manual_hint_->Wrap(480);
    } else {
        status_text_->SetLabel(wxString::FromUTF8(L("firmware.missing_manual")));
        status_text_->SetForegroundColour(TM().get(ThemeColor::FirmwareWarning));
        download_btn_->Enable(true);
        std::string hint = std::string(L("firmware.required_files")) + "\n"
                         + fw_name + ".bin, " + cfg_name + ".bin";
        manual_hint_->SetLabel(wxString::FromUTF8(hint));
        manual_hint_->Wrap(480);
    }
}

void FirmwareDialog::OnDownload(wxCommandEvent &) {
    /* Open the kernel.org firmware directory in the user's browser */
    ShellExecuteA(nullptr, "open",
        "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/"
        "linux-firmware.git/tree/rtl_bt",
        nullptr, nullptr, SW_SHOWDEFAULT);
}

void FirmwareDialog::OnOpenFolder(wxCommandEvent &) {
    std::string dir = service_->get_config_dir();
    /* Create directory if it doesn't exist */
    CreateDirectoryA(dir.c_str(), nullptr);
    ShellExecuteA(nullptr, "open", dir.c_str(), nullptr, nullptr, SW_SHOWDEFAULT);
}
