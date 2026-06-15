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
            Screenshot.CaptureSettings(args[1], args.Length >= 3 ? args[2] : null, tab);
            return;
        }

        _single = new Mutex(initiallyOwned: true, "MouseFence_SingleInstance_2C7A", out bool createdNew);
        if (!createdNew) return;   // already running

        ApplicationConfiguration.Initialize();   // applies PerMonitorV2 + visual styles from the .csproj
        Application.Run(new TrayApplicationContext());

        GC.KeepAlive(_single);
    }
}
