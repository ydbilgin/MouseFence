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
