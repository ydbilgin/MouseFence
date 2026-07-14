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
    public static string TrayOpen(string combo) => string.Format(G("TrayOpen"), combo);
    public static string TrayClosed(string combo) => string.Format(G("TrayClosed"), combo);
    public static string TipIsolated(string screens, string hotkey) => string.Format(G("TipIsolated"), screens, hotkey);
    public static string IsolatedNote(string screens) => string.Format(G("IsolatedNote"), screens);
    public static string TipIntelRotation(string keys) => string.Format(G("TipIntelRotation"), keys);
    public static string TipIntelDefaults(string up, string side) => string.Format(G("TipIntelDefaults"), up, side);

    private static readonly Dictionary<string, string> En = new()
    {
        ["SettingsTitle"] = "MouseFence — Settings",
        ["Subtitle"] = "Cursor barrier settings",
        ["HotkeyHead"] = "Toggle hotkey",
        ["HotkeyHint"] = "Click the box, then press your key combo.",
        ["GeneralIntro"] = "Choose the keyboard shortcuts and how MouseFence starts.",
        ["HotkeyPressKeys"] = "(press keys)",
        ["Clear"] = "Clear",
        ["StartupHead"] = "Startup",
        ["StartGateClosed"] = "Start with main → top crossing closed",
        ["StartGateClosedTip"] = "When MouseFence starts, upward crossing stays blocked until you open it with the toggle hotkey.",
        ["StartWithWindows"] = "Start with Windows",
        ["StartWithWindowsTip"] = "Launch MouseFence automatically after you sign in to Windows.",
        ["AppearanceHead"] = "Appearance",
        ["AppearanceIntro"] = "Choose the language and colors used by the settings window and tray menu.",
        ["LanguageLabel"] = "Language",
        ["LanguageTip"] = "Automatic follows the Windows display language; English and Turkish override it.",
        ["ThemeLabel"] = "Theme",
        ["ThemeTip"] = "Follow the Windows app theme, or always use the light or dark MouseFence theme.",
        ["LangAuto"] = "Automatic",
        ["LangEnglish"] = "English",
        ["LangTurkish"] = "Turkish",
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
        ["NoSealValidation"] = "{0} would have no way out. Allow at least one boundary before saving.",
        ["GeometrySealDemoted"] = "The display layout removed every exit from a screen. MouseFence temporarily changed its widest blocked boundary to Push.",
        ["MenuPause"] = "Pause",
        ["MenuResume"] = "Resume",
        ["MenuSettings"] = "Settings...",
        ["MenuExit"] = "Exit",
        ["GateOpenMenu"] = "Crossing up: OPEN — click to close",
        ["GateClosedMenu"] = "Crossing up: CLOSED — click to open",
        ["GatePausedMenu"] = "Crossing up (paused)",
        ["TipOpen"] = "Crossing up is OPEN 🔓  (only the allowed screens)",
        ["TipClosed"] = "Crossing up is CLOSED 🔒",
        ["SideContainOnMenu"] = "Side containment: ON — click to disable",
        ["SideContainOffMenu"] = "Side containment: OFF — click to enable",
        ["SideGatePausedMenu"] = "Side containment (paused)",
        ["TipSideOn"] = "Side containment ON 🛡️  (blocks drift onto a side screen — push to cross)",
        ["TipSideOff"] = "Side containment OFF 🔓  (cursor moves to side screens freely)",
        ["TabGeneral"] = "General",
        ["TabAppearance"] = "Appearance",
        ["TabMonitors"] = "Monitors",
        ["DisplaysTab"] = "Displays",
        ["DisplayMapHead"] = "Your display layout",
        ["IdentifyAll"] = "Identify all",
        ["MapHint"] = "Click to flash a screen. MouseFence numbers may differ from Windows.",
        ["LegendMain"] = "★ Main",
        ["LegendKeptOut"] = "▧ Kept out",
        ["MonitorNameHead"] = "Name the selected display",
        ["MonitorNameHint"] = "The name stays with this physical display even if Windows renumbers it. Clear the field to remove it.",
        ["MonitorNamePlaceholder"] = "Name this display…",
        ["KeepOutHead"] = "Keep the cursor OUT of these screens:",
        ["MainAlwaysAvailable"] = "Main — always available",
        ["AdvancedHead"] = "Advanced",
        ["AdvancedHint"] = "Rarely changed layout, crossing-path, and side-barrier controls.",
        ["TopSelectionHead"] = "Which screens count as top (blocked)",
        ["TopSelectionHint"] = "Automatic is best for most layouts; use Manual only when an upper display needs special treatment.",
        ["TopModeAuto"] = "Automatically use displays above Main",
        ["TopModeManual"] = "Choose upper displays manually",
        ["NotAbovePrimary"] = "Not above Main",
        ["CrossingPathsHead"] = "Crossing paths",
        ["CrossingPathsHint"] = "Choose which lower screens may move upward into which upper screens.",
        ["CrossingDefault"] = "Default paths — Main can move into every upper display",
        ["CrossingCustom"] = "Custom paths",
        ["BehaviorHead"] = "Behavior",
        ["ResetCrossing"] = "Reset crossing settings",
        ["LayoutChangedBanner"] = "Display layout changed — this view was updated.",
        ["RulesHead"] = "Crossing rules",
        ["FromScreenLabel"] = "From screen",
        ["AllowedTopsLabel"] = "May cross up into",
        ["RuleHint"] = "Pick a bottom screen, then check which top screens it may cross up into. (Empty = the primary may cross up into every top.)",
        ["NoTopsHint"] = "No top monitors detected — nothing to configure.",
        ["DeliberateCrossLabel"] = "Require a deliberate push to cross up / sideways (off = any move across a barrier crosses)",
        ["DeliberateCrossHint"] = "On requires a clear push through a soft barrier; off lets any movement across an open path pass.",
        ["DescentRoutingLabel"] = "Stay in the same column when descending",
        ["DescentRoutingHint"] = "When a linked upper screen overhangs another display, guide downward movement back to its linked lower screen.",
        ["ExcludeHead"] = "Exclude displays (wall cursor out — for dummy/headless)",
        ["ExcludeHint"] = "Walls the cursor out of an excluded display so it can't get lost there. The main display can't be excluded. (Display ids are machine-specific.)",
        ["ExcludePruned"] = "Couldn't exclude a display — the main display and the last usable display are always kept.",
        ["ResetLayoutLabel"] = "Reset layout",
        ["IdentifyLabel"] = "Identify",
        ["IdentifyHint"] = "Flash each monitor's MouseFence number and name on screen (won't show over an exclusive-fullscreen game).",
        ["ExportLabel"] = "Export…",
        ["ImportLabel"] = "Import…",
        ["ExportTitle"] = "Export MouseFence settings",
        ["ImportTitle"] = "Import MouseFence settings",
        ["ImportConfirm"] = "Replace your current settings with the imported file? The new settings are applied right away.",
        ["ImportInvalid"] = "That file isn't a valid MouseFence settings file.",
        ["SettingsFileFilter"] = "MouseFence settings (*.json)|*.json|All files (*.*)|*.*",
        ["MenuGame"] = "Game mode (confine cursor)",
        ["GameHotkeyHead"] = "Game mode hotkey",
        ["GameHotkeyHint"] = "Confine the cursor to its monitor (windowed/borderless games; not exclusive-fullscreen).",
        ["TipGameOn"] = "Game mode ON 🎮  cursor confined to its monitor",
        ["TipGameOff"] = "Game mode OFF",
        ["PauseHotkeyHead"] = "Pause hotkey",
        ["PauseHotkeyHint"] = "Pause/resume the barrier — a keyboard escape if the cursor ever gets trapped.",
        ["TipPaused"] = "MouseFence: PAUSED — barrier off",
        ["TipResumed"] = "MouseFence: resumed — barrier on",
        ["NoneRegistered"] = "(not registered)",
        ["SideHotkeyHead"] = "Side containment hotkey",
        ["SideHotkeyHint"] = "Toggle side containment on/off — when on, a soft barrier blocks accidental drift off the MAIN screen left/right, but a deliberate push still crosses (the sideways mirror of the up barrier). Note: Ctrl+Alt+Arrow may rotate the screen on some Intel GPUs — pick another combo if so.",
        ["StartSideContainOn"] = "Start with side containment on (blocks accidental drift; push to cross)",
        ["StartSideContainTip"] = "Engage the soft left/right barrier at launch; a deliberate sideways push can still cross it.",
        ["SideTuningHead"] = "Side barrier tuning",
        ["SideTuningHint"] = "Adjust only if sideways crossing feels too easy, too hard, or rejects diagonal pushes.",
        ["SideSensitivityLabel"] = "Push strength (higher = firmer)",
        ["SideSensitivityHint"] = "How hard you must flick sideways to break through the side barrier (pixels per move). Higher = a firmer barrier that catches more accidental drift; lower = easier to cross. Applies both ways (onto and off a side screen). Default is 3.",
        ["DefaultLabel"] = "Default (3)",
        ["SideSlackLabel"] = "Diagonal allowance (higher = more diagonal)",
        ["SideSlackHint"] = "Extra vertical movement allowed during a sideways push. Higher accepts more diagonal motion; lower requires a straighter push. Default is 5.",
        ["DefaultSlackLabel"] = "Default (5)",
        ["TipIsolated"] = "Screen {0} has no side/down exit — only UP. MouseFence auto-opened an up-escape there; align its edges, or press {1} to pause.",
        ["IsolatedNote"] = "⚠ Screen {0} is isolated (a gap in the layout). Up-crossing from it is auto-allowed so it can't trap the cursor.",
        ["TipHotkeyFail"] = "Couldn't register hotkey: {0} — another app may be using it.",
        ["TrayPaused"] = "MouseFence: PAUSED ({0})",
        ["TrayOpen"] = "MouseFence: main→top OPEN ({0})",
        ["TrayClosed"] = "MouseFence: main→top CLOSED ({0})",
        ["TipIntelRotation"] = "Intel graphics detected: the {0} hotkey can rotate the screen (Ctrl+Alt+Arrow). Open Settings to add Shift or change it.",
        ["TipIntelDefaults"] = "Intel graphics detected — added Shift to the arrow hotkeys to dodge screen rotation (toggle up: {0}, side: {1}).",
    };

    private static readonly Dictionary<string, string> Tr = new()
    {
        ["SettingsTitle"] = "MouseFence — Ayarlar",
        ["Subtitle"] = "İmleç bariyeri ayarları",
        ["HotkeyHead"] = "Geçiş kısayolu",
        ["HotkeyHint"] = "Kutuya tıklayın, sonra tuş kombinasyonuna basın.",
        ["GeneralIntro"] = "Klavye kısayollarını ve MouseFence'in nasıl başlayacağını seçin.",
        ["HotkeyPressKeys"] = "(tuşlara basın)",
        ["Clear"] = "Temizle",
        ["StartupHead"] = "Başlangıç",
        ["StartGateClosed"] = "Açılışta ana → üst geçiş kapalı başlasın",
        ["StartGateClosedTip"] = "MouseFence başladığında, geçiş kısayoluyla açana kadar yukarı geçiş engelli kalır.",
        ["StartWithWindows"] = "Windows ile birlikte başlat",
        ["StartWithWindowsTip"] = "Windows oturumunu açtığınızda MouseFence'i otomatik başlatır.",
        ["AppearanceHead"] = "Görünüm",
        ["AppearanceIntro"] = "Ayarlar penceresi ve tepsi menüsünde kullanılacak dili ve renkleri seçin.",
        ["LanguageLabel"] = "Dil",
        ["LanguageTip"] = "Otomatik seçeneği Windows görüntüleme dilini izler; İngilizce ve Türkçe bunu geçersiz kılar.",
        ["ThemeLabel"] = "Tema",
        ["ThemeTip"] = "Windows uygulama temasını izleyin veya MouseFence'i her zaman açık ya da koyu kullanın.",
        ["LangAuto"] = "Otomatik",
        ["LangEnglish"] = "İngilizce",
        ["LangTurkish"] = "Türkçe",
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
        ["NoSealValidation"] = "{0} ekranından çıkış yolu kalmaz. Kaydetmeden önce en az bir sınıra izin verin.",
        ["GeometrySealDemoted"] = "Ekran düzeni bir ekranın tüm çıkışlarını kaldırdı. MouseFence en geniş engellenmiş sınırı geçici olarak İt olarak değiştirdi.",
        ["MenuPause"] = "Duraklat",
        ["MenuResume"] = "Devam et",
        ["MenuSettings"] = "Ayarlar...",
        ["MenuExit"] = "Çıkış",
        ["GateOpenMenu"] = "Yukarı geçiş: AÇIK — kapatmak için tıkla",
        ["GateClosedMenu"] = "Yukarı geçiş: KAPALI — açmak için tıkla",
        ["GatePausedMenu"] = "Yukarı geçiş (duraklatıldı)",
        ["TipOpen"] = "Yukarı geçiş AÇIK 🔓  (yalnızca izinli ekranlar)",
        ["TipClosed"] = "Yukarı geçiş KAPALI 🔒",
        ["SideContainOnMenu"] = "Yan kilit: AÇIK — kapatmak için tıkla",
        ["SideContainOffMenu"] = "Yan kilit: KAPALI — açmak için tıkla",
        ["SideGatePausedMenu"] = "Yan kilit (duraklatıldı)",
        ["TipSideOn"] = "Yan kilit AÇIK 🛡️  (yan ekrana kazara kaymayı engeller — geçmek için itin)",
        ["TipSideOff"] = "Yan kilit KAPALI 🔓  (imleç yan ekranlara serbestçe geçer)",
        ["TabGeneral"] = "Genel",
        ["TabAppearance"] = "Görünüm",
        ["TabMonitors"] = "Ekranlar",
        ["DisplaysTab"] = "Ekranlar",
        ["DisplayMapHead"] = "Ekran yerleşiminiz",
        ["IdentifyAll"] = "Tümünü tanımla",
        ["MapHint"] = "Flaş için ekrana tıklayın. Numaralar Windows'tan farklı olabilir.",
        ["LegendMain"] = "★ Ana",
        ["LegendKeptOut"] = "▧ Dışarıda tutulur",
        ["MonitorNameHead"] = "Seçili ekrana ad ver",
        ["MonitorNameHint"] = "Windows ekranı yeniden numaralandırsa bile ad bu fiziksel ekranla kalır. Silmek için alanı boşaltın.",
        ["MonitorNamePlaceholder"] = "Bu ekrana ad ver…",
        ["KeepOutHead"] = "İmleci şu ekranların DIŞINDA tut:",
        ["MainAlwaysAvailable"] = "Ana — her zaman kullanılabilir",
        ["AdvancedHead"] = "Gelişmiş",
        ["AdvancedHint"] = "Nadiren değiştirilen düzen, geçiş yolu ve yan bariyer denetimleri.",
        ["TopSelectionHead"] = "Hangi ekranlar üst (engellenen) sayılır",
        ["TopSelectionHint"] = "Çoğu düzen için Otomatik en iyisidir; yalnızca bir üst ekran özel işlem gerektiriyorsa Elle seçin.",
        ["TopModeAuto"] = "Ana ekranın üstündeki ekranları otomatik kullan",
        ["TopModeManual"] = "Üst ekranları elle seç",
        ["NotAbovePrimary"] = "Ana ekranın üstünde değil",
        ["CrossingPathsHead"] = "Geçiş yolları",
        ["CrossingPathsHint"] = "Hangi alt ekranlardan hangi üst ekranlara yukarı geçilebileceğini seçin.",
        ["CrossingDefault"] = "Varsayılan yollar — Ana, her üst ekrana geçebilir",
        ["CrossingCustom"] = "Özel yollar",
        ["BehaviorHead"] = "Davranış",
        ["ResetCrossing"] = "Geçiş ayarlarını sıfırla",
        ["LayoutChangedBanner"] = "Ekran yerleşimi değişti — bu görünüm güncellendi.",
        ["RulesHead"] = "Geçiş kuralları",
        ["FromScreenLabel"] = "Şu ekrandan",
        ["AllowedTopsLabel"] = "Çıkabileceği üst ekranlar",
        ["RuleHint"] = "Bir alt ekran seç, sonra çıkabileceği üst ekranları işaretle. (Boş = ana ekran her üst ekrana çıkabilir.)",
        ["NoTopsHint"] = "Üst ekran algılanmadı — yapılandırılacak bir şey yok.",
        ["DeliberateCrossLabel"] = "Yukarı / yana geçmek için bilinçli itiş gerekir (kapalı = bariyere doğru her hareket geçer)",
        ["DeliberateCrossHint"] = "Açıkken yumuşak bariyeri aşmak için belirgin bir itiş gerekir; kapalıyken açık yoldan geçen her hareket kabul edilir.",
        ["DescentRoutingLabel"] = "İnişte imleci aynı sütunda tut",
        ["DescentRoutingHint"] = "Bağlı bir üst ekran başka bir ekranın üzerine taşıyorsa, aşağı hareketi bağlı alt ekrana geri yönlendirir.",
        ["ExcludeHead"] = "Ekranları hariç tut (imleci dışarıda tut — dummy/başsız için)",
        ["ExcludeHint"] = "İmleci hariç tutulan ekranın dışında tutar; imleç orada kaybolmaz. Ana ekran hariç tutulamaz. (Ekran kimlikleri makineye özeldir.)",
        ["ExcludePruned"] = "Bir ekran hariç tutulamadı — ana ekran ve son kullanılabilir ekran her zaman korunur.",
        ["ResetLayoutLabel"] = "Düzeni sıfırla",
        ["IdentifyLabel"] = "Tanımla",
        ["IdentifyHint"] = "Her monitörün MouseFence numarasını ve adını ekranda flaşlatır (tam ekran oyunun üzerinde görünmez).",
        ["ExportLabel"] = "Dışa aktar…",
        ["ImportLabel"] = "İçe aktar…",
        ["ExportTitle"] = "MouseFence ayarlarını dışa aktar",
        ["ImportTitle"] = "MouseFence ayarlarını içe aktar",
        ["ImportConfirm"] = "Mevcut ayarların içe aktarılan dosyayla değiştirilsin mi? Yeni ayarlar hemen uygulanır.",
        ["ImportInvalid"] = "Bu dosya geçerli bir MouseFence ayar dosyası değil.",
        ["SettingsFileFilter"] = "MouseFence ayarları (*.json)|*.json|Tüm dosyalar (*.*)|*.*",
        ["MenuGame"] = "Oyun modu (imleci hapset)",
        ["GameHotkeyHead"] = "Oyun modu kısayolu",
        ["GameHotkeyHint"] = "İmleci bulunduğu ekrana hapset (windowed/borderless oyunlar; exclusive-fullscreen hariç).",
        ["TipGameOn"] = "Oyun modu AÇIK 🎮  imleç ekrana hapsedildi",
        ["TipGameOff"] = "Oyun modu KAPALI",
        ["PauseHotkeyHead"] = "Duraklatma kısayolu",
        ["PauseHotkeyHint"] = "Bariyeri duraklat/sürdür — imleç kapana kısılırsa klavyeden çıkış.",
        ["TipPaused"] = "MouseFence: DURAKLATILDI — bariyer kapalı",
        ["TipResumed"] = "MouseFence: devam etti — bariyer açık",
        ["NoneRegistered"] = "(kaydedilmedi)",
        ["SideHotkeyHead"] = "Yan kilit kısayolu",
        ["SideHotkeyHint"] = "Yan kilidi aç/kapat — açıkken ANA ekrandan SAĞA/SOLA kazara kaymayı engelleyen yumuşak bir bariyer kurar, ama kasıtlı bir itiş yine de geçer (yukarı bariyerin yana uyarlanmış hâli). Not: Ctrl+Alt+Ok bazı Intel ekran kartlarında ekranı döndürebilir — öyleyse başka kombinasyon seçin.",
        ["StartSideContainOn"] = "Açılışta yan kilit açık başlasın (kazara kaymayı engelle; geçmek için it)",
        ["StartSideContainTip"] = "Açılışta yumuşak sağ/sol bariyeri etkinleştirir; bilinçli bir yana itiş yine geçebilir.",
        ["SideTuningHead"] = "Yan bariyer ayarı",
        ["SideTuningHint"] = "Yana geçiş fazla kolay, fazla zor geliyorsa veya çapraz itişleri reddediyorsa ayarlayın.",
        ["SideSensitivityLabel"] = "İtiş gücü (yüksek = daha sert)",
        ["SideSensitivityHint"] = "Yan bariyeri kırmak için yana ne kadar sert itmen gerektiği (hareket başına piksel). Yüksek = daha sert bariyer, daha çok kazara kaymayı yakalar; düşük = daha kolay geçilir. Her iki yönde de geçerli (yan ekrana çıkış ve yan ekrandan dönüş). Varsayılan 3.",
        ["DefaultLabel"] = "Varsayılan (3)",
        ["SideSlackLabel"] = "Çapraz hareket payı (yüksek = daha çapraz)",
        ["SideSlackHint"] = "Yana itiş sırasında izin verilen ek dikey hareket. Yüksek değer daha çapraz hareketi kabul eder; düşük değer daha düz bir itiş ister. Varsayılan 5.",
        ["DefaultSlackLabel"] = "Varsayılan (5)",
        ["TipIsolated"] = "{0} numaralı ekranın yana/aşağı çıkışı yok — yalnızca YUKARI. MouseFence oradan yukarı çıkışı otomatik açtı; kenarlarını hizala veya duraklatmak için {1} tuşuna bas.",
        ["IsolatedNote"] = "⚠ {0} numaralı ekran yalıtılmış (yerleşimde boşluk). Kapana kısmaması için oradan yukarı geçiş otomatik açıldı.",
        ["TipHotkeyFail"] = "Kısayol kaydedilemedi: {0} — başka bir uygulama kullanıyor olabilir.",
        ["TrayPaused"] = "MouseFence: DURAKLATILDI ({0})",
        ["TrayOpen"] = "MouseFence: ana→üst AÇIK ({0})",
        ["TrayClosed"] = "MouseFence: ana→üst KAPALI ({0})",
        ["TipIntelRotation"] = "Intel ekran kartı algılandı: {0} kısayolu ekranı döndürebilir (Ctrl+Alt+ok). Ayarlar'dan Shift ekleyin veya değiştirin.",
        ["TipIntelDefaults"] = "Intel ekran kartı algılandı — ekran dönmesini önlemek için ok kısayollarına Shift eklendi (üst geçiş: {0}, yan: {1}).",
    };
}
