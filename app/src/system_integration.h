/*
 * System Integration - Windows startup
 * SPDX-License-Identifier: MIT
 */

#ifndef SYSTEM_INTEGRATION_H
#define SYSTEM_INTEGRATION_H

#include <windows.h>
#include <string>

/* Windows Startup */
bool RegisterStartup(const std::string &args = "--minimized");
bool UnregisterStartup();
bool IsStartupRegistered();

/* Utility */
std::wstring GetExePathW();

#endif /* SYSTEM_INTEGRATION_H */
