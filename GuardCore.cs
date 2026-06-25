namespace MouseFence;

public enum GuardAction { Pass, Block }

public enum Dir { Up, Down, Left, Right }

/// <summary>
/// Pure, Win32-free decision logic for the one-way barrier (unit-tested).
///
/// Barrier line at <see cref="BarrierY"/> (the bottom row's top edge). While the cursor is on the
/// bottom row it may not go above that line — blocking the top monitors AND the voids above side
/// screens. Crossing up is honoured only through a GATE: an inset X-range opening that a deliberate
/// upward push, originating in the same opening, may pass. Multiple gates = per-screen rules (each
/// allowed bottom→top crossing contributes one X-range). Once above the line ("on top") the cursor
/// roams freely and may always descend (one-way-up tool).
/// </summary>
public sealed class GuardCore
{
    public int CrossMinUp = 3;     // min upward px in a move to count as a deliberate cross
    public int CrossSlack = 5;     // allowed horizontal beyond the vertical component
    public bool DeliberateCross = true;   // true: a deliberate push is required; false: any upward move crosses

    public bool HasTop;
    public int BarrierY;
    public List<(int Min, int Max)> Gates = new();   // inset X-ranges where crossing up is allowed
    // Implicit anti-trap gates (owner-bound): an isolated bottom screen's up-overlap X-span + its own Y-band.
    // Crossing up here is always allowed (even when the master toggle is closed) so the cursor can never be
    // trapped — but a deliberate push is still required, and the cursor must START on the owning screen (the
    // Y-band) so a different screen stacked in the same X-column can't ride this gate up.
    public List<(int Min, int Max, int OwnerT, int OwnerB)> SafetyGates = new();

    // game mode: confine the cursor to whichever monitor it is currently on
    public bool Confine;
    public List<(int L, int T, int R, int B)> Monitors = new();

    // Side containment (horizontal mirror of the up-barrier): a SOFT barrier that stops accidental drift out of the
    // MAIN screen LEFT or RIGHT into a side screen. The barrier lines are the main screen's left/right edges
    // (MainL/MainR); containment applies ONLY while the move STARTS on the main screen, so returning from a side
    // screen back into main is always free — the cursor can never be trapped on a side screen. Toggled by
    // sideContain (passed to Decide): ON = block drift but a deliberate horizontal push still crosses (same
    // DeliberateCross rule as up); OFF = free. MainT/MainB bound the main screen's own Y-band (origin test).
    public bool HasSides;
    public int MainL, MainR, MainT, MainB;

    // Forbidden rectangles the cursor may never ENTER (e.g. a headless/dummy HDMI display the user can't see).
    // Half-open like every other rect here: [L,R) x [T,B). Empty list => feature inert (a true no-op). The data
    // model is intentionally a plain rect list so an Ignore-mode / per-layout profiles can reuse it later without
    // rework (a future Ignore mode is a strict subset: it would drop these from topology but NOT clamp).
    public List<(int L, int T, int R, int B)> Forbidden = new();

    /// <summary>The hook only needs to run (and the clamp/barrier logic only matters) when at least one of these
    /// holds. Factored so it is testable and so a dummy with NO top monitors still installs the hook to wall it off.</summary>
    public bool NeedsProcessing => HasTop || Confine || Forbidden.Count > 0 || HasSides;

    // descent routing (opt-in, default OFF): when descending out of a top monitor through its overhang onto the
    // WRONG side screen, clamp the exit X into the LINKED bottom monitor's full X-range so the cursor lands on the
    // intended screen. Each route pairs the top it leaves (Top) with the bottom it should fall into (Landing).
    // Pure int value tuples (no Native.RECT here) so cond7's structural != compares by value, not reference.
    public bool DescentRouting;
    public List<((int L, int T, int R, int B) Top, (int L, int T, int R, int B) Landing)> DescentRoutes = new();

    public int LastX, LastY;
    public bool HaveLast;
    public bool OnTop;

    public void Reset()
    {
        HaveLast = false;
        OnTop = false;
    }

    private static bool InSameGate(int x, int lastX, List<(int Min, int Max)> gates)
    {
        foreach (var g in gates)
            if (x >= g.Min && x < g.Max && lastX >= g.Min && lastX < g.Max)
                return true;
        return false;
    }

    // An origin-aware safety gate: both ends inside the up-overlap X-span AND the origin inside the owning
    // screen's Y-band, so only the isolated screen this gate belongs to can cross up through it.
    private bool InSafetyGate(int x, int lastX, int lastY)
    {
        foreach (var g in SafetyGates)
            if (x >= g.Min && x < g.Max && lastX >= g.Min && lastX < g.Max && lastY >= g.OwnerT && lastY < g.OwnerB)
                return true;
        return false;
    }

    // ---- pure topology: feeds B's isolation warning and C's safety gates (Win32-free, unit-tested) ----

    public static int SeamTol = 2;   // edge-equality tolerance (px): absorbs mixed-DPI 1px seam rounding,
                                     // far below any real gap (the 270px trap is detected with huge margin).

    /// <summary>A monitor entirely at/above the desktop origin (Windows primary top = 0) is a "top" monitor.</summary>
    public static bool IsTop((int L, int T, int R, int B) m) => m.B <= 0;

    /// <summary>True if the cursor can physically walk from a into b in direction d (touching edge + real overlap).</summary>
    public static bool Adjacent((int L, int T, int R, int B) a, (int L, int T, int R, int B) b, Dir d)
    {
        bool v = Math.Min(a.B, b.B) - Math.Max(a.T, b.T) > 0;   // shared vertical extent (strictly positive)
        bool h = Math.Min(a.R, b.R) - Math.Max(a.L, b.L) > 0;   // shared horizontal extent (strictly positive)
        return d switch
        {
            Dir.Right => Math.Abs(a.R - b.L) <= SeamTol && v,
            Dir.Left  => Math.Abs(a.L - b.R) <= SeamTol && v,
            Dir.Down  => Math.Abs(a.B - b.T) <= SeamTol && h,
            Dir.Up    => Math.Abs(a.T - b.B) <= SeamTol && h,
            _ => false,
        };
    }

    /// <summary>
    /// One pass over all monitors. A bottom screen with no Down/Left/Right neighbour is ISOLATED (-> warn). The
    /// subset that still has a top monitor above gets a SAFETY GATE: the X-overlap with the up-neighbour(s) plus
    /// the screen's own Y-band. A screen isolated on all four sides (no up-neighbour) warns but gets NO gate —
    /// gating into the void above it would let the cursor cross the barrier into empty space.
    /// </summary>
    public static (List<int> warn, List<(int Min, int Max, int OwnerT, int OwnerB)> gates) AntiTrap(
        IReadOnlyList<(int L, int T, int R, int B)> mons, ISet<int> topIdx)
    {
        var warn = new List<int>();
        var gates = new List<(int Min, int Max, int OwnerT, int OwnerB)>();
        for (int i = 0; i < mons.Count; i++)
        {
            if (topIdx.Contains(i)) continue;            // top screens roam free above the barrier
            bool nonUp = false, up = false;
            int upMin = int.MaxValue, upMax = int.MinValue;
            for (int j = 0; j < mons.Count; j++)
            {
                if (i == j) continue;
                if (Adjacent(mons[i], mons[j], Dir.Down) || Adjacent(mons[i], mons[j], Dir.Left) || Adjacent(mons[i], mons[j], Dir.Right))
                    nonUp = true;
                if (Adjacent(mons[i], mons[j], Dir.Up))
                {
                    up = true;
                    upMin = Math.Min(upMin, Math.Max(mons[i].L, mons[j].L));
                    upMax = Math.Max(upMax, Math.Min(mons[i].R, mons[j].R));
                }
            }
            if (nonUp) continue;                          // has a real side/down escape -> nothing to do
            warn.Add(i);                                  // isolated -> B warns (names this screen)
            if (up && upMax > upMin)                      // sole-up-exit -> C opens an owner-bound safety gate
                gates.Add((upMin, upMax, mons[i].T, mons[i].B));
        }
        return (warn, gates);
    }

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
    public static List<string> DisambiguateKeys(IReadOnlyList<(string Key, int Left, int Top)> monitors)
    {
        var result = new string[monitors.Count];
        foreach (var grp in monitors.Select((m, i) => (m, i)).GroupBy(t => t.m.Key))
        {
            var members = grp.ToList();
            if (members.Count < 2)
            {
                result[members[0].i] = members[0].m.Key;   // unique key -> leave exactly as-is
                continue;
            }
            int ord = 0;
            foreach (var (m, i) in members.OrderBy(t => t.m.Left).ThenBy(t => t.m.Top))
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

    /// <summary>Deterministic UI label that avoids mixing a positional # with renumber-prone DISPLAY names.</summary>
    public static string MonitorLabel(int ordinal, int width, int height, int left, int top, bool primary, string mainTag)
    {
        var suffix = primary && !string.IsNullOrEmpty(mainTag) ? "  " + mainTag : "";
        return $"{ordinal}  {width}x{height}  @ {left},{top}{suffix}";
    }

    private static bool InRect((int L, int T, int R, int B) r, int x, int y) =>
        x >= r.L && x < r.R && y >= r.T && y < r.B;

    private bool TryActiveMonitor(int x, int y, out (int L, int T, int R, int B) m)
    {
        foreach (var r in Monitors)
            if (x >= r.L && x < r.R && y >= r.T && y < r.B) { m = r; return true; }
        m = default;
        return false;
    }

    /// <summary>
    /// Pure wall-off clamp: if (x,y) lands inside ANY forbidden rect, push it back OUT to a legal pixel,
    /// preserving the orthogonal axis (no jitter), and never landing inside another forbidden rect. Returns
    /// true if it moved the point. GUARANTEES the returned (x,y) is outside EVERY forbidden rect (and terminates).
    ///
    /// ENTRY-SIDE-AWARE (binding fix #1): the move came from (LastX,LastY). When we have a last position that is
    /// OUTSIDE the rect, exit on the edge the move ENTERED from — the rect wall the entry SEGMENT (Last -> target)
    /// crosses FIRST (smallest valid parametric t), NOT the nearest edge of the target and NOT the larger-delta axis.
    /// A large delta (e.g. a fast flick or OS easing teleport deep into / past the centre of the dummy) must not
    /// "tunnel" to the far/void side and re-lose the cursor. Nearest-edge is only the fallback for !HaveLast or when
    /// Last is itself inside the connected region (no usable entry direction).
    ///
    /// ADJACENT/OVERLAPPING rects (binding fix #3 / ROUND-3 FIX A): a single per-rect exit can oscillate — exiting
    /// rect A on its nearest edge lands inside the adjacent rect B, exiting B lands back inside A, and a bounded
    /// loop ends STILL INSIDE. So we treat the touching forbidden rects as ONE connected region (connected component
    /// of the rect-overlap/adjacency graph) and, once an exit AXIS + SIDE is chosen, march that SAME direction to the
    /// far edge of the WHOLE component (never reversing), landing one pixel outside the entire region. This is the
    /// fixed point — re-scanning then finds the point outside every rect.
    /// </summary>
    private bool ClampOutOfForbidden(ref int x, ref int y)
    {
        bool moved = false;
        // Bounded: each iteration exits one whole connected component along a fixed direction; the next iteration can
        // only be triggered by a DIFFERENT (non-touching) component, of which there are at most Forbidden.Count. The
        // +1 guards a final no-op verification pass. This hard-caps work so the hot path can never spin.
        for (int pass = 0; pass <= Forbidden.Count; pass++)
        {
            // Find a forbidden rect the point is currently inside.
            int hit = -1;
            for (int i = 0; i < Forbidden.Count; i++)
            {
                var f = Forbidden[i];
                if (x >= f.L && x < f.R && y >= f.T && y < f.B) { hit = i; break; }
            }
            if (hit < 0) break;   // outside every forbidden rect -> done

            // The connected region (component) of all forbidden rects transitively touching/overlapping the hit rect.
            var region = ConnectedRegion(hit);

            // Decide which wall to exit through (axis + side), relative to the HIT rect.
            // Default = nearest-edge of the HIT rect (smallest penetration), used for !HaveLast / Last-inside-region.
            // Tie-order is deterministic: Left > Right > Top > Bottom (documented, intentional, not incidental).
            var h = Forbidden[hit];
            int dl = x - h.L, dr = h.R - x, dt = y - h.T, db = h.B - y;
            int min = Math.Min(Math.Min(dl, dr), Math.Min(dt, db));
            int exitAxis;   // 0 = clamp X (vertical wall), 1 = clamp Y (horizontal wall)
            bool exitToLow; // true => exit on the LOW side (L/T), false => HIGH side (R/B)
            if (min == dl) { exitAxis = 0; exitToLow = true; }
            else if (min == dr) { exitAxis = 0; exitToLow = false; }
            else if (min == dt) { exitAxis = 1; exitToLow = true; }
            else { exitAxis = 1; exitToLow = false; }

            // Entry-side override: if the move ORIGINATED OUTSIDE the whole connected region, exit back the way it
            // came in so a big delta can't tunnel through to the far side. The exit edge is the region wall the move
            // SEGMENT (Last -> target) crosses FIRST, NOT the axis with the larger delta. The region's combined
            // extent along each axis is [regL,regR) x [regT,regB); treat that as the entry box. Single-axis-outside
            // reduces to exiting that one axis. When Last is INSIDE the region there is no usable entry direction, so
            // the nearest-edge fallback (above) stands.
            ComponentExtent(region, out int regL, out int regT, out int regR, out int regB);
            bool lastOutsideRegion = HaveLast && (LastX < regL || LastX >= regR || LastY < regT || LastY >= regB);
            if (lastOutsideRegion)
            {
                bool lastLeft = LastX < regL, lastRight = LastX >= regR;
                bool lastAbove = LastY < regT, lastBelow = LastY >= regB;
                bool xOutside = lastLeft || lastRight;
                bool yOutside = lastAbove || lastBelow;

                bool useX;
                if (xOutside && yOutside)
                {
                    // First-crossed wall via segment entry against the REGION box: the X wall on Last's side is the
                    // plane wallX = lastLeft ? regL : regR; reached at tX = (wallX - LastX) / (x - LastX). Likewise Y.
                    // A NaN/negative/out-of-range t means that wall isn't reached on this segment; guard it as
                    // "not crossed" so the other axis wins.
                    int wallX = lastLeft ? regL : regR;
                    int wallY = lastAbove ? regT : regB;
                    int denomX = x - LastX, denomY = y - LastY;
                    double tX = denomX != 0 ? (double)(wallX - LastX) / denomX : double.PositiveInfinity;
                    double tY = denomY != 0 ? (double)(wallY - LastY) / denomY : double.PositiveInfinity;
                    bool validX = denomX != 0 && tX >= 0 && tX <= 1;
                    bool validY = denomY != 0 && tY >= 0 && tY <= 1;

                    if (validX && validY) useX = tX <= tY;          // smallest valid t = first crossed
                    else if (validX) useX = true;                   // only the X wall is reached
                    else if (validY) useX = false;                  // only the Y wall is reached
                    else useX = Math.Abs(x - LastX) >= Math.Abs(y - LastY);   // degenerate fallback: dominant axis
                }
                else
                    useX = xOutside;                                 // crossed exactly one axis

                if (useX) { exitAxis = 0; exitToLow = lastLeft; }
                else { exitAxis = 1; exitToLow = lastAbove; }
            }

            // March the chosen direction to the FAR edge of the WHOLE connected region (not just the hit rect), so an
            // adjacent rect on the way out can never re-capture the point. This is the key ROUND-3 FIX A change: the
            // exit target is the component's combined extent, so the result is outside every touching rect at once.
            if (exitAxis == 0) x = exitToLow ? regL - 1 : regR;   // exit the region's vertical wall, keep Y
            else y = exitToLow ? regT - 1 : regB;                 // exit the region's horizontal wall, keep X

            moved = true;
            // Re-scan: the new point is outside this whole component; only a SEPARATE (non-touching) component could
            // still contain it, which the next pass handles.
        }
        return moved;
    }

    /// <summary>Two half-open rects touch if they overlap OR abut (share an edge) along one axis while overlapping or
    /// abutting along the other — i.e. their CLOSED extents intersect. Adjacent dummies (sharing a seam) count as one
    /// connected region so the clamp marches past BOTH.</summary>
    private static bool RectsTouch((int L, int T, int R, int B) a, (int L, int T, int R, int B) b) =>
        a.L <= b.R && b.L <= a.R && a.T <= b.B && b.T <= a.B;

    /// <summary>Indices of every forbidden rect transitively touching <paramref name="seed"/> (the connected component
    /// of the rect adjacency/overlap graph). Pure, allocation-light flood fill over <see cref="Forbidden"/>.</summary>
    private List<int> ConnectedRegion(int seed)
    {
        var region = new List<int> { seed };
        var seen = new bool[Forbidden.Count];
        seen[seed] = true;
        for (int qi = 0; qi < region.Count; qi++)
        {
            var cur = Forbidden[region[qi]];
            for (int j = 0; j < Forbidden.Count; j++)
                if (!seen[j] && RectsTouch(cur, Forbidden[j])) { seen[j] = true; region.Add(j); }
        }
        return region;
    }

    /// <summary>The combined bounding extent of a connected region (its union's outer box). Marching to this box's far
    /// edge guarantees an exit outside every rect in the component.</summary>
    private void ComponentExtent(List<int> region, out int l, out int t, out int r, out int b)
    {
        l = int.MaxValue; t = int.MaxValue; r = int.MinValue; b = int.MinValue;
        foreach (var idx in region)
        {
            var f = Forbidden[idx];
            if (f.L < l) l = f.L;
            if (f.T < t) t = f.T;
            if (f.R > r) r = f.R;
            if (f.B > b) b = f.B;
        }
    }

    /// <summary>
    /// Decide a NON-injected move to (x,y). Returns Pass or Block (caller clamps to bx,by). Mutates state.
    /// <paramref name="gatesEnabled"/> is the up-barrier master toggle: when false, no upward crossing is allowed.
    /// <paramref name="sideContain"/> is the side-containment toggle: when true a soft barrier blocks accidental
    /// drift out of the MAIN screen left/right (a deliberate horizontal push still crosses); when false the side is
    /// free (see <see cref="HasSides"/>).
    /// </summary>
    public GuardAction Decide(int x, int y, bool gatesEnabled, bool sideContain, out int bx, out int by)
    {
        bx = x; by = y;

        // (0) Wall-off: the cursor may never ENTER an excluded display (a headless/dummy screen). Clamp the
        //     attempted target back to a legal edge FIRST, so every downstream rule (barrier, gates, confine,
        //     descent routing) only ever sees a position outside every forbidden rect. Runs even when there are no
        //     top monitors and game mode is off, so a dummy with no barrier still gets walled (NeedsProcessing).
        //     Only genuine incursions clamp: a target already outside every rect returns false here (no clamp,
        //     falls through), so sliding ALONG a seam on the real side is never touched -> no jitter.
        if (Forbidden.Count > 0 && ClampOutOfForbidden(ref bx, ref by))
        {
            // Record the clamped point as the new last position so the next move's intent is measured from where
            // the cursor actually is. The hook applies SetCursorPos(bx,by); that re-enters as injected -> ignored.
            Accept(bx, by);
            return GuardAction.Block;
        }
        // from here on, (x,y) == (bx,by) is guaranteed outside every forbidden rect.

        if (!HasTop && !Confine && !HasSides) { Accept(x, y); return GuardAction.Pass; }

        if (!HaveLast)
        {
            Accept(x, y);
            OnTop = HasTop && y < BarrierY;
            return GuardAction.Pass;
        }

        // Game mode: confine the cursor to the monitor it is currently on (overrides the up-barrier).
        if (Confine)
        {
            if (TryActiveMonitor(LastX, LastY, out var m))
            {
                if (x >= m.L && x < m.R && y >= m.T && y < m.B) { Accept(x, y); return GuardAction.Pass; }
                bx = Math.Clamp(x, m.L, m.R - 1);
                by = Math.Clamp(y, m.T, m.B - 1);
                Accept(bx, by);
                return GuardAction.Block;
            }
            Accept(x, y);
            return GuardAction.Pass;
        }

        // (S) Side containment: a SOFT barrier that stops the cursor from ACCIDENTALLY drifting out of the MAIN
        // screen LEFT/RIGHT into a side screen — the horizontal mirror of the up-barrier's feel. Only when the move
        // STARTS on the main screen (origin-aware) so a side->main return is never blocked and the cursor can't be
        // trapped on a side screen. When containment is ON, a slow drift / steep diagonal is blocked but a DELIBERATE
        // horizontal push still crosses (same DeliberateCross rule as up); when OFF the side is free. On a block we
        // clamp X back into the main screen AND keep the up-barrier honoured on the same move (a diagonal that also
        // goes above the line must not slip up at the clamped edge — that edge is outside the inset up-gate anyway),
        // so a corner move can't escape on either axis.
        if (HasSides)
        {
            bool startedOnMain = LastX >= MainL && LastX < MainR && LastY >= MainT && LastY < MainB;
            if (startedOnMain && (x < MainL || x >= MainR))
            {
                int sideDx = Math.Abs(x - LastX);
                int sideDy = Math.Abs(y - LastY);
                bool sideIntent = DeliberateCross ? (sideDx >= CrossMinUp && sideDy <= sideDx + CrossSlack) : sideDx > 0;
                if (sideContain && !sideIntent)
                {
                    bx = Math.Clamp(x, MainL, MainR - 1);   // never leave main horizontally
                    by = y;                                  // keep the vertical component by default
                    // Compose the up-barrier: a diagonal that also goes above the line must not slip UP at the clamped
                    // edge (that edge is outside the inset up-gate anyway). MainT == BarrierY here because main is the
                    // Windows primary (its top-left is the desktop origin 0,0), so this never lifts a legal point.
                    if (HasTop && by < BarrierY) by = BarrierY;
                    // Y-void correction ONLY: if clamping X left the point on NO real monitor (e.g. a side screen
                    // TALLER than main, or a corner beyond every screen), pull Y back onto the main band so the cursor
                    // lands on a real pixel. We must NOT clamp Y unconditionally — a down-diagonal toward a screen
                    // stacked BELOW main is a legal descent and must keep its Y (the clamped-X point is on that screen).
                    if (!TryActiveMonitor(bx, by, out _)) by = Math.Clamp(by, MainT, MainB - 1);
                    Accept(bx, by);
                    return GuardAction.Block;
                }
                // allowed: fall through so the up-barrier still governs the vertical component of the crossing.
            }
        }

        if (!HasTop) { Accept(x, y); return GuardAction.Pass; }

        // (A) Already above the line -> free roam; clears once the cursor descends past the line.
        if (OnTop)
        {
            // Descent routing (opt-in): on a real descent crossing a top's LOCAL bottom edge, if the exit X lands
            // on a different present monitor than the one linked to that top, clamp X into the linked bottom
            // monitor's full X-range. Horizontal routing on descent only — vertical escape stays ungated. Gate the
            // whole block behind the toggle so OFF is a true no-op and the ungated-descent path is untouched. Fire
            // on the FIRST route whose cond2-7 ALL hold against that SAME route (never a flat Any/All split).
            if (DescentRouting)
            {
                foreach (var route in DescentRoutes)
                {
                    if (route.Landing.R <= route.Landing.L) continue;   // defensive: a zero/negative-width landing
                                                                        // would make the Math.Clamp below throw
                    bool descending = y > LastY;                                             // 2
                    bool originInTop = InRect(route.Top, LastX, LastY);                      // 3
                    bool crossesLocalEdge = y >= route.Top.B && x >= route.Top.L && x < route.Top.R;   // 4
                    bool outsideLanding = !InRect(route.Landing, x, y);                      // 5
                    int landX = Math.Clamp(x, route.Landing.L, route.Landing.R - 1);
                    bool realLanding = InRect(route.Landing, landX, y);                      // 6
                    bool wrongMonitor = TryActiveMonitor(x, y, out var a) && a != route.Landing;   // 7
                    if (descending && originInTop && crossesLocalEdge && outsideLanding && realLanding && wrongMonitor)
                    {
                        bx = landX; by = y;
                        // Match the ungated path (line below): only leave "on top" once the cursor is at/below the
                        // GLOBAL barrier. A shallow top (Top.B < BarrierY) can fire this route while still above the
                        // barrier; clearing OnTop there would strip free-roam and warp the next move down to BarrierY.
                        if (by >= BarrierY) OnTop = false;
                        Accept(bx, by);
                        return GuardAction.Block;   // hook applies the X correction onto the linked screen
                    }
                }
            }

            if (y >= BarrierY) OnTop = false;
            Accept(x, y);
            return GuardAction.Pass;
        }

        // (B) On the bottom row, staying at/below the line -> normal move.
        if (y >= BarrierY)
        {
            Accept(x, y);
            return GuardAction.Pass;
        }

        // (C) Upward crossing attempt. Honour only a deliberate push that BOTH starts and ends inside the SAME
        // gate (origin-aware -> a side-screen diagonal can't sneak through another column). A configured user
        // gate needs the master toggle open; a SAFETY gate (an isolated screen's only escape) is honoured even
        // when the toggle is closed so the cursor can never be trapped. Both still require the deliberate push.
        bool startedBelow = LastY >= BarrierY;
        bool inUserGate = gatesEnabled && startedBelow && InSameGate(x, LastX, Gates);
        bool inSafetyGate = startedBelow && InSafetyGate(x, LastX, LastY);
        int dxAbs = Math.Abs(x - LastX);
        int dyUp = LastY - y;
        bool intent = DeliberateCross ? (dyUp >= CrossMinUp && dxAbs <= dyUp + CrossSlack) : dyUp > 0;
        if ((inUserGate || inSafetyGate) && intent)
        {
            OnTop = true;
            Accept(x, y);
            return GuardAction.Pass;
        }

        // Block: clamp down to the barrier line, keep X so horizontal sliding stays smooth.
        bx = x; by = BarrierY;
        Accept(bx, by);
        return GuardAction.Block;
    }

    private void Accept(int x, int y)
    {
        LastX = x; LastY = y; HaveLast = true;
    }
}
