using System.Threading;

namespace MouseFence;

internal static class Program
{
    private static Mutex _single;

    [STAThread]
    private static void Main()
    {
        _single = new Mutex(initiallyOwned: true, "MouseFence_SingleInstance_2C7A", out bool createdNew);
        if (!createdNew) return;   // already running

        ApplicationConfiguration.Initialize();   // applies PerMonitorV2 + visual styles from the .csproj
        Application.Run(new TrayApplicationContext());

        GC.KeepAlive(_single);
    }
}
