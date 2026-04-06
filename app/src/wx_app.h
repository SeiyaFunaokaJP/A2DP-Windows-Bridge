/*
 * wxWidgets Application - Entry point
 * SPDX-License-Identifier: MIT
 */

#ifndef WX_APP_H
#define WX_APP_H

#include <wx/wx.h>

class MainFrame;

class A2dpBridgeApp : public wxApp {
public:
    bool OnInit() override;
    int  OnExit() override;

    bool start_minimized() const { return start_minimized_; }

private:
    MainFrame *frame_ = nullptr;
    bool start_minimized_ = false;
};

wxDECLARE_APP(A2dpBridgeApp);

#endif /* WX_APP_H */
