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
    public static string Subtitle => G("Subtitle");
    public static string HotkeyHead => G("HotkeyHead");
    public static string HotkeyHint => G("HotkeyHint");
    public static string GeneralIntro => G("GeneralIntro");
    public static string HotkeyPressKeys => G("HotkeyPressKeys");
    public static string Clear => G("Clear");
    public static string StartupHead => G("StartupHead");
    public static string StartGateClosed => G("StartGateClosed");
    public static string StartGateClosedTip => G("StartGateClosedTip");
    public static string StartWithWindows => G("StartWithWindows");
    public static string StartWithWindowsTip => G("StartWithWindowsTip");
    public static string AppearanceHead => G("AppearanceHead");
    public static string AppearanceIntro => G("AppearanceIntro");
    public static string LanguageLabel => G("LanguageLabel");
    public static string LanguageTip => G("LanguageTip");
    public static string ThemeLabel => G("ThemeLabel");
    public static string ThemeTip => G("ThemeTip");
    public static string LangAuto => G("LangAuto");
    public static string LangEnglish => G("LangEnglish");
    public static string LangTurkish => G("LangTurkish");
    public static string ThemeSystem => G("ThemeSystem");
    public static string ThemeLight => G("ThemeLight");
    public static string ThemeDark => G("ThemeDark");
    public static string MonitorsHead => G("MonitorsHead");
    public static string ModeAuto => G("ModeAuto");
    public static string ModeManual => G("ModeManual");
    public static string Note => G("Note");
    public static string Save => G("Save");
    public static string Cancel => G("Cancel");
    public static string MainTag => G("MainTag");
    public static string ValidationPickModifier => G("ValidationPickModifier");
    public static string NoSealValidation => G("NoSealValidation");
    public static string GeometrySealDemoted => G("GeometrySealDemoted");

    // ---- tray ----
    public static string MenuPause => G("MenuPause");
    public static string MenuResume => G("MenuResume");
    public static string MenuSettings => G("MenuSettings");
    public static string MenuExit => G("MenuExit");
    public static string GateOpenMenu => G("GateOpenMenu");
    public static string GateClosedMenu => G("GateClosedMenu");
    public static string GatePausedMenu => G("GatePausedMenu");
    public static string TipOpen => G("TipOpen");
    public static string TipClosed => G("TipClosed");
    public static string SideContainOnMenu => G("SideContainOnMenu");
    public static string SideContainOffMenu => G("SideContainOffMenu");
    public static string SideGatePausedMenu => G("SideGatePausedMenu");
    public static string TipSideOn => G("TipSideOn");
    public static string TipSideOff => G("TipSideOff");

    // ---- tabs + crossing rules ----
    public static string TabGeneral => G("TabGeneral");
    public static string TabAppearance => G("TabAppearance");
    public static string TabMonitors => G("TabMonitors");
    public static string DisplaysTab => G("DisplaysTab");
    public static string DisplayMapHead => G("DisplayMapHead");
    public static string IdentifyAll => G("IdentifyAll");
    public static string MapHint => G("MapHint");
    public static string LegendMain => G("LegendMain");
    public static string LegendKeptOut => G("LegendKeptOut");
    public static string MonitorNameHead => G("MonitorNameHead");
    public static string MonitorNameHint => G("MonitorNameHint");
    public static string MonitorNamePlaceholder => G("MonitorNamePlaceholder");
    public static string KeepOutHead => G("KeepOutHead");
    public static string MainAlwaysAvailable => G("MainAlwaysAvailable");
    public static string AdvancedHead => G("AdvancedHead");
    public static string AdvancedHint => G("AdvancedHint");
    public static string TopSelectionHead => G("TopSelectionHead");
    public static string TopSelectionHint => G("TopSelectionHint");
    public static string TopModeAuto => G("TopModeAuto");
    public static string TopModeManual => G("TopModeManual");
    public static string NotAbovePrimary => G("NotAbovePrimary");
    public static string CrossingPathsHead => G("CrossingPathsHead");
    public static string CrossingPathsHint => G("CrossingPathsHint");
    public static string CrossingDefault => G("CrossingDefault");
    public static string CrossingCustom => G("CrossingCustom");
    public static string BehaviorHead => G("BehaviorHead");
    public static string ResetCrossing => G("ResetCrossing");
    public static string LayoutChangedBanner => G("LayoutChangedBanner");
    public static string RulesHead => G("RulesHead");
    public static string FromScreenLabel => G("FromScreenLabel");
    public static string AllowedTopsLabel => G("AllowedTopsLabel");
    public static string RuleHint => G("RuleHint");
    public static string NoTopsHint => G("NoTopsHint");
    public static string DeliberateCrossLabel => G("DeliberateCrossLabel");
    public static string DeliberateCrossHint => G("DeliberateCrossHint");
    public static string DescentRoutingLabel => G("DescentRoutingLabel");
    public static string DescentRoutingHint => G("DescentRoutingHint");
    public static string ExcludeHead => G("ExcludeHead");
    public static string ExcludeHint => G("ExcludeHint");
    public static string ExcludePruned => G("ExcludePruned");
    public static string ResetLayoutLabel => G("ResetLayoutLabel");
    public static string IdentifyLabel => G("IdentifyLabel");
    public static string IdentifyHint => G("IdentifyHint");
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
    public static string SideTuningHead => G("SideTuningHead");
    public static string SideTuningHint => G("SideTuningHint");
    public static string SideSensitivityLabel => G("SideSensitivityLabel");
    public static string SideSensitivityHint => G("SideSensitivityHint");
    public static string DefaultLabel => G("DefaultLabel");
    public static string SideSlackLabel => G("SideSlackLabel");
    public static string SideSlackHint => G("SideSlackHint");
    public static string DefaultSlackLabel => G("DefaultSlackLabel");

    // ---- interpolated (keep {0} out of the table) ----
    public static string TipHotkeyFail(string combo) => string.Format(G("TipHotkeyFail"), combo);
    public static string TrayPaused(string combo) => string.Format(G("TrayPaused"), combo);
    public static string TrayGuardLost(string combo) => string.Format(G("TrayGuardLost"), combo);
    public static string TrayOpen(string combo) => string.Format(G("TrayOpen"), combo);
    public static string TrayClosed(string combo) => string.Format(G("TrayClosed"), combo);
    public static string TipIsolated(string screens, string hotkey) => string.Format(G("TipIsolated"), screens, hotkey);
    public static string IsolatedNote(string screens) => string.Format(G("IsolatedNote"), screens);
    public static string TipIntelRotation(string keys) => string.Format(G("TipIntelRotation"), keys);
    public static string TipIntelDefaults(string up, string side) => string.Format(G("TipIntelDefaults"), up, side);

}
