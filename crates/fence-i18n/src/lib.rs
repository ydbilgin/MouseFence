//! MouseFence UI strings (port of C# `I18n/Strings*.cs`). Complete in the skeleton.
//!
//! Shared by Track C (tray menu, tooltips, balloons) and Track D (settings window). Additive only: a new key
//! goes into BOTH `en.rs` and `tr.rs` in the same commit, with the same `{n}` placeholders; the tests below
//! fail otherwise; user-facing strings must use this shared table.
#![forbid(unsafe_code)]

mod en;
mod tr;

/// A UI language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    Tr,
}

/// C# `Strings.Resolve`: "tr" and "en" are explicit; anything else ("auto", unknown) follows the OS UI
/// language (Turkish -> tr, everything else -> en). The caller passes the OS fact (fence-win reads it).
pub fn resolve(setting: &str, os_ui_is_turkish: bool) -> Lang {
    match setting {
        "tr" => Lang::Tr,
        "en" => Lang::En,
        _ if os_ui_is_turkish => Lang::Tr,
        _ => Lang::En,
    }
}

fn table(lang: Lang) -> &'static [(&'static str, &'static str)] {
    match lang {
        Lang::En => en::EN,
        Lang::Tr => tr::TR,
    }
}

/// C# `Strings.Raw(lang, key)`: the value in exactly that language, if present.
pub fn raw(lang: Lang, key: &str) -> Option<&'static str> {
    table(lang).iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// C# `Strings.G` / `Strings.Ui`: the value in `lang`, else the English value, else the key itself.
pub fn get(lang: Lang, key: &str) -> &str {
    raw(lang, key).or_else(|| raw(Lang::En, key)).unwrap_or(key)
}

/// C# `string.Format(G(key), args...)` for the interpolated strings (`TipHotkeyFail`, `TrayPaused`, ...).
/// `{n}` is replaced by `args[n]`; `{{` and `}}` are literal braces; a missing argument leaves `{n}` as is
/// (C# would throw; the arity test makes that unreachable for shipped keys).
pub fn format(lang: Lang, key: &str, args: &[&str]) -> String {
    let template = get(lang, key);
    let mut out = String::with_capacity(template.len() + 16);
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            out.push('{');
            i += 2;
            continue;
        }
        if c == b'}' && i + 1 < bytes.len() && bytes[i + 1] == b'}' {
            out.push('}');
            i += 2;
            continue;
        }
        if c == b'{'
            && let Some(end) = template[i + 1..].find('}')
            && let Ok(n) = template[i + 1..i + 1 + end].parse::<usize>()
            && let Some(arg) = args.get(n)
        {
            out.push_str(arg);
            i += end + 2;
            continue;
        }
        // Copy one UTF-8 character.
        let ch_len = template[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        out.push_str(&template[i..i + ch_len]);
        i += ch_len;
    }
    out
}

/// Every key of one language, in table order.
pub fn keys(lang: Lang) -> impl Iterator<Item = &'static str> {
    table(lang).iter().map(|(k, _)| *k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn count(s: &str, n: usize) -> usize {
        s.matches(&format!("{{{n}}}")).count()
    }

    // Parity row P225 (C# tests/Tests.cs:2872).
    #[test]
    fn p225_i18n_en_and_tr_have_identical_keys() {
        let en: BTreeSet<_> = keys(Lang::En).collect();
        let tr: BTreeSet<_> = keys(Lang::Tr).collect();
        let missing_in_tr: Vec<_> = en.difference(&tr).collect();
        let missing_in_en: Vec<_> = tr.difference(&en).collect();
        assert!(
            missing_in_tr.is_empty() && missing_in_en.is_empty(),
            "missingInTr={missing_in_tr:?} missingInEn={missing_in_en:?}"
        );
        assert_eq!(
            en.len(),
            304,
            "the 1.x tables have 282 keys; 2.0 adds safety, crossing-model and local tuning controls"
        );
    }

    // Parity row P226 (C# tests/Tests.cs:2876-2890): placeholders {0}..{3} have the same count in EN and TR.
    #[test]
    fn p226_i18n_placeholder_arity_matches_across_en_and_tr() {
        let mut bad = Vec::new();
        for key in keys(Lang::En) {
            let en = raw(Lang::En, key).unwrap_or("");
            let tr = raw(Lang::Tr, key).unwrap_or("");
            for n in 0..4 {
                if count(en, n) != count(tr, n) {
                    bad.push(format!("{key}{{{n}}}"));
                }
            }
        }
        assert!(bad.is_empty(), "placeholder mismatch: {bad:?}");
    }

    #[test]
    fn s_i18n_placeholder_arity_matches_up_to_ten() {
        for key in keys(Lang::En) {
            let en = raw(Lang::En, key).unwrap_or("");
            let tr = raw(Lang::Tr, key).unwrap_or("");
            for n in 0..10 {
                assert_eq!(count(en, n), count(tr, n), "{key} {{{n}}}");
            }
        }
    }

    #[test]
    fn s_i18n_no_duplicate_keys() {
        for lang in [Lang::En, Lang::Tr] {
            let all: Vec<_> = keys(lang).collect();
            let set: BTreeSet<_> = all.iter().collect();
            assert_eq!(all.len(), set.len());
        }
    }

    #[test]
    fn s_i18n_resolve_follows_csharp() {
        assert_eq!(resolve("tr", false), Lang::Tr);
        assert_eq!(resolve("en", true), Lang::En);
        assert_eq!(resolve("auto", true), Lang::Tr);
        assert_eq!(resolve("auto", false), Lang::En);
        assert_eq!(resolve("de", true), Lang::Tr);
    }

    #[test]
    fn s_i18n_fallback_is_english_then_key() {
        assert_eq!(get(Lang::Tr, "NoSuchKey_x"), "NoSuchKey_x");
        assert_eq!(get(Lang::En, "MenuSettings"), raw(Lang::En, "MenuSettings").unwrap());
        assert_ne!(get(Lang::Tr, "SettingsTitle"), get(Lang::En, "SettingsTitle"));
    }

    #[test]
    fn s_i18n_format_replaces_placeholders_like_string_format() {
        let s = format(Lang::En, "TipHotkeyFail", &["Ctrl+Alt+Up"]);
        assert!(s.contains("Ctrl+Alt+Up"), "{s}");
        assert!(!s.contains("{0}"));
        let template = raw(Lang::En, "TipIntelDefaults").unwrap();
        assert!(template.contains("{0}") && template.contains("{1}"));
        let s2 = format(Lang::Tr, "TipIntelDefaults", &["A+B", "C+D"]);
        assert!(s2.contains("A+B") && s2.contains("C+D"));
    }
}
