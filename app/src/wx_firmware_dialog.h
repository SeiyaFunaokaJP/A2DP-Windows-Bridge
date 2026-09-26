/*
 * Firmware Dialog
 * SPDX-License-Identifier: MIT
 */

#ifndef WX_FIRMWARE_DIALOG_H
#define WX_FIRMWARE_DIALOG_H

#include "a2dp_service.h"
#include "app_settings.h"
#include "bt_adapter_enum.h"
#include <wx/wx.h>
#include <vector>

class FirmwareDialog : public wxDialog {
public:
    FirmwareDialog(wxWindow *parent, A2dpService *service, AppSettings *settings);

    /* Returns true if the chip PID was changed (caller should reset BTstack) */
    bool chip_changed() const { return chip_changed_; }

private:
    void OnChipChanged(wxCommandEvent &evt);
    void OnDownload(wxCommandEvent &evt);
    void OnOpenFolder(wxCommandEvent &evt);
    void UpdateFirmwareStatus();

    A2dpService  *service_;
    AppSettings  *settings_;
    wxChoice     *chip_ctrl_ = nullptr;
    wxStaticText *adapter_info_ = nullptr;
    wxStaticText *status_text_ = nullptr;
    wxButton     *download_btn_ = nullptr;
    wxStaticText *manual_hint_ = nullptr;
    bool          chip_changed_ = false;

    /* Detected non-Realtek adapter vendor (Unknown if none, or if a
     * Realtek adapter is also present) */
    BtChipVendor  other_vendor_ = BtChipVendor::Unknown;

    /* Firmware file entries from config directory scan */
    std::vector<FirmwareFileEntry> fw_entries_;
};

#endif /* WX_FIRMWARE_DIALOG_H */
