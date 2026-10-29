namespace MouseFence;

public sealed partial class GuardCore
{

    private RuntimeTransition MatchTransition(int x, int y, out bool inPassage)
    {
        inPassage = false;
        RuntimeTransition best = null;
        double bestT = double.PositiveInfinity;
        for (int i = 0; i < Transitions.Length; i++)
        {
            var t = Transitions[i];
            if (!InRect(t.From, LastX, LastY)) continue;
            int delta, plane, originAxis, targetAxis, originOrth, targetOrth;
            switch (t.Direction)
            {
                case BoundaryDirection.Up:
                    delta = LastY - y; plane = t.From.T; originAxis = LastY; targetAxis = y;
                    originOrth = LastX; targetOrth = x; break;
                case BoundaryDirection.Down:
                    delta = y - LastY; plane = t.From.B; originAxis = LastY; targetAxis = y;
                    originOrth = LastX; targetOrth = x; break;
                case BoundaryDirection.Left:
                    delta = LastX - x; plane = t.From.L; originAxis = LastX; targetAxis = x;
                    originOrth = LastY; targetOrth = y; break;
                case BoundaryDirection.Right:
                    delta = x - LastX; plane = t.From.R; originAxis = LastX; targetAxis = x;
                    originOrth = LastY; targetOrth = y; break;
                default: continue;
            }
            if (delta <= 0 || targetAxis == originAxis) continue;
            double hit = (double)(plane - originAxis) / (targetAxis - originAxis);
            if (hit < 0 || hit > 1) continue;
            double orth = originOrth + (targetOrth - originOrth) * hit;
            int inset = Math.Min(t.PassageInsetPx, Math.Max(0, (t.CorridorWidth - 1) / 2));
            if (orth < t.CorridorStart || orth >= t.CorridorEnd) continue;
            // A gap is enforceable only when Windows establishes this exact ordered transition by landing in To.
            if (t.GapPx > t.SeamTolerancePx && !InRect(t.To, x, y)) continue;

            if (best == null || hit < bestT
                || hit == bestT && (t.GapPx < best.GapPx
                || t.GapPx == best.GapPx && t.SortOrder < best.SortOrder))
            {
                best = t; bestT = hit;
                inPassage = orth >= t.CorridorStart + inset && orth < t.CorridorEnd - inset;
            }
        }
        return best;
    }

    internal static BoundaryBehavior EffectiveBehavior(RuntimeTransition t, bool gatesEnabled, bool sideContain)
    {
        // Compile-time safety exits still require deliberate Push intent, but live Up/side toggles may never close
        // them. SoleUpSafetyEscape carries the legacy isolated-screen rule; SafetyDemoted is the broader geometry-
        // seal backstop selected when a later layout change leaves every authored exit blocked.
        if (t.SoleUpSafetyEscape || t.SafetyDemoted)
            return t.Behavior == BoundaryBehavior.Free ? BoundaryBehavior.Free : BoundaryBehavior.Push;
        if (t.Control == BoundaryControl.Always) return t.Behavior;
        return t.Direction switch
        {
            BoundaryDirection.Up when !gatesEnabled => BoundaryBehavior.Blocked,
            BoundaryDirection.Left or BoundaryDirection.Right when !sideContain => BoundaryBehavior.Free,
            _ => t.Behavior,
        };
    }

    private bool HasTransitionIntent(RuntimeTransition t, int x, int y)
    {
        int primary = t.Direction switch
        {
            BoundaryDirection.Up => LastY - y,
            BoundaryDirection.Down => y - LastY,
            BoundaryDirection.Left => LastX - x,
            BoundaryDirection.Right => x - LastX,
            _ => 0,
        };
        int orthogonal = t.Direction is BoundaryDirection.Up or BoundaryDirection.Down
            ? Math.Abs(x - LastX) : Math.Abs(y - LastY);
        return HasEventIntent(primary, orthogonal, t.PushMin, t.PushSlack,
            t.Direction is BoundaryDirection.Left or BoundaryDirection.Right);
    }

    private bool HasEventIntent(int primary, int orthogonal, int minimum, int slack, bool side)
    {
        if (!DeliberateCross) return primary > 0;
        if (!_timedDecision) return primary >= minimum && orthogonal <= primary + slack;
        var learner = side ? _sideArrivals : _upArrivals;
        double speed = LearnFromMyArrivals && learner.Threshold > 0
            ? learner.Threshold : InstantCrossSpeedPxPerSecond;
        return InstantaneousPush.Crosses(primary, orthogonal, _eventSeconds, speed * minimum / 3.0, slack);
    }

    private bool OriginTouchesEdge(RuntimeTransition t) => t.Direction switch
    {
        BoundaryDirection.Up => LastY <= t.From.T + t.EdgeHoldbackPx,
        BoundaryDirection.Down => LastY >= t.From.B - 1 - t.EdgeHoldbackPx,
        BoundaryDirection.Left => LastX <= t.From.L + t.EdgeHoldbackPx,
        BoundaryDirection.Right => LastX >= t.From.R - 1 - t.EdgeHoldbackPx,
        _ => false,
    };

    // Port only the corner-repair part of 851158f, without its dwell behavior.
    private bool RectIsFreelyReachableFromLast((int L, int T, int R, int B) monitor)
    {
        if (Transitions.Length == 0 || !HaveLast || !TryActiveMonitor(LastX, LastY, out var source)) return true;
        if (monitor == source) return true;
        bool hasSourceTransition = false;
        for (int i = 0; i < Transitions.Length; i++)
        {
            var edge = Transitions[i];
            if (edge.From != source) continue;
            hasSourceTransition = true;
            if (EffectiveBehavior(edge, _gatesEnabledForDecision, _sideContainForDecision) != BoundaryBehavior.Free)
                continue;
            var allowed = edge.AllowedLandingsFor(_gatesEnabledForDecision, _sideContainForDecision);
            for (int j = 0; j < allowed.Length; j++) if (allowed[j] == monitor) return true;
        }
        return !hasSourceTransition;
    }

    private static void HoldOnSource(RuntimeTransition t, int x, int y, ref int bx, ref int by)
    {
        int holdX = Math.Min(t.EdgeHoldbackPx, Math.Max(0, t.From.R - t.From.L - 1));
        int holdY = Math.Min(t.EdgeHoldbackPx, Math.Max(0, t.From.B - t.From.T - 1));
        bx = Math.Clamp(x, t.From.L, t.From.R - 1);
        by = Math.Clamp(y, t.From.T, t.From.B - 1);
        switch (t.Direction)
        {
            case BoundaryDirection.Up: by = t.From.T + holdY; break;
            case BoundaryDirection.Down: by = t.From.B - 1 - holdY; break;
            case BoundaryDirection.Left: bx = t.From.L + holdX; break;
            case BoundaryDirection.Right: bx = t.From.R - 1 - holdX; break;
        }
    }

    private bool PointIsAuthorized(int x, int y, RuntimeTransition transition, bool passed)
    {
        if (!passed) return InRect(transition.From, x, y);
        foreach (var monitor in transition.AllowedLandingsFor(_gatesEnabledForDecision, _sideContainForDecision))
            if (InRect(monitor, x, y)) return true;
        return false;
    }

    private bool RectIsAuthorized((int L, int T, int R, int B) monitor,
                                  RuntimeTransition transition, bool passed)
    {
        if (!passed) return monitor == transition.From;
        foreach (var allowed in transition.AllowedLandingsFor(_gatesEnabledForDecision, _sideContainForDecision))
            if (allowed == monitor) return true;
        return false;
    }

    private bool PointIsGloballyAuthorizedFromLast(int x, int y)
    {
        if (!TryActiveMonitor(x, y, out var monitor)) return true; // void is repaired through the rect overload below
        return RectIsGloballyAuthorizedFromLast(monitor);
    }

    private bool RectIsGloballyAuthorizedFromLast((int L, int T, int R, int B) monitor)
    {
        if (Transitions.Length == 0 || !HaveLast || !TryActiveMonitor(LastX, LastY, out var source)) return true;
        if (monitor == source) return true;
        bool hasSourceTransition = false;
        for (int i = 0; i < Transitions.Length; i++)
        {
            var edge = Transitions[i];
            if (edge.From != source) continue;
            hasSourceTransition = true;
            if (EffectiveBehavior(edge, _gatesEnabledForDecision, _sideContainForDecision) == BoundaryBehavior.Blocked)
                continue;
            var allowed = edge.AllowedLandingsFor(_gatesEnabledForDecision, _sideContainForDecision);
            for (int j = 0; j < allowed.Length; j++)
                if (allowed[j] == monitor) return true;
        }
        // With no geometric boundary from the source (true diagonal/clone), there is no ordered rule to violate.
        return !hasSourceTransition;
    }

}
