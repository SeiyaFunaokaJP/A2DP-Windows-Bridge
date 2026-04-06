/*
 * wxWidgets Application Implementation
 * SPDX-License-Identifier: MIT
 */

#include "wx_app.h"
#include "wx_main_frame.h"
#include "app_settings.h"
#include "config_path.h"
#include "localization.h"
#include "theme_manager.h"
#include <wx/stdpaths.h>
#include <cstdio>
#include <windows.h>

wxIMPLEMENT_APP_NO_MAIN(A2dpBridgeApp);

bool A2dpBridgeApp::OnInit() {
    if (!wxApp::OnInit()) return false;

    /* Parse --minimized from argv */
    for (int i = 1; i < argc; i++) {
        if (wxString(argv[i]) == "--minimized")
            start_minimized_ = true;
    }

    /* Redirect stderr based on debug mode setting */
    AppSettings boot_settings;
    boot_settings.load();
    if (boot_settings.debug_mode) {
        std::string log_path = get_config_dir() + "\\debug.log";
        freopen(log_path.c_str(), "w", stderr);
        setvbuf(stderr, nullptr, _IONBF, 0);
        fprintf(stderr, "=== A2DPWB started (debug mode) ===\n");
    } else {
        freopen("NUL", "w", stderr);
    }
    freopen("NUL", "w", stdout);

    ThemeManager::instance().set_mode(ThemeManager::mode_from_string(boot_settings.theme));

    frame_ = new MainFrame();
    SetTopWindow(frame_);

    if (start_minimized_) {
        frame_->Show(false);
    } else {
        frame_->Show(true);
    }

    return true;
}

int A2dpBridgeApp::OnExit() {
    return wxApp::OnExit();
}
