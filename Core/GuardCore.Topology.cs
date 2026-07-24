namespace MouseFence;

public sealed partial class GuardCore
{
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

}
