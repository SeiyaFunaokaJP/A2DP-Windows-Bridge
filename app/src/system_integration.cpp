/*
 * System Integration - Windows startup
 * SPDX-License-Identifier: MIT
 */

#include "system_integration.h"

#include <windows.h>
#include <shlobj.h>
#include <string>

/* -------------------------------------------------------------------------- */
/*  Windows Startup                                                           */
/* -------------------------------------------------------------------------- */

bool RegisterStartup(const std::string &args)
{
    std::wstring exePath = GetExePathW();
    if (exePath.empty()) {
        return false;
    }

    /* Convert args to wide string */
    std::wstring argsWide(args.begin(), args.end());

    std::wstring command = L"\"" + exePath + L"\" " + argsWide;

    HKEY hKey = nullptr;
    LONG result = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        L"Software\\Microsoft\\Windows\\CurrentVersion\\Run",
        0,
        KEY_SET_VALUE,
        &hKey);

    if (result != ERROR_SUCCESS) {
        return false;
    }

    result = RegSetValueExW(
        hKey,
        L"A2DPWB",
        0,
        REG_SZ,
        reinterpret_cast<const BYTE *>(command.c_str()),
        static_cast<DWORD>((command.size() + 1) * sizeof(wchar_t)));

    RegCloseKey(hKey);
    return result == ERROR_SUCCESS;
}

bool UnregisterStartup()
{
    HKEY hKey = nullptr;
    LONG result = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        L"Software\\Microsoft\\Windows\\CurrentVersion\\Run",
        0,
        KEY_SET_VALUE,
        &hKey);

    if (result != ERROR_SUCCESS) {
        return false;
    }

    result = RegDeleteValueW(hKey, L"A2DPWB");

    RegCloseKey(hKey);
    return result == ERROR_SUCCESS;
}

bool IsStartupRegistered()
{
    HKEY hKey = nullptr;
    LONG result = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        L"Software\\Microsoft\\Windows\\CurrentVersion\\Run",
        0,
        KEY_QUERY_VALUE,
        &hKey);

    if (result != ERROR_SUCCESS) {
        return false;
    }

    result = RegQueryValueExW(hKey, L"A2DPWB",
                              nullptr, nullptr, nullptr, nullptr);

    RegCloseKey(hKey);
    return result == ERROR_SUCCESS;
}

/* -------------------------------------------------------------------------- */
/*  Utility                                                                   */
/* -------------------------------------------------------------------------- */

std::wstring GetExePathW()
{
    wchar_t path[MAX_PATH];
    GetModuleFileNameW(nullptr, path, MAX_PATH);
    return std::wstring(path);
}
