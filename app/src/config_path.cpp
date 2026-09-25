/*
 * Config path utility
 *
 * SPDX-License-Identifier: MIT
 */

#include "config_path.h"
#include <cstdlib>
#include <windows.h>

std::string get_config_dir() {
    char *appdata = nullptr;
    size_t len = 0;

    /* Development / testing: A2DPWB_CONFIG_DIR isolates settings, link keys
     * and logs (tools/emu pairs with virtual devices and must not touch the
     * user's real link keys). */
    if (_dupenv_s(&appdata, &len, "A2DPWB_CONFIG_DIR") == 0 && appdata && appdata[0]) {
        std::string dir(appdata);
        free(appdata);
        CreateDirectoryA(dir.c_str(), nullptr);
        return dir;
    }
    free(appdata);
    appdata = nullptr;

    if (_dupenv_s(&appdata, &len, "APPDATA") != 0 || !appdata)
        return ".";
    std::string base(appdata);
    free(appdata);

    std::string dir = base + "\\A2DPWB";
    CreateDirectoryA(dir.c_str(), nullptr);
    return dir;
}
