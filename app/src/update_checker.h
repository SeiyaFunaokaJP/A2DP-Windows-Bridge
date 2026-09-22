/*
 * Update Checker - queries the GitHub Releases API for the latest version
 * SPDX-License-Identifier: MIT
 */

#ifndef UPDATE_CHECKER_H
#define UPDATE_CHECKER_H

#include <string>

struct UpdateCheckResult {
    bool ok = false;
    std::string latest_version; /* e.g. "1.0.2" (leading 'v' stripped) */
    std::string release_url;    /* html_url of the latest release */
    std::string error;          /* diagnostic detail, not localized */
};

/* Blocking HTTPS request to the GitHub Releases API.
 * Call from a worker thread, never from the GUI thread. */
UpdateCheckResult CheckLatestRelease();

/* Compare dotted version strings numerically ("1.2.0" > "1.10.0" is false).
 * A leading 'v'/'V' and pre-release suffixes ("-beta") are ignored.
 * Returns <0, 0 or >0 like strcmp. */
int CompareVersions(const std::string &a, const std::string &b);

/* Releases page, used as fallback when the API is unreachable. */
extern const char *const kReleasesPageUrl;

#endif /* UPDATE_CHECKER_H */
