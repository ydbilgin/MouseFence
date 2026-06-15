using System.Globalization;

namespace MouseFence;

/// <summary>
/// Tiny in-memory localization table (no .resx, no satellite assemblies). Add a language by adding
/// one dictionary and a case in <see cref="Use"/>/<see cref="Resolve"/>.
/// </summary>
public static class Strings
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

    // ---- settings dialog ----
    public static string SettingsTitle => G("SettingsTitle");
    public static string Subtitle => G("Subtitle");
    public static string HotkeyHead => G("HotkeyHead");
    public static string HotkeyHint => G("HotkeyHint");
    public static string HotkeyPressKeys => G("HotkeyPressKeys");
    public static string Clear => G("Clear");
    public static string StartupHead => G("StartupHead");
    public static string StartGateClosed => G("StartGateClosed");
    public static string StartWithWindows => G("StartWithWindows");
    public static string AppearanceHead => G("AppearanceHead");
    public static string LanguageLabel => G("LanguageLabel");
    public static string ThemeLabel => G("ThemeLabel");
    public static string LangAuto => G("LangAuto");
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

    // ---- tabs + crossing rules ----
    public static string TabGeneral => G("TabGeneral");
    public static string TabAppearance => G("TabAppearance");
    public static string TabMonitors => G("TabMonitors");
    public static string RulesHead => G("RulesHead");
    public static string FromScreenLabel => G("FromScreenLabel");
    public static string AllowedTopsLabel => G("AllowedTopsLabel");
    public static string RuleHint => G("RuleHint");
    public static string NoTopsHint => G("NoTopsHint");
    public static string DeliberateCrossLabel => G("DeliberateCrossLabel");
    public static string MenuGame => G("MenuGame");
    public static string GameHotkeyHead => G("GameHotkeyHead");
    public static string GameHotkeyHint => G("GameHotkeyHint");
    public static string TipGameOn => G("TipGameOn");
    public static string TipGameOff => G("TipGameOff");

    // ---- interpolated (keep {0} out of the table) ----
    public static string TipHotkeyFail(string combo) => string.Format(G("TipHotkeyFail"), combo);
    public static string TrayPaused(string combo) => string.Format(G("TrayPaused"), combo);
    public static string TrayOpen(string combo) => string.Format(G("TrayOpen"), combo);
    public static string TrayClosed(string combo) => string.Format(G("TrayClosed"), combo);

    private static readonly Dictionary<string, string> En = new()
    {
        ["SettingsTitle"] = "MouseFence — Settings",
        ["Subtitle"] = "Cursor barrier settings",
        ["HotkeyHead"] = "Toggle hotkey",
        ["HotkeyHint"] = "Click the box, then press your key combo.",
        ["HotkeyPressKeys"] = "(press keys)",
        ["Clear"] = "Clear",
        ["StartupHead"] = "Startup",
        ["StartGateClosed"] = "Start with main → top crossing closed",
        ["StartWithWindows"] = "Start with Windows",
        ["AppearanceHead"] = "Appearance",
        ["LanguageLabel"] = "Language",
        ["ThemeLabel"] = "Theme",
        ["LangAuto"] = "Automatic",
        ["ThemeSystem"] = "Follow system",
        ["ThemeLight"] = "Light",
        ["ThemeDark"] = "Dark",
        ["MonitorsHead"] = "Monitors to block",
        ["ModeAuto"] = "Auto — block the monitor(s) above the primary",
        ["ModeManual"] = "Manual — pick below",
        ["Note"] = "The hotkey only toggles crossing from the MAIN screen.\nCrossing up from side screens is always blocked.",
        ["Save"] = "Save",
        ["Cancel"] = "Cancel",
        ["MainTag"] = "[MAIN]",
        ["ValidationPickModifier"] = "Please pick at least one modifier (Ctrl / Alt / Shift / Win) and a key.",
        ["MenuPause"] = "Pause",
        ["MenuResume"] = "Resume",
        ["MenuSettings"] = "Settings...",
        ["MenuExit"] = "Exit",
        ["GateOpenMenu"] = "Crossing up: OPEN — click to close",
        ["GateClosedMenu"] = "Crossing up: CLOSED — click to open",
        ["GatePausedMenu"] = "Crossing up (paused)",
        ["TipOpen"] = "Crossing up is OPEN 🔓  (only the allowed screens)",
        ["TipClosed"] = "Crossing up is CLOSED 🔒",
        ["TabGeneral"] = "General",
        ["TabAppearance"] = "Appearance",
        ["TabMonitors"] = "Monitors",
        ["RulesHead"] = "Crossing rules",
        ["FromScreenLabel"] = "From screen",
        ["AllowedTopsLabel"] = "May cross up into",
        ["RuleHint"] = "Pick a bottom screen, then check which top screens it may cross up into. (Empty = the primary may cross up into every top.)",
        ["NoTopsHint"] = "No top monitors detected — nothing to configure.",
        ["DeliberateCrossLabel"] = "Require a deliberate push to cross up (off = any upward move crosses)",
        ["MenuGame"] = "Game mode (confine cursor)",
        ["GameHotkeyHead"] = "Game mode hotkey",
        ["GameHotkeyHint"] = "Confine the cursor to its monitor (windowed/borderless games; not exclusive-fullscreen).",
        ["TipGameOn"] = "Game mode ON 🎮  cursor confined to its monitor",
        ["TipGameOff"] = "Game mode OFF",
        ["TipHotkeyFail"] = "Couldn't register hotkey: {0} — another app may be using it.",
        ["TrayPaused"] = "MouseFence: PAUSED ({0})",
        ["TrayOpen"] = "MouseFence: main→top OPEN ({0})",
        ["TrayClosed"] = "MouseFence: main→top CLOSED ({0})",
    };

    private static readonly Dictionary<string, string> Tr = new()
    {
        ["SettingsTitle"] = "MouseFence — Ayarlar",
        ["Subtitle"] = "İmleç bariyeri ayarları",
        ["HotkeyHead"] = "Geçiş kısayolu",
        ["HotkeyHint"] = "Kutuya tıklayın, sonra tuş kombinasyonuna basın.",
        ["HotkeyPressKeys"] = "(tuşlara basın)",
        ["Clear"] = "Temizle",
        ["StartupHead"] = "Başlangıç",
        ["StartGateClosed"] = "Açılışta ana → üst geçiş kapalı başlasın",
        ["StartWithWindows"] = "Windows ile birlikte başlat",
        ["AppearanceHead"] = "Görünüm",
        ["LanguageLabel"] = "Dil",
        ["ThemeLabel"] = "Tema",
        ["LangAuto"] = "Otomatik",
        ["ThemeSystem"] = "Sisteme uy",
        ["ThemeLight"] = "Açık",
        ["ThemeDark"] = "Koyu",
        ["MonitorsHead"] = "Engellenecek ekranlar",
        ["ModeAuto"] = "Otomatik — ana ekranın üstündeki ekranları engelle",
        ["ModeManual"] = "Elle seç",
        ["Note"] = "Kısayol yalnızca ANA ekrandan yukarı geçişi açıp kapatır.\nYan ekranlardan yukarı geçiş her zaman engellidir.",
        ["Save"] = "Kaydet",
        ["Cancel"] = "İptal",
        ["MainTag"] = "[ANA]",
        ["ValidationPickModifier"] = "Lütfen en az bir değiştirici (Ctrl / Alt / Shift / Win) ve bir tuş seçin.",
        ["MenuPause"] = "Duraklat",
        ["MenuResume"] = "Devam et",
        ["MenuSettings"] = "Ayarlar...",
        ["MenuExit"] = "Çıkış",
        ["GateOpenMenu"] = "Yukarı geçiş: AÇIK — kapatmak için tıkla",
        ["GateClosedMenu"] = "Yukarı geçiş: KAPALI — açmak için tıkla",
        ["GatePausedMenu"] = "Yukarı geçiş (duraklatıldı)",
        ["TipOpen"] = "Yukarı geçiş AÇIK 🔓  (yalnızca izinli ekranlar)",
        ["TipClosed"] = "Yukarı geçiş KAPALI 🔒",
        ["TabGeneral"] = "Genel",
        ["TabAppearance"] = "Görünüm",
        ["TabMonitors"] = "Ekranlar",
        ["RulesHead"] = "Geçiş kuralları",
        ["FromScreenLabel"] = "Şu ekrandan",
        ["AllowedTopsLabel"] = "Çıkabileceği üst ekranlar",
        ["RuleHint"] = "Bir alt ekran seç, sonra çıkabileceği üst ekranları işaretle. (Boş = ana ekran her üst ekrana çıkabilir.)",
        ["NoTopsHint"] = "Üst ekran algılanmadı — yapılandırılacak bir şey yok.",
        ["MenuGame"] = "Oyun modu (imleci hapset)",
        ["GameHotkeyHead"] = "Oyun modu kısayolu",
        ["GameHotkeyHint"] = "İmleci bulunduğu ekrana hapset (windowed/borderless oyunlar; exclusive-fullscreen hariç).",
        ["TipGameOn"] = "Oyun modu AÇIK 🎮  imleç ekrana hapsedildi",
        ["TipGameOff"] = "Oyun modu KAPALI",
        ["TipHotkeyFail"] = "Kısayol kaydedilemedi: {0} — başka bir uygulama kullanıyor olabilir.",
        ["TrayPaused"] = "MouseFence: DURAKLATILDI ({0})",
        ["TrayOpen"] = "MouseFence: ana→üst AÇIK ({0})",
        ["TrayClosed"] = "MouseFence: ana→üst KAPALI ({0})",
    };
}
