/*
 * Localization - i18n via embedded JSON language data
 * SPDX-License-Identifier: MIT
 */

#ifndef LOCALIZATION_H
#define LOCALIZATION_H

#include <string>
#include <unordered_map>
#include <vector>

class Localization {
public:
    struct LanguageInfo {
        std::string code;         /* e.g. "en", "ja" */
        std::string display_name; /* e.g. "English", "日本語" */
    };

    /* Load embedded language data for given code.
     * Returns false if the language is not available. */
    bool load(const std::string &lang_code);

    /* Get translated string by key. Returns key itself if not found. */
    const char *get(const char *key) const;

    /* List all embedded languages. */
    std::vector<LanguageInfo> get_available_languages() const;

    /* Current language code */
    const std::string &current_language() const { return current_lang_; }

    /* Singleton access */
    static Localization &instance();

private:
    Localization() = default;
    std::unordered_map<std::string, std::string> strings_;
    std::string current_lang_ = "en";
};

/* Convenience function */
inline const char *L(const char *key) {
    return Localization::instance().get(key);
}

#endif /* LOCALIZATION_H */
