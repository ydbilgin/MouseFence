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

    // behaviour
    public bool StartGateClosed { get; set; } = true;  // main->top crossing blocked at launch
    public bool AutoStart { get; set; } = false;       // launch with Windows

    // which monitors to block
    public string Mode { get; set; } = "AutoTop";   // "AutoTop" | "Manual"
    public List<string> ManualMonitors { get; set; } = new();

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
        return parts.Count == 0 ? "(yok)" : string.Join("+", parts);
    }
}
