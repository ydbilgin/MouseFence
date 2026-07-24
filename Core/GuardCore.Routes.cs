namespace MouseFence;

public sealed partial class GuardCore
{
    /// <summary>
    /// Pure descent-route derivation (Win32-free, unit-tested). From the allowed up-crossings, derive (Top, Landing)
    /// pairs so a descent off a top's overhang clamps back onto the intended bottom. Three guards:
    ///  - AMBIGUITY: route a top only when it has exactly ONE distinct FromDevice (else no derivable "intended" landing).
    ///  - SF-1 (vertical): the landing must sit genuinely below the top (Landing.T >= Top.B), else a From=side link
    ///    would yank a center descent sideways.
    ///  - X-OVERLAP (horizontal): the Top/Landing must share a positive X-span (a real downward seam), else a link that
    ///    became non-overlapping after a layout change would teleport the cursor across the gap.
    /// <paramref name="links"/> are (FromDevice, ToDevice) pairs; <paramref name="rects"/> maps device -> bounds;
    /// <paramref name="topDevices"/> is the set of top-monitor devices.
    /// </summary>
    public static List<((int L, int T, int R, int B) Top, (int L, int T, int R, int B) Landing)> DeriveRoutes(
        IEnumerable<(string From, string To)> links,
        IReadOnlyDictionary<string, (int L, int T, int R, int B)> rects,
        ISet<string> topDevices)
    {
        var routes = new List<((int L, int T, int R, int B) Top, (int L, int T, int R, int B) Landing)>();
        foreach (var grp in links.Where(lk => topDevices.Contains(lk.To)).GroupBy(lk => lk.To))
        {
            var froms = grp.Select(lk => lk.From).Distinct().ToList();
            if (froms.Count != 1) continue;                                  // ambiguity guard
            if (!rects.TryGetValue(froms[0], out var landing)) continue;
            if (!rects.TryGetValue(grp.Key, out var top)) continue;
            if (landing.T < top.B) continue;                                 // SF-1: Landing.T >= Top.B
            if (Math.Min(top.R, landing.R) <= Math.Max(top.L, landing.L)) continue;   // positive X-overlap
            routes.Add((top, landing));
        }
        return routes;
    }

    /// <summary>
    /// Pure, Win32-free exclude sanitizer (binding fix #2 / FIX 3): given the live monitor keys, the primary key,
    /// and the requested exclude set, return the keys that may SAFELY be walled off. Two hard invariants:
    ///   - the PRIMARY display is never excluded (walling it would lock the user out), and
    ///   - at least ONE usable monitor always remains (never wall off every screen).
    /// <paramref name="presentKeys"/> are the stable keys of the monitors actually present in the live layout
    /// (in display order). A requested key absent from this list is simply dormant (an imported/stale id) and is
    /// NOT a prune. <paramref name="pruned"/> is set true iff a PRESENT, requested key was refused (the primary, or
    /// the keep-one rule) so the caller can warn. Result preserves the input order of <paramref name="presentKeys"/>.
    /// Both <see cref="TrayApplicationContext"/>.Configure() and the unit tests call THIS one method.
    /// </summary>
    /// <summary>True when Last -> target crosses a top monitor's local bottom edge through its half-open X span.
    /// The intersection X matters: on a fast diagonal the endpoint may already be beyond the top's side.</summary>
    private bool CrossesLocalBottomEdge((int L, int T, int R, int B) top, int x, int y)
    {
        int verticalDelta = y - LastY;
        if (verticalDelta <= 0 || LastY >= top.B || y < top.B) return false;
        double hit = (double)(top.B - LastY) / verticalDelta;
        double crossingX = LastX + (x - LastX) * hit;
        return crossingX >= top.L && crossingX < top.R;
    }

    private bool TryGuideDescent(int x, int y, out int bx, out int by)
    {
        bx = x; by = y;
        if (!DescentRouting) return false;
        foreach (var route in DescentRoutes)
        {
            if (route.Landing.R <= route.Landing.L) continue;
            bool descending = y > LastY;
            bool originInTop = InRect(route.Top, LastX, LastY);
            bool crossesLocalEdge = CrossesLocalBottomEdge(route.Top, x, y);
            bool outsideLanding = !InRect(route.Landing, x, y);
            int landX = Math.Clamp(x, route.Landing.L, route.Landing.R - 1);
            bool realLanding = InRect(route.Landing, landX, y);
            bool wrongMonitor = TryActiveMonitor(x, y, out var active) && active != route.Landing;
            if (!descending || !originInTop || !crossesLocalEdge || !outsideLanding || !realLanding || !wrongMonitor)
                continue;
            bx = landX;
            return true;
        }
        return false;
    }

}
