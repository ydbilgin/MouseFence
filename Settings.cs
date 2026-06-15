using System.Text.Json;
using System.Text.Json.Serialization;

namespace MouseFence;

public sealed class Settings
{
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

    // behaviour
    public bool StartGateClosed { get; set; } = true;  // main->top crossing blocked at launch
    public bool AutoStart { get; set; } = false;       // launch with Windows
    public bool DeliberateCross { get; set; } = true;  // true: deliberate push to cross; false: any upward move

    // which monitors to block
    public string Mode { get; set; } = "AutoTop";   // "AutoTop" | "Manual"
    public List<string> ManualMonitors { get; set; } = new();

    // appearance / localization
    public string Language { get; set; } = "auto";   // "auto" | "en" | "tr"
    public string Theme { get; set; } = "System";    // "System" | "Light" | "Dark"

    // per-screen crossing rules: which bottom monitor may cross UP into which top monitor.
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
                return JsonSerializer.Deserialize<Settings>(File.ReadAllText(FilePath)) ?? new Settings();
        }
        catch { /* fall through to defaults */ }
        return new Settings();
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
}

/// <summary>An allowed upward crossing: the cursor on <see cref="FromDevice"/> may go up into <see cref="ToDevice"/>.</summary>
public sealed class UpLink
{
    public string FromDevice { get; set; } = "";
    public string ToDevice { get; set; } = "";
}
