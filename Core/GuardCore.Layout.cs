namespace MouseFence;

public sealed partial class GuardCore
{
    public static List<string> SanitizeExcluded(IReadOnlyList<string> presentKeys, string primaryKey,
                                                ISet<string> requested, out bool pruned)
    {
        // Present, requested keys (absent ids are dormant, not a prune) — keep present order for a stable result.
        var asked = presentKeys.Where(requested.Contains).ToList();
        var excluded = asked.Where(k => k != primaryKey).ToList();
        // Never leave zero usable monitors: if the request would wall off every present screen, keep the last usable.
        if (excluded.Count >= presentKeys.Count && excluded.Count > 0)
            excluded.RemoveAt(excluded.Count - 1);
        // pruned only when a PRESENT, requested key was refused (primary, or the keep-one rule).
        pruned = excluded.Count != asked.Count;
        return excluded;
    }

    /// <summary>
    /// Pure, Win32-free per-physical-monitor key disambiguation (ROUND-3 FIX B): given each monitor's raw stable key
    /// plus a STABLE ordering attribute (its left, then top px), return a key per monitor that is UNIQUE across the
    /// live layout. A raw key shared by 2+ monitors (two identical EDID panels the driver reports identically) gets a
    /// deterministic ordinal suffix (<c>#0</c>, <c>#1</c>, ... in ascending Left, then Top) so excluding ONE physical
    /// monitor never collapses to "every monitor with that key". A monitor whose raw key is already unique is returned
    /// UNCHANGED (no suffix) so existing single-panel ids are stable. Result is in the SAME order as the input.
    /// <see cref="MonitorInfo"/>.All() and the unit tests call THIS one method.
    /// </summary>
    public static List<string> DisambiguateKeys(IReadOnlyList<(string Key, int Left, int Top)> monitors) =>
        DisambiguateKeys(monitors.Select(m => (m.Key, m.Left, m.Top, RawIdentityKey: m.Key)).ToList());

    /// <summary>A-FIX-2 overload: exact geometry ties resolve by ordinal raw identity, never enumeration order.</summary>
    public static List<string> DisambiguateKeys(
        IReadOnlyList<(string Key, int Left, int Top, string RawIdentityKey)> monitors)
    {
        var result = new string[monitors.Count];
        foreach (var grp in monitors.Select((m, i) => (m, i)).GroupBy(t => t.m.Key, StringComparer.OrdinalIgnoreCase))
        {
            var members = grp.ToList();
            if (members.Count < 2)
            {
                result[members[0].i] = members[0].m.Key;   // unique key -> leave exactly as-is
                continue;
            }
            int ord = 0;
            foreach (var (m, i) in members.OrderBy(t => t.m.Left).ThenBy(t => t.m.Top)
                         .ThenBy(t => t.m.RawIdentityKey, StringComparer.Ordinal))
                result[i] = $"{m.Key}#{ord++}";
        }
        return result.ToList();
    }

    private static bool LooksLikeGdiDisplayName(string key) =>
        key != null && key.StartsWith(@"\\.\DISPLAY", StringComparison.OrdinalIgnoreCase);

    /// <summary>
    /// One-time settings migration for layout-dependent monitor keys. Old settings stored renumber-prone
    /// <c>\\.\DISPLAYn</c> names in ManualMonitors and UpLinks; new settings store the same stable per-monitor
    /// key used by ExcludedDevices. Present GDI names are converted through <paramref name="deviceToStable"/>;
    /// stale GDI references are dropped instead of being allowed to retarget another live monitor.
    /// Already-stable keys are preserved so imported machine-specific ids remain dormant if absent.
    /// </summary>
    public static (List<string> ManualKeys, List<(string From, string To)> Links) MigrateLayoutKeysToStable(
        IEnumerable<string> manualKeys,
        IEnumerable<(string From, string To)> links,
        IReadOnlyDictionary<string, string> deviceToStable)
    {
        string ConvertOrNull(string key)
        {
            if (string.IsNullOrWhiteSpace(key)) return null;
            if (deviceToStable.TryGetValue(key, out var stable)) return stable;
            return LooksLikeGdiDisplayName(key) ? null : key;
        }

        var manual = new List<string>();
        foreach (var key in manualKeys ?? Enumerable.Empty<string>())
        {
            var stable = ConvertOrNull(key);
            if (stable != null && !manual.Contains(stable))
                manual.Add(stable);
        }

        var migratedLinks = new List<(string From, string To)>();
        var seenLinks = new HashSet<string>();
        foreach (var (from, to) in links ?? Enumerable.Empty<(string From, string To)>())
        {
            var stableFrom = ConvertOrNull(from);
            var stableTo = ConvertOrNull(to);
            if (stableFrom == null || stableTo == null) continue;
            var sig = stableFrom + "\n" + stableTo;
            if (seenLinks.Add(sig))
                migratedLinks.Add((stableFrom, stableTo));
        }

        return (manual, migratedLinks);
    }

    /// <summary>Reset layout-dependent configuration to the live-layout default: AutoTop with no manual picks/rules.</summary>
    public static (string Mode, List<string> ManualKeys, List<(string From, string To)> Links) ResetLayoutConfig() =>
        ("AutoTop", new List<string>(), new List<(string From, string To)>());

    /// <summary>
    /// True if a hotkey is EXACTLY Ctrl+Alt+Arrow (no Shift, no Win) — the combo Intel GPU drivers bind to screen
    /// ROTATION, so MouseFence should warn or pick a safe default. <paramref name="vk"/> is the Win32 virtual-key code
    /// (which equals <c>(int)System.Windows.Forms.Keys</c> for the arrows: Left 0x25, Up 0x26, Right 0x27, Down 0x28).
    /// Pure + WinForms-free so it is unit-tested without pulling in WinForms.
    /// </summary>
    public static bool IsArrowRotationHotkey(int vk, bool ctrl, bool alt, bool shift, bool win) =>
        ctrl && alt && !shift && !win && (vk == 0x25 || vk == 0x26 || vk == 0x27 || vk == 0x28);

    /// <summary>
    /// Scale virtual-desktop rectangles into a target area while preserving their relative positions and a single
    /// common scale. The result uses the same half-open L/T/R/B convention as the input, making it suitable for both
    /// painting and hit-testing. Invalid/empty target areas return one empty rectangle per input rectangle.
    /// </summary>
    public static List<(int L, int T, int R, int B)> MapLayout(
        IReadOnlyList<(int L, int T, int R, int B)> rects, int targetW, int targetH, int pad)
    {
        var result = new List<(int L, int T, int R, int B)>(rects?.Count ?? 0);
        if (rects == null || rects.Count == 0) return result;

        targetW = Math.Max(0, targetW);
        targetH = Math.Max(0, targetH);
        pad = Math.Clamp(pad, 0, Math.Min(targetW, targetH) / 2);
        int usableW = Math.Max(0, targetW - pad * 2);
        int usableH = Math.Max(0, targetH - pad * 2);
        int minL = rects.Min(r => r.L), minT = rects.Min(r => r.T);
        int maxR = rects.Max(r => r.R), maxB = rects.Max(r => r.B);
        long virtualW = (long)maxR - minL, virtualH = (long)maxB - minT;

        if (usableW == 0 || usableH == 0 || virtualW <= 0 || virtualH <= 0)
        {
            for (int i = 0; i < rects.Count; i++) result.Add((pad, pad, pad, pad));
            return result;
        }

        double scale = Math.Min(usableW / (double)virtualW, usableH / (double)virtualH);
        double scaledW = virtualW * scale, scaledH = virtualH * scale;
        double offsetX = pad + (usableW - scaledW) / 2d;
        double offsetY = pad + (usableH - scaledH) / 2d;

        int Edge(double value) => (int)Math.Round(value, MidpointRounding.AwayFromZero);
        foreach (var r in rects)
        {
            int l = Edge(offsetX + ((long)r.L - minL) * scale);
            int t = Edge(offsetY + ((long)r.T - minT) * scale);
            int rr = Edge(offsetX + ((long)r.R - minL) * scale);
            int bb = Edge(offsetY + ((long)r.B - minT) * scale);
            l = Math.Clamp(l, pad, targetW - pad); t = Math.Clamp(t, pad, targetH - pad);
            rr = Math.Clamp(rr, pad, targetW - pad); bb = Math.Clamp(bb, pad, targetH - pad);

            // A-FIX-1: even a 640x480 dummy at the far end of a ±32K span remains visibly present. Preserve the
            // projected centre as far as the band permits, then clamp the 12x12 logical footprint inside the band.
            int minW = Math.Min(12, usableW), minH = Math.Min(12, usableH);
            if (r.R > r.L && rr - l < minW)
            {
                int centre = (l + rr) / 2;
                l = Math.Clamp(centre - minW / 2, pad, targetW - pad - minW);
                rr = l + minW;
            }
            if (r.B > r.T && bb - t < minH)
            {
                int centre = (t + bb) / 2;
                t = Math.Clamp(centre - minH / 2, pad, targetH - pad - minH);
                bb = t + minH;
            }
            result.Add((l, t, rr, bb));
        }
        return result;
    }

}
