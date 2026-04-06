/*
 * Zadig Helper - Open Zadig download page
 * SPDX-License-Identifier: MIT
 */

#include "zadig_helper.h"
#include <windows.h>
#include <shellapi.h>

void ZadigHelper::open_zadig_website() {
    ShellExecuteW(nullptr, L"open", L"https://zadig.akeo.ie/", nullptr, nullptr, SW_SHOWNORMAL);
}
