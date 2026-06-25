using System.Threading;

namespace MouseFence;

internal static class Program
{
    private static Mutex _single;

    [STAThread]
    private static void Main(string[] args)
    {
        // Dev/CI helper: render the settings window off-screen to a PNG (for docs). Runs before the
        // single-instance check so it works while the tray app is already running.
        if (args.Length >= 2 && args[0] == "--screenshot")
        {
            ApplicationConfiguration.Initialize();
            int tab = args.Length >= 4 && int.TryParse(args[3], out var ti) ? ti : 0;
            string lang = args.Length >= 5 ? args[4] : "en";
            Screenshot.CaptureSettings(args[1], args.Length >= 3 ? args[2] : null, tab, lang);
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
}
