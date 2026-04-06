/*
 * Localization - i18n via embedded JSON language data
 * SPDX-License-Identifier: MIT
 */

#include "localization.h"
#include "embedded_langs.h"

#include <nlohmann/json.hpp>

#include <algorithm>
#include <string>

using json = nlohmann::json;

/* Parse a JSON string into the strings map. Returns true on success. */
static bool parse_lang_json(const std::string &json_str,
                            std::unordered_map<std::string, std::string> &out)
{
    json j;
    try {
        j = json::parse(json_str);
    } catch (const json::parse_error &) {
        return false;
    }

    out.clear();
    for (auto it = j.begin(); it != j.end(); ++it) {
        if (it.value().is_string())
            out[it.key()] = it.value().get<std::string>();
    }
    return true;
}

Localization &Localization::instance()
{
    static Localization inst;
    return inst;
}

bool Localization::load(const std::string &lang_code)
{
    const auto &langs = get_embedded_languages();
    auto it = langs.find(lang_code);
    if (it == langs.end())
        return false;

    if (!parse_lang_json(it->second, strings_))
        return false;

    current_lang_ = lang_code;
    return true;
}

const char *Localization::get(const char *key) const
{
    auto it = strings_.find(key);
    if (it != strings_.end())
        return it->second.c_str();
    return key;
}

std::vector<Localization::LanguageInfo> Localization::get_available_languages() const
{
    std::vector<LanguageInfo> result;

    for (const auto &[code, json_str] : get_embedded_languages()) {
        std::string display_name = code;
        try {
            auto j = json::parse(json_str);
            if (j.contains("_language_name") && j["_language_name"].is_string())
                display_name = j["_language_name"].get<std::string>();
        } catch (...) {}

        result.push_back({code, display_name});
    }

    std::sort(result.begin(), result.end(),
        [](const LanguageInfo &a, const LanguageInfo &b) { return a.code < b.code; });

    if (result.empty())
        result.push_back({"en", "English"});

    return result;
}
