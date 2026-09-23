/*
 * Update Checker - queries the GitHub Releases API for the latest version
 * SPDX-License-Identifier: MIT
 */

#include "update_checker.h"

#include <windows.h>
#include <winhttp.h>

#include <nlohmann/json.hpp>

#include <algorithm>
#include <cctype>
#include <vector>

const char *const kReleasesPageUrl =
    "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases";

namespace {

constexpr wchar_t kApiHost[] = L"api.github.com";
constexpr wchar_t kApiPath[] =
    L"/repos/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/latest";

/* Response cap — the /releases/latest payload is a few KB */
constexpr size_t kMaxResponseBytes = 1024 * 1024;

std::string strip_v(const std::string &s)
{
    if (!s.empty() && (s[0] == 'v' || s[0] == 'V'))
        return s.substr(1);
    return s;
}

/* "1.0.2-beta" -> {1, 0, 2} (stops at the first non-digit, non-dot char) */
std::vector<long> parse_version_fields(const std::string &s)
{
    std::vector<long> fields;
    size_t i = 0;
    while (i < s.size()) {
        if (std::isdigit(static_cast<unsigned char>(s[i]))) {
            long v = 0;
            while (i < s.size() && std::isdigit(static_cast<unsigned char>(s[i]))) {
                v = v * 10 + (s[i] - '0');
                ++i;
            }
            fields.push_back(v);
        } else if (s[i] == '.') {
            ++i;
        } else {
            break;
        }
    }
    return fields;
}

} /* namespace */

int CompareVersions(const std::string &a, const std::string &b)
{
    std::vector<long> fa = parse_version_fields(strip_v(a));
    std::vector<long> fb = parse_version_fields(strip_v(b));
    size_t n = std::max(fa.size(), fb.size());
    for (size_t i = 0; i < n; ++i) {
        long va = i < fa.size() ? fa[i] : 0;
        long vb = i < fb.size() ? fb[i] : 0;
        if (va != vb)
            return va < vb ? -1 : 1;
    }
    return 0;
}

UpdateCheckResult CheckLatestRelease()
{
    UpdateCheckResult result;
    std::string body;

    HINTERNET session = WinHttpOpen(
        L"A2DPWB",
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
        WINHTTP_NO_PROXY_NAME,
        WINHTTP_NO_PROXY_BYPASS,
        0);
    HINTERNET connect = nullptr;
    HINTERNET request = nullptr;

    do {
        if (!session) {
            result.error = "WinHttpOpen failed";
            break;
        }
        /* resolve / connect / send / receive timeouts (ms) */
        WinHttpSetTimeouts(session, 5000, 5000, 5000, 10000);

        connect = WinHttpConnect(session, kApiHost,
                                 INTERNET_DEFAULT_HTTPS_PORT, 0);
        if (!connect) {
            result.error = "WinHttpConnect failed";
            break;
        }

        request = WinHttpOpenRequest(
            connect, L"GET", kApiPath, nullptr,
            WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES,
            WINHTTP_FLAG_SECURE);
        if (!request) {
            result.error = "WinHttpOpenRequest failed";
            break;
        }

        if (!WinHttpSendRequest(
                request,
                L"Accept: application/vnd.github+json\r\n"
                L"X-GitHub-Api-Version: 2022-11-28",
                static_cast<DWORD>(-1),
                WINHTTP_NO_REQUEST_DATA, 0, 0, 0)) {
            result.error = "WinHttpSendRequest failed";
            break;
        }

        if (!WinHttpReceiveResponse(request, nullptr)) {
            result.error = "WinHttpReceiveResponse failed";
            break;
        }

        DWORD status = 0;
        DWORD status_size = sizeof(status);
        WinHttpQueryHeaders(request,
                            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                            WINHTTP_HEADER_NAME_BY_INDEX,
                            &status, &status_size, WINHTTP_NO_HEADER_INDEX);
        if (status != 200) {
            result.error = "HTTP status " + std::to_string(status);
            break;
        }

        for (;;) {
            DWORD available = 0;
            if (!WinHttpQueryDataAvailable(request, &available) || available == 0)
                break;
            std::vector<char> buf(available);
            DWORD read = 0;
            if (!WinHttpReadData(request, buf.data(), available, &read) || read == 0)
                break;
            body.append(buf.data(), read);
            if (body.size() > kMaxResponseBytes) {
                body.clear();
                result.error = "response too large";
                break;
            }
        }
    } while (false);

    if (request) WinHttpCloseHandle(request);
    if (connect) WinHttpCloseHandle(connect);
    if (session) WinHttpCloseHandle(session);

    if (!result.error.empty())
        return result;
    if (body.empty()) {
        result.error = "empty response";
        return result;
    }

    try {
        nlohmann::json j = nlohmann::json::parse(body);
        std::string tag = j.value("tag_name", std::string());
        if (tag.empty()) {
            result.error = "no tag_name in response";
            return result;
        }
        result.latest_version = strip_v(tag);
        result.release_url = j.value("html_url", std::string(kReleasesPageUrl));
        result.ok = true;
    } catch (const nlohmann::json::exception &) {
        result.error = "JSON parse error";
    }
    return result;
}
