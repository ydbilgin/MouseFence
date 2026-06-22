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

    // behaviour
    public bool StartGateClosed { get; set; } = true;  // main->top crossing blocked at launch
    public bool AutoStart { get; set; } = false;       // launch with Windows
    public bool DeliberateCross { get; set; } = true;  // true: deliberate push to cross; false: any upward move
    public bool DescentRouting { get; set; } = false;  // opt-in: clamp a descent back onto the linked bottom screen

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
        return new Settings();
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
}

/// <summary>An allowed upward crossing: the cursor on <see cref="FromDevice"/> may go up into <see cref="ToDevice"/>.</summary>
public sealed class UpLink
{
    public string FromDevice { get; set; } = "";
    public string ToDevice { get; set; } = "";
}
