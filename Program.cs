using System.Threading;
using System.Globalization;

namespace MouseFence;

internal static class Program
{
    private static Mutex _single;

    [STAThread]
    private static void Main(string[] args)
    {
        // Dev/CI helpers: render the settings window off-screen to a PNG. --screenshot uses the live
        // monitor list for local QA; --screenshot-demo uses deterministic synthetic data for public docs.
        // Both run before the single-instance check so they work while the tray app is already running.
        if (args.Length >= 2 && args[0] is "--screenshot" or "--screenshot-demo")
        {
            ApplicationConfiguration.Initialize();
            int tab = args.Length >= 4 && int.TryParse(args[3], out var ti) ? ti : 0;
            string lang = args.Length >= 5 ? args[4] : "en";
            float scale = 1f;
            Size? clientSize = null;
            if (args.Length >= 6)
            {
                if (TryParseClientSize(args[5], out var parsedSize)) clientSize = parsedSize;
                else if (float.TryParse(args[5], NumberStyles.Float, CultureInfo.InvariantCulture, out var parsedScale))
                    scale = parsedScale;
            }
            if (args.Length >= 7)
            {
                if (!TryParseClientSize(args[6], out var parsedSize))
                    throw new ArgumentException("Screenshot client size must use WxH, for example 1600x1040.");
                clientSize = parsedSize;
            }
            var metrics = Screenshot.CaptureSettings(args[1], args.Length >= 3 ? args[2] : null, tab, lang, scale,
                useDemoLayout: args[0] == "--screenshot-demo", clientSizeOverride: clientSize);
            Console.WriteLine(string.Format(CultureInfo.InvariantCulture,
                "capture={0} client={1}x{2} mode={3} body={4}px map={5}px band={6}px bandPct={7:F2}%",
                args[1], metrics.ClientSize.Width, metrics.ClientSize.Height, metrics.Mode,
                metrics.BodyHeight, metrics.MapHeight, metrics.BandHeight, metrics.BandPercent));
            return;
        }

        // Dev/QA helper: flash the Identify overlays on every monitor, then exit. Runs BEFORE the single-instance
        // check (like --screenshot) so it works while the tray app is already running.
        if (args.Length >= 1 && args[0] == "--identify")
        {
            ApplicationConfiguration.Initialize();
            // Bounded, guaranteed-to-exit harness (no Application.Run -> can never hang): flash the overlays, then pump
            // messages via DoEvents until they self-close (proving the life timer works) or a hard ~2.8s cap, then
            // force-close any stragglers. Writes "iters=.. remaining=.." to args[1] so a caller can verify self-close.
            IdentifyOverlay.Flash(MonitorInfo.All(), Theming.Resolve("System"), null);
            int iters = 0;
            for (; iters < 280 && Application.OpenForms.Count > 0; iters++)
            {
                Application.DoEvents();
                Thread.Sleep(10);
            }
            int remaining = Application.OpenForms.Count;
            foreach (Form f in Application.OpenForms.Cast<Form>().ToArray())
            {
                try { f.Close(); f.Dispose(); } catch { }
            }
            if (args.Length >= 2)
            {
                try { System.IO.File.WriteAllText(args[1], $"iters={iters} remaining={remaining}"); } catch { }
            }
            return;
        }

        _single = new Mutex(initiallyOwned: true, "MouseFence_SingleInstance_2C7A", out bool createdNew);
        if (!createdNew) return;   // already running

        ApplicationConfiguration.Initialize();   // applies PerMonitorV2 + visual styles from the .csproj
        Application.Run(new TrayApplicationContext());

        GC.KeepAlive(_single);
    }

    private static bool TryParseClientSize(string value, out Size size)
    {
        size = Size.Empty;
        if (string.IsNullOrWhiteSpace(value)) return false;
        int separator = value.IndexOfAny(new[] { 'x', 'X', '×' });
        if (separator <= 0 || separator >= value.Length - 1) return false;
        if (!int.TryParse(value[..separator], NumberStyles.Integer, CultureInfo.InvariantCulture, out int width)
            || !int.TryParse(value[(separator + 1)..], NumberStyles.Integer, CultureInfo.InvariantCulture, out int height)
            || width < 320 || height < 320 || width > 10000 || height > 10000)
            return false;
        size = new Size(width, height);
        return true;
    }
}
