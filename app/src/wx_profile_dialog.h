/*
 * Profile Dialog - Create/Edit connection profiles
 * SPDX-License-Identifier: MIT
 */

#ifndef WX_PROFILE_DIALOG_H
#define WX_PROFILE_DIALOG_H

#include "a2dp_service.h"
#include "profile_manager.h"
#include "audio_device_enum.h"

#include <wx/wx.h>
#include <wx/listbox.h>
#include <wx/spinctrl.h>


class ProfileDialog : public wxDialog {
public:
    /* edit_index < 0 for new profile */
    ProfileDialog(wxWindow *parent, A2dpService *service,
                  ProfileManager *mgr, int edit_index);

private:
    void create_ui();
    void populate_from_profile(const ConnectionProfile &p);
    void update_codec_dependent();
    void on_scan_complete();
    void OnSave(wxCommandEvent &evt);
    void OnCancel(wxCommandEvent &evt);
    void OnScan(wxCommandEvent &evt);
    void OnCodecChange(wxCommandEvent &evt);
    void OnDeviceSelect(wxCommandEvent &evt);
    void OnDeviceEditName(wxCommandEvent &evt);
    void OnDeviceDelete(wxCommandEvent &evt);
    void OnCaptureChange(wxCommandEvent &evt);
    void OnAddrChange(wxCommandEvent &evt);
    bool validate_address(const wxString &addr);
    void update_addr_visual();
    void update_format_info();

    A2dpService    *service_;
    ProfileManager *mgr_;
    int             edit_index_;

    /* Controls */
    wxTextCtrl  *addr_ctrl_ = nullptr;
    wxTextCtrl  *devname_ctrl_ = nullptr;
    wxListBox   *device_list_ = nullptr;
    wxChoice    *codec_ctrl_ = nullptr;
    wxChoice    *quality_ctrl_ = nullptr;
    wxCheckBox  *abr_ctrl_ = nullptr;
    wxChoice    *sample_rate_ctrl_ = nullptr;
    wxChoice    *bit_depth_ctrl_ = nullptr;
    wxChoice    *capture_ctrl_ = nullptr;
    wxChoice    *audio_dev_ctrl_ = nullptr;
    wxCheckBox  *auto_switch_ctrl_ = nullptr;
    wxSpinCtrl  *max_packet_ctrl_ = nullptr;
    wxStaticText *audio_dev_label_ = nullptr;
    wxStaticText *auto_switch_label_ = nullptr;
    wxStaticText *format_info_ = nullptr;
    wxButton    *scan_btn_ = nullptr;
    wxButton    *dev_edit_btn_ = nullptr;
    wxButton    *dev_del_btn_ = nullptr;

    std::vector<AudioDeviceInfo> audio_devices_;
};

#endif /* WX_PROFILE_DIALOG_H */
