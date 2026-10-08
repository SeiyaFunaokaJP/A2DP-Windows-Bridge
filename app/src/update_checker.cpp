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
constexpr wchar_t kLatestPath[] =
    L"/repos/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/latest";
/* Newest first; 20 reaches back past any run of pre-releases */
constexpr wchar_t kListPath[] =
    L"/repos/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases?per_page=20";

/* Response cap — /releases/latest is a few KB, 20 list entries some 100 KB */
constexpr size_t kMaxResponseBytes = 1024 * 1024;

std::string strip_v(const std::string &s)
{
    if (!s.empty() && (s[0] == 'v' || s[0] == 'V'))
        return s.substr(1);
    return s;
}

bool all_digits(const std::string &s)
{
    return !s.empty() &&
           std::all_of(s.begin(), s.end(),
                       [](unsigned char c) { return std::isdigit(c) != 0; });
}

struct ParsedVersion {
    std::vector<long> core;       /* "1.0.2-beta.1" -> {1, 0, 2} */
    std::vector<std::string> pre; /* "1.0.2-beta.1" -> {"beta", "1"} */
};

ParsedVersion parse_version(const std::string &s)
{
    ParsedVersion v;
    size_t i = 0;
    while (i < s.size()) {
        if (std::isdigit(static_cast<unsigned char>(s[i]))) {
            long n = 0;
            while (i < s.size() && std::isdigit(static_cast<unsigned char>(s[i]))) {
                n = n * 10 + (s[i] - '0');
                ++i;
            }
            v.core.push_back(n);
        } else if (s[i] == '.') {
            ++i;
        } else {
            break;
        }
    }
    /* "-beta.1+build" -> {"beta", "1"} */
    if (i < s.size() && s[i] == '-') {
        size_t end = s.find('+', i);
        if (end == std::string::npos)
            end = s.size();
        size_t start = i + 1;
        for (;;) {
            size_t dot = s.find('.', start);
            if (dot == std::string::npos || dot > end)
                dot = end;
            v.pre.push_back(s.substr(start, dot - start));
            if (dot == end)
                break;
            start = dot + 1;
        }
    }
    return v;
}

/* One pre-release identifier: numeric ones compare numerically and rank
 * below alphanumeric ones, which compare in ASCII order (semver 11.4) */
int compare_identifier(const std::string &a, const std::string &b)
{
    bool na = all_digits(a), nb = all_digits(b);
    if (na && nb) {
        std::string ta = a.substr(std::min(a.find_first_not_of('0'), a.size() - 1));
        std::string tb = b.substr(std::min(b.find_first_not_of('0'), b.size() - 1));
        if (ta.size() != tb.size())
            return ta.size() < tb.size() ? -1 : 1;
        int c = ta.compare(tb);
        return c < 0 ? -1 : (c > 0 ? 1 : 0);
    }
    if (na != nb)
        return na ? -1 : 1;
    int c = a.compare(b);
    return c < 0 ? -1 : (c > 0 ? 1 : 0);
}

/* GET https://api.github.com<path> into body; returns "" or an error */
std::string http_get(const wchar_t *path, std::string &body)
{
    std::string error;

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
            error = "WinHttpOpen failed";
            break;
        }
        /* resolve / connect / send / receive timeouts (ms) */
        WinHttpSetTimeouts(session, 5000, 5000, 5000, 10000);

        connect = WinHttpConnect(session, kApiHost,
                                 INTERNET_DEFAULT_HTTPS_PORT, 0);
        if (!connect) {
            error = "WinHttpConnect failed";
            break;
        }

        request = WinHttpOpenRequest(
            connect, L"GET", path, nullptr,
            WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES,
            WINHTTP_FLAG_SECURE);
        if (!request) {
            error = "WinHttpOpenRequest failed";
            break;
        }

        if (!WinHttpSendRequest(
                request,
                L"Accept: application/vnd.github+json\r\n"
                L"X-GitHub-Api-Version: 2022-11-28",
                static_cast<DWORD>(-1),
                WINHTTP_NO_REQUEST_DATA, 0, 0, 0)) {
            error = "WinHttpSendRequest failed";
            break;
        }

        if (!WinHttpReceiveResponse(request, nullptr)) {
            error = "WinHttpReceiveResponse failed";
            break;
        }

        DWORD status = 0;
        DWORD status_size = sizeof(status);
        WinHttpQueryHeaders(request,
                            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                            WINHTTP_HEADER_NAME_BY_INDEX,
                            &status, &status_size, WINHTTP_NO_HEADER_INDEX);
        if (status != 200) {
            error = "HTTP status " + std::to_string(status);
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
                error = "response too large";
                break;
            }
        }
    } while (false);

    if (request) WinHttpCloseHandle(request);
    if (connect) WinHttpCloseHandle(connect);
    if (session) WinHttpCloseHandle(session);

    if (error.empty() && body.empty())
        error = "empty response";
    return error;
}

} /* namespace */

int CompareVersions(const std::string &a, const std::string &b)
{
    ParsedVersion va = parse_version(strip_v(a));
    ParsedVersion vb = parse_version(strip_v(b));
    size_t n = std::max(va.core.size(), vb.core.size());
    for (size_t i = 0; i < n; ++i) {
        long ca = i < va.core.size() ? va.core[i] : 0;
        long cb = i < vb.core.size() ? vb.core[i] : 0;
        if (ca != cb)
            return ca < cb ? -1 : 1;
    }

    /* Same core: the release outranks any of its pre-releases */
    if (va.pre.empty() || vb.pre.empty()) {
        if (va.pre.empty() == vb.pre.empty())
            return 0;
        return va.pre.empty() ? 1 : -1;
    }
    size_t m = std::min(va.pre.size(), vb.pre.size());
    for (size_t i = 0; i < m; ++i) {
        int c = compare_identifier(va.pre[i], vb.pre[i]);
        if (c != 0)
            return c;
    }
    if (va.pre.size() != vb.pre.size())
        return va.pre.size() < vb.pre.size() ? -1 : 1;
    return 0;
}

UpdateCheckResult CheckLatestRelease(bool include_prereleases)
{
    UpdateCheckResult result;
    std::string body;

    result.error = http_get(include_prereleases ? kListPath : kLatestPath, body);
    if (!result.error.empty())
        return result;

    try {
        nlohmann::json j = nlohmann::json::parse(body);
        if (include_prereleases) {
            /* Highest version among the published releases, pre-release or
             * not (drafts are listed only to authenticated maintainers) */
            if (!j.is_array()) {
                result.error = "unexpected response";
                return result;
            }
            nlohmann::json best;
            std::string best_tag;
            for (const auto &rel : j) {
                if (!rel.is_object() || rel.value("draft", false))
                    continue;
                std::string tag = rel.value("tag_name", std::string());
                if (tag.empty())
                    continue;
                if (best_tag.empty() || CompareVersions(tag, best_tag) > 0) {
                    best = rel;
                    best_tag = tag;
                }
            }
            if (best_tag.empty()) {
                result.error = "no releases in response";
                return result;
            }
            j = std::move(best);
        }
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
