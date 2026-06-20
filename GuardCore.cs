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

    private bool TryActiveMonitor(int x, int y, out (int L, int T, int R, int B) m)
    {
        foreach (var r in Monitors)
            if (x >= r.L && x < r.R && y >= r.T && y < r.B) { m = r; return true; }
        m = default;
        return false;
    }

    /// <summary>
    /// Decide a NON-injected move to (x,y). Returns Pass or Block (caller clamps to bx,by). Mutates state.
    /// <paramref name="gatesEnabled"/> is the master toggle: when false, no crossing is allowed anywhere.
    /// </summary>
    public GuardAction Decide(int x, int y, bool gatesEnabled, out int bx, out int by)
    {
        bx = x; by = y;

        if (!HasTop && !Confine) { Accept(x, y); return GuardAction.Pass; }

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

        if (!HasTop) { Accept(x, y); return GuardAction.Pass; }

        // (A) Already above the line -> free roam; clears once the cursor descends past the line.
        if (OnTop)
        {
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
