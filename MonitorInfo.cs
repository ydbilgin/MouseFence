using System.Runtime.InteropServices;

namespace MouseFence;

/// <summary>A physical monitor (coordinates are physical pixels because the process is PerMonitorV2 aware).</summary>
public sealed class MonitorInfo
{
    public string Device { get; set; }
    public Native.RECT Bounds { get; set; }
    public bool Primary { get; set; }
    public int Index { get; set; }

    public static List<MonitorInfo> All()
    {
        var list = new List<MonitorInfo>();

        Native.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero,
            (IntPtr h, IntPtr hdc, ref Native.RECT r, IntPtr d) =>
            {
                var mi = new Native.MONITORINFOEX { cbSize = Marshal.SizeOf<Native.MONITORINFOEX>() };
                if (Native.GetMonitorInfo(h, ref mi))
                {
                    list.Add(new MonitorInfo
                    {
                        Device = mi.szDevice,
                        Bounds = mi.rcMonitor,
                        Primary = (mi.dwFlags & Native.MONITORINFOF_PRIMARY) != 0,
                    });
                }
                return true;
            }, IntPtr.Zero);

        list.Sort((a, b) => a.Bounds.Left.CompareTo(b.Bounds.Left));
        for (int i = 0; i < list.Count; i++)
            list[i].Index = i + 1;

        return list;
    }

    /// <summary>"Top" monitors = those that sit entirely above the primary's top edge (y &lt;= 0).</summary>
    public static bool IsAbovePrimary(MonitorInfo m) =>
        GuardCore.IsTop((m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom));
}
