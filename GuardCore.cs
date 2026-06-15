namespace MouseFence;

public enum GuardAction { Pass, Block }

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

    public bool HasTop;
    public int BarrierY;
    public List<(int Min, int Max)> Gates = new();   // inset X-ranges where crossing up is allowed

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

    private bool InSameGate(int x, int lastX)
    {
        foreach (var g in Gates)
            if (x >= g.Min && x < g.Max && lastX >= g.Min && lastX < g.Max)
                return true;
        return false;
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

        // (C) Upward crossing attempt. Honour only a deliberate push that BOTH starts and ends inside
        // the SAME open gate (origin-aware -> a side-screen diagonal can't sneak through another column).
        bool inGate = gatesEnabled && LastY >= BarrierY && InSameGate(x, LastX);
        int dxAbs = Math.Abs(x - LastX);
        int dyUp = LastY - y;
        if (inGate && dyUp >= CrossMinUp && dxAbs <= dyUp + CrossSlack)
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
