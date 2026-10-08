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
 * include_prereleases: also consider releases marked as pre-release
 * (/releases/latest never returns one).
 * Call from a worker thread, never from the GUI thread. */
UpdateCheckResult CheckLatestRelease(bool include_prereleases);

/* Compare version strings with semver precedence: the dotted core numerically
 * ("1.2.0" > "1.10.0" is false), then a pre-release ranks below its release
 * ("1.1.0-beta.2" < "1.1.0") and pre-release identifiers compare field by
 * field ("beta.2" < "beta.10" < "rc.1"). A leading 'v'/'V' and build
 * metadata ("+abc") are ignored. Returns <0, 0 or >0 like strcmp. */
int CompareVersions(const std::string &a, const std::string &b);

/* Releases page, used as fallback when the API is unreachable. */
extern const char *const kReleasesPageUrl;

#endif /* UPDATE_CHECKER_H */
