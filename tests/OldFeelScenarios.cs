using MouseFence;
using System.Text.Json;

internal static class OldFeelScenarios
{
    internal static void Instantaneous(Action<string, bool, string> check)
    {
        foreach (int hz in new[] { 1000, 2000, 4000, 8000 })
        {
            foreach (double speed in new[] { 100.0, 1500, 2999, 3000, 3001, 8000, 12000, 16000 })
            {
                var clock = new HookEventClock();
                clock.Interval(10, 80000, 8000000);
                double seconds = clock.Interval((uint)(10 + 1000 / hz), 80000 + 8000000 / hz, 8000000);
                bool crosses = InstantaneousPush.Crosses(speed / hz, 0, seconds, 3000, 5);
                check($"old-feel ideal interval: {speed} px/s at {hz} Hz", crosses == (speed >= 3000), $"dt={seconds} cross={crosses}");
            }
        }
        foreach (double seconds in new[] { 0, -1, double.NaN, double.PositiveInfinity })
            check($"old-feel invalid duration {seconds}", !InstantaneousPush.Crosses(10, 0, seconds, 3000, 5), "");
        check("old-feel direction/slack preserved", !InstantaneousPush.Crosses(3, 9, .001, 3000, 5)
            && InstantaneousPush.Crosses(3, 8, .001, 3000, 5), "");
        var wraps = new HookEventClock();
        wraps.Interval(uint.MaxValue, 1000, 1000000);
        check("old-feel timestamp wraps", wraps.Interval(0, 2000, 1000000) == .001, "");
        var invalid = new HookEventClock();
        check("old-feel first event has no invented interval", double.IsNaN(invalid.Interval(100, 100000, 1000000)), "");
        check("old-feel same-ms event uses high resolution", invalid.Interval(100, 100250, 1000000) == .00025, "");
        check("old-feel zero counter delta rejected", double.IsNaN(invalid.Interval(100, 100250, 1000000)), "");
        check("old-feel backwards message rejected", double.IsNaN(invalid.Interval(99, 100500, 1000000)), "");
        check("old-feel dispatch burst rejected", double.IsNaN(invalid.Interval(99, 100501, 1000000)), "");
        var coalesced = new HookEventClock();
        coalesced.Interval(100, 100000, 1000000);
        double coalescedSeconds = coalesced.Interval(108, 108000, 1000000);
        check("old-feel coalesced slow move uses full 8ms", !InstantaneousPush.Crosses(16, 0, coalescedSeconds, 3000, 5), "");
        check("old-feel coalesced threshold uses full 8ms", InstantaneousPush.Crosses(24, 0, coalescedSeconds, 3000, 5), "");
        check("old-feel idle gap rejected", double.IsNaN(coalesced.Interval(500, 500000, 1000000)), "");

        var core = Rig();
        core.Decide((100, 20), 100, 0, true, true, .001, out _, out _); // fast arrival inside source
        bool held = true;
        for (int i = 0; i < 200; i++)
            held &= core.Decide((100, 0), 100, -1, true, true, .001, out _, out int by) == GuardAction.Block && by == 0;
        check("old-feel fast arrival gives no credit to 200 slow edge pushes", held, "");
        check("old-feel one fast event crosses immediately from inside",
            Rig().Decide((100, 4), 100, -4, true, true, .001, out _, out _) == GuardAction.Pass, "");
        core = Rig();
        core.InstantCrossSpeedPxPerSecond = 12000;
        check("old-feel 4000Hz legacy firmness preset is 12000 px/s",
            core.Decide((100, 0), 100, -2, true, true, .00025, out _, out _) == GuardAction.Block
            && core.Decide((100, 0), 100, -3, true, true, .00025, out _, out _) == GuardAction.Pass, "");
        core = Rig();
        core.Decide((100, 0), 100, -5, true, true, .001, out _, out _);
        check("old-feel stale prior target cannot create a large push",
            core.Decide((100, 0), 100, -1, true, true, .001, out _, out _) == GuardAction.Block, "");
        core.Decide((100, 0), 100, -1, true, true, .001, out _, out _);
        check("old-feel injected placement above the barrier is observed",
            core.Decide((100, -30), 100, -31, true, true, .001, out _, out _) == GuardAction.Pass, "");
        check("old-feel missing observation does not reuse previous source",
            core.Decide(null, 100, -32, true, true, .001, out _, out _) == GuardAction.Pass, "");
        foreach (bool side in new[] { true, false })
        {
            core = Rig();
            var act = core.Decide((-1, 1079), -1, 1081, true, side, .001, out int bx, out int by);
            check($"old-feel timed corner respects protection={side}",
                act == GuardAction.Block && (side ? bx == -1 && by == 1079 : bx == 0 && by == 1081), $"({bx},{by})");
        }
        var learning = new ArrivalLearner();
        for (int i = 0; i < 39; i++) learning.Observe(2000);
        check("old-feel learning waits for 40 arrivals", learning.Threshold == 0, "");
        learning.Observe(2000);
        check("old-feel learning percentile and scale", learning.Threshold == 2400, "");
        learning.Reset();
        check("old-feel learning reset", learning.Threshold == 0 && !new Settings().LearnFromMyArrivals, "");
        core = Rig(); core.LearnFromMyArrivals = true;
        for (int i = 0; i < 200; i++) core.Decide((100, 0), 100, -2, true, true, .001, out _, out _);
        check("old-feel holding an edge counts as one arrival",
            core.Decide((100, 0), 100, -5, true, true, .002, out _, out _) == GuardAction.Block, "");
        for (int i = 0; i < 39; i++)
        {
            core.Decide((100, 10), 100, 9, true, true, .001, out _, out _);
            core.Decide((100, 0), 100, -2, true, true, .001, out _, out _);
        }
        check("old-feel learned threshold reaches the actual guard decision",
            core.Decide((100, 0), 100, -5, true, true, .002, out _, out _) == GuardAction.Pass, "");
        check("old-feel learning is independent by axis",
            core.Decide((2559, 500), 2564, 500, true, true, .002, out _, out _) == GuardAction.Block, "");
        foreach (int hz in new[] { 1000, 2000, 4000, 8000 })
        {
            int pixels = 8000 / hz;
            core = Rig();
            check($"old-feel actual integer guard 8000px/s crosses at {hz}Hz",
                core.Decide((100, 0), 100, -pixels, true, true, 1.0 / hz, out _, out _) == GuardAction.Pass, "");
            core.InstantCrossSpeedPxPerSecond = 12000;
            check($"old-feel actual integer guard 8000px/s held at {hz}Hz with 12000 threshold",
                core.Decide((100, 0), 100, -pixels, true, true, 1.0 / hz, out _, out _) == GuardAction.Block, "");
        }
        core = Rig();
        check("old-feel closed upward gate stays closed at high speed",
            core.Decide((100, 0), 100, -50, false, true, .001, out _, out _) == GuardAction.Block, "");
        core.Confine = true;
        check("old-feel confine overrides high speed",
            core.Decide((100, 0), 100, -50, true, true, .001, out _, out _) == GuardAction.Block, "");
        core = Rig(); core.DeliberateCross = false;
        check("old-feel deliberate off permits an untimed small push",
            core.Decide((100, 0), 100, -1, true, true, double.NaN, out _, out _) == GuardAction.Pass, "");
        core = Rig();
        for (int i = 0; i < 500; i++) core.Decide((100, 0), 100, -1, true, true, .001, out _, out _);
        long allocatedBefore = GC.GetAllocatedBytesForCurrentThread();
        for (int i = 0; i < 10000; i++) core.Decide((100, 0), 100, -1, true, true, .001, out _, out _);
        long allocated = GC.GetAllocatedBytesForCurrentThread() - allocatedBefore;
        check("old-feel timed edge path allocates zero bytes", allocated == 0, $"allocated={allocated}");
        var snapshot = new Settings { SettingsVersion = 4, InstantCrossSpeedPxPerSecond = 12000, LearnFromMyArrivals = true };
        check("old-feel new settings roundtrip", snapshot.DeepClone().InstantCrossSpeedPxPerSecond == 12000
            && snapshot.DeepClone().LearnFromMyArrivals, "");
        snapshot.ResetLayoutConfig();
        check("old-feel reset restores 3000 and learning off", snapshot.InstantCrossSpeedPxPerSecond == 3000 && !snapshot.LearnFromMyArrivals, "");

        // Exact production adapter, including both injected flags; no hook installation or OS input.
        using var guard = new MouseGuard { GateOpen = true, SideContain = true };
        var screens = Screens();
        Native.RECT Rect((int L, int T, int R, int B) r) => new() { Left = r.L, Top = r.T, Right = r.R, Bottom = r.B };
        guard.Configure(new[] { Rect(screens[3].Bounds) }, new[] { (0, 2560) }, screens.Select(s => Rect(s.Bounds)),
            Rig().Transitions, Array.Empty<((int, int, int, int), (int, int, int, int))>(),
            Array.Empty<Native.RECT>(), true, Rect(screens[0].Bounds));
        long tick = System.Diagnostics.Stopwatch.Frequency / 4000;
        var data = new Native.MSLLHOOKSTRUCT { pt = new() { X = 100, Y = 0 }, time = 10 };
        guard.DecideHookMove(data, (100, 1), tick * 40, out _, out _);
        foreach (uint flag in new[] { Native.LLMHF_INJECTED, Native.LLMHF_LOWER_IL_INJECTED })
        {
            data.flags = flag; data.pt.Y = -50;
            check($"old-feel injected flag {flag} bypasses timing", guard.DecideHookMove(data, (100, 0), tick * 40 + 1, out _, out _) == GuardAction.Pass, "");
        }
        data.flags = 0; data.pt.Y = -1;
        check("old-feel actual hook adapter refines equal message timestamps",
            guard.DecideHookMove(data, (100, 0), tick * 41, out _, out _) == GuardAction.Pass, "");
        foreach (string language in new[] { "en", "tr" })
        {
            Strings.Use(language);
            using var form = new SettingsForm(new Settings { SettingsVersion = 4 }, Screenshot.DemoMonitors(), showWithoutActivation: true);
            IEnumerable<Control> Controls(Control control) => new[] { control }.Concat(control.Controls.Cast<Control>().SelectMany(Controls));
            var controls = Controls(form).ToList();
            var row = controls.Single(c => c.Name == "InstantSpeedRow");
            var number = Controls(row).OfType<NumericUpDown>().Single();
            var checkbox = controls.OfType<CheckBox>().Single(c => c.Name == "LearnFromMyArrivals");
            number.Value = 12000; checkbox.Checked = true;
            var draft = (Settings)typeof(SettingsForm).GetField("_draft", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic).GetValue(form);
            check($"old-feel UI {language}: speed and learning edit transactional draft",
                draft.InstantCrossSpeedPxPerSecond == 12000 && draft.LearnFromMyArrivals, "");
        }
        Strings.Use("en");

        // Keep the platform limit observable: the same continuous speed quantizes differently at different Hz.
        bool at1k = InstantaneousPush.Crosses(Math.Round(1600.0 / 1000), 0, .001, 3000, 5);
        bool at4k = InstantaneousPush.Crosses(Math.Round(1600.0 / 4000), 0, .00025, 3000, 5);
        bool next4k = InstantaneousPush.Crosses(1, 0, .00025, 3000, 5);
        check("old-feel pixel quantization limitation is exposed, not an equivalence claim", !at1k && !at4k && next4k, "");

        if (Environment.GetEnvironmentVariable("MOUSEFENCE_EVIDENCE_DIR") is { Length: > 0 } output)
            CalibrationEvidence(check, output);
    }

    private static void CalibrationEvidence(Action<string, bool, string> check, string output)
    {
        string input = Environment.GetEnvironmentVariable("MOUSEFENCE_INPUT_DIR")
            ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "MouseFence");
        using var calibration = JsonDocument.Parse(File.ReadAllText(Path.Combine(input, "calibration.json")));
        string settingsText = File.ReadAllText(Path.Combine(input, "settings.json"));
        var settings = JsonSerializer.Deserialize<Settings>(settingsText).DeepClone();
        using var before = JsonDocument.Parse(settingsText);
        using var after = JsonDocument.Parse(JsonSerializer.Serialize(settings));
        foreach (var property in before.RootElement.EnumerateObject())
            check($"old-feel live v4 setting retained: {property.Name}",
                after.RootElement.TryGetProperty(property.Name, out var value)
                && JsonElement.DeepEquals(property.Value, value), "");
        var lines = new List<string> { "axis,sample,speed_px_s,hz,dt_ms,ideal_primary_px,ideal_cross,rounded_primary_px,rounded_cross,v111_4000_cross" };
        int count = 0;
        foreach (string axis in new[] { "Up", "Side" })
        {
            int n = calibration.RootElement.GetProperty(axis).GetProperty("RecentCount").GetInt32();
            var speeds = calibration.RootElement.GetProperty(axis).GetProperty("Recent").EnumerateArray().Take(n).ToArray();
            check($"old-feel real {axis} calibration is nonempty", speeds.Length > 0, "");
            for (int i = 0; i < speeds.Length; i++)
            {
                double speed = speeds[i].GetDouble();
                foreach (int hz in new[] { 1000, 2000, 4000, 8000 })
                {
                    var clock = new HookEventClock();
                    clock.Interval(0, 8000, 8000000);
                    double seconds = clock.Interval((uint)(1000 / hz), 8000 + 8000000 / hz, 8000000);
                    double primary = speed / hz;
                    bool crosses = InstantaneousPush.Crosses(primary, 0, seconds, 3000, 5);
                    int rounded = (int)Math.Round(primary, MidpointRounding.AwayFromZero);
                    var core = Rig();
                    var action = axis == "Up"
                        ? core.Decide((100, 0), 100, -rounded, true, true, seconds, out _, out _)
                        : core.Decide((2559, 500), 2559 + rounded, 500, true, true, seconds, out _, out _);
                    bool actualCross = rounded > 0 && action == GuardAction.Pass;
                    bool original4k = Math.Round(speed / 4000, MidpointRounding.AwayFromZero) >= 3;
                    lines.Add(FormattableString.Invariant($"{axis},{i + 1},{speed:R},{hz},{seconds * 1000:R},{primary:R},{crosses},{rounded},{actualCross},{original4k}"));
                    check($"old-feel real speed ideal replay {axis}/{i + 1}/{hz}", crosses == (speed >= 3000), "");
                    count++;
                }
            }
        }
        check("old-feel real arrival table has nonzero rows", count > 0, $"rows={count}");
        Directory.CreateDirectory(output);
        File.WriteAllLines(Path.Combine(output, "per-hz-equivalence.csv"), lines);
    }

    internal static List<BoundaryScreen> Screens() => new()
    {
        new() { StableId = "MAIN", Primary = true, Bounds = (0, 0, 2560, 1440) },
        new() { StableId = "LEFT", Bounds = (-1920, 0, 0, 1080) },
        new() { StableId = "RIGHT", Bounds = (2560, 0, 4480, 1080) },
        new() { StableId = "TOP", Bounds = (-421, -1440, 3019, 0) },
    };

    internal static GuardCore Rig()
    {
        var screens = Screens();
        var settings = new Settings { SettingsVersion = 4, PassageInsetPx = 0 };
        return new GuardCore
        {
            HasTop = true, BarrierY = 0, HasSides = true,
            MainL = 0, MainT = 0, MainR = 2560, MainB = 1440,
            Monitors = screens.Select(s => s.Bounds).ToList(),
            Transitions = BoundaryGraph.Compile(settings, screens).Transitions,
        };
    }

    internal static void Regressions(Action<string, bool, string> check)
    {
        foreach (int x in new[] { -1, 2560 })
        {
            var core = Rig();
            core.LastX = x; core.LastY = 1079; core.HaveLast = true;
            var action = core.Decide(x, 1081, true, true, out int bx, out int by);
            check($"old-feel corner: {x} bottom drift stays on source",
                action == GuardAction.Block && bx == x && by == 1079, $"{action} ({bx},{by})");
        }
        const string json = """{"SettingsVersion":4,"PushIntentMode":"Pressure","FutureField":{"Nested":[1,2,3]},"UpCrossMin":7,"SideCrossMin":9}""";
        var original = JsonSerializer.Deserialize<Settings>(json);
        var clone = original.DeepClone();
        using var result = JsonDocument.Parse(JsonSerializer.Serialize(clone));
        check("old-feel settings: unknown v4 fields survive clone",
            result.RootElement.TryGetProperty("FutureField", out var extra)
            && extra.GetProperty("Nested").GetArrayLength() == 3
            && result.RootElement.GetProperty("PushIntentMode").GetString() == "Pressure",
            result.RootElement.ToString());
    }
}
