namespace MouseFence;

public enum GuardAction { Pass, Block }

/// <summary>
/// Pure, Win32-free decision logic for the one-way barrier, so it can be unit-tested deterministically.
/// <see cref="MouseGuard"/> wraps this with the actual hook + SetCursorPos.
///
/// Barrier model: a horizontal line at <see cref="BarrierY"/> (the top monitors' bottom edge). While the
/// cursor is on the bottom row it may not go above the line — this blocks the top monitor AND the empty
/// voids above the side screens. Crossing up is honoured only for a deliberate upward push through the
/// open, inset gate. Once above the line the cursor roams freely until it descends back.
/// </summary>
public sealed class GuardCore
{
    public int CrossMinUp = 3;     // min upward px in a move to count as a deliberate cross
    public int CrossSlack = 5;     // allowed horizontal beyond the vertical component

    public bool HasTop;
    public int BarrierY;
    public int GateLeft, GateRight; // already inset

    public int LastX, LastY;
    public bool HaveLast;
    public bool OnTop;

    public void Reset()
    {
        HaveLast = false;
        OnTop = false;
    }

    /// <summary>
    /// Decide what to do with a NON-injected mouse move to (x,y). Returns Pass (let it through) or Block
    /// (caller must SetCursorPos to bx,by). Mutates state.
    /// </summary>
    public GuardAction Decide(int x, int y, bool gateOpen, out int bx, out int by)
    {
        bx = x; by = y;

        if (!HasTop) { Accept(x, y); return GuardAction.Pass; }

        // Baseline on the first event so we never act on a stale (0,0) previous position.
        if (!HaveLast)
        {
            Accept(x, y);
            OnTop = y < BarrierY;
            return GuardAction.Pass;
        }

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

        // (C) Upward crossing attempt. Honour only a deliberate push that BOTH starts and ends in the
        // open, inset main gate — checking the origin (Last) too, so a fast diagonal originating on a
        // side screen can't sneak up just because its landing X falls in the gate column.
        bool inGate = gateOpen
            && x >= GateLeft && x < GateRight              // candidate within the main gate column
            && LastX >= GateLeft && LastX < GateRight      // origin also within the gate column
            && LastY >= BarrierY;                          // origin was on the bottom row, not already above
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
