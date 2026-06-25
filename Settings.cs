using System.Text.Json;
using System.Text.Json.Serialization;

namespace MouseFence;

public sealed class Settings
{
    public const int CurrentSettingsVersion = 2;

    public int SettingsVersion { get; set; } = CurrentSettingsVersion;

    // hotkey
    public Keys HotKey { get; set; } = Keys.Up;
    public bool ModCtrl { get; set; } = true;
    public bool ModAlt { get; set; } = true;
    public bool ModShift { get; set; } = false;
    public bool ModWin { get; set; } = false;

    // game-mode (confine cursor to its monitor) hotkey
    public Keys ConfineHotKey { get; set; } = Keys.G;
    public bool ConfineModCtrl { get; set; } = true;
    public bool ConfineModAlt { get; set; } = true;
    public bool ConfineModShift { get; set; } = false;
    public bool ConfineModWin { get; set; } = false;

    // global pause/resume hotkey — a universal keyboard escape if the cursor ever gets trapped
    public Keys PauseHotKey { get; set; } = Keys.P;
    public bool PauseModCtrl { get; set; } = true;
    public bool PauseModAlt { get; set; } = true;
    public bool PauseModShift { get; set; } = false;
    public bool PauseModWin { get; set; } = false;

    // side-containment hotkey — toggles blocking the cursor from leaving the MAIN screen LEFT/RIGHT into a
    // side screen (the horizontal mirror of the up-barrier). Default Ctrl+Alt+Right.
    public Keys SideHotKey { get; set; } = Keys.Right;
    public bool SideModCtrl { get; set; } = true;
    public bool SideModAlt { get; set; } = true;
    public bool SideModShift { get; set; } = false;
    public bool SideModWin { get; set; } = false;

    // behaviour
    public bool StartGateClosed { get; set; } = true;  // main->top crossing blocked at launch
    // Default ON (true): side containment is a SOFT barrier — it only blocks accidental drift out of the main screen
    // sideways; a deliberate horizontal push always crosses, so shipping it on costs the user nothing but stops the
    // pixel-creep onto a side monitor. An absent JSON key deserializes to this default; toggle off via the tray menu
    // or the Ctrl+Alt+Right hotkey for fully free side movement.
    public bool StartSideContainOn { get; set; } = true;  // start with the soft side barrier engaged?
    public bool AutoStart { get; set; } = false;       // launch with Windows
    public bool DeliberateCross { get; set; } = true;  // true: deliberate push to cross; false: any upward move
    public bool DescentRouting { get; set; } = false;  // opt-in: clamp a descent back onto the linked bottom screen

    // Side-barrier sensitivity (independent of the up barrier): the min horizontal px in one move that counts as a
    // DELIBERATE side crossing, and the vertical slack allowed beyond it. Higher SideCrossMin = a firmer/stiffer side
    // barrier (a faster horizontal flick is needed to cross, so accidental drift onto/off a side screen is caught more
    // aggressively). Defaults match the up barrier's feel; an absent JSON key deserializes to these.
    public const int DefaultSideCrossMin = 3;   // shared so the UI's "reset to default" button and the seed agree
    public int SideCrossMin { get; set; } = DefaultSideCrossMin;
    public int SideCrossSlack { get; set; } = 5;

    // which monitors to block. Layout-dependent monitor keys are stable MonitorInfo.StableId values.
    public string Mode { get; set; } = "AutoTop";   // "AutoTop" | "Manual"
    public List<string> ManualMonitors { get; set; } = new();

    // Displays to WALL OFF: the cursor can never enter these (headless/dummy screens it would get lost in).
    // Keyed on the STABLE per-monitor id (MonitorInfo.StableId — the EDID-derived instance path) so the exclusion
    // SURVIVES \\.\DISPLAYn renumbering when the layout changes (FIX 4). Still machine-specific, so an imported list
    // from another PC simply lies dormant — a missing id is skipped in Configure(). Empty default => feature inert,
    // backward-compatible (an existing settings.json lacking the key deserializes to empty; no migration needed —
    // no ExcludedDevices has shipped yet).
    public List<string> ExcludedDevices { get; set; } = new();

    // appearance / localization
    public string Language { get; set; } = "auto";   // "auto" | "en" | "tr"
    public string Theme { get; set; } = "System";    // "System" | "Light" | "Dark"

    // per-screen crossing rules: which bottom monitor may cross UP into which top monitor.
    // FromDevice/ToDevice store stable MonitorInfo.StableId values; the property names stay for JSON compatibility.
    // Empty = default behaviour (the primary may cross up into every top monitor).
    public List<UpLink> UpLinks { get; set; } = new();

    // Transient (not persisted): set when a fresh install on an Intel GPU had Shift added to its arrow hotkeys,
    // so the tray can show an info balloon explaining the safe defaults.
    [JsonIgnore]
    public bool IntelDefaultsApplied { get; private set; }

    [JsonIgnore]
    public static string Dir =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "MouseFence");

    [JsonIgnore]
    public static string FilePath => Path.Combine(Dir, "settings.json");

    public static Settings Load()
    {
        try
        {
            if (File.Exists(FilePath))
            {
                var json = File.ReadAllText(FilePath);
                var settings = JsonSerializer.Deserialize<Settings>(json) ?? new Settings();
                if (ReadVersion(json) < CurrentSettingsVersion)
                {
                    try
                    {
                        settings.MigrateLayoutKeysToStable(MonitorInfo.All());
                        settings.SettingsVersion = CurrentSettingsVersion;
                        settings.Save();
                    }
                    catch
                    {
                        // Keep the loaded settings rather than falling back to defaults; migration can retry next load.
                    }
                }
                return settings;
            }
        }
        catch { /* fall through to defaults */ }

        // Fresh install (no settings file yet): on an Intel GPU, the Ctrl+Alt+Arrow defaults clash with the driver's
        // screen-rotation hotkeys, so add Shift to keep the directional arrow but dodge rotation.
        var fresh = new Settings();
        try { if (Native.HasIntelGpu()) fresh.AddShiftToArrowHotkeys(); }
        catch { /* detection is best-effort */ }
        return fresh;
    }

    /// <summary>On an Intel GPU, the Ctrl+Alt+Arrow defaults (up-barrier + side containment) collide with the driver's
    /// screen-rotation hotkeys. Add Shift to any arrow-rotation hotkey so it keeps the arrow but no longer rotates the
    /// screen. Sets <see cref="IntelDefaultsApplied"/> if anything changed (for the tray's info balloon).</summary>
    public void AddShiftToArrowHotkeys()
    {
        bool changed = false;
        if (GuardCore.IsArrowRotationHotkey((int)HotKey, ModCtrl, ModAlt, ModShift, ModWin)) { ModShift = true; changed = true; }
        if (GuardCore.IsArrowRotationHotkey((int)SideHotKey, SideModCtrl, SideModAlt, SideModShift, SideModWin)) { SideModShift = true; changed = true; }
        IntelDefaultsApplied = changed;
    }

    public static int ReadVersion(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            return doc.RootElement.TryGetProperty(nameof(SettingsVersion), out var v) && v.TryGetInt32(out int n) ? n : 0;
        }
        catch
        {
            return 0;
        }
    }

    public void MigrateLayoutKeysToStable(List<MonitorInfo> monitors)
    {
        var map = monitors
            .GroupBy(m => m.Device)
            .ToDictionary(g => g.Key, g => g.First().StableId, StringComparer.OrdinalIgnoreCase);
        var migrated = GuardCore.MigrateLayoutKeysToStable(
            ManualMonitors,
            UpLinks.Select(lk => (lk.FromDevice, lk.ToDevice)),
            map);
        ManualMonitors = migrated.ManualKeys;
        UpLinks = migrated.Links
            .Select(lk => new UpLink { FromDevice = lk.From, ToDevice = lk.To })
            .ToList();
    }

    public void ResetLayoutConfig()
    {
        var reset = GuardCore.ResetLayoutConfig();
        Mode = reset.Mode;
        ManualMonitors = reset.ManualKeys;
        UpLinks = reset.Links.Select(lk => new UpLink { FromDevice = lk.From, ToDevice = lk.To }).ToList();
    }

    public void Save()
    {
        try
        {
            Directory.CreateDirectory(Dir);
            File.WriteAllText(FilePath, JsonSerializer.Serialize(this, new JsonSerializerOptions { WriteIndented = true }));
        }
        catch { /* non-fatal */ }
    }

    public uint Modifiers()
    {
        uint m = 0;
        if (ModCtrl) m |= Native.MOD_CONTROL;
        if (ModAlt) m |= Native.MOD_ALT;
        if (ModShift) m |= Native.MOD_SHIFT;
        if (ModWin) m |= Native.MOD_WIN;
        return m;
    }

    public string HotKeyText()
    {
        var parts = new List<string>();
        if (ModCtrl) parts.Add("Ctrl");
        if (ModAlt) parts.Add("Alt");
        if (ModShift) parts.Add("Shift");
        if (ModWin) parts.Add("Win");
        if (HotKey != Keys.None) parts.Add(HotKey.ToString());
        return parts.Count == 0 ? "(none)" : string.Join("+", parts);
    }

    public uint ConfineModifiers()
    {
        uint m = 0;
        if (ConfineModCtrl) m |= Native.MOD_CONTROL;
        if (ConfineModAlt) m |= Native.MOD_ALT;
        if (ConfineModShift) m |= Native.MOD_SHIFT;
        if (ConfineModWin) m |= Native.MOD_WIN;
        return m;
    }

    public string ConfineHotKeyText()
    {
        var parts = new List<string>();
        if (ConfineModCtrl) parts.Add("Ctrl");
        if (ConfineModAlt) parts.Add("Alt");
        if (ConfineModShift) parts.Add("Shift");
        if (ConfineModWin) parts.Add("Win");
        if (ConfineHotKey != Keys.None) parts.Add(ConfineHotKey.ToString());
        return parts.Count == 0 ? "(none)" : string.Join("+", parts);
    }

    public uint PauseModifiers()
    {
        uint m = 0;
        if (PauseModCtrl) m |= Native.MOD_CONTROL;
        if (PauseModAlt) m |= Native.MOD_ALT;
        if (PauseModShift) m |= Native.MOD_SHIFT;
        if (PauseModWin) m |= Native.MOD_WIN;
        return m;
    }

    public string PauseHotKeyText()
    {
        var parts = new List<string>();
        if (PauseModCtrl) parts.Add("Ctrl");
        if (PauseModAlt) parts.Add("Alt");
        if (PauseModShift) parts.Add("Shift");
        if (PauseModWin) parts.Add("Win");
        if (PauseHotKey != Keys.None) parts.Add(PauseHotKey.ToString());
        return parts.Count == 0 ? "(none)" : string.Join("+", parts);
    }

    public uint SideModifiers()
    {
        uint m = 0;
        if (SideModCtrl) m |= Native.MOD_CONTROL;
        if (SideModAlt) m |= Native.MOD_ALT;
        if (SideModShift) m |= Native.MOD_SHIFT;
        if (SideModWin) m |= Native.MOD_WIN;
        return m;
    }

    public string SideHotKeyText()
    {
        var parts = new List<string>();
        if (SideModCtrl) parts.Add("Ctrl");
        if (SideModAlt) parts.Add("Alt");
        if (SideModShift) parts.Add("Shift");
        if (SideModWin) parts.Add("Win");
        if (SideHotKey != Keys.None) parts.Add(SideHotKey.ToString());
        return parts.Count == 0 ? "(none)" : string.Join("+", parts);
    }
}

/// <summary>An allowed upward crossing: the cursor on <see cref="FromDevice"/> may go up into <see cref="ToDevice"/>.</summary>
public sealed class UpLink
{
    public string FromDevice { get; set; } = "";
    public string ToDevice { get; set; } = "";
}
