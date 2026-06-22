using MouseFence;

// Deterministic tests for the barrier decision logic, using the user's real monitor geometry:
//   DISPLAY3 left  X -1920..0    Y 14..1094
//   DISPLAY1 MAIN  X 0..2560     Y 0..1440   (primary; barrier line = Y 0)
//   DISPLAY2 right X 2560..4480  Y 0..1080
//   DISPLAY4 TOP   X -440..3000  Y -1440..0  (above; void above DISPLAY2 right part: X 3000..4480, Y<0)
// Gate = MAIN inset by 24 -> [24, 2536).

int fails = 0;

GuardCore New()
{
    var c = new GuardCore { HasTop = true, BarrierY = 0, Gates = new List<(int Min, int Max)> { (24, 2536) } };
    c.Reset();
    return c;
}

GuardCore NewGates(params (int Min, int Max)[] gates)
{
    var c = new GuardCore { HasTop = true, BarrierY = 0, Gates = gates.ToList() };
    c.Reset();
    return c;
}

GuardCore NewConfine(params (int L, int T, int R, int B)[] mons)
{
    var c = new GuardCore { HasTop = false, Confine = true, Monitors = mons.ToList() };
    c.Reset();
    return c;
}

GuardCore NewSafety(params (int Min, int Max, int OwnerT, int OwnerB)[] safety)
{
    var c = new GuardCore { HasTop = true, BarrierY = 0, SafetyGates = safety.ToList() };
    c.Reset();
    return c;
}

// Wall-off only: no top monitors, no game mode — just forbidden rects. Proves (0) runs before the
// !HasTop && !Confine bail-out and that NeedsProcessing flips the hook on.
GuardCore NewForbidden(params (int L, int T, int R, int B)[] forbidden)
{
    var c = new GuardCore { HasTop = false, Confine = false, Forbidden = forbidden.ToList() };
    c.Reset();
    return c;
}

(GuardAction act, int by, bool onTop, int bx) Move(GuardCore c, int x, int y, bool gate)
{
    var a = c.Decide(x, y, gate, out int bx, out int by);
    return (a, by, c.OnTop, bx);
}

void Check(string name, bool ok, string detail = "")
{
    Console.WriteLine($"{(ok ? "PASS" : "FAIL")}  {name}{(ok ? "" : "   <-- " + detail)}");
    if (!ok) fails++;
}

Console.WriteLine("MouseFence barrier logic — scenario tests\n");

// 1) THE right-screen bug: at top of DISPLAY2, moving up-right into the VOID (X>3000) must be blocked
//    and clamped to Y=0 (not snapped onto the top monitor's far wall).
{
    var c = New(); Move(c, 3500, 0, true);
    var r = Move(c, 3600, -20, true);
    Check("right-screen void: up into void is blocked & clamped to Y=0",
        r.act == GuardAction.Block && r.by == 0 && !r.onTop, $"act={r.act} by={r.by} onTop={r.onTop}");
}

// 2) DISPLAY2 over the top monitor overlap (X 2560..3000): up is blocked.
{
    var c = New(); Move(c, 2700, 0, true);
    var r = Move(c, 2700, -10, true);
    Check("right-screen overlap: up is blocked", r.act == GuardAction.Block && r.by == 0 && !r.onTop,
        $"act={r.act} by={r.by}");
}

// 3) Sliding right along DISPLAY2's top with up-jitter never leaks up.
{
    var c = New(); Move(c, 3000, 0, true);
    var a = Move(c, 3100, 0, true);
    var b = Move(c, 3200, -1, true);
    var d = Move(c, 3300, 0, true);
    Check("right-screen slide+jitter stays on bottom row",
        a.act == GuardAction.Pass && b.act == GuardAction.Block && d.act == GuardAction.Pass
        && !a.onTop && !b.onTop && !d.onTop, $"a={a.act} b={b.act} d={d.act}");
}

// 4) Side-left (DISPLAY3): up is always blocked.
{
    var c = New(); Move(c, -500, 50, true);
    var r = Move(c, -500, -10, true);
    Check("side-left: up is blocked", r.act == GuardAction.Block && !r.onTop, $"act={r.act} onTop={r.onTop}");
}

// 5) MAIN gate, deliberate vertical push (gate open): cross is honoured.
{
    var c = New(); Move(c, 1280, 0, true);
    var r = Move(c, 1280, -15, true);
    Check("main gate: deliberate up push crosses", r.act == GuardAction.Pass && r.onTop, $"act={r.act} onTop={r.onTop}");
}

// 5b) Cross with slight up-right tilt still allowed.
{
    var c = New(); Move(c, 1280, 0, true);
    var r = Move(c, 1290, -15, true); // dx=10, dyUp=15 -> 10 <= 23
    Check("main gate: up-right deliberate push crosses", r.act == GuardAction.Pass && r.onTop, $"act={r.act}");
}

// 6) MAIN horizontal slide leaks blocked: small up-jitter and fast diagonal both blocked.
{
    var c = New(); Move(c, 1280, 0, true);
    var r = Move(c, 1340, -2, true); // dyUp=2 < CrossMinUp
    Check("main slide: small up-jitter blocked", r.act == GuardAction.Block && !r.onTop, $"act={r.act}");
}
{
    var c = New(); Move(c, 1280, 0, true);
    var r = Move(c, 1400, -10, true); // dyUp=10, dx=120 -> 120 > 18
    Check("main slide: fast diagonal blocked", r.act == GuardAction.Block && !r.onTop, $"act={r.act}");
}

// 7) Gate CLOSED: even a deliberate up push from MAIN is blocked.
{
    var c = New(); Move(c, 1280, 0, false);
    var r = Move(c, 1280, -15, false);
    Check("gate closed: main up push blocked", r.act == GuardAction.Block && !r.onTop, $"act={r.act}");
}

// 8) Inset edges: pushing up at the extreme left/right of MAIN (outside the inset gate) is blocked.
{
    var c = New(); Move(c, 10, 0, true);
    var r = Move(c, 10, -15, true);
    Check("inset: up at far-left of main blocked", r.act == GuardAction.Block, $"act={r.act}");
}
{
    var c = New(); Move(c, 2550, 0, true);
    var r = Move(c, 2550, -15, true);
    Check("inset: up at far-right of main blocked", r.act == GuardAction.Block, $"act={r.act}");
}

// 9) After crossing: free roam over the whole top monitor, then descend returns to bottom row.
{
    var c = New(); Move(c, 1280, 0, true); Move(c, 1280, -15, true); // now onTop
    var roam1 = Move(c, 1500, -200, true);
    var roam2 = Move(c, 2900, -50, true);   // roam over the part above DISPLAY2
    var down = Move(c, 2900, 5, true);       // push down -> escape to DISPLAY2
    Check("roam on top is free; descending escapes",
        roam1.act == GuardAction.Pass && roam1.onTop &&
        roam2.act == GuardAction.Pass && roam2.onTop &&
        down.act == GuardAction.Pass && !down.onTop, $"r1={roam1.onTop} r2={roam2.onTop} down.onTop={down.onTop}");
}

// 10) Normal bottom-row movement is never blocked.
{
    var c = New(); Move(c, 1280, 500, true);
    var r = Move(c, 1700, 500, true);
    Check("bottom-row horizontal move passes", r.act == GuardAction.Pass && !r.onTop, $"act={r.act}");
}

// 11) COUNCIL REGRESSION — side-origin coalesced diagonal must NOT leak up via the gate column.
{
    var c = New(); Move(c, 2600, 0, true);        // origin on DISPLAY2 (a side screen)
    var r = Move(c, 2500, -200, true);            // fast diagonal landing in the gate column, going up
    Check("side-origin diagonal from DISPLAY2 is blocked",
        r.act == GuardAction.Block && r.by == 0 && !r.onTop, $"act={r.act} by={r.by} onTop={r.onTop}");
}
{
    var c = New(); Move(c, -100, 30, true);        // origin on DISPLAY3 (left side screen)
    var r = Move(c, 100, -200, true);              // diagonal up into the gate column
    Check("side-origin diagonal from DISPLAY3 is blocked",
        r.act == GuardAction.Block && !r.onTop, $"act={r.act} onTop={r.onTop}");
}

// 12) Legit MAIN-origin cross still works after the origin-aware change.
{
    var c = New(); Move(c, 1280, 0, true);
    var r = Move(c, 1280, -15, true);
    Check("main-origin deliberate cross still works (origin-aware)", r.act == GuardAction.Pass && r.onTop, $"act={r.act}");
}

// 13) Tighter slack (CrossSlack=5): a slow slide with 3px up-jitter no longer leaks; near-vertical still crosses.
{
    var c = New(); Move(c, 1280, 0, true);
    var r = Move(c, 1290, -3, true);              // dx=10, dyUp=3 -> 10 > 3+5 -> Block
    Check("slow slide+jitter (dx10,dy3) blocked with slack=5", r.act == GuardAction.Block && !r.onTop, $"act={r.act}");
}
{
    var c = New(); Move(c, 1280, 0, true);
    var r = Move(c, 1283, -10, true);            // dx=3, dyUp=10 -> 3 <= 15 -> Pass
    Check("near-vertical push still crosses with slack=5", r.act == GuardAction.Pass && r.onTop, $"act={r.act}");
}

// ---- MULTI-GATE (per-screen directional rules): two separate gate columns ----
// Gate A = [0,1000), Gate B = [2000,3000). e.g. left-bottom may only go to left-top (A),
// right-bottom may only go to right-top (B). Each column is independent.
{
    var c = NewGates((0, 1000), (2000, 3000));
    Move(c, 500, 0, true);
    var r = Move(c, 500, -15, true);
    Check("multi-gate: deliberate up in gate A crosses", r.act == GuardAction.Pass && r.onTop, $"act={r.act}");
}
{
    var c = NewGates((0, 1000), (2000, 3000));
    Move(c, 2500, 0, true);
    var r = Move(c, 2500, -15, true);
    Check("multi-gate: deliberate up in gate B crosses", r.act == GuardAction.Pass && r.onTop, $"act={r.act}");
}
{
    var c = NewGates((0, 1000), (2000, 3000));
    Move(c, 1500, 0, true);                 // column BETWEEN the two gates (no UpLink there)
    var r = Move(c, 1500, -15, true);
    Check("multi-gate: up between gates is blocked", r.act == GuardAction.Block && !r.onTop, $"act={r.act}");
}
{
    var c = NewGates((0, 1000), (2000, 3000));
    Move(c, 500, 0, true);                  // origin in gate A
    var r = Move(c, 2500, -200, true);      // landing in gate B -> different gate, must be blocked
    Check("multi-gate: cross-gate diagonal blocked (origin-aware)", r.act == GuardAction.Block && !r.onTop, $"act={r.act}");
}

// ---- GAME MODE (confine cursor to its monitor) ----
{
    var c = NewConfine((0, 0, 2560, 1440), (2560, 0, 4480, 1080));
    Move(c, 1000, 500, true);                  // baseline on monitor 1
    var inside = Move(c, 2000, 600, true);     // still on monitor 1
    var cross = Move(c, 3000, 500, true);      // try to leave to monitor 2
    Check("game mode: move within active monitor passes", inside.act == GuardAction.Pass, $"act={inside.act}");
    Check("game mode: leaving active monitor is blocked", cross.act == GuardAction.Block, $"act={cross.act}");
}
{
    var c = NewConfine((0, 0, 2560, 1440), (2560, 0, 4480, 1080));
    Move(c, 1000, 500, true);                  // on monitor 1
    var up = Move(c, 1000, -10, true);         // try to go above monitor 1 (no top above it here)
    Check("game mode: leaving upward is also blocked", up.act == GuardAction.Block, $"act={up.act}");
}

// ---- deliberate-cross toggle: OFF = any upward move crosses ----
{
    var c = New(); c.DeliberateCross = false;
    Move(c, 1280, 0, true);
    var r = Move(c, 1290, -3, true);          // would be blocked when deliberate is ON
    Check("deliberate OFF: any upward move through the gate crosses", r.act == GuardAction.Pass && r.onTop, $"act={r.act}");
}

// ---- REGRESSION: barrier must track the LIVE seam (auto-reconfigure on display change) ----
// The "cursor leaks up from a side screen and Windows easing teleports it to a far corner" bug was caused
// by the barrier going STALE: monitors were realigned so a side screen's top edge became the seam (Y=0),
// but MouseFence still held the OLD barrier (Y=-6). That left a gap above the side screen that the up-move
// slipped through. TrayApplicationContext now rebuilds the barrier on DisplaySettingsChanged so BarrierY
// always equals the live seam. These pin the decision logic the reconfigure relies on.
{
    var c = New();                         // BarrierY = 0 (the live seam), gate [24,2536]
    Move(c, 3000, 50, true);               // on a side screen whose top edge IS the seam
    var r = Move(c, 3000, -3, true);       // push up past the seam, outside any gate
    Check("reconfigure: correct barrier blocks the side-screen up-leak at the seam",
        r.act == GuardAction.Block && r.by == 0 && !r.onTop, $"act={r.act} by={r.by} onTop={r.onTop}");
}
{
    var c = NewGates((24, 2536)); c.BarrierY = -6; c.Reset();   // STALE barrier from a previous layout
    Move(c, 3000, 50, true);
    var r = Move(c, 3000, -3, true);       // y=-3 >= stale -6 -> treated as a normal move -> leaks up
    Check("reconfigure: a STALE barrier leaks (the regression auto-reconfigure prevents)",
        r.act == GuardAction.Pass, $"act={r.act}");
}

// ---- ANTI-TRAP SAFETY (C): an isolated screen's sole up-exit is always honoured, but stays owner-bound ----
// owner = a side screen at Y [0,1080); its only escape is the up-overlap X-span [2830,3000).
{
    var c = NewSafety((2830, 3000, 0, 1080));
    Move(c, 2900, 0, false);                       // origin on the owner screen, crossing CLOSED
    var r = Move(c, 2900, -15, false);             // deliberate up push
    Check("safety: sole-up-exit crosses even when crossing is CLOSED", r.act == GuardAction.Pass && r.onTop, $"act={r.act} onTop={r.onTop}");
}
{
    var c = NewSafety((2830, 3000, 0, 1080));
    Move(c, 2900, 0, false);
    var r = Move(c, 2960, -2, false);              // dx=60, dyUp=2 -> slide/jitter, not a deliberate push
    Check("safety: slide-jitter does NOT leak through the safety gate", r.act == GuardAction.Block && !r.onTop, $"act={r.act}");
}
{
    // OWNER-BOUND (fix #2): a different screen stacked in the SAME X-column (different Y) must NOT ride the gate.
    var c = NewSafety((2830, 3000, 0, 1080));
    Move(c, 2900, 2500, false);                    // origin Y=2500 -> below the owner's Y-band
    var r = Move(c, 2900, -5, false);
    Check("safety: a screen in the same X-column but different Y can't cross the safety gate", r.act == GuardAction.Block && !r.onTop, $"act={r.act}");
}
{
    // Independent of user gates: with the master toggle CLOSED, a user gate blocks but the safety gate passes.
    var c = new GuardCore { HasTop = true, BarrierY = 0,
        Gates = new List<(int, int)> { (24, 2536) },
        SafetyGates = new List<(int, int, int, int)> { (2830, 3000, 0, 1080) } };
    c.Reset();
    Move(c, 2900, 0, false); var s = Move(c, 2900, -15, false);
    var c2 = new GuardCore { HasTop = true, BarrierY = 0,
        Gates = new List<(int, int)> { (24, 2536) },
        SafetyGates = new List<(int, int, int, int)> { (2830, 3000, 0, 1080) } };
    c2.Reset();
    Move(c2, 1280, 0, false); var u = Move(c2, 1280, -15, false);
    Check("safety: independent of user gates (safety passes, user gate blocks, toggle closed)",
        s.act == GuardAction.Pass && u.act == GuardAction.Block, $"safety={s.act} user={u.act}");
}

// ---- ANTI-TRAP TOPOLOGY (B + C feed): pure detection from rects ----
{
    // Real bug: left, MAIN, right (270px gap from MAIN), top. Only the right screen is isolated.
    var mons = new List<(int, int, int, int)>
    {
        (-1920, 0, 0, 1080),     // 0 left
        (0, 0, 2560, 1440),      // 1 MAIN
        (2830, 0, 4480, 1080),   // 2 right (gap: 2830 vs MAIN's 2560)
        (-450, -1440, 2990, 0),  // 3 top
    };
    var (warn, gates) = GuardCore.AntiTrap(mons, new HashSet<int> { 3 });
    Check("topology: the 270px-gap right screen is the only isolated one",
        warn.Count == 1 && warn[0] == 2, $"warn=[{string.Join(",", warn)}]");
    Check("topology: its safety gate is clipped to the up-overlap, owner-bound",
        gates.Count == 1 && gates[0] == (2830, 2990, 0, 1080), $"gates=[{string.Join(";", gates)}]");
}
{
    var mons = new List<(int, int, int, int)>
    { (-1920, 0, 0, 1080), (0, 0, 2560, 1440), (2560, 0, 4480, 1080), (-450, -1440, 2990, 0) };
    var (warn, gates) = GuardCore.AntiTrap(mons, new HashSet<int> { 3 });
    Check("topology: edge-touching right screen is NOT isolated", warn.Count == 0 && gates.Count == 0, $"warn=[{string.Join(",", warn)}]");
}
{
    // SeamTol (fix #1): a 1px seam (mixed-DPI rounding) is treated as aligned, NOT isolated.
    var mons = new List<(int, int, int, int)>
    { (-1920, 0, 0, 1080), (0, 0, 2560, 1440), (2561, 0, 4480, 1080), (-450, -1440, 2990, 0) };
    var (warn, _) = GuardCore.AntiTrap(mons, new HashSet<int> { 3 });
    Check("topology: a 1px seam is tolerated (SeamTol) -> NOT isolated", warn.Count == 0, $"warn=[{string.Join(",", warn)}]");
}
{
    // Fully-islanded screen (no neighbour any side, no top above): warns but gets NO gate (would gate into a void).
    var mons = new List<(int, int, int, int)>
    { (-1920, 0, 0, 1080), (0, 0, 2560, 1440), (5000, 5000, 6000, 6000), (-450, -1440, 2990, 0) };
    var (warn, gates) = GuardCore.AntiTrap(mons, new HashSet<int> { 3 });
    Check("topology: a four-sided island warns but yields NO safety gate",
        warn.Contains(2) && gates.Count == 0, $"warn=[{string.Join(",", warn)}] gates={gates.Count}");
}
{
    var a = (0, 0, 100, 100); var b = (100, 100, 200, 200);   // touch only at the corner
    Check("topology: zero-width corner touch is NOT adjacency", !GuardCore.Adjacent(a, b, Dir.Right) && !GuardCore.Adjacent(a, b, Dir.Down));
}
{
    var a = (0, 0, 100, 100); var b = (100, 50, 200, 300);    // share [50,100) vertically
    Check("topology: partial vertical overlap is Right-adjacency", GuardCore.Adjacent(a, b, Dir.Right));
}
{
    var mons = new List<(int, int, int, int)> { (0, 0, 2560, 1440), (-450, -1440, 2990, 0) };
    var (warn, _) = GuardCore.AntiTrap(mons, new HashSet<int> { 1 });
    Check("topology: a top-set screen is never in warn/gates", !warn.Contains(1), $"warn=[{string.Join(",", warn)}]");
}
{
    Check("topology: IsTop classifies an above-origin monitor",
        GuardCore.IsTop((-450, -1440, 2990, 0)) && !GuardCore.IsTop((0, 0, 2560, 1440)));
}

// ---- DESCENT ROUTING (opt-in): clamp a descent's exit X back onto the linked bottom monitor ----
// Layout: wide TOP (-440,-1440,3000,0); MAIN landing (0,0,2560,1440); right side screen (2560,0,4480,1080) sits
// under the top's overhang (X 2560..3000). Barrier = top.B = 0. Route: Top=TOP, Landing=MAIN.
var TOP = (-440, -1440, 3000, 0);
var MAINrect = (0, 0, 2560, 1440);
var RIGHT = (2560, 0, 4480, 1080);

// A core that is already OnTop (cross up through MAIN's column first), with an explicit monitor set.
GuardCore OnTopCore(((int L, int T, int R, int B) Top, (int L, int T, int R, int B) Landing) route,
                    params (int L, int T, int R, int B)[] monitors)
{
    var c = new GuardCore { HasTop = true, BarrierY = 0, DescentRouting = true,
        DescentRoutes = new List<((int, int, int, int), (int, int, int, int))> { route },
        Monitors = monitors.ToList(),
        Gates = new List<(int Min, int Max)> { (24, 2536) } };
    c.Reset();
    Move(c, 1280, 0, true);
    Move(c, 1280, -15, true);   // deliberate cross -> OnTop
    return c;
}

// DescentRouting_OverhangIntoSide_ClampsXToLinkedLandingEdge — exit on the wrong side screen clamps to MAIN.R-1=2559.
{
    var c = OnTopCore((TOP, MAINrect), TOP, MAINrect, RIGHT);
    Move(c, 2800, -100, true);             // roam over the overhang (above the right screen)
    var r = Move(c, 2800, 10, true);       // descend: lands on RIGHT (X 2800) instead of MAIN
    Check("DescentRouting_OverhangIntoSide_ClampsXToLinkedLandingEdge",
        r.act == GuardAction.Block && r.bx == 2559 && r.by == 10 && !r.onTop,
        $"act={r.act} bx={r.bx} by={r.by} onTop={r.onTop}");
}
// Mirror: LEFT-wide top, RIGHT screen is the landing, overhang spills onto a left screen -> clamp to landing.L=0.
{
    var TOPl = (-2000, -1440, 1000, 0);
    var RIGHTl = (0, 0, 1000, 1080);       // landing
    var LEFTl = (-2000, 0, 0, 1080);       // wrong side under the overhang
    var c = new GuardCore { HasTop = true, BarrierY = 0, DescentRouting = true,
        DescentRoutes = new List<((int, int, int, int), (int, int, int, int))> { (TOPl, RIGHTl) },
        Monitors = new List<(int, int, int, int)> { TOPl, RIGHTl, LEFTl },
        Gates = new List<(int Min, int Max)> { (24, 976) } };
    c.Reset();
    Move(c, 500, 0, true); Move(c, 500, -15, true);   // OnTop
    Move(c, -1000, -100, true);            // roam over the left overhang
    var r = Move(c, -1000, 10, true);      // descend onto LEFT
    Check("DescentRouting_OverhangIntoSide_ClampsXToLinkedLandingEdge (mirror left)",
        r.act == GuardAction.Block && r.bx == 0 && !r.onTop, $"act={r.act} bx={r.bx} onTop={r.onTop}");
}
// DescentRouting_Disabled_PreservesUngatedDescent — scenario 9 shape with routing off.
{
    var c = new GuardCore { HasTop = true, BarrierY = 0, DescentRouting = false,
        DescentRoutes = new List<((int, int, int, int), (int, int, int, int))> { (TOP, MAINrect) },
        Monitors = new List<(int, int, int, int)> { TOP, MAINrect, RIGHT },
        Gates = new List<(int Min, int Max)> { (24, 2536) } };
    c.Reset();
    Move(c, 1280, 0, true); Move(c, 1280, -15, true);  // OnTop
    Move(c, 2800, -100, true);
    var r = Move(c, 2800, 10, true);        // descends freely (ungated)
    Check("DescentRouting_Disabled_PreservesUngatedDescent",
        r.act == GuardAction.Pass && !r.onTop, $"act={r.act} onTop={r.onTop}");
}
// DescentRouting_WithinLinkedLanding_PassesUnchanged — a descent already inside MAIN is not clamped.
{
    var c = OnTopCore((TOP, MAINrect), TOP, MAINrect, RIGHT);
    Move(c, 1500, -100, true);
    var r = Move(c, 1500, 10, true);        // lands inside MAIN
    Check("DescentRouting_WithinLinkedLanding_PassesUnchanged",
        r.act == GuardAction.Pass && !r.onTop, $"act={r.act} onTop={r.onTop}");
}
// DescentRouting_Void_PassesUnchanged — exit over no present monitor (cond7 false) -> ungated, fail-safe.
{
    var c = OnTopCore((TOP, MAINrect), TOP, MAINrect);   // RIGHT deliberately absent -> X 2800 below TOP is a void
    Move(c, 2800, -100, true);
    var r = Move(c, 2800, 10, true);        // exit at (2800,10): not in MAIN, not in any monitor -> void
    Check("DescentRouting_Void_PassesUnchanged",
        r.act == GuardAction.Pass && !r.onTop, $"act={r.act} onTop={r.onTop}");
}
// DescentRouting_LateralRoam_SelectsCurrentTopRoute — origin inside TOPa picks route A, not B.
{
    var TOPa = (0, -1440, 1200, 0); var LANDa = (0, 0, 1000, 1080);
    var TOPb = (1200, -1440, 2400, 0); var LANDb = (1400, 0, 2400, 1080);
    var WRONGa = (1000, 0, 1200, 1080);   // wrong screen under TOPa's right overhang
    var c = new GuardCore { HasTop = true, BarrierY = 0, DescentRouting = true,
        DescentRoutes = new List<((int, int, int, int), (int, int, int, int))> { (TOPa, LANDa), (TOPb, LANDb) },
        Monitors = new List<(int, int, int, int)> { TOPa, LANDa, TOPb, LANDb, WRONGa },
        Gates = new List<(int Min, int Max)> { (24, 976) } };
    c.Reset();
    Move(c, 500, 0, true); Move(c, 500, -15, true);   // OnTop via LANDa column
    Move(c, 1100, -100, true);              // roam to above TOPa's right overhang (origin inside TOPa)
    var r = Move(c, 1100, 10, true);        // descend onto WRONGa -> route A clamps to LANDa R-1 = 999
    Check("DescentRouting_LateralRoam_SelectsCurrentTopRoute",
        r.act == GuardAction.Block && r.bx == 999 && !r.onTop, $"act={r.act} bx={r.bx}");
}
// DescentRouting_MixedTopBottomEdges_UsesLocalTopEdge — clamp keys off the route's local Top.B, not global BarrierY,
// AND clearing OnTop is gated on the GLOBAL barrier (a shallow top's route must NOT strip free-roam above the line).
// Geometry: a deeper top elsewhere makes BarrierY = 0 while TOPshallow's local B = -700 (strictly above the barrier).
{
    var TOPshallow = (2560, -1440, 3000, -700);   // shallow top: local B = -700, strictly ABOVE BarrierY (0)
    var LAND2 = (2560, -700, 2900, 1080);         // landing starts at the shallow top's local edge
    var WRONG2 = (2900, -700, 3400, 1080);        // wrong screen to the right under the shallow top's overhang
    var c = new GuardCore { HasTop = true, BarrierY = 0, DescentRouting = true,
        DescentRoutes = new List<((int, int, int, int), (int, int, int, int))> { (TOPshallow, LAND2) },
        Monitors = new List<(int, int, int, int)> { TOPshallow, LAND2, WRONG2 },
        Gates = new List<(int Min, int Max)> { (2580, 2980) } };
    c.Reset();
    Move(c, 2700, -1000, true);             // first move -> OnTop (y < BarrierY)
    Move(c, 2950, -900, true);              // roam above, origin still in TOPshallow (Y -900 >= -1440)
    // Descend onto WRONG2 at y=-690 (past the local edge -700, but STILL ABOVE the global barrier 0):
    // X clamps to LAND2.R-1 = 2899, and OnTop must STAY TRUE because by (-690) < BarrierY (0).
    var r = Move(c, 2950, -690, true);
    Check("DescentRouting_MixedTopBottomEdges_UsesLocalTopEdge (clamps X, keeps OnTop above barrier)",
        r.act == GuardAction.Block && r.bx == 2899 && r.onTop, $"act={r.act} bx={r.bx} onTop={r.onTop}");
    // Follow-up: a move just above the barrier must NOT be blocked/warped — free roam is intact.
    var up = Move(c, 2900, -5, true);
    Check("DescentRouting_MixedTopBottomEdges: free roam intact above the barrier after a shallow-top route",
        up.act == GuardAction.Pass && up.onTop, $"act={up.act} onTop={up.onTop}");
}
// Deep top (Top.B == BarrierY): descending past it clamps X and DOES clear OnTop (by >= BarrierY).
{
    var TOPdeep = (2560, -700, 3000, 0);      // local B = 0 == BarrierY
    var LANDd = (2560, 0, 2900, 1080);
    var WRONGd = (2900, 0, 3400, 1080);
    var c = new GuardCore { HasTop = true, BarrierY = 0, DescentRouting = true,
        DescentRoutes = new List<((int, int, int, int), (int, int, int, int))> { (TOPdeep, LANDd) },
        Monitors = new List<(int, int, int, int)> { TOPdeep, LANDd, WRONGd },
        Gates = new List<(int Min, int Max)> { (2580, 2980) } };
    c.Reset();
    Move(c, 2700, 0, true); Move(c, 2700, -15, true);  // OnTop
    Move(c, 2950, -300, true);
    var r = Move(c, 2950, 10, true);          // descend below the barrier -> clamp to LANDd.R-1 = 2899, OnTop cleared
    Check("DescentRouting_DeepTop_ClampsX_AndClearsOnTopBelowBarrier",
        r.act == GuardAction.Block && r.bx == 2899 && !r.onTop, $"act={r.act} bx={r.bx} onTop={r.onTop}");
}
// DescentRouting_AmbiguousMultipleFromLinks_PassesUnchanged — the tray emits no route for an ambiguous top; with no
// route the firing loop is a no-op and descent stays ungated.
{
    var c = new GuardCore { HasTop = true, BarrierY = 0, DescentRouting = true,
        DescentRoutes = new List<((int, int, int, int), (int, int, int, int))>(),   // ambiguity guard dropped it
        Monitors = new List<(int, int, int, int)> { TOP, MAINrect, RIGHT },
        Gates = new List<(int Min, int Max)> { (24, 2536) } };
    c.Reset();
    Move(c, 1280, 0, true); Move(c, 1280, -15, true);  // OnTop
    Move(c, 2800, -100, true);
    var r = Move(c, 2800, 10, true);
    Check("DescentRouting_AmbiguousMultipleFromLinks_PassesUnchanged",
        r.act == GuardAction.Pass && !r.onTop, $"act={r.act}");
}
// DescentRouting_ConfineOverridesRoute — game mode's early return wins; the route is never consulted.
{
    var c = new GuardCore { HasTop = true, BarrierY = 0, DescentRouting = true, Confine = true,
        DescentRoutes = new List<((int, int, int, int), (int, int, int, int))> { (TOP, MAINrect) },
        Monitors = new List<(int, int, int, int)> { MAINrect, RIGHT } };
    c.Reset();
    Move(c, 2800, 500, true);                // baseline on RIGHT
    var r = Move(c, 4500, 500, true);        // try to leave RIGHT (X 2560..4480) -> confine blocks; route untouched
    Check("DescentRouting_ConfineOverridesRoute",
        r.act == GuardAction.Block, $"act={r.act}");
}
// DescentRouting_SideFromDevice_NotBelowTop_NoRoute (SF-1) — a From=side link whose Landing.T < Top.B emits NO
// route, so a center descent is NOT yanked sideways. Emulate the tray's emission rule (Landing.T >= Top.B) here.
{
    var sideLanding = (2560, -200, 4480, 880);       // overlaps the top's Y -> Landing.T (-200) < Top.B (0)
    bool emitted = sideLanding.Item2 >= TOP.Item4;   // the tray's SF-1 test
    var routes = new List<((int, int, int, int), (int, int, int, int))>();
    if (emitted) routes.Add((TOP, sideLanding));
    var c = new GuardCore { HasTop = true, BarrierY = 0, DescentRouting = true, DescentRoutes = routes,
        Monitors = new List<(int, int, int, int)> { TOP, MAINrect },
        Gates = new List<(int Min, int Max)> { (24, 2536) } };
    c.Reset();
    Move(c, 1280, -200, true);               // first move -> OnTop (y < BarrierY)
    Move(c, 1280, -100, true);
    var r = Move(c, 1280, 10, true);         // a CENTER descent into MAIN
    Check("DescentRouting_SideFromDevice_NotBelowTop_NoRoute",
        routes.Count == 0 && r.act == GuardAction.Pass && !r.onTop, $"routes={routes.Count} act={r.act}");
}

// ---- DESCENT ROUTE DERIVATION (pure GuardCore.DeriveRoutes): the tray's emission guards, unit-tested ----
{
    // Standard valid link: MAIN (from) -> TOP (to). TOP is a top device; MAIN sits below it and overlaps in X.
    var rects = new Dictionary<string, (int, int, int, int)>
    {
        ["MAIN"] = (0, 0, 2560, 1440),
        ["TOP"]  = (-440, -1440, 3000, 0),
    };
    var routes = GuardCore.DeriveRoutes(new[] { ("MAIN", "TOP") }, rects, new HashSet<string> { "TOP" });
    Check("derive: a valid below-and-overlapping link emits one route",
        routes.Count == 1 && routes[0].Top == (-440, -1440, 3000, 0) && routes[0].Landing == (0, 0, 2560, 1440),
        $"routes={routes.Count}");
}
{
    // X-OVERLAP guard (blocker): a link whose From sits entirely to the side of the top (no X-overlap) emits NO route,
    // so an overhang descent can't teleport the cursor across the horizontal gap.
    var rects = new Dictionary<string, (int, int, int, int)>
    {
        ["FAR"] = (5000, 0, 6000, 1080),       // far to the right of TOP -> no X-overlap
        ["TOP"] = (-440, -1440, 3000, 0),
    };
    var routes = GuardCore.DeriveRoutes(new[] { ("FAR", "TOP") }, rects, new HashSet<string> { "TOP" });
    Check("derive: a non-X-overlapping link emits NO route (no teleport across the gap)",
        routes.Count == 0, $"routes={routes.Count}");
}
{
    // SF-1 vertical guard: a From=side link whose Landing.T < Top.B (overlaps the top's Y band) emits NO route.
    var rects = new Dictionary<string, (int, int, int, int)>
    {
        ["SIDE"] = (2560, -200, 4480, 880),    // Landing.T (-200) < Top.B (0)
        ["TOP"]  = (-440, -1440, 3000, 0),
    };
    var routes = GuardCore.DeriveRoutes(new[] { ("SIDE", "TOP") }, rects, new HashSet<string> { "TOP" });
    Check("derive: a not-genuinely-below landing (Landing.T < Top.B) emits NO route", routes.Count == 0, $"routes={routes.Count}");
}
{
    // AMBIGUITY guard: a top with TWO distinct FromDevices emits NO route (no derivable intended landing).
    var rects = new Dictionary<string, (int, int, int, int)>
    {
        ["A"]   = (0, 0, 1500, 1440),
        ["B"]   = (1500, 0, 3000, 1440),
        ["TOP"] = (-440, -1440, 3000, 0),
    };
    var routes = GuardCore.DeriveRoutes(new[] { ("A", "TOP"), ("B", "TOP") }, rects, new HashSet<string> { "TOP" });
    Check("derive: an ambiguous top (multiple distinct FromDevices) emits NO route", routes.Count == 0, $"routes={routes.Count}");
}
{
    // A link whose target is NOT a top device is ignored.
    var rects = new Dictionary<string, (int, int, int, int)>
    {
        ["MAIN"] = (0, 0, 2560, 1440),
        ["TOP"]  = (-440, -1440, 3000, 0),
    };
    var routes = GuardCore.DeriveRoutes(new[] { ("MAIN", "TOP") }, rects, new HashSet<string>());   // TOP not in top-set
    Check("derive: a link whose target is not a top device emits NO route", routes.Count == 0, $"routes={routes.Count}");
}

// ---- WALL-OFF (Forbidden-rect exclusion): the cursor may never enter an excluded display ----
// Dummy under test: (2560,0,3360,600) — an 800x600 headless screen to the right of MAIN.
var DUMMY = (2560, 0, 3360, 600);

// 1) ENTRY-SIDE large delta (binding fix #1): Last well inside MAIN, target deep on the FAR side of the dummy.
//    Nearest-edge would tunnel to bx=3360 (the void side) and re-lose the cursor; entry-side must exit LEFT.
{
    var c = NewForbidden(DUMMY);
    Move(c, 2500, 300, true);                     // last position inside MAIN, LEFT of the dummy
    var r = Move(c, 3340, 300, true);             // big flick deep into the dummy (near its far-right wall)
    Check("wall-off: large-delta entry from left clamps to LEFT edge (no far-side tunnel)",
        r.act == GuardAction.Block && r.bx == 2559 && r.by == 300,
        $"act={r.act} bx={r.bx} by={r.by}  (expected Block bx=2559)");
}

// 1a) DIAGONAL entry (binding fix #1, segment-entry): Last outside on BOTH axes. The exit edge is the wall the
//     MOVE SEGMENT crosses FIRST, NOT the larger-delta axis. forbidden=(100,100,200,200), Last=(0,99) (left+above),
//     target=(150,150). dx=150 dominates dy=51, but the segment reaches the TOP plane (t=0.0196) long before the
//     LEFT plane (t=0.667), so the cursor must exit the TOP edge: bx==150 (X kept), by==99 (T-1). Delta-dominance
//     would have wrongly exited LEFT (bx==99).
{
    var c = NewForbidden((100, 100, 200, 200));
    Move(c, 0, 99, true);                         // last position diagonally outside: LEFT and ABOVE
    var r = Move(c, 150, 150, true);              // dive into the rect; TOP edge is crossed first
    Check("wall-off diagonal: first-crossed edge is TOP (bx kept, by=T-1) despite larger dx",
        r.act == GuardAction.Block && r.bx == 150 && r.by == 99,
        $"act={r.act} bx={r.bx} by={r.by}  (expected Block bx=150 by=99)");
}

// 1b) DIAGONAL mirror "shallow-left then dive": Last=(99,0) (left+above) into the same rect with target (150,150).
//     Here dy=150 dominates dx=51, but the segment reaches the LEFT plane (t=0.0196) first, so the cursor exits LEFT:
//     bx==99 (L-1), by==150 (Y kept). Proves the choice is segment-entry, not delta-dominance, in the mirror case.
{
    var c = NewForbidden((100, 100, 200, 200));
    Move(c, 99, 0, true);                         // last position diagonally outside: LEFT and ABOVE
    var r = Move(c, 150, 150, true);              // dive in; LEFT edge is crossed first this time
    Check("wall-off diagonal mirror: first-crossed edge is LEFT (by kept, bx=L-1) despite larger dy",
        r.act == GuardAction.Block && r.bx == 99 && r.by == 150,
        $"act={r.act} bx={r.bx} by={r.by}  (expected Block bx=99 by=150)");
}

// 2) Blocks entry from the LEFT, keeping the orthogonal (Y) axis.
{
    var c = NewForbidden(DUMMY);
    Move(c, 2500, 250, true);
    var r = Move(c, 2600, 250, true);             // crosses the left seam into the dummy
    Check("wall-off: entry from left -> clamp X to L-1, keep Y",
        r.act == GuardAction.Block && r.bx == 2559 && r.by == 250, $"act={r.act} bx={r.bx} by={r.by}");
}

// 3) Blocks entry from the TOP, keeping the orthogonal (X) axis.
//    Place the dummy ABOVE so a downward move enters across its top edge.
{
    var TOPDUMMY = (2560, -600, 3360, 0);
    var c = NewForbidden(TOPDUMMY);
    Move(c, 2900, -650, true);                    // last position ABOVE the dummy
    var r = Move(c, 2900, -300, true);            // move down into the dummy across its top edge
    Check("wall-off: entry from top -> clamp Y to T-1, keep X",
        r.act == GuardAction.Block && r.by == -601 && r.bx == 2900, $"act={r.act} bx={r.bx} by={r.by}");
}

// 4) Slide ALONG the seam on the real side (just OUTSIDE) -> Pass, no clamp, no jitter.
{
    var c = NewForbidden(DUMMY);
    Move(c, 2559, 100, true);
    var a = Move(c, 2559, 200, true);
    var b = Move(c, 2559, 300, true);
    var d = Move(c, 2559, 400, true);
    Check("wall-off: sliding along the seam OUTSIDE passes untouched (no sticking)",
        a.act == GuardAction.Pass && b.act == GuardAction.Pass && d.act == GuardAction.Pass
        && a.bx == 2559 && d.by == 400, $"a={a.act} b={b.act} d={d.act}");
}

// 5) Empty Forbidden list is inert (true no-op / OFF safety regression guard).
{
    var c = NewForbidden();                       // no forbidden rects, no top, no confine
    var r = Move(c, 3000, 300, true);             // anywhere
    Check("wall-off: empty list is inert (Pass, no clamp)",
        r.act == GuardAction.Pass && r.bx == 3000 && r.by == 300, $"act={r.act} bx={r.bx} by={r.by}");
}

// 6) (0) runs BEFORE the barrier: with a top present, a move into the dummy is walled by step 0,
//    and a normal gate cross elsewhere still works.
{
    var c = new GuardCore { HasTop = true, BarrierY = 0,
        Gates = new List<(int Min, int Max)> { (24, 2536) },
        Forbidden = new List<(int, int, int, int)> { DUMMY } };
    c.Reset();
    Move(c, 2500, 300, true);
    var wall = Move(c, 2600, 300, true);          // into the dummy -> walled by (0), never reaches the barrier
    Check("wall-off: runs before the barrier (dummy entry is walled, not gated)",
        wall.act == GuardAction.Block && wall.bx == 2559 && wall.by == 300 && !wall.onTop,
        $"act={wall.act} bx={wall.bx} by={wall.by}");
    var c2 = new GuardCore { HasTop = true, BarrierY = 0,
        Gates = new List<(int Min, int Max)> { (24, 2536) },
        Forbidden = new List<(int, int, int, int)> { DUMMY } };
    c2.Reset();
    Move(c2, 1280, 0, true);
    var cross = Move(c2, 1280, -15, true);        // a normal deliberate gate cross still works
    Check("wall-off: a normal gate cross is unaffected by the wall",
        cross.act == GuardAction.Pass && cross.onTop, $"act={cross.act} onTop={cross.onTop}");
}

// 7) Composes with Confine: (0) runs before the confine branch, so a move toward the dummy is walled first.
{
    var c = new GuardCore { HasTop = false, Confine = true,
        Monitors = new List<(int, int, int, int)> { (0, 0, 2560, 1440) },
        Forbidden = new List<(int, int, int, int)> { DUMMY } };
    c.Reset();
    Move(c, 2500, 300, true);                     // baseline on MAIN
    var r = Move(c, 2600, 300, true);             // toward the dummy -> step 0 walls before confine
    Check("wall-off: composes with Confine (walled by step 0 first)",
        r.act == GuardAction.Block && r.bx == 2559 && r.by == 300, $"act={r.act} bx={r.bx} by={r.by}");
}

// 8) No top, no confine — still walls (proves (0) runs before the !HasTop && !Confine bail-out), and NeedsProcessing.
{
    var c = NewForbidden(DUMMY);
    Check("wall-off: NeedsProcessing is true with only Forbidden set", c.NeedsProcessing, "NeedsProcessing");
    Move(c, 2500, 300, true);
    var r = Move(c, 2600, 300, true);
    Check("wall-off: no top + no confine still walls the dummy",
        r.act == GuardAction.Block && r.bx == 2559, $"act={r.act} bx={r.bx}");
}
{
    var c = new GuardCore();                      // nothing configured
    Check("wall-off: NeedsProcessing is false when nothing is set (hook stays out)", !c.NeedsProcessing, "NeedsProcessing");
}

// 9) Adjacent/overlapping forbidden rects (binding fix #3): a single-rect exit could land inside a neighbour;
//    the bounded loop must leave the point OUTSIDE ALL forbidden rects. Two abutting dummies span X 2560..4160.
{
    var D1 = (2560, 0, 3360, 600);
    var D2 = (3360, 0, 4160, 600);                // shares the seam at x=3360 with D1
    var c = NewForbidden(D1, D2);
    Move(c, 2500, 300, true);                     // last inside MAIN, left of both
    var r = Move(c, 3400, 300, true);             // target inside D2; entry-side from the left -> exit left of D1
    bool outsideBoth = !(r.bx >= D1.Item1 && r.bx < D1.Item3 && r.by >= D1.Item2 && r.by < D1.Item4)
                    && !(r.bx >= D2.Item1 && r.bx < D2.Item3 && r.by >= D2.Item2 && r.by < D2.Item4);
    Check("wall-off: adjacent dummies — clamped point is outside ALL forbidden rects",
        r.act == GuardAction.Block && outsideBoth && r.bx == 2559 && r.by == 300,
        $"act={r.act} bx={r.bx} by={r.by} outsideBoth={outsideBoth}");
}

// 9-FIXA-1) ROUND-3 FIX A — cx repro: adjacent pair, NO HaveLast. D1=(0,0,100,100), D2=(100,0,200,100); target
//    (99,50) lands inside D1. The OLD per-rect nearest-edge fallback exited D1 RIGHT to x=100 (inside D2), then D2
//    LEFT back to x=99 (inside D1) and the bounded loop ended STILL INSIDE D2. The connected-region exit must march
//    the chosen direction past the WHOLE component (D1∪D2 = X[0,200)) -> x=200, OUTSIDE BOTH. (No Last => first move.)
{
    var D1 = (0, 0, 100, 100);
    var D2 = (100, 0, 200, 100);
    var c = NewForbidden(D1, D2);
    var r = Move(c, 99, 50, true);                // first move (no Last) lands inside D1, adjacent to D2
    bool outsideBoth = !(r.bx >= D1.Item1 && r.bx < D1.Item3 && r.by >= D1.Item2 && r.by < D1.Item4)
                    && !(r.bx >= D2.Item1 && r.bx < D2.Item3 && r.by >= D2.Item2 && r.by < D2.Item4);
    Check("wall-off FIX A: no-HaveLast into adjacent pair -> outside BOTH (cx repro: not stuck at x=100)",
        r.act == GuardAction.Block && outsideBoth && r.bx == 200 && r.by == 50,
        $"act={r.act} bx={r.bx} by={r.by} outsideBoth={outsideBoth}");
}

// 9-FIXA-2) ROUND-3 FIX A — Last INSIDE a neighbouring forbidden rect, move into the OTHER. Last=(50,50) inside D1,
//    target (150,50) inside D2. Last is inside the connected region (no usable entry direction) -> nearest-edge of the
//    HIT rect (D2: dl=50,dr=50,dt=50,db=50 -> tie LEFT) would re-enter D1; the region march to regL-1 = -1 leaves the
//    point OUTSIDE BOTH. Either way the GUARANTEE (outside every rect) must hold and it must terminate.
{
    var D1 = (0, 0, 100, 100);
    var D2 = (100, 0, 200, 100);
    var c = NewForbidden(D1, D2);
    Move(c, 50, 50, true);                        // Last inside D1 (a neighbour of D2)
    var r = Move(c, 150, 50, true);               // move into D2
    bool outsideBoth = !(r.bx >= D1.Item1 && r.bx < D1.Item3 && r.by >= D1.Item2 && r.by < D1.Item4)
                    && !(r.bx >= D2.Item1 && r.bx < D2.Item3 && r.by >= D2.Item2 && r.by < D2.Item4);
    Check("wall-off FIX A: Last inside neighbour, move into the other -> outside BOTH (no oscillation)",
        r.act == GuardAction.Block && outsideBoth, $"act={r.act} bx={r.bx} by={r.by} outsideBoth={outsideBoth}");
}

// 9-FIXA-3) ROUND-3 FIX A — three-rect chain, deep tunnel from the left. D1,D2,D3 abut to span X[0,300). Last left of
//    all, a fast flick deep into D3: entry-side exits LEFT to the whole region's regL-1 = -1, never re-captured by D2/D1.
{
    var D1 = (0, 0, 100, 100);
    var D2 = (100, 0, 200, 100);
    var D3 = (200, 0, 300, 100);
    var c = NewForbidden(D1, D2, D3);
    Move(c, -10, 50, true);                       // Last left of the whole chain
    var r = Move(c, 280, 50, true);               // deep flick into D3
    bool outsideAll = !(r.bx >= 0 && r.bx < 300 && r.by >= 0 && r.by < 100);
    Check("wall-off FIX A: three-rect chain, deep tunnel from left -> exits LEFT past the whole region",
        r.act == GuardAction.Block && outsideAll && r.bx == -1 && r.by == 50,
        $"act={r.act} bx={r.bx} by={r.by} outsideAll={outsideAll}");
}

// 9-FIXC-1) ROUND-3 FIX C — boundary corner pixel: Last sits EXACTLY ON the rect's top-left INSIDE corner (100,100),
//    which is INSIDE the half-open rect [100,200)x[100,200), so there is NO usable entry direction (Last not outside
//    the region) and the nearest-edge fallback stands. Target (150,150): dl=dr=dt=db=50 -> the documented tie order
//    Left>Right>Top>Bottom picks LEFT. Exit to regL-1 == 99, Y kept == 150. (Guards the t-degenerate corner case.)
{
    var c = NewForbidden((100, 100, 200, 200));
    Move(c, 100, 100, true);                      // Last ON the TL corner pixel -> INSIDE the half-open rect
    var r = Move(c, 150, 150, true);
    Check("wall-off FIX C: Last on TL corner pixel (inside) -> nearest-edge tie picks LEFT, exit L-1",
        r.act == GuardAction.Block && r.bx == 99 && r.by == 150, $"act={r.act} bx={r.bx} by={r.by}");
}

// 9-FIXC-2) ROUND-3 FIX C — segment-entry t==1 INCLUSIVE bound, both-axes branch. Last=(0,50) is LEFT+ABOVE the rect
//    [100,200)x[100,200). Target (150,100) lands exactly on the TOP row: the Y (top) wall is reached at tY==1.0 EXACTLY
//    (validY only because the bound is inclusive, tY<=1), while tX=(100-0)/150=0.667. With both valid, useX = tX<=tY =
//    0.667<=1.0 -> true -> exit the LEFT wall: bx == L-1 == 99, by kept == 100. (Pins that t==1 is treated as a valid
//    crossing, not rejected by an exclusive bound.)
{
    var c = NewForbidden((100, 100, 200, 200));
    Move(c, 0, 50, true);                         // diagonally outside: LEFT and ABOVE
    var r = Move(c, 150, 100, true);              // target on the top row -> tY == 1.0 exactly (inclusive bound)
    Check("wall-off FIX C: t==1 inclusive (target on top row) -> both valid, tie to earlier-t LEFT wall",
        r.act == GuardAction.Block && r.bx == 99 && r.by == 100, $"act={r.act} bx={r.bx} by={r.by}");
}

// 9-FIXC-3) ROUND-3 FIX C — segment-entry t==0 / boundary-grazing case, both-axes branch. Last=(99,99) is LEFT+ABOVE
//    by exactly ONE px (the diagonal corner just outside the TL of [100,200)x[100,200)). The TOP and LEFT planes are
//    reached at the smallest possible positive t: tX=(100-99)/(150-99)=1/51, tY=(100-99)/(150-99)=1/51 -> EQUAL, so the
//    tie picks X (useX = tX<=tY). Exit LEFT: bx == L-1 == 99, by kept == 150. Guards the near-zero-t / equal-t corner.
{
    var c = NewForbidden((100, 100, 200, 200));
    Move(c, 99, 99, true);                        // one px diagonally outside the TL corner
    var r = Move(c, 150, 150, true);              // dive in; tX == tY (equal earliest crossing) -> tie picks X
    Check("wall-off FIX C: equal-earliest-t corner entry -> tie picks X, exit L-1",
        r.act == GuardAction.Block && r.bx == 99 && r.by == 150, $"act={r.act} bx={r.bx} by={r.by}");
}

// 10) Topology: an excluded device is dropped from the AntiTrap inputs, so it can't become a spurious gate/owner.
//     Mirrors existing test #10 — feed the DUMMY-DROPPED rect list and assert no spurious gate for the right screen.
{
    // left, MAIN, right (edge-touching MAIN), top, and a DUMMY abutting the right screen (56px-style seam).
    // With the dummy present it would give the right screen a side neighbour; WALL-OFF drops the dummy from
    // topology so AntiTrap is computed over the real screens only — and the right screen stays non-isolated
    // via its real seam with MAIN, with NO gate spuriously created by the dummy.
    var withDummy = new List<(int, int, int, int)>
    {
        (-1920, 0, 0, 1080),     // 0 left
        (0, 0, 2560, 1440),      // 1 MAIN
        (2560, 0, 4480, 1080),   // 2 right (touches MAIN at x=2560)
        (-450, -1440, 2990, 0),  // 3 top
        (4480, 0, 5280, 600),    // 4 DUMMY abutting the right screen
    };
    // Drop the excluded dummy (index 4) before deriving topology — what Configure() does with the two-list split.
    var dropped = withDummy.Where((m, i) => i != 4).ToList();
    var topIdx = new HashSet<int> { 3 };
    var (warn, gates) = GuardCore.AntiTrap(dropped, topIdx);
    Check("wall-off topology: excluded device absent -> no spurious isolation/gate; right screen unaffected",
        warn.Count == 0 && gates.Count == 0, $"warn=[{string.Join(",", warn)}] gates={gates.Count}");
}

// 11) Primary-sanitization (binding fix #2 / FIX 3): the primary display can never be walled off, and never leave
//     zero usable monitors. The logic now lives in the SHARED pure helper GuardCore.SanitizeExcluded — Configure()
//     AND these tests call THE SAME method (no duplicated list math). Keys are STABLE ids (FIX 4).
{
    // monitors: MAIN (primary) + a dummy. User (or an imported file) asks to exclude BOTH.
    var present = new List<string> { "MONITOR\\MAIN\\{guid}", "MONITOR\\DUMMY\\{guid}" };
    var primary = "MONITOR\\MAIN\\{guid}";
    var requested = new HashSet<string> { "MONITOR\\MAIN\\{guid}", "MONITOR\\DUMMY\\{guid}" };
    var excluded = GuardCore.SanitizeExcluded(present, primary, requested, out bool pruned);
    Check("wall-off safety: primary is never excluded; one usable monitor remains; prune flagged",
        excluded.Count == 1 && excluded[0] == "MONITOR\\DUMMY\\{guid}" && !excluded.Contains(primary) && pruned,
        $"excluded=[{string.Join(",", excluded)}] pruned={pruned}");
}
{
    // Keep-one rule: a single-monitor PC where the user asks to exclude that one (non-primary path edge): never zero.
    var present = new List<string> { "MONITOR\\ONLY\\{guid}" };
    var primary = "MONITOR\\PRIMARY-ELSEWHERE";    // primary not present (e.g. stale import) -> keep-one must still fire
    var requested = new HashSet<string> { "MONITOR\\ONLY\\{guid}" };
    var excluded = GuardCore.SanitizeExcluded(present, primary, requested, out bool pruned);
    Check("wall-off safety: never leaves zero usable monitors (keep-one)",
        excluded.Count == 0 && pruned, $"excluded={excluded.Count} pruned={pruned}");
}
{
    // A requested key that is ABSENT from the live layout (imported/stale id) is dormant — NOT a prune, and the
    // present, requested non-primary monitor is still excluded normally.
    var present = new List<string> { "MONITOR\\MAIN\\{guid}", "MONITOR\\DUMMY\\{guid}" };
    var primary = "MONITOR\\MAIN\\{guid}";
    var requested = new HashSet<string> { "MONITOR\\DUMMY\\{guid}", "MONITOR\\GONE\\{guid}" };
    var excluded = GuardCore.SanitizeExcluded(present, primary, requested, out bool pruned);
    Check("wall-off safety: an absent (imported) id is dormant -> not a prune; present dummy still excluded",
        excluded.Count == 1 && excluded[0] == "MONITOR\\DUMMY\\{guid}" && !pruned,
        $"excluded=[{string.Join(",", excluded)}] pruned={pruned}");
}

// 12) STABLE-KEY identity (FIX 4): the exclude set keys on MonitorInfo.StableId, NOT \\.\DISPLAYn, so the SAME
//     monitor stays excluded after a layout re-arrange renumbers DISPLAYn. Simulate two device lists with the SAME
//     StableId but DIFFERENT \\.\DISPLAYn ordering; the sanitize/match must still pick the right monitor by StableId.
{
    // A small stand-in for MonitorInfo's (Device, StableId) pairing — the production match keys on StableId.
    var primaryStable = "MONITOR\\MAIN-EDID\\{guid}";
    var dummyStable = "MONITOR\\DUMMY-EDID\\{guid}";
    var requested = new HashSet<string> { dummyStable };   // the user excluded the dummy by its STABLE id

    // Layout #1: dummy is \\.\DISPLAY2.   Layout #2 (after re-arrange): the SAME dummy is now \\.\DISPLAY4.
    var layout1 = new List<(string Device, string Stable)>
        { ("\\\\.\\DISPLAY1", primaryStable), ("\\\\.\\DISPLAY2", dummyStable) };
    var layout2 = new List<(string Device, string Stable)>
        { ("\\\\.\\DISPLAY3", primaryStable), ("\\\\.\\DISPLAY4", dummyStable) };

    // Configure() matches by StableId: excluded = SanitizeExcluded(presentStableIds, primaryStable, requested) then
    // map back to the monitor whose StableId is in the result.
    (string Device, string Stable)? ExcludedMonitor(List<(string Device, string Stable)> layout)
    {
        var present = layout.Select(m => m.Stable).ToList();
        var ex = new HashSet<string>(GuardCore.SanitizeExcluded(present, primaryStable, requested, out _));
        var hit = layout.Where(m => ex.Contains(m.Stable)).ToList();
        return hit.Count == 1 ? hit[0] : null;
    }

    var e1 = ExcludedMonitor(layout1);
    var e2 = ExcludedMonitor(layout2);
    Check("wall-off stable-key: the SAME dummy is excluded under both DISPLAYn orderings (identity survives renumber)",
        e1.HasValue && e2.HasValue
        && e1.Value.Stable == dummyStable && e2.Value.Stable == dummyStable
        && e1.Value.Device == "\\\\.\\DISPLAY2" && e2.Value.Device == "\\\\.\\DISPLAY4",
        $"e1={e1?.Device}/{e1?.Stable} e2={e2?.Device}/{e2?.Stable}");
}

// 13) ROUND-3 FIX B — DUPLICATE StableId disambiguation: two PHYSICALLY-DISTINCT monitors with the SAME raw EDID key
//     (identical panels) must get DISTINCT StableIds so excluding ONE excludes ONLY that one. GuardCore.DisambiguateKeys
//     is the pure rule MonitorInfo.All() feeds; here we feed present=[PRIMARY, DUP, DUP] and assert the two DUPs become
//     distinct keys and excluding one leaves the other usable. (cx probe: present=[PRIMARY,DUP,DUP], exclude DUP.)
{
    var primaryRaw = "MONITOR\\PRIMARY-EDID\\{guid}";
    var dupRaw = "MONITOR\\DUP-EDID\\{guid}";       // SAME raw key on two physical panels (identical EDID)
    // present, in display order, with each monitor's stable geometric attribute (Left, Top) for the deterministic suffix.
    var present = new List<(string Key, int Left, int Top)>
    {
        (primaryRaw, 0, 0),        // 0 PRIMARY
        (dupRaw, 2560, 0),         // 1 DUP-A (left=2560)
        (dupRaw, 3360, 0),         // 2 DUP-B (left=3360)
    };
    var keys = GuardCore.DisambiguateKeys(present);

    // The two identical-panel monitors now have DISTINCT keys; the unique primary is left untouched (no suffix).
    bool distinct = keys[1] != keys[2];
    bool primaryUntouched = keys[0] == primaryRaw;
    // Deterministic ordinal by ascending Left: DUP-A (2560) -> #0, DUP-B (3360) -> #1.
    bool deterministicOrder = keys[1] == dupRaw + "#0" && keys[2] == dupRaw + "#1";
    Check("wall-off FIX B: identical-EDID panels get DISTINCT StableIds (deterministic by Left); primary untouched",
        distinct && primaryUntouched && deterministicOrder,
        $"keys=[{string.Join(",", keys)}]");

    // Now the user excludes ONE physical DUP (DUP-A's disambiguated key). Configure()'s per-monitor match must wall
    // EXACTLY that one and leave the other DUP usable (the OLD raw-key HashSet collapse would have excluded BOTH).
    var primaryKey = keys[0];
    var requested = new HashSet<string> { keys[1] };   // exclude only DUP-A
    var excluded = new HashSet<string>(
        GuardCore.SanitizeExcluded(keys, primaryKey, requested, out bool pruned));
    // Match per-monitor by the disambiguated key (exactly what TrayApplicationContext.Configure does).
    int excludedCount = present.Select((m, i) => keys[i]).Count(k => excluded.Contains(k));
    bool dupAExcluded = excluded.Contains(keys[1]);
    bool dupBUsable = !excluded.Contains(keys[2]);
    Check("wall-off FIX B: excluding ONE identical panel excludes ONLY that one (the other stays usable)",
        excludedCount == 1 && dupAExcluded && dupBUsable && !pruned,
        $"excludedCount={excludedCount} dupAExcluded={dupAExcluded} dupBUsable={dupBUsable} pruned={pruned}");
}

// 14) STABLE-KEY identity for manual top monitors and UpLinks: the SAME physical monitors keep their rules when
//     Windows renumbers \\.\DISPLAYn. Production now stores ManualMonitors/UpLinks by StableId, just like excludes.
{
    var bottomStable = "MONITOR\\BOTTOM\\{guid}";
    var topStable = "MONITOR\\TOP\\{guid}";
    var requestedManual = new HashSet<string> { topStable };
    var requestedLinks = new List<(string From, string To)> { (bottomStable, topStable) };

    var layout1 = new List<(string Device, string Stable)>
        { ("\\\\.\\DISPLAY1", bottomStable), ("\\\\.\\DISPLAY4", topStable) };
    var layout2 = new List<(string Device, string Stable)>
        { ("\\\\.\\DISPLAY3", bottomStable), ("\\\\.\\DISPLAY2", topStable) };

    (string FromDevice, string ToDevice)? ResolveLink(List<(string Device, string Stable)> layout)
    {
        var present = layout.ToDictionary(m => m.Stable, m => m.Device);
        var topSet = new HashSet<string>(layout.Where(m => requestedManual.Contains(m.Stable)).Select(m => m.Stable));
        foreach (var link in requestedLinks)
            if (topSet.Contains(link.To) && present.ContainsKey(link.From) && present.ContainsKey(link.To))
                return (present[link.From], present[link.To]);
        return null;
    }

    var l1 = ResolveLink(layout1);
    var l2 = ResolveLink(layout2);
    Check("layout rules stable-key: ManualMonitors/UpLinks survive DISPLAYn renumber and target the same physical pair",
        l1.HasValue && l2.HasValue
        && l1.Value.FromDevice == "\\\\.\\DISPLAY1" && l1.Value.ToDevice == "\\\\.\\DISPLAY4"
        && l2.Value.FromDevice == "\\\\.\\DISPLAY3" && l2.Value.ToDevice == "\\\\.\\DISPLAY2",
        $"l1={l1?.FromDevice}->{l1?.ToDevice} l2={l2?.FromDevice}->{l2?.ToDevice}");
}

// 15) Stale StableIds are inert: absent ManualMonitors/UpLinks/Excludes do not match a different live monitor.
{
    var present = new List<string> { "MONITOR\\MAIN\\{guid}", "MONITOR\\TOP\\{guid}" };
    var primary = "MONITOR\\MAIN\\{guid}";
    var stale = "MONITOR\\OLD-DUMMY\\{guid}";
    var excluded = GuardCore.SanitizeExcluded(present, primary, new HashSet<string> { stale }, out bool pruned);

    var liveByStable = present.ToDictionary(k => k, k => k);
    var manual = new HashSet<string> { stale };
    var links = new List<(string From, string To)> { (primary, stale), (stale, "MONITOR\\TOP\\{guid}") };
    bool manualMatched = present.Any(manual.Contains);
    bool anyLinkMatched = links.Any(l => liveByStable.ContainsKey(l.From) && liveByStable.ContainsKey(l.To));

    Check("layout stale refs: absent stable ids are inert and never retarget another monitor",
        excluded.Count == 0 && !pruned && !manualMatched && !anyLinkMatched,
        $"excluded={excluded.Count} pruned={pruned} manualMatched={manualMatched} anyLinkMatched={anyLinkMatched}");
}

// 16) Migration: old \\.\DISPLAY-keyed manual/rule settings convert through the live device map; stale DISPLAY refs drop.
{
    var map = new Dictionary<string, string>
    {
        ["\\\\.\\DISPLAY1"] = "MONITOR\\MAIN\\{guid}",
        ["\\\\.\\DISPLAY4"] = "MONITOR\\TOP\\{guid}",
    };
    var oldManual = new List<string> { "\\\\.\\DISPLAY4", "\\\\.\\DISPLAY9" };
    var oldLinks = new List<(string From, string To)>
    {
        ("\\\\.\\DISPLAY1", "\\\\.\\DISPLAY4"),
        ("\\\\.\\DISPLAY1", "\\\\.\\DISPLAY9"),
    };
    var migrated = GuardCore.MigrateLayoutKeysToStable(oldManual, oldLinks, map);

    Check("layout migration: present DISPLAY keys convert to StableId and stale DISPLAY keys are dropped",
        migrated.ManualKeys.SequenceEqual(new[] { "MONITOR\\TOP\\{guid}" })
        && migrated.Links.Count == 1
        && migrated.Links[0] == ("MONITOR\\MAIN\\{guid}", "MONITOR\\TOP\\{guid}"),
        $"manual=[{string.Join(",", migrated.ManualKeys)}] links={migrated.Links.Count}");
}

// 17) Reset current layout config: clear manual picks and explicit links, return to AutoTop defaults.
{
    var reset = GuardCore.ResetLayoutConfig();
    Check("layout reset: returns to AutoTop and clears manual monitors plus explicit crossing rules",
        reset.Mode == "AutoTop" && reset.ManualKeys.Count == 0 && reset.Links.Count == 0,
        $"mode={reset.Mode} manual={reset.ManualKeys.Count} links={reset.Links.Count}");
}

// 18) Deterministic label: no DISPLAY name and no conflicting '#N'; includes ordinal, resolution, position, primary tag.
{
    var label1 = GuardCore.MonitorLabel(2, 2560, 1440, 0, 0, true, "[MAIN]");
    var label2 = GuardCore.MonitorLabel(2, 2560, 1440, 0, 0, true, "[MAIN]");
    Check("layout label: deterministic and not DISPLAY-number based",
        label1 == label2 && label1.Contains("2") && label1.Contains("2560x1440")
        && label1.Contains("@ 0,0") && label1.Contains("[MAIN]") && !label1.Contains("DISPLAY") && !label1.Contains("#"),
        label1);
}

// ---- localization parity: every key exists in BOTH languages (no cross-language fallback) ----
{
    var en = new HashSet<string>(Strings.EnglishKeys);
    var tr = new HashSet<string>(Strings.TurkishKeys);
    var missingInTr = en.Except(tr).ToList();
    var missingInEn = tr.Except(en).ToList();
    Check("i18n: EN and TR dictionaries have identical keys",
        missingInTr.Count == 0 && missingInEn.Count == 0,
        $"missingInTr=[{string.Join(",", missingInTr)}] missingInEn=[{string.Join(",", missingInEn)}]");
}
{
    // placeholder arity (fix #5): an interpolated string must carry the SAME {0}/{1}... in both languages,
    // else string.Format throws at runtime on the warning path. The parity test above checks keys only.
    bool ok = true; string bad = "";
    int Count(string s, int n) { var ph = "{" + n + "}"; return s == null ? 0 : (s.Length - s.Replace(ph, "").Length) / ph.Length; }
    foreach (var key in Strings.EnglishKeys)
    {
        var en = Strings.Raw("en", key); var tr = Strings.Raw("tr", key);
        for (int n = 0; n < 4; n++)
            if (Count(en, n) != Count(tr, n)) { ok = false; bad = $"{key} {{{n}}}"; break; }
        if (!ok) break;
    }
    Check("i18n: placeholder arity matches across EN and TR", ok, bad);
}

Console.WriteLine();
Console.WriteLine(fails == 0 ? "ALL TESTS PASSED" : $"{fails} TEST(S) FAILED");
Environment.Exit(fails == 0 ? 0 : 1);
