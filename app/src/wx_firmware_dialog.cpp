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
    adapters_ = BtAdapterEnumerator::enumerate();
    bool realtek_seen = false;
    for (const auto &a : adapters_) {
        if (a.vendor == BtChipVendor::Realtek) realtek_seen = true;
        else if (a.vendor != BtChipVendor::Unknown && other_vendor_ == BtChipVendor::Unknown)
            other_vendor_ = a.vendor;
    }
    if (realtek_seen) other_vendor_ = BtChipVendor::Unknown;
    if (adapters_.empty()) {
        adapter_info_->SetLabel(wxString::FromUTF8(L("firmware.no_adapters")));
    } else {
        std::string info_text;
        for (const auto &a : adapters_) {
            if (!info_text.empty()) info_text += "\n";
            info_text += a.display_name;
        }
        adapter_info_->SetLabel(wxString::FromUTF8(info_text));
        adapter_info_->Wrap(480);
    }

    vbox->Add(new wxStaticLine(this), 0, wxEXPAND | wxALL, 8);

    /* ---- Chip selector ----
     * Auto (from the detected adapter), Not Realtek, every Realtek chip in
     * the chip table whether its files are there or not, chips known only
     * by firmware files in the config folder, and Custom. Nothing is saved
     * until the user picks an entry. */
    auto *chip_row = new wxBoxSizer(wxHORIZONTAL);
    auto *chip_label = new wxStaticText(this, wxID_ANY, wxString::FromUTF8(L("settings.bt_chip")));
    chip_label->SetForegroundColour(TM().get(ThemeColor::TextPrimary));
    chip_row->Add(chip_label, 0, wxALIGN_CENTER_VERTICAL | wxRIGHT, 8);
    chip_ctrl_ = new wxChoice(this, wxID_ANY);
    chip_ctrl_->SetBackgroundColour(TM().get(ThemeColor::CtrlBg));
    chip_ctrl_->SetForegroundColour(TM().get(ThemeColor::CtrlFg));

    auto add_option = [this](uint16_t pid, const std::string &stem, const wxString &label) {
        options_.push_back({pid, stem});
        chip_ctrl_->Append(label);
    };

    uint16_t auto_pid = BtAdapterEnumerator::resolve_chip_pid(0, "", adapters_);
    std::string auto_chip;
    if (auto_pid != 0) {
        const char *name = BtAdapterEnumerator::realtek_chip_name(auto_pid);
        char buf[32];
        snprintf(buf, sizeof(buf), "Realtek 0x%04X", auto_pid);
        auto_chip = name ? name : buf;
    }
    add_option(0, "", wxString::Format(wxString::FromUTF8(L("firmware.chip_auto")),
                                       auto_pid ? wxString::FromUTF8(auto_chip)
                                                : wxString::FromUTF8(L("firmware.chip_auto_none"))));
    add_option(NON_REALTEK_CHIP_PID, "", wxString::FromUTF8(L("firmware.chip_non_realtek")));
    for (const auto &c : BtAdapterEnumerator::get_realtek_chips()) {
        if (c.pid == CUSTOM_CHIP_PID) continue;
        std::string stem = c.fw_name;
        stem.resize(stem.size() - 3);  /* drop "_fw" */
        add_option(c.pid, stem, wxString::FromUTF8(c.chip_name));
    }
    for (const auto &e : BtAdapterEnumerator::scan_firmware_files(service->get_config_dir())) {
        if (e.pid == 0)
            add_option(0, e.stem, wxString::FromUTF8(e.display_name + " (" + e.stem + "_fw.bin)"));
    }
    add_option(CUSTOM_CHIP_PID, "custom",
               wxString::FromUTF8("Custom (custom_fw.bin, custom_config.bin)"));

    int chip_sel = -1;
    for (int i = 0; i < static_cast<int>(options_.size()) && chip_sel < 0; i++) {
        const auto &o = options_[i];
        if (o.pid != 0 ? o.pid == settings->bt_chip_pid
                       : settings->bt_chip_pid == 0 && o.stem == settings->bt_chip_fw_stem)
            chip_sel = i;
    }
    if (chip_sel < 0) {
        /* A setting no entry stands for (e.g. a chip since dropped from the
         * table): keep it selectable as it is */
        char buf[64];
        snprintf(buf, sizeof(buf), "0x%04X %s", settings->bt_chip_pid,
                 settings->bt_chip_fw_stem.c_str());
        add_option(settings->bt_chip_pid, settings->bt_chip_fw_stem, wxString::FromUTF8(buf));
        chip_sel = static_cast<int>(options_.size()) - 1;
    }
    chip_ctrl_->SetSelection(chip_sel);

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
    if (sel < 0 || sel >= static_cast<int>(options_.size())) return;

    uint16_t new_pid = options_[sel].pid;
    std::string new_stem = options_[sel].stem;

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

uint16_t FirmwareDialog::effective_pid() const {
    return BtAdapterEnumerator::resolve_chip_pid(settings_->bt_chip_pid,
                                                 settings_->bt_chip_fw_stem, adapters_);
}

void FirmwareDialog::UpdateFirmwareStatus() {
    uint16_t pid = effective_pid();
    bool is_auto = BtAdapterEnumerator::is_auto_chip(settings_->bt_chip_pid,
                                                     settings_->bt_chip_fw_stem);
    bool non_realtek = settings_->bt_chip_pid == NON_REALTEK_CHIP_PID;

    /* Determine firmware/config filenames */
    std::string fw_name, cfg_name;
    const char *fw = BtAdapterEnumerator::realtek_fw_name(pid);
    const char *cfg = BtAdapterEnumerator::realtek_cfg_name(pid);
    if (fw && cfg) {
        fw_name = fw;
        cfg_name = cfg;
    } else if (!non_realtek && !settings_->bt_chip_fw_stem.empty()) {
        fw_name = settings_->bt_chip_fw_stem + "_fw";
        cfg_name = settings_->bt_chip_fw_stem + "_config";
    }

    if (fw_name.empty() && other_vendor_ == BtChipVendor::Unknown && (non_realtek || is_auto)) {
        /* No Realtek init and no vendor we can say more about */
        const char *key = non_realtek      ? "firmware.non_realtek"
                        : adapters_.empty() ? "firmware.auto_no_adapter"
                                            : "firmware.auto_unrecognized";
        status_text_->SetLabel(wxString::FromUTF8(L(key)));
        status_text_->SetForegroundColour(TM().get(non_realtek ? ThemeColor::FirmwareOk
                                                               : ThemeColor::FirmwareWarning));
        status_text_->Wrap(480);
        download_btn_->Enable(false);
        manual_hint_->SetLabel("");
    } else if (fw_name.empty() && other_vendor_ != BtChipVendor::Unknown) {
        /* Non-Realtek adapter: no rtl_bt files; explain what (if anything)
         * this vendor needs instead */
        const char *key = "firmware.vendor_unsupported";
        bool ok = true;
        switch (other_vendor_) {
        case BtChipVendor::Intel:    key = "firmware.vendor_intel"; break;
        case BtChipVendor::Broadcom: key = "firmware.vendor_broadcom"; break;
        case BtChipVendor::Csr:      key = "firmware.vendor_csr"; break;
        default:                     ok = false; break;
        }
        status_text_->SetLabel(wxString::FromUTF8(L(key)));
        status_text_->SetForegroundColour(TM().get(ok ? ThemeColor::FirmwareOk
                                                      : ThemeColor::FirmwareWarning));
        status_text_->Wrap(480);
        download_btn_->Enable(other_vendor_ == BtChipVendor::Intel ||
                              other_vendor_ == BtChipVendor::Broadcom);
        manual_hint_->SetLabel("");
    } else if (fw_name.empty()) {
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
    /* Open the firmware source for the adapter's vendor in the user's browser */
    const char *url =
        "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/"
        "linux-firmware.git/tree/rtl_bt";
    bool realtek_selected = BtAdapterEnumerator::realtek_fw_name(effective_pid()) ||
                            (settings_->bt_chip_pid != NON_REALTEK_CHIP_PID &&
                             !settings_->bt_chip_fw_stem.empty());
    if (!realtek_selected && other_vendor_ == BtChipVendor::Intel) {
        url = "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/"
              "linux-firmware.git/tree/intel";
    } else if (!realtek_selected && other_vendor_ == BtChipVendor::Broadcom) {
        /* linux-firmware carries no brcm .hcd; this collects them from
         * the Windows drivers */
        url = "https://github.com/winterheart/broadcom-bt-firmware";
    }
    ShellExecuteA(nullptr, "open", url, nullptr, nullptr, SW_SHOWDEFAULT);
}

void FirmwareDialog::OnOpenFolder(wxCommandEvent &) {
    std::string dir = service_->get_config_dir();
    /* Create directory if it doesn't exist */
    CreateDirectoryA(dir.c_str(), nullptr);
    ShellExecuteA(nullptr, "open", dir.c_str(), nullptr, nullptr, SW_SHOWDEFAULT);
}
