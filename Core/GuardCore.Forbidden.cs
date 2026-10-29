namespace MouseFence;

public sealed partial class GuardCore
{
    private static bool InRect((int L, int T, int R, int B) r, int x, int y) =>
        x >= r.L && x < r.R && y >= r.T && y < r.B;

    private bool TryActiveMonitor(int x, int y, out (int L, int T, int R, int B) m)
    {
        // Extended-desktop monitors normally do not overlap, but clone/driver edge cases can. Pick by geometry rather
        // than enumeration order so the same rectangles always yield the same active monitor.
        bool found = false;
        m = default;
        foreach (var r in Monitors)
        {
            if (!InRect(r, x, y)) continue;
            if (!found || CompareRect(r, m) < 0) { m = r; found = true; }
        }
        return found;
    }

    /// <summary>
    /// Pure wall-off clamp: if (x,y) lands inside the excluded-only part of ANY forbidden rect, push it back OUT to a legal pixel on one of
    /// <see cref="Monitors"/> (the usable, non-excluded screens). Preserve the orthogonal axis when that screen has
    /// such a pixel; otherwise use its nearest edge pixel. Returns true if it moved the point. With a configured
    /// usable monitor list, GUARANTEES the result is outside effective forbidden-only geometry AND on a usable monitor.
    ///
    /// ENTRY-SIDE-AWARE (binding fix #1): the move came from (LastX,LastY). When we have a last position that is
    /// OUTSIDE the rect, exit on the edge the move ENTERED from — the rect wall the entry SEGMENT (Last -> target)
    /// crosses FIRST (smallest valid parametric t), NOT the nearest edge of the target and NOT the larger-delta axis.
    /// A large delta (e.g. a fast flick or OS easing teleport deep into / past the centre of the dummy) must not
    /// "tunnel" to the far/void side and re-lose the cursor. When Last is a legal usable pixel outside the connected
    /// region, that exact monitor owns the entry and is preferred over every other exit. With no usable entry direction
    /// (!HaveLast, Last inside the region, or Last not on a usable monitor), the nearest usable monitor wins. Equal-
    /// distance ties use L,T,R,B geometry order.
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
            // Clone/overlap layouts can put the same coordinate on both an excluded display and a usable one. There is
            // no distinct cursor coordinate to wall in that shared area, so usable geometry wins.
            if (!InsideForbidden(x, y)) break;

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
            bool lastOutsideRegion = HaveLast && !RegionContains(region, LastX, LastY);
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
                else if (xOutside || yOutside)
                    useX = xOutside;                                 // crossed exactly one axis
                else
                    useX = exitAxis == 0;                            // outside a non-rectangular region hole

                if (useX) { exitAxis = 0; exitToLow = lastLeft; }
                else { exitAxis = 1; exitToLow = lastAbove; }
            }

            // MONITOR-AWARE RESOLUTION. The old geometry-only exit below can be outside every dummy yet still be a
            // desktop void (the reporter case was x=4479,y=1400 below the real neighbour). If Last is a legal usable
            // point outside this region, return to THAT monitor: it is the unambiguous entry-side owner. Otherwise
            // choose the nearest legal usable monitor. Candidate generation is finite (monitor/forbidden boundaries),
            // and ties are geometry-ordered, so this is deterministic and cannot oscillate between adjacent dummies.
            if (Monitors.Count > 0)
            {
                (int L, int T, int R, int B) entryMonitor = default;
                bool usableEntry = lastOutsideRegion
                    && TryActiveMonitor(LastX, LastY, out entryMonitor)
                    && !InsideForbidden(LastX, LastY);
                if (usableEntry)
                {
                    int preferredAxis = PreferredAxis(entryMonitor, x, y, exitAxis);
                    // WALL FEEL: clamp only the CROSSING axis to the neighbour's edge; on the ORTHOGONAL axis keep the
                    // cursor's ENTRY coordinate (LastX/LastY), not the attempted target. Otherwise a target produced by
                    // Windows' "ease cursor movement between displays" (a large one-frame teleport deep into the dummy)
                    // drags the cursor to that eased height, so hitting the seam looks like a ~600px vertical JUMP
                    // instead of the cursor simply stopping at the edge. preferredAxis 0 = X is the wall (keep LastY);
                    // 1 = Y is the wall (keep LastX).
                    int desiredX = preferredAxis == 0 ? x : LastX;
                    int desiredY = preferredAxis == 1 ? y : LastY;
                    if (TryLegalPointOnMonitor(entryMonitor, desiredX, desiredY, preferredAxis,
                                               out int entryX, out int entryY, out _))
                    {
                        x = entryX;
                        y = entryY;
                        return true;
                    }
                }

                if (TryNearestLegalMonitorPoint(x, y, out int legalX, out int legalY))
                {
                    x = legalX;
                    y = legalY;
                    return true;
                }
                // A valid extended-desktop configuration always has a usable pixel: exclusion sanitization keeps a
                // real monitor and normal monitor rectangles do not overlap. If malformed synthetic data covers every
                // usable pixel with Forbidden, no legal point exists mathematically; retain the bounded geometric
                // fallback below rather than suppressing the move or freezing the cursor.
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

    private bool InsideForbidden(int x, int y)
    {
        bool excluded = false;
        foreach (var f in Forbidden)
            if (InRect(f, x, y)) { excluded = true; break; }
        if (!excluded) return false;
        return !TryActiveMonitor(x, y, out _);
    }

    private bool RegionContains(List<int> region, int x, int y)
    {
        foreach (int index in region)
            if (InRect(Forbidden[index], x, y)) return true;
        return false;
    }

    /// <summary>Choose which coordinate is orthogonal to the entry wall. If the target lies outside the source
    /// monitor on only one axis, that axis is necessarily the crossing axis; diagonal/corner cases keep the region's
    /// deterministic entry-axis decision.</summary>
    private static int PreferredAxis((int L, int T, int R, int B) source, int targetX, int targetY, int regionAxis)
    {
        bool outsideX = targetX < source.L || targetX >= source.R;
        bool outsideY = targetY < source.T || targetY >= source.B;
        if (outsideX != outsideY) return outsideX ? 0 : 1;
        return regionAxis;
    }

    /// <summary>
    /// Find the closest legal pixel on one usable monitor without enumerating pixels. Axis-aligned rectangle
    /// subtraction changes only at monitor/forbidden edges, so the desired coordinate plus L/R-1/T/B-1 and every
    /// forbidden outside edge (L-1/R/T-1/B) form a complete finite candidate grid. For an entry-side clamp,
    /// <paramref name="preferredAxis"/> is 0 (X wall) or 1 (Y wall): minimize movement on the orthogonal coordinate
    /// first, then the wall coordinate. A value of -1 selects ordinary squared distance for nearest-monitor fallback.
    /// </summary>
    private bool TryLegalPointOnMonitor((int L, int T, int R, int B) monitor,
                                        int desiredX, int desiredY, int preferredAxis,
                                        out int bestX, out int bestY, out decimal bestDistance)
    {
        bestX = bestY = 0;
        bestDistance = decimal.MaxValue;
        if (monitor.R <= monitor.L || monitor.B <= monitor.T) return false;

        var xs = new SortedSet<int>();
        var ys = new SortedSet<int>();
        void AddX(long value) => xs.Add((int)Math.Clamp(value, monitor.L, (long)monitor.R - 1));
        void AddY(long value) => ys.Add((int)Math.Clamp(value, monitor.T, (long)monitor.B - 1));

        AddX(desiredX); AddX(monitor.L); AddX((long)monitor.R - 1);
        AddY(desiredY); AddY(monitor.T); AddY((long)monitor.B - 1);
        foreach (var f in Forbidden)
        {
            AddX((long)f.L - 1); AddX(f.R);
            AddY((long)f.T - 1); AddY(f.B);
        }

        bool found = false;
        long bestFirst = long.MaxValue, bestSecond = long.MaxValue;
        foreach (int candidateX in xs)
        foreach (int candidateY in ys)
        {
            if (InsideForbidden(candidateX, candidateY)) continue;

            long dx = Math.Abs((long)candidateX - desiredX);
            long dy = Math.Abs((long)candidateY - desiredY);
            decimal distance = (decimal)dx * dx + (decimal)dy * dy;
            long first = preferredAxis switch { 0 => dy, 1 => dx, _ => 0 };
            long second = preferredAxis switch { 0 => dx, 1 => dy, _ => 0 };

            bool better = !found;
            if (!better && preferredAxis >= 0)
                better = first < bestFirst
                      || first == bestFirst && (second < bestSecond
                      || second == bestSecond && (candidateX < bestX
                      || candidateX == bestX && candidateY < bestY));
            else if (!better)
                better = distance < bestDistance
                      || distance == bestDistance && (candidateX < bestX
                      || candidateX == bestX && candidateY < bestY);

            if (!better) continue;
            found = true;
            bestX = candidateX;
            bestY = candidateY;
            bestDistance = distance;
            bestFirst = first;
            bestSecond = second;
        }
        return found;
    }

    /// <summary>Nearest usable-monitor fallback. Distance is measured from the attempted target to the nearest legal
    /// pixel. Equal distances select the monitor with the lexicographically smaller (L,T,R,B), then smaller X,Y;
    /// input list order is never a tie-break, so identical calls are deterministic.</summary>
    private bool TryNearestLegalMonitorPoint(int desiredX, int desiredY, out int bestX, out int bestY,
                                             RuntimeTransition transition = null, bool transitionPassed = false)
    {
        if (transition == null && TryNearestLegalMonitorPoint(desiredX, desiredY, out bestX, out bestY,
                                                             null, false, freeOnly: true)) return true;
        return TryNearestLegalMonitorPoint(desiredX, desiredY, out bestX, out bestY, transition, transitionPassed, false);
    }

    private bool TryNearestLegalMonitorPoint(int desiredX, int desiredY, out int bestX, out int bestY,
                                             RuntimeTransition transition, bool transitionPassed, bool freeOnly)
    {
        bestX = bestY = 0;
        bool found = false;
        decimal bestDistance = decimal.MaxValue;
        (int L, int T, int R, int B) bestMonitor = default;

        foreach (var monitor in Monitors)
        {
            if (freeOnly && !RectIsFreelyReachableFromLast(monitor)) continue;
            if (transition != null && !RectIsAuthorized(monitor, transition, transitionPassed)) continue;
            if (transition == null && !RectIsGloballyAuthorizedFromLast(monitor)) continue;
            if (!TryLegalPointOnMonitor(monitor, desiredX, desiredY, -1,
                                        out int candidateX, out int candidateY, out decimal distance))
                continue;

            int rectOrder = found ? CompareRect(monitor, bestMonitor) : -1;
            bool better = !found || distance < bestDistance
                       || distance == bestDistance && (rectOrder < 0
                       || rectOrder == 0 && (candidateX < bestX
                       || candidateX == bestX && candidateY < bestY));
            if (!better) continue;
            found = true;
            bestDistance = distance;
            bestMonitor = monitor;
            bestX = candidateX;
            bestY = candidateY;
        }
        return found;
    }

    private static int CompareRect((int L, int T, int R, int B) a, (int L, int T, int R, int B) b)
    {
        int c = a.L.CompareTo(b.L);
        if (c != 0) return c;
        c = a.T.CompareTo(b.T);
        if (c != 0) return c;
        c = a.R.CompareTo(b.R);
        return c != 0 ? c : a.B.CompareTo(b.B);
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

}
