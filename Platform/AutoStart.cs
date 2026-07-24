using Microsoft.Win32;

namespace MouseFence;

/// <summary>Adds/removes the app from the per-user Windows startup (HKCU ...\Run).</summary>
internal static class AutoStart
{
    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "MouseFence";

    public static void Apply(bool enable)
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(RunKey, writable: true);
            if (key == null) return;

            if (enable)
                key.SetValue(ValueName, $"\"{Application.ExecutablePath}\"");
            else
                key.DeleteValue(ValueName, throwOnMissingValue: false);
        }
        catch { /* non-fatal */ }
    }
}
