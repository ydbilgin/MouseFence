namespace MouseFence;

public enum BoundaryDirection { Unknown, Up, Down, Left, Right }
public enum BoundaryBehavior { Unknown, Free, Push, Blocked, OverallFeel }
public enum BoundaryControl { Unknown, Automatic, Always }
public enum BoundaryDefault { Unknown, Automatic, Free, Push, Blocked }

/// <summary>A persisted directional rule. Its identity is the ordered, case-insensitive
/// (FromStableId, ToStableId, Direction) tuple; geometry is deliberately never persisted.</summary>
public sealed class BoundaryRule
{
    public string FromStableId { get; set; } = "";
    public string ToStableId { get; set; } = "";
    [System.Text.Json.Serialization.JsonConverter(typeof(TolerantEnumConverter<BoundaryDirection>))]
    public BoundaryDirection Direction { get; set; }
    [System.Text.Json.Serialization.JsonConverter(typeof(TolerantEnumConverter<BoundaryBehavior>))]
    public BoundaryBehavior Behavior { get; set; }
    [System.Text.Json.Serialization.JsonConverter(typeof(TolerantEnumConverter<BoundaryControl>))]
    public BoundaryControl Control { get; set; }
}

/// <summary>Pure live-layout input to the boundary compiler.</summary>
public sealed class BoundaryScreen
{
    public string StableId { get; set; } = "";
    public (int L, int T, int R, int B) Bounds { get; set; }
    public bool Primary { get; set; }
    public bool Excluded { get; set; }
}

/// <summary>One precompiled, allocation-free hook-path transition. A split corridor produces
/// multiple entries with the same ordered identity, which keeps intersection matching simple.</summary>
public sealed class RuntimeTransition
{
    public string FromStableId { get; internal set; } = "";
    public string ToStableId { get; internal set; } = "";
    public BoundaryDirection Direction { get; internal set; }
    public (int L, int T, int R, int B) From { get; internal set; }
    public (int L, int T, int R, int B) To { get; internal set; }
    public int CorridorStart { get; internal set; }
    public int CorridorEnd { get; internal set; }
    public int GapPx { get; internal set; }
    public int SeamTolerancePx { get; internal set; }
    public BoundaryBehavior Behavior { get; internal set; }
    public BoundaryControl Control { get; internal set; }
    public int PushMin { get; internal set; }
    public int PushSlack { get; internal set; }
    public int PassageInsetPx { get; internal set; }
    public int EdgeHoldbackPx { get; internal set; }
    public bool ExplicitRule { get; internal set; }
    /// <summary>The legacy owner-bound anti-trap escape: this source has no adjacent Down/Left/Right exit and this
    /// Up corridor is its only way out. When enabled at compile time it remains at least Push regardless of the live
    /// Up-gate toggle.</summary>
    public bool SoleUpSafetyEscape { get; internal set; }
    public bool SafetyDemoted { get; internal set; }
    public int SortOrder { get; internal set; }
    /// <summary>Authorized landings with the ordinary Up and side-protection toggles both enabled.</summary>
    public (int L, int T, int R, int B)[] AllowedLandings { get; internal set; } = Array.Empty<(int, int, int, int)>();
    internal (int L, int T, int R, int B)[] AllowedLandingsUpClosed { get; set; } = Array.Empty<(int, int, int, int)>();
    internal (int L, int T, int R, int B)[] AllowedLandingsSideOff { get; set; } = Array.Empty<(int, int, int, int)>();
    internal (int L, int T, int R, int B)[] AllowedLandingsUpClosedSideOff { get; set; } = Array.Empty<(int, int, int, int)>();

    internal int CorridorWidth => CorridorEnd - CorridorStart;

    internal (int L, int T, int R, int B)[] AllowedLandingsFor(bool gatesEnabled, bool sideContain) =>
        gatesEnabled
            ? sideContain ? AllowedLandings : AllowedLandingsSideOff
            : sideContain ? AllowedLandingsUpClosed : AllowedLandingsUpClosedSideOff;
}

public sealed class BoundaryValidationIssue
{
    public string StableId { get; init; } = "";
    public string FocusToStableId { get; init; } = "";
    public BoundaryDirection FocusDirection { get; init; }
    public int FocusCorridorWidth { get; init; }
}

public sealed class BoundaryCompileResult
{
    public RuntimeTransition[] Transitions { get; init; } = Array.Empty<RuntimeTransition>();
    public BoundaryValidationIssue[] SealedScreens { get; init; } = Array.Empty<BoundaryValidationIssue>();
    public bool HasSafetyDemotions => Transitions.Any(t => t.SafetyDemoted);
}

/// <summary>Pure rectangle compiler and save/import validator. No Win32 calls and no display mutation.</summary>
public static class BoundaryGraph
{
    private sealed class Candidate
    {
        public BoundaryScreen From;
        public BoundaryScreen To;
        public BoundaryDirection Direction;
        public int Start;
        public int End;
        public int Gap;
    }

    public static BoundaryCompileResult Compile(Settings settings, IReadOnlyList<BoundaryScreen> screens,
                                                 bool applyGeometrySafety = true)
    {
        settings ??= new Settings();
        screens ??= Array.Empty<BoundaryScreen>();
        var usable = screens.Where(s => s != null && !s.Excluded && Valid(s.Bounds)
                                             && !string.IsNullOrWhiteSpace(s.StableId)).ToList();
        var rules = NormalizeRuleMap(settings.BoundaryRules);
        var candidates = BuildCandidates(usable);
        var transitions = new List<RuntimeTransition>(candidates.Count);

        foreach (var c in candidates)
        {
            string key = RuleKey(c.From.StableId, c.To.StableId, c.Direction);
            rules.TryGetValue(key, out var rule);
            var behavior = rule == null
                ? ResolveDefault(settings, c.Direction, c.From.Primary || c.To.Primary, c.Gap,
                    IsConfiguredTop(settings, c.To))
                : rule.Behavior == BoundaryBehavior.OverallFeel ? BoundaryBehavior.Push : rule.Behavior;
            var control = rule?.Control ?? BoundaryControl.Automatic;
            int pushMin = c.Direction is BoundaryDirection.Left or BoundaryDirection.Right
                ? settings.SideCrossMin : settings.UpCrossMin;
            int pushSlack = c.Direction is BoundaryDirection.Left or BoundaryDirection.Right
                ? settings.SideCrossSlack : settings.UpCrossSlack;
            transitions.Add(new RuntimeTransition
            {
                FromStableId = c.From.StableId,
                ToStableId = c.To.StableId,
                Direction = c.Direction,
                From = c.From.Bounds,
                To = c.To.Bounds,
                CorridorStart = c.Start,
                CorridorEnd = c.End,
                GapPx = c.Gap,
                SeamTolerancePx = settings.SeamTolerancePx,
                Behavior = behavior,
                Control = control,
                PushMin = pushMin,
                PushSlack = pushSlack,
                PassageInsetPx = settings.PassageInsetPx,
                EdgeHoldbackPx = settings.EdgeHoldbackPx,
                ExplicitRule = rule != null,
            });
        }

        transitions.Sort(CompareTransition);
        for (int i = 0; i < transitions.Count; i++) transitions[i].SortOrder = i;

        MarkLegacySoleUpEscapes(settings, usable, transitions);

        var sealedBeforeSafety = FindSealed(usable, transitions);
        if (applyGeometrySafety && settings.SafetyEscapes)
        {
            // Explicit all-blocked configurations are rejected by ValidateNoSeal before persistence. This first pass
            // is the later-geometry backstop; the second catches a graph that becomes sealed only when Automatic Up
            // resolves Blocked at runtime. Side protection ON is the restrictive side state (OFF can only free edges).
            DemoteWidestExits(sealedBeforeSafety, transitions);
            var sealedWithUpClosed = FindSealed(usable, transitions, useRuntimeState: true,
                                                gatesEnabled: false, sideContain: true);
            DemoteWidestExits(sealedWithUpClosed, transitions);
        }

        BuildAllowedLandingSets(usable, transitions);
        return new BoundaryCompileResult
        {
            Transitions = transitions.ToArray(),
            SealedScreens = sealedBeforeSafety.ToArray(),
        };
    }

    /// <summary>Unconditional persistence validation. Safety demotion is intentionally disabled: a user-authored
    /// zero-exit configuration must be repaired, not silently saved and overridden.</summary>
    public static BoundaryValidationIssue[] ValidateNoSeal(Settings settings, IReadOnlyList<BoundaryScreen> screens) =>
        Compile(settings, screens, applyGeometrySafety: false).SealedScreens;

    public static List<BoundaryScreen> FromMonitors(IEnumerable<MonitorInfo> monitors, ISet<string> excluded = null) =>
        (monitors ?? Enumerable.Empty<MonitorInfo>()).Select(m => new BoundaryScreen
        {
            StableId = m.StableId,
            Primary = m.Primary,
            Excluded = excluded?.Contains(m.StableId) == true,
            Bounds = (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom),
        }).ToList();

    private static Dictionary<string, BoundaryRule> NormalizeRuleMap(IEnumerable<BoundaryRule> input)
    {
        var result = new Dictionary<string, BoundaryRule>(StringComparer.OrdinalIgnoreCase);
        foreach (var rule in input ?? Enumerable.Empty<BoundaryRule>())
        {
            if (rule == null || string.IsNullOrWhiteSpace(rule.FromStableId) || string.IsNullOrWhiteSpace(rule.ToStableId)
                || Same(rule.FromStableId, rule.ToStableId)
                || rule.Direction == BoundaryDirection.Unknown || rule.Behavior == BoundaryBehavior.Unknown
                || rule.Control == BoundaryControl.Unknown)
                continue;
            result[RuleKey(rule.FromStableId, rule.ToStableId, rule.Direction)] = rule; // last-valid-wins
        }
        return result;
    }

    private static bool IsConfiguredTop(Settings settings, BoundaryScreen screen) => settings.Mode == "Manual"
        ? settings.ManualMonitors.Contains(screen.StableId, StringComparer.OrdinalIgnoreCase)
        : screen.Bounds.B <= 0;

    private static void MarkLegacySoleUpEscapes(Settings settings, IReadOnlyList<BoundaryScreen> screens,
                                                 List<RuntimeTransition> transitions)
    {
        if (!settings.SafetyEscapes || screens.Count == 0) return;

        var topIdx = new HashSet<int>();
        for (int i = 0; i < screens.Count; i++)
            if (IsConfiguredTop(settings, screens[i])) topIdx.Add(i);
        var rects = screens.Select(s => s.Bounds).ToList();
        var (_, gates) = GuardCore.AntiTrap(rects, topIdx);

        foreach (var transition in transitions.Where(t => t.Direction == BoundaryDirection.Up))
            transition.SoleUpSafetyEscape = gates.Any(g => g.OwnerT == transition.From.T
                && g.OwnerB == transition.From.B
                && Math.Max(g.Min, transition.CorridorStart) < Math.Min(g.Max, transition.CorridorEnd));
    }

    private static BoundaryBehavior ResolveDefault(Settings s, BoundaryDirection direction, bool touchesPrimary,
                                                    int gap, bool automaticUpper) =>
        Resolve(direction switch
        {
            BoundaryDirection.Up => s.UpwardBoundaryDefault,
            BoundaryDirection.Down => s.DownwardBoundaryDefault,
            _ => s.SideBoundaryDefault,
        }, direction, touchesPrimary, gap, s.SeamTolerancePx, automaticUpper);

    private static BoundaryBehavior Resolve(BoundaryDefault value, BoundaryDirection direction,
                                            bool touchesPrimary, int gap, int seamTolerance, bool automaticUpper) => value switch
    {
        BoundaryDefault.Free => BoundaryBehavior.Free,
        BoundaryDefault.Push => BoundaryBehavior.Push,
        BoundaryDefault.Blocked => BoundaryBehavior.Blocked,
        BoundaryDefault.Automatic when direction == BoundaryDirection.Up && automaticUpper => BoundaryBehavior.Push,
        BoundaryDefault.Automatic when direction == BoundaryDirection.Up => BoundaryBehavior.Free,
        BoundaryDefault.Automatic when direction == BoundaryDirection.Down => BoundaryBehavior.Free,
        BoundaryDefault.Automatic when touchesPrimary && gap <= seamTolerance => BoundaryBehavior.Push,
        BoundaryDefault.Automatic => BoundaryBehavior.Free,
        _ => BoundaryBehavior.Blocked,
    };

    private static List<Candidate> BuildCandidates(IReadOnlyList<BoundaryScreen> screens)
    {
        var result = new List<Candidate>();
        for (int i = 0; i < screens.Count; i++)
        for (int j = 0; j < screens.Count; j++)
        {
            if (i == j) continue;
            var from = screens[i]; var to = screens[j];
            var a = from.Bounds; var b = to.Bounds;
            BoundaryDirection direction;
            int overlapStart, overlapEnd, gap;
            bool vertical;

            if (b.B <= a.T && Math.Min(a.R, b.R) > Math.Max(a.L, b.L))
            {
                direction = BoundaryDirection.Up; vertical = true;
                overlapStart = Math.Max(a.L, b.L); overlapEnd = Math.Min(a.R, b.R); gap = a.T - b.B;
            }
            else if (b.T >= a.B && Math.Min(a.R, b.R) > Math.Max(a.L, b.L))
            {
                direction = BoundaryDirection.Down; vertical = true;
                overlapStart = Math.Max(a.L, b.L); overlapEnd = Math.Min(a.R, b.R); gap = b.T - a.B;
            }
            else if (b.R <= a.L && Math.Min(a.B, b.B) > Math.Max(a.T, b.T))
            {
                direction = BoundaryDirection.Left; vertical = false;
                overlapStart = Math.Max(a.T, b.T); overlapEnd = Math.Min(a.B, b.B); gap = a.L - b.R;
            }
            else if (b.L >= a.R && Math.Min(a.B, b.B) > Math.Max(a.T, b.T))
            {
                direction = BoundaryDirection.Right; vertical = false;
                overlapStart = Math.Max(a.T, b.T); overlapEnd = Math.Min(a.B, b.B); gap = b.L - a.R;
            }
            else continue; // overlaps/clones and true diagonals have no geometric boundary

            var spans = new List<(int Start, int End)> { (overlapStart, overlapEnd) };
            foreach (var third in screens)
            {
                if (ReferenceEquals(third, from) || ReferenceEquals(third, to)) continue;
                var c = third.Bounds;
                bool between = vertical
                    ? c.B > Math.Min(a.B, b.B) && c.T < Math.Max(a.T, b.T)
                    : c.R > Math.Min(a.R, b.R) && c.L < Math.Max(a.L, b.L);
                if (!between) continue;
                int cutStart = vertical ? c.L : c.T;
                int cutEnd = vertical ? c.R : c.B;
                spans = Subtract(spans, cutStart, cutEnd);
                if (spans.Count == 0) break;
            }
            foreach (var span in spans.Where(s => s.End > s.Start))
                result.Add(new Candidate { From = from, To = to, Direction = direction,
                                           Start = span.Start, End = span.End, Gap = Math.Max(0, gap) });
        }
        return result;
    }

    private static List<(int Start, int End)> Subtract(List<(int Start, int End)> spans, int cutStart, int cutEnd)
    {
        var result = new List<(int, int)>();
        foreach (var span in spans)
        {
            if (cutEnd <= span.Start || cutStart >= span.End) { result.Add(span); continue; }
            if (cutStart > span.Start) result.Add((span.Start, Math.Min(cutStart, span.End)));
            if (cutEnd < span.End) result.Add((Math.Max(cutEnd, span.Start), span.End));
        }
        return result;
    }

    private static List<BoundaryValidationIssue> FindSealed(IReadOnlyList<BoundaryScreen> screens,
                                                            IReadOnlyList<RuntimeTransition> transitions,
                                                            bool useRuntimeState = false,
                                                            bool gatesEnabled = true, bool sideContain = true)
    {
        var issues = new List<BoundaryValidationIssue>();
        if (screens.Count <= 1) return issues;
        foreach (var screen in screens)
        {
            var exits = transitions.Where(t => Same(t.FromStableId, screen.StableId)).ToList();
            bool usableExit = exits.Any(t => useRuntimeState
                ? GuardCore.EffectiveBehavior(t, gatesEnabled, sideContain) != BoundaryBehavior.Blocked
                : t.Behavior is BoundaryBehavior.Free or BoundaryBehavior.Push);
            if (exits.Count == 0 || usableExit) continue;
            var focus = exits.OrderByDescending(t => t.CorridorWidth).ThenBy(t => t.SortOrder).First();
            issues.Add(new BoundaryValidationIssue
            {
                StableId = screen.StableId,
                FocusToStableId = focus.ToStableId,
                FocusDirection = focus.Direction,
                FocusCorridorWidth = focus.CorridorWidth,
            });
        }
        return issues;
    }

    private static void DemoteWidestExits(IEnumerable<BoundaryValidationIssue> issues,
                                          IReadOnlyList<RuntimeTransition> transitions)
    {
        foreach (var issue in issues)
        {
            // One deterministic exit per sealed screen: widest corridor, then the compiler's stable order.
            var exit = transitions.Where(t => Same(t.FromStableId, issue.StableId))
                .OrderByDescending(t => t.CorridorWidth).ThenBy(t => t.SortOrder).FirstOrDefault();
            if (exit == null) continue;
            if (exit.Behavior == BoundaryBehavior.Blocked) exit.Behavior = BoundaryBehavior.Push;
            exit.SafetyDemoted = true;
        }
    }

    private static void BuildAllowedLandingSets(IReadOnlyList<BoundaryScreen> screens, List<RuntimeTransition> transitions)
    {
        foreach (var transition in transitions)
        {
            transition.AllowedLandings = BuildAllowedLandingSet(screens, transitions, transition,
                                                                 gatesEnabled: true, sideContain: true);
            transition.AllowedLandingsUpClosed = BuildAllowedLandingSet(screens, transitions, transition,
                                                                         gatesEnabled: false, sideContain: true);
            transition.AllowedLandingsSideOff = BuildAllowedLandingSet(screens, transitions, transition,
                                                                        gatesEnabled: true, sideContain: false);
            transition.AllowedLandingsUpClosedSideOff = BuildAllowedLandingSet(screens, transitions, transition,
                                                                                gatesEnabled: false, sideContain: false);
        }
    }

    private static (int L, int T, int R, int B)[] BuildAllowedLandingSet(
        IReadOnlyList<BoundaryScreen> screens, IReadOnlyList<RuntimeTransition> transitions,
        RuntimeTransition transition, bool gatesEnabled, bool sideContain)
    {
        if (GuardCore.EffectiveBehavior(transition, gatesEnabled, sideContain) == BoundaryBehavior.Blocked)
            return new[] { transition.From };

        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
            { transition.FromStableId, transition.ToStableId };
        var queue = new Queue<string>(); queue.Enqueue(transition.ToStableId);
        while (queue.Count > 0)
        {
            string from = queue.Dequeue();
            foreach (var edge in transitions)
                if (Same(edge.FromStableId, from)
                    && GuardCore.EffectiveBehavior(edge, gatesEnabled, sideContain) != BoundaryBehavior.Blocked
                    && seen.Add(edge.ToStableId))
                    queue.Enqueue(edge.ToStableId);
        }
        // A transitive alternate route must never authorize a correction directly onto a monitor whose ordered
        // transition from this source is blocked in the current live toggle state. Real future moves may take the
        // alternate route one seam at a time; a one-frame void/corner repair may not bypass the blocked direction.
        foreach (var blocked in transitions.Where(t => Same(t.FromStableId, transition.FromStableId)
            && GuardCore.EffectiveBehavior(t, gatesEnabled, sideContain) == BoundaryBehavior.Blocked))
            seen.Remove(blocked.ToStableId);
        return screens.Where(s => seen.Contains(s.StableId)).Select(s => s.Bounds).ToArray();
    }

    private static int CompareTransition(RuntimeTransition a, RuntimeTransition b)
    {
        int Orientation(BoundaryDirection d) => d is BoundaryDirection.Up or BoundaryDirection.Down ? 0 : 1;
        int c = Orientation(a.Direction).CompareTo(Orientation(b.Direction));
        if (c != 0) return c;
        c = a.CorridorStart.CompareTo(b.CorridorStart); if (c != 0) return c;
        c = a.CorridorEnd.CompareTo(b.CorridorEnd); if (c != 0) return c;
        c = StringComparer.Ordinal.Compare(a.FromStableId, b.FromStableId); if (c != 0) return c;
        c = StringComparer.Ordinal.Compare(a.ToStableId, b.ToStableId); if (c != 0) return c;
        return a.Direction.CompareTo(b.Direction);
    }

    private static string RuleKey(string from, string to, BoundaryDirection direction) =>
        (from ?? "").Trim() + "\n" + (to ?? "").Trim() + "\n" + direction;
    private static bool Same(string a, string b) => string.Equals(a?.Trim(), b?.Trim(), StringComparison.OrdinalIgnoreCase);
    private static bool Valid((int L, int T, int R, int B) r) => r.R > r.L && r.B > r.T;
}
