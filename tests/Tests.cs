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

(GuardAction act, int by, bool onTop) Move(GuardCore c, int x, int y, bool gate)
{
    var a = c.Decide(x, y, gate, out _, out int by);
    return (a, by, c.OnTop);
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
