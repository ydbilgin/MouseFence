//! The window's view of the string tables (C# `Strings.Ui(key)`, `Strings.X` and `string.Format`).

use fence_i18n::Lang;

/// One resolved UI language. The window resolves it once when it opens, as C# `Strings.Use(_draft.Language)` did
/// in the constructor; a language change takes effect the next time the window opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tr(pub Lang);

impl Tr {
    /// C# `Strings.Ui(key)` / `Strings.X`.
    pub fn t(&self, key: &str) -> String {
        fence_i18n::get(self.0, key).to_string()
    }

    /// C# `string.Format(Strings.Ui(key), args...)`.
    pub fn f(&self, key: &str, args: &[&str]) -> String {
        fence_i18n::format(self.0, key, args)
    }

    pub fn lang(&self) -> Lang {
        self.0
    }
}

/// C# `Strings.Use(setting)` with the OS fact passed in ("auto" follows the Windows display language).
pub fn resolve(setting: &str, os_ui_is_turkish: bool) -> Tr {
    Tr(fence_i18n::resolve(setting, os_ui_is_turkish))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_ui_text_formats_and_falls_back_like_strings_ui() {
        let en = Tr(Lang::En);
        let tr = Tr(Lang::Tr);
        assert_eq!(en.t("Save"), "Save");
        assert_eq!(tr.t("Save"), "Kaydet");
        assert_eq!(en.f("DefaultScreenName", &["3"]), "Screen 3");
        assert_eq!(
            en.t("NoSuchKey"),
            "NoSuchKey",
            "a missing key shows the key, as C# Strings.Ui"
        );
        assert_eq!(resolve("auto", true).lang(), Lang::Tr);
        assert_eq!(resolve("en", true).lang(), Lang::En);
    }
}
