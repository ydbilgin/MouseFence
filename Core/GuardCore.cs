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
public sealed partial class GuardCore
{
    public int CrossMinUp = 3;     // min upward px in a move to count as a deliberate cross
    public int CrossSlack = 5;     // allowed horizontal beyond the vertical component
    public bool DeliberateCross = true;   // true: a deliberate push is required; false: any upward move crosses
    public double InstantCrossSpeedPxPerSecond = 3000;
    public bool LearnFromMyArrivals;
    private bool _timedDecision;
    private double _eventSeconds;
    private readonly ArrivalLearner _upArrivals = new();
    private readonly ArrivalLearner _sideArrivals = new();
    private RuntimeTransition _arrivalContact;

    // Side-barrier sensitivity, kept INDEPENDENT of the up-barrier's CrossMinUp/CrossSlack so the user can dial the
    // side barrier's stiffness on its own: the min horizontal px in one move to count as a DELIBERATE side crossing,
    // and the vertical slack allowed beyond it. A HIGHER SideCrossMin = a firmer side barrier (a faster horizontal
    // flick is needed to cross, so accidental drift is caught more aggressively). Defaults match the up-barrier feel.
    public int SideCrossMin = 3;
    public int SideCrossSlack = 5;

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
    public RuntimeTransition[] Transitions = Array.Empty<RuntimeTransition>();

    // Side containment (horizontal mirror of the up-barrier): a SOFT, SYMMETRIC barrier on the MAIN screen's left/right
    // edges (MainL/MainR). It stops accidental drift in BOTH directions across those edges — OUT of main onto a side
    // screen, AND back IN from a side screen onto main — while a DELIBERATE horizontal push always crosses either way.
    // It is origin-aware (the OUT rule only fires for moves that START on main; the IN rule only for moves that start
    // on a side screen and land genuinely ON main), so a within-side move is never touched. Because it is soft (a real
    // push always crosses) plus the toggle/pause hotkeys exist, the cursor can never be trapped on either side. Toggled
    // by sideContain (passed to Decide): ON = block drift; OFF = free. MainT/MainB bound main's own Y-band (origin/landing test).
    public bool HasSides;
    public int MainL, MainR, MainT, MainB;

    // Forbidden rectangles the cursor may never ENTER (e.g. a headless/dummy HDMI display the user can't see).
    // Half-open like every other rect here: [L,R) x [T,B). In clone/overlap layouts, a coordinate also covered by a
    // usable monitor remains usable; only the excluded-only portion is effectively forbidden. Empty list => inert.
    // The data model is intentionally a plain rect list so an Ignore-mode / per-layout profiles can reuse it later.
    public List<(int L, int T, int R, int B)> Forbidden = new();

    /// <summary>The hook only needs to run (and the clamp/barrier logic only matters) when at least one of these
    /// holds. Factored so it is testable and so a dummy with NO top monitors still installs the hook to wall it off.</summary>
    public bool NeedsProcessing => HasTop || Confine || Forbidden.Count > 0 || HasSides || Transitions.Length > 0;

    // descent routing (opt-in, default OFF): when descending out of a top monitor through its overhang onto the
    // WRONG side screen, clamp the exit X into the LINKED bottom monitor's full X-range so the cursor lands on the
    // intended screen. Each route pairs the top it leaves (Top) with the bottom it should fall into (Landing).
    // Pure int value tuples (no Native.RECT here) so cond7's structural != compares by value, not reference.
    public bool DescentRouting;
    public List<((int L, int T, int R, int B) Top, (int L, int T, int R, int B) Landing)> DescentRoutes = new();

    public int LastX, LastY;
    public bool HaveLast;
    public bool OnTop;
    private bool _gatesEnabledForDecision;
    private bool _sideContainForDecision;

    public void Reset()
    {
        HaveLast = false;
        OnTop = false;
        _upArrivals.Reset(); _sideArrivals.Reset(); _arrivalContact = null;
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

}
