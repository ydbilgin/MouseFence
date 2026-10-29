namespace MouseFence;

public sealed partial class GuardCore
{
    /// <summary>
    /// Decide a NON-injected move to (x,y). Returns Pass or Block (caller clamps to bx,by). Mutates state.
    /// <paramref name="gatesEnabled"/> is the up-barrier master toggle: when false, no upward crossing is allowed.
    /// <paramref name="sideContain"/> is the side-containment toggle: when true a soft barrier blocks accidental
    /// drift out of the MAIN screen left/right (a deliberate horizontal push still crosses); when false the side is
    /// free (see <see cref="HasSides"/>).
    /// </summary>
    public GuardAction Decide(int x, int y, bool gatesEnabled, bool sideContain, out int bx, out int by)
    {
        _timedDecision = false;
        return DecideMove(x, y, gatesEnabled, sideContain, out bx, out by);
    }

    /// <summary>Production entry point: replace cached cursor state with this event's observed origin.
    /// An unknown origin never reuses a previous target or requested clamp (8cf37f8).</summary>
    public GuardAction Decide((int X, int Y)? origin, int x, int y, bool gatesEnabled, bool sideContain,
                              double eventSeconds, out int bx, out int by)
    {
        _timedDecision = true; _eventSeconds = eventSeconds;
        HaveLast = origin.HasValue;
        LastX = origin.GetValueOrDefault().X; LastY = origin.GetValueOrDefault().Y;
        OnTop = HaveLast && HasTop && LastY < BarrierY;
        return DecideMove(x, y, gatesEnabled, sideContain, out bx, out by);
    }

    private GuardAction DecideMove(int x, int y, bool gatesEnabled, bool sideContain, out int bx, out int by)
    {
        bx = x; by = y;
        _gatesEnabledForDecision = gatesEnabled;
        _sideContainForDecision = sideContain;

        // (0) Wall-off: the cursor may never ENTER an excluded display (a headless/dummy screen). Clamp the
        //     attempted target back to a legal edge FIRST, so every downstream rule (barrier, gates, confine,
        //     descent routing) only ever sees a position outside every forbidden rect. Runs even when there are no
        //     top monitors and game mode is off, so a dummy with no barrier still gets walled (NeedsProcessing).
        //     Only genuine incursions clamp: a target already outside every rect returns false here (no clamp,
        //     falls through), so sliding ALONG a seam on the real side is never touched -> no jitter.
        if (Forbidden.Count > 0 && ClampOutOfForbidden(ref bx, ref by))
            return FinishDecision(GuardAction.Block, x, y, ref bx, ref by);
        // from here on, (x,y) == (bx,by) is guaranteed outside every forbidden rect.

        if (!HasTop && !Confine && !HasSides && Transitions.Length == 0)
            return FinishDecision(GuardAction.Pass, x, y, ref bx, ref by);

        if (!HaveLast)
            return FinishDecision(GuardAction.Pass, x, y, ref bx, ref by);

        // Game mode: confine the cursor to the monitor it is currently on (overrides the up-barrier).
        if (Confine)
        {
            if (TryActiveMonitor(LastX, LastY, out var m))
            {
                if (x >= m.L && x < m.R && y >= m.T && y < m.B)
                    return FinishDecision(GuardAction.Pass, x, y, ref bx, ref by);
                bx = Math.Clamp(x, m.L, m.R - 1);
                by = Math.Clamp(y, m.T, m.B - 1);
                return FinishDecision(GuardAction.Block, x, y, ref bx, ref by);
            }
            return FinishDecision(GuardAction.Pass, x, y, ref bx, ref by);
        }

        // Schema-v4 directional graph. All geometry/rule/default work was compiled during Configure; this hot path
        // only intersects the movement segment with an already sorted array. It intentionally runs before the legacy
        // BarrierY/OnTop path so Down and side rules are first-class and a fast move cannot skip a blocked seam.
        var transition = MatchTransition(x, y, out bool inPassage);
        if (_arrivalContact != null && (!HaveLast || !InRect(_arrivalContact.From, LastX, LastY)
            || !OriginTouchesEdge(_arrivalContact))) _arrivalContact = null;
        if (transition != null)
        {
            BoundaryBehavior behavior = EffectiveBehavior(transition, gatesEnabled, sideContain);
            bool crosses = behavior == BoundaryBehavior.Free
                || behavior == BoundaryBehavior.Push && inPassage && HasTransitionIntent(transition, x, y);
            if (crosses)
            {
                if (transition.Direction == BoundaryDirection.Down && TryGuideDescent(x, y, out bx, out by))
                    return FinishDecision(GuardAction.Block, x, y, ref bx, ref by, transition, transitionPassed: true);
                return FinishDecision(GuardAction.Pass, x, y, ref bx, ref by, transition, transitionPassed: true);
            }

            HoldOnSource(transition, x, y, ref bx, ref by);
            if (_timedDecision && LearnFromMyArrivals && behavior == BoundaryBehavior.Push
                && !ReferenceEquals(_arrivalContact, transition))
            {
                int primary = transition.Direction switch
                {
                    BoundaryDirection.Up => LastY - y, BoundaryDirection.Down => y - LastY,
                    BoundaryDirection.Left => LastX - x, _ => x - LastX,
                };
                var learner = transition.Direction is BoundaryDirection.Left or BoundaryDirection.Right
                    ? _sideArrivals : _upArrivals;
                learner.Observe(primary / _eventSeconds);
                _arrivalContact = transition;
            }
            return FinishDecision(GuardAction.Block, x, y, ref bx, ref by, transition, transitionPassed: false);
        }

        // (S) Side containment: a SOFT, SYMMETRIC barrier with the up-barrier's feel. It stops the cursor from
        // ACCIDENTALLY drifting BOTH ways across the MAIN screen's left/right edge — OUT of main onto a side screen,
        // AND back IN from a side screen onto main (the "my mouse keeps slipping back to the main screen" case). Either
        // way a slow drift / steep diagonal is blocked while a DELIBERATE horizontal push still crosses — using the
        // side barrier's OWN tunable threshold (SideCrossMin/SideCrossSlack) so its stiffness can be dialled apart from
        // the up barrier. It is never a hard lock: a real push always crosses and the toggle/pause hotkeys remain, so
        // the cursor can't be trapped on either side. When OFF the side is fully free.
        if (HasSides)
        {
            bool startedOnMain = LastX >= MainL && LastX < MainR && LastY >= MainT && LastY < MainB;
            int sideDx = Math.Abs(x - LastX);
            int sideDy = Math.Abs(y - LastY);
            bool sideIntent = HasEventIntent(sideDx, sideDy, SideCrossMin, SideCrossSlack, true);

            // Direction OUT (main -> side): the move starts on main and its target leaves main horizontally.
            if (startedOnMain && (x < MainL || x >= MainR))
            {
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
                    return FinishDecision(GuardAction.Block, x, y, ref bx, ref by);
                }
                // allowed: fall through so the up-barrier still governs the vertical component of the crossing.
            }
            // Direction IN (side -> main): the move starts OFF main (outside its X-span, i.e. on a side screen) and its
            // target lands genuinely ON main (inside main's full rect). Block an accidental drift back onto main; a
            // deliberate push crosses. Clamp X back JUST OUTSIDE main on the side we came from, keeping Y, so the cursor
            // stays on the side screen — origin-aware, so only this exact re-entry is gated, never a within-side move.
            else if (!startedOnMain && (LastX < MainL || LastX >= MainR)
                     && x >= MainL && x < MainR && y >= MainT && y < MainB)
            {
                if (sideContain && !sideIntent)
                {
                    bx = LastX < MainL ? MainL - 1 : MainR;   // hold one pixel outside main, on the origin side
                    by = y;
                    // Void correction: if that pixel isn't on any real monitor (a gap between main and the side screen),
                    // pull the point back onto the origin side screen so the cursor still lands on a real pixel.
                    if (!TryActiveMonitor(bx, by, out _) && TryActiveMonitor(LastX, LastY, out var origin))
                    {
                        bx = Math.Clamp(bx, origin.L, origin.R - 1);
                        by = Math.Clamp(by, origin.T, origin.B - 1);
                    }
                    return FinishDecision(GuardAction.Block, x, y, ref bx, ref by);
                }
                // allowed: fall through (the move lands on main; the up-barrier / onTop logic governs from here).
            }
        }

        if (!HasTop)
            return FinishDecision(GuardAction.Pass, x, y, ref bx, ref by);

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
                    bool crossesLocalEdge = CrossesLocalBottomEdge(route.Top, x, y);                 // 4
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
                        // The hook applies the X correction onto the linked screen.
                        return FinishDecision(GuardAction.Block, x, y, ref bx, ref by);
                    }
                }
            }

            if (y >= BarrierY) OnTop = false;
            return FinishDecision(GuardAction.Pass, x, y, ref bx, ref by);
        }

        // (B) On the bottom row, staying at/below the line -> normal move.
        if (y >= BarrierY)
            return FinishDecision(GuardAction.Pass, x, y, ref bx, ref by);

        // (C) Upward crossing attempt. Honour only a deliberate push that BOTH starts and ends inside the SAME
        // gate (origin-aware -> a side-screen diagonal can't sneak through another column). A configured user
        // gate needs the master toggle open; a SAFETY gate (an isolated screen's only escape) is honoured even
        // when the toggle is closed so the cursor can never be trapped. Both still require the deliberate push.
        bool startedBelow = LastY >= BarrierY;
        bool inUserGate = gatesEnabled && startedBelow && InSameGate(x, LastX, Gates);
        bool inSafetyGate = startedBelow && InSafetyGate(x, LastX, LastY);
        int dxAbs = Math.Abs(x - LastX);
        int dyUp = LastY - y;
        bool intent = HasEventIntent(dyUp, dxAbs, CrossMinUp, CrossSlack, false);
        if ((inUserGate || inSafetyGate) && intent)
        {
            OnTop = true;
            return FinishDecision(GuardAction.Pass, x, y, ref bx, ref by);
        }

        // Block: clamp down to the barrier line, keep X so horizontal sliding stays smooth.
        bx = x; by = BarrierY;
        return FinishDecision(GuardAction.Block, x, y, ref bx, ref by);
    }

    /// <summary>
    /// The single exit for every decision. A valid target is preserved exactly. A Pass target in a desktop void resolves
    /// to the globally nearest legal pixel, preserving the move's natural direction. A Block clamp in a void first
    /// resolves onto the usable monitor that owned the move's last legal point; this keeps exclusion-wall and other
    /// anti-tunnel clamps on their entry side. With no usable entry monitor, Block also uses the globally nearest legal
    /// pixel. Both paths use finite boundary candidates and geometry-only tie-breaks.
    /// </summary>
    private GuardAction FinishDecision(GuardAction action, int attemptedX, int attemptedY, ref int bx, ref int by,
                                       RuntimeTransition transition = null, bool transitionPassed = false)
    {
        int finalX = action == GuardAction.Block ? bx : attemptedX;
        int finalY = action == GuardAction.Block ? by : attemptedY;

        bool unauthorized = transition != null
            ? !PointIsAuthorized(finalX, finalY, transition, transitionPassed)
            : !PointIsGloballyAuthorizedFromLast(finalX, finalY);
        if (Monitors.Count > 0 && (!TryActiveMonitor(finalX, finalY, out _) || InsideForbidden(finalX, finalY) || unauthorized))
        {
            bool resolved = false;
            int legalX = 0, legalY = 0;

            if (action == GuardAction.Block && HaveLast
                && TryActiveMonitor(LastX, LastY, out var entryMonitor) && !InsideForbidden(LastX, LastY))
            {
                bool entryAuthorized = transition == null || RectIsAuthorized(entryMonitor, transition, transitionPassed);
                if (entryAuthorized)
                    resolved = TryLegalPointOnMonitor(entryMonitor, finalX, finalY, -1,
                                                      out legalX, out legalY, out _);
            }

            if (!resolved)
                resolved = TryNearestLegalMonitorPoint(finalX, finalY, out legalX, out legalY,
                                                       transition, transitionPassed);

            if (resolved)
            {
                bx = finalX = legalX;
                by = finalY = legalY;
                action = GuardAction.Block;
            }
        }

        // Derive free-roam from where the cursor actually lands, never the off-screen attempted Y or an earlier branch.
        // This both clears state after a correction below the barrier and restores it after a correction onto a top.
        if (!HaveLast) OnTop = HasTop && finalY < BarrierY;
        else if (HasTop) OnTop = finalY < BarrierY;

        Accept(finalX, finalY);
        return action;
    }
    private void Accept(int x, int y)
    {
        LastX = x; LastY = y; HaveLast = true;
    }
}
