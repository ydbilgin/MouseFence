using System.Runtime.InteropServices;

namespace MouseFence;

/// <summary>A physical monitor (coordinates are physical pixels because the process is PerMonitorV2 aware).</summary>
public sealed class MonitorInfo
{
    /// <summary>GDI display name (<c>\\.\DISPLAYn</c>). RENUMBER-PRONE: n changes across layout changes.</summary>
    public string Device { get; set; }

    /// <summary>
    /// A STABLE, PER-PHYSICAL-MONITOR identity (FIX 4 + ROUND-3 FIX B): the per-monitor-instance device INTERFACE
    /// path read READ-ONLY via EnumDisplayDevices on the MONITOR child with <c>EDD_GET_DEVICE_INTERFACE_NAME</c>.
    /// That path is unique per physical PORT even for two identical panels (same EDID), and still survives
    /// <c>\\.\DISPLAYn</c> renumbering when the layout changes. The exclude list keys on this so "exclude THIS dummy"
    /// targets exactly one physical monitor (two identical panels are NOT collapsed). If two live monitors still
    /// share a raw key, <see cref="All"/> disambiguates deterministically by appending an index. Falls back to
    /// <see cref="Device"/> when no instance id is available. NOTE: this only stabilizes the exclude IDENTITY; the
    /// live auto-reconfigure (DisplaySettingsChanged -> Configure reading live positions) already tracks geometry.
    /// </summary>
    public string StableId { get; set; }

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
                        StableId = StableIdFor(mi.szDevice),
                        Bounds = mi.rcMonitor,
                        Primary = (mi.dwFlags & Native.MONITORINFOF_PRIMARY) != 0,
                    });
                }
                return true;
            }, IntPtr.Zero);

        list.Sort((a, b) =>
        {
            int byLeft = a.Bounds.Left.CompareTo(b.Bounds.Left);
            return byLeft != 0 ? byLeft : a.Bounds.Top.CompareTo(b.Bounds.Top);
        });
        for (int i = 0; i < list.Count; i++)
            list[i].Index = i + 1;

        DisambiguateStableIds(list);

        return list;
    }

    /// <summary>
    /// ROUND-3 FIX B: guarantee each PHYSICAL monitor has a DISTINCT StableId even when the raw per-port id collides
    /// (two identical panels whose interface path the driver reports identically, or both fell back to a GDI name).
    /// For each group sharing a raw key, append a stable, deterministic ordinal so excluding ONE monitor never
    /// collapses to "every monitor with that key". The ordinal is keyed on a STABLE geometric attribute
    /// (Bounds.Left, then Top) so the same physical monitor keeps the same disambiguated id across \\.\DISPLAYn
    /// renumbering (a re-arrange that MOVES a monitor is a deliberate layout change the live reconfigure handles).
    /// A lone monitor with a unique key is left untouched (no suffix), so existing single-panel ids are unchanged.
    /// </summary>
    private static void DisambiguateStableIds(List<MonitorInfo> list)
    {
        // The disambiguation rule is pure + unit-tested in GuardCore; this only feeds it the raw key + stable geometry.
        var keys = GuardCore.DisambiguateKeys(
            list.Select(m => (m.StableId, m.Bounds.Left, m.Bounds.Top)).ToList());
        for (int i = 0; i < list.Count; i++)
            list[i].StableId = keys[i];
    }

    // Read the per-PHYSICAL-monitor instance id behind the adapter named by \\.\DISPLAYn. READ-ONLY (EnumDisplayDevices
    // never mutates the display config). Prefer the MONITOR child's device INTERFACE path (unique per physical port even
    // for identical panels) when EDD_GET_DEVICE_INTERFACE_NAME yields it; else fall back to the monitor instance DeviceID,
    // and finally to the renumber-prone GDI device name so an exclude entry is never keyless. Cross-monitor collisions
    // (identical panels the driver reports identically) are resolved deterministically in DisambiguateStableIds.
    private static string StableIdFor(string gdiDeviceName)
    {
        try
        {
            // iDevNum 0 = the monitor child attached to this adapter; the interface-name flag puts the per-port
            // device interface path in DeviceID.
            var dd = new Native.DISPLAY_DEVICE { cb = Marshal.SizeOf<Native.DISPLAY_DEVICE>() };
            if (Native.EnumDisplayDevices(gdiDeviceName, 0, ref dd, Native.EDD_GET_DEVICE_INTERFACE_NAME)
                && !string.IsNullOrEmpty(dd.DeviceID))
                return dd.DeviceID;

            // Fallback: same monitor child WITHOUT the interface-name flag yields the MONITOR\<EDID>\... instance path.
            var dd2 = new Native.DISPLAY_DEVICE { cb = Marshal.SizeOf<Native.DISPLAY_DEVICE>() };
            if (Native.EnumDisplayDevices(gdiDeviceName, 0, ref dd2, 0)
                && !string.IsNullOrEmpty(dd2.DeviceID))
                return dd2.DeviceID;
        }
        catch { /* fall through to the GDI name */ }
        return gdiDeviceName;
    }

    /// <summary>"Top" monitors = those that sit entirely above the primary's top edge (y &lt;= 0).</summary>
    public static bool IsAbovePrimary(MonitorInfo m) =>
        GuardCore.IsTop((m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom));
}
