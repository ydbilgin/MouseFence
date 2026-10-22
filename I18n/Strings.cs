using System.Globalization;

namespace MouseFence;

/// <summary>
/// Tiny in-memory localization table (no .resx, no satellite assemblies). Add a language by adding
/// one dictionary and a case in <see cref="Use"/>/<see cref="Resolve"/>.
/// </summary>
public static partial class Strings
{
    private static Dictionary<string, string> _t = En;

    /// <summary>Apply a language. value is "auto" | "en" | "tr" (anything else -> en).</summary>
    public static void Use(string language) => _t = Resolve(language) == "tr" ? Tr : En;

    /// <summary>"auto" resolves to the OS UI culture (Turkish -> tr, everything else -> en).</summary>
    public static string Resolve(string language)
    {
        if (language is "tr" or "en") return language;
        return CultureInfo.CurrentUICulture.TwoLetterISOLanguageName == "tr" ? "tr" : "en";
    }

    private static string G(string key) =>
        _t.TryGetValue(key, out var v) ? v : (En.TryGetValue(key, out var e) ? e : key);

    internal static string Ui(string key) => G(key);

    // For tests: every key must exist in BOTH languages, so nothing ever falls back across languages.
    public static IReadOnlyCollection<string> EnglishKeys => En.Keys;
    public static IReadOnlyCollection<string> TurkishKeys => Tr.Keys;

    // For tests: raw value for a key in a specific language ("tr" -> Turkish, else English) — used to verify
    // that interpolated strings carry identical {0}/{1} placeholders in both languages.
    public static string Raw(string lang, string key) => (lang == "tr" ? Tr : En).TryGetValue(key, out var v) ? v : null;

    // ---- settings dialog ----
    public static string SettingsTitle => G("SettingsTitle");
    public static string HotkeyHead => G("HotkeyHead");
    public static string HotkeyHint => G("HotkeyHint");
    public static string HotkeyPressKeys => G("HotkeyPressKeys");
    public static string Clear => G("Clear");
    public static string StartupHead => G("StartupHead");
    public static string StartGateClosed => G("StartGateClosed");
    public static string StartGateClosedTip => G("StartGateClosedTip");
    public static string StartWithWindows => G("StartWithWindows");
    public static string StartWithWindowsTip => G("StartWithWindowsTip");
    public static string AppearanceHead => G("AppearanceHead");
    public static string LanguageLabel => G("LanguageLabel");
    public static string ThemeLabel => G("ThemeLabel");
    public static string LangAuto => G("LangAuto");
    public static string LangEnglish => G("LangEnglish");
    public static string LangTurkish => G("LangTurkish");
    public static string ThemeSystem => G("ThemeSystem");
    public static string ThemeLight => G("ThemeLight");
    public static string ThemeDark => G("ThemeDark");
    public static string Save => G("Save");
    public static string Cancel => G("Cancel");
    public static string ValidationPickModifier => G("ValidationPickModifier");
    public static string GeometrySealDemoted => G("GeometrySealDemoted");

    // ---- tray ----
    public static string MenuPause => G("MenuPause");
    public static string MenuSettings => G("MenuSettings");
    public static string MenuExit => G("MenuExit");
    public static string GatePausedMenu => G("GatePausedMenu");
    public static string TipOpen => G("TipOpen");
    public static string TipClosed => G("TipClosed");
    public static string SideGatePausedMenu => G("SideGatePausedMenu");
    public static string TipSideOn => G("TipSideOn");
    public static string TipSideOff => G("TipSideOff");

    // ---- tabs + crossing rules ----
    public static string TabGeneral => G("TabGeneral");
    public static string LayoutChangedBanner => G("LayoutChangedBanner");
    public static string ExcludePruned => G("ExcludePruned");
    public static string ExportLabel => G("ExportLabel");
    public static string ImportLabel => G("ImportLabel");
    public static string ExportTitle => G("ExportTitle");
    public static string ImportTitle => G("ImportTitle");
    public static string ImportConfirm => G("ImportConfirm");
    public static string ImportInvalid => G("ImportInvalid");
    public static string SettingsFileFilter => G("SettingsFileFilter");
    public static string MenuGame => G("MenuGame");
    public static string GameHotkeyHead => G("GameHotkeyHead");
    public static string GameHotkeyHint => G("GameHotkeyHint");
    public static string TipGameOn => G("TipGameOn");
    public static string TipGameOff => G("TipGameOff");
    public static string PauseHotkeyHead => G("PauseHotkeyHead");
    public static string PauseHotkeyHint => G("PauseHotkeyHint");
    public static string TipPaused => G("TipPaused");
    public static string TipResumed => G("TipResumed");
    public static string TipGuardLost => G("TipGuardLost");
    public static string TipGuardRestored => G("TipGuardRestored");
    public static string NoneRegistered => G("NoneRegistered");
    public static string SideHotkeyHead => G("SideHotkeyHead");
    public static string SideHotkeyHint => G("SideHotkeyHint");
    public static string StartSideContainOn => G("StartSideContainOn");
    public static string StartSideContainTip => G("StartSideContainTip");

    // ---- interpolated (keep {0} out of the table) ----
    public static string TipHotkeyFail(string combo) => string.Format(G("TipHotkeyFail"), combo);
    public static string TrayPaused(string combo) => string.Format(G("TrayPaused"), combo);
    public static string TrayGuardLost(string combo) => string.Format(G("TrayGuardLost"), combo);
    public static string TrayOpen(string combo) => string.Format(G("TrayOpen"), combo);
    public static string TrayClosed(string combo) => string.Format(G("TrayClosed"), combo);
    public static string TipIsolated(string screens, string hotkey) => string.Format(G("TipIsolated"), screens, hotkey);
    public static string TipIntelRotation(string keys) => string.Format(G("TipIntelRotation"), keys);
    public static string TipIntelDefaults(string up, string side) => string.Format(G("TipIntelDefaults"), up, side);

}
