namespace MouseFence;

/// <summary>Best-effort diagnostics for handled UI and fatal startup failures.</summary>
internal static class CrashLogger
{
    private const int MaxBytes = 256 * 1024;
    private static readonly object Sync = new();

    internal static void Write(Exception exception, bool fatal, string directory = null)
    {
        try
        {
            lock (Sync)
            {
                directory ??= Settings.Dir;
                Directory.CreateDirectory(directory);
                string path = Path.Combine(directory, "error.log");
                File.AppendAllText(path, $"[{DateTime.UtcNow:O}] {(fatal ? "FATAL" : "HANDLED")} MouseFence\n{exception}\n---\n");
                if (new FileInfo(path).Length > MaxBytes)
                {
                    byte[] bytes = File.ReadAllBytes(path);
                    int start = bytes.Length - MaxBytes;
                    while (start < bytes.Length && (bytes[start] & 0xC0) == 0x80) start++;
                    File.WriteAllBytes(path, bytes[start..]);
                }
            }
        }
        catch { /* Diagnostics must never replace the original failure. */ }
    }
}
