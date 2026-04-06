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
    if (_dupenv_s(&appdata, &len, "APPDATA") != 0 || !appdata)
        return ".";
    std::string base(appdata);
    free(appdata);

    std::string dir = base + "\\A2DPWB";
    CreateDirectoryA(dir.c_str(), nullptr);
    return dir;
}
