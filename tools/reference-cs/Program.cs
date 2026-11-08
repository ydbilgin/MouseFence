// MouseFence 2.0 parity reference: answers JSON-lines requests with the behaviour of the untouched C# 1.1.1
// sources. One request per input line, one compact JSON response per output line, until stdin closes.
// Verbs and semantics: crates/fence-trace/src/case.rs. Errors come back as {"error": "..."}.
//
// The Configure() port below copies the pure part of Ui/TrayApplicationContext.cs:134-237 and
// Platform/MouseGuard.cs:84-110 statement for statement (they live in WinForms classes that cannot be
// constructed here). Keep it in step with those lines; nothing else in this file decides behaviour.
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace MouseFence.Reference;

internal static class Program
{
    private const int GateInset = 24;   // TrayApplicationContext.GateInset

    private static int Main()
    {
        Console.InputEncoding = new UTF8Encoding(false);
        Console.OutputEncoding = new UTF8Encoding(false);
        var output = Console.Out;
        string line;
        while ((line = Console.ReadLine()) != null)
        {
            if (line.Length == 0) continue;
            JsonNode response;
            try
            {
                var req = JsonNode.Parse(line)!.AsObject();
                response = req["verb"]?.GetValue<string>() switch
                {
                    "run" => Run(req),
                    "normalize" => Normalize(req),
                    "keys" => KeyNames(),
                    "helper" => Helper(req),
                    // Track B verbs (shared file, additive 2026-10-13): key names, text rules, settings flows.
                    "keys_enum" => KeysEnum(),
                    "keys_text" => KeysText(req),
                    "case_groups" => CaseGroups(req),
                    "text_elements" => TextElements(req),
                    "settings_op" => SettingsOp(req),
                    "load_sim" => LoadSim(req),
                    "core" => CoreRun(req),
                    "is_top" => new JsonObject { ["result"] = GuardCore.IsTop(RectArr(req["rect"]!)) },
                    var v => throw new ArgumentException($"unknown verb '{v}'"),
                };
                if (req["id"] != null) response["id"] = req["id"]!.DeepClone();
            }
            catch (Exception ex)
            {
                response = new JsonObject { ["error"] = ex.GetType().Name + ": " + ex.Message };
            }
            output.WriteLine(response.ToJsonString());
            output.Flush();
        }
        return 0;
    }

    // ---------------------------------------------------------------- inputs

    private static (int L, int T, int R, int B) Rect(JsonNode n) =>
        (n["l"]!.GetValue<int>(), n["t"]!.GetValue<int>(), n["r"]!.GetValue<int>(), n["b"]!.GetValue<int>());

    private static (int L, int T, int R, int B) RectArr(JsonNode n)
    {
        var a = n.AsArray();
        return (a[0]!.GetValue<int>(), a[1]!.GetValue<int>(), a[2]!.GetValue<int>(), a[3]!.GetValue<int>());
    }

    /// <summary>MonitorInfo.All() without Win32: SortAndNumber, then DisambiguateStableIds (MonitorInfo.cs:62-90).</summary>
    private static List<MonitorInfo> Monitors(JsonNode screens)
    {
        var list = new List<MonitorInfo>();
        if (screens == null) return list;
        foreach (var s in screens.AsArray())
        {
            var r = Rect(s!);
            list.Add(new MonitorInfo
            {
                Device = s!["device"]!.GetValue<string>(),
                StableId = s["id"]!.GetValue<string>(),
                Bounds = new Native.RECT { Left = r.L, Top = r.T, Right = r.R, Bottom = r.B },
                Primary = s["primary"]!.GetValue<bool>(),
            });
        }
        MonitorInfo.SortAndNumber(list);
        var keys = GuardCore.DisambiguateKeys(
            list.Select(m => (m.StableId, m.Bounds.Left, m.Bounds.Top,
                              RawIdentityKey: (m.StableId ?? "") + "\n" + (m.Device ?? ""))).ToList());
        for (int i = 0; i < list.Count; i++) list[i].StableId = keys[i];
        return list;
    }

    private static Settings ReadSettings(JsonNode node)
    {
        if (node == null) return new Settings { SettingsVersion = Settings.CurrentSettingsVersion };
        var s = JsonSerializer.Deserialize<Settings>(node.ToJsonString()) ?? new Settings();
        s.Normalize();
        return s;
    }

    // ---------------------------------------------------------------- run

    private sealed class Plan
    {
        public GuardCore Core;
        public JsonObject Json;
    }

    /// <summary>TrayApplicationContext.Configure (pure part) followed by MouseGuard.Configure.</summary>
    private static Plan Configure(Settings settings, List<MonitorInfo> allMonitors)
    {
        settings.Normalize();

        var primaryStable = (allMonitors.FirstOrDefault(m => m.Primary) ?? allMonitors.FirstOrDefault())?.StableId;
        var requested = new HashSet<string>(settings.ExcludedDevices, StringComparer.OrdinalIgnoreCase);
        var presentStable = allMonitors.Select(m => m.StableId).ToList();
        var excludedStable = new HashSet<string>(
            GuardCore.SanitizeExcluded(presentStable, primaryStable, requested, out bool pruned));
        var excludedMonitors = allMonitors.Where(m => excludedStable.Contains(m.StableId)).ToList();
        var monitors = allMonitors.Where(m => !excludedStable.Contains(m.StableId)).ToList();

        var tops = (settings.Mode == "Manual"
            ? monitors.Where(m => settings.ManualMonitors.Contains(m.StableId))
            : monitors.Where(MonitorInfo.IsAbovePrimary)).ToList();
        var topSet = new HashSet<string>(tops.Select(t => t.StableId));
        var primary = monitors.FirstOrDefault(m => m.Primary) ?? monitors.FirstOrDefault();

        var links = settings.UpwardBoundaryDefault == BoundaryDefault.Blocked
            ? settings.UpLinks
            : (primary != null
                ? tops.Select(t => new UpLink { FromDevice = primary.StableId, ToDevice = t.StableId }).ToList()
                : new List<UpLink>());

        var byStable = monitors.ToDictionary(m => m.StableId);
        var gates = new List<(int Min, int Max)>();
        foreach (var lk in links)
        {
            if (!topSet.Contains(lk.ToDevice)) continue;
            if (!byStable.TryGetValue(lk.FromDevice, out var from)) continue;
            if (!byStable.TryGetValue(lk.ToDevice, out var to)) continue;
            int lo = Math.Max(from.Bounds.Left, to.Bounds.Left);
            int hi = Math.Min(from.Bounds.Right, to.Bounds.Right);
            if (hi <= lo) continue;
            int inset = Math.Min(GateInset, Math.Max(0, (hi - lo) / 4));
            gates.Add((lo + inset, hi - inset));
        }

        var rects = monitors.Select(m => (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom)).ToList();
        var topIdx = new HashSet<int>();
        for (int i = 0; i < monitors.Count; i++)
            if (topSet.Contains(monitors[i].StableId)) topIdx.Add(i);
        var (warn, _) = GuardCore.AntiTrap(rects, topIdx);

        var compiled = BoundaryGraph.Compile(settings, BoundaryGraph.FromMonitors(monitors));

        var deviceRects = byStable.ToDictionary(
            kv => kv.Key,
            kv => (kv.Value.Bounds.Left, kv.Value.Bounds.Top, kv.Value.Bounds.Right, kv.Value.Bounds.Bottom));
        var compiledUpLinks = compiled.Transitions
            .Where(t => t.Direction == BoundaryDirection.Up && t.Behavior != BoundaryBehavior.Blocked)
            .Select(t => (t.FromStableId, t.ToStableId)).Distinct().ToList();
        var routes = GuardCore.DeriveRoutes(compiledUpLinks, deviceRects, topSet);

        var main = primary;
        bool hasSides = false;
        if (main != null)
        {
            var mainRect = (main.Bounds.Left, main.Bounds.Top, main.Bounds.Right, main.Bounds.Bottom);
            hasSides = monitors.Any(m => m.StableId != main.StableId &&
                (GuardCore.Adjacent(mainRect, (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom), Dir.Left) ||
                 GuardCore.Adjacent(mainRect, (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom), Dir.Right)));
        }
        var mainBounds = main?.Bounds ?? default;

        // ---- MouseGuard.Configure
        var core = new GuardCore();
        var blocked = tops.Select(t => t.Bounds).ToList();
        core.HasTop = blocked.Count > 0;
        core.BarrierY = core.HasTop ? blocked.Max(r => r.Bottom) : 0;
        core.Gates = gates.Where(g => g.Max > g.Min).ToList();
        core.Monitors = monitors.Select(m => (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom)).ToList();
        core.SafetyGates.Clear();
        core.Transitions = compiled.Transitions.ToArray();
        core.DescentRoutes = routes.ToList();
        core.Forbidden = excludedMonitors.Select(m => (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom))
                                         .Where(r => r.Right > r.Left && r.Bottom > r.Top).ToList();
        core.HasSides = hasSides;
        core.MainL = mainBounds.Left; core.MainR = mainBounds.Right;
        core.MainT = mainBounds.Top; core.MainB = mainBounds.Bottom;
        core.Reset();
        // ---- back in TrayApplicationContext.Configure
        core.DeliberateCross = settings.DeliberateCross;
        core.DescentRouting = settings.DescentRouting;
        core.SideCrossMin = Math.Max(1, settings.SideCrossMin);
        core.SideCrossSlack = Math.Max(0, settings.SideCrossSlack);

        var json = new JsonObject
        {
            ["monitors"] = new JsonArray(allMonitors.Select(m => (JsonNode)new JsonObject
            {
                ["id"] = m.StableId, ["device"] = m.Device, ["index"] = m.Index, ["primary"] = m.Primary,
                ["rect"] = R((m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom)),
            }).ToArray()),
            ["excluded"] = Strs(excludedMonitors.Select(m => m.StableId)),
            ["pruned"] = pruned,
            ["tops"] = Strs(tops.Select(t => t.StableId)),
            ["has_top"] = core.HasTop,
            ["barrier_y"] = core.BarrierY,
            ["gates"] = new JsonArray(core.Gates.Select(g => (JsonNode)new JsonArray(g.Min, g.Max)).ToArray()),
            ["usable"] = new JsonArray(core.Monitors.Select(r => (JsonNode)R(r)).ToArray()),
            ["transitions"] = new JsonArray(compiled.Transitions.Select(T).ToArray()),
            ["sealed"] = new JsonArray(compiled.SealedScreens.Select(Issue).ToArray()),
            ["has_safety_demotions"] = compiled.HasSafetyDemotions,
            ["routes"] = new JsonArray(core.DescentRoutes.Select(r => (JsonNode)new JsonArray(R(r.Top), R(r.Landing))).ToArray()),
            ["forbidden"] = new JsonArray(core.Forbidden.Select(r => (JsonNode)R(r)).ToArray()),
            ["has_sides"] = hasSides,
            ["main"] = R((mainBounds.Left, mainBounds.Top, mainBounds.Right, mainBounds.Bottom)),
            ["isolated"] = Strs(warn.Select(i => monitors[i].StableId)),
            ["deliberate_cross"] = core.DeliberateCross,
            ["descent_routing"] = core.DescentRouting,
            ["side_cross_min"] = core.SideCrossMin,
            ["side_cross_slack"] = core.SideCrossSlack,
            ["cross_min_up"] = core.CrossMinUp,
            ["cross_slack"] = core.CrossSlack,
        };
        return new Plan { Core = core, Json = json };
    }

    private static JsonObject Run(JsonObject req)
    {
        var settings = ReadSettings(req["settings"]);
        var plan = Configure(settings, Monitors(req["screens"]));
        var core = plan.Core;
        var toggles = req["toggles"];
        bool gate = toggles?["gate_open"]?.GetValue<bool>() ?? !settings.StartGateClosed;
        bool side = toggles?["side_contain"]?.GetValue<bool>() ?? settings.StartSideContainOn;
        core.Confine = toggles?["confine"]?.GetValue<bool>() ?? false;
        bool running = true;   // MouseGuard.Start() ran at startup

        var results = new JsonArray();
        int i = 0;
        foreach (var op in req["ops"]?.AsArray() ?? new JsonArray())
        {
            string kind = op!["op"]!.GetValue<string>();
            string act = "Ok";
            int x = 0, y = 0;
            switch (kind)
            {
                case "move":
                    x = op["x"]!.GetValue<int>(); y = op["y"]!.GetValue<int>();
                    if (!running || !core.NeedsProcessing) act = "Unprocessed";
                    else if (core.Decide(x, y, gate, side, out int bx, out int by) == GuardAction.Block)
                    { act = "Block"; x = bx; y = by; }
                    else act = "Pass";
                    break;
                case "gate": gate = op["on"]!.GetValue<bool>(); break;
                case "side": side = op["on"]!.GetValue<bool>(); break;
                case "confine": core.Confine = op["on"]!.GetValue<bool>(); break;
                case "pause": running = false; break;
                case "resume": if (!running) { running = true; core.Reset(); } break;
                case "reset": core.Reset(); break;
                default: throw new ArgumentException($"unknown op '{kind}'");
            }
            results.Add(new JsonObject
            {
                ["i"] = i++, ["act"] = act, ["x"] = x, ["y"] = y, ["on_top"] = core.OnTop, ["have_last"] = core.HaveLast,
            });
        }
        return new JsonObject { ["plan"] = plan.Json, ["results"] = results };
    }

    // ---------------------------------------------------------------- normalize

    /// <summary>Settings.Load (without file I/O) followed by what Settings.Save would write.</summary>
    private static JsonObject Normalize(JsonObject req)
    {
        string json = req["json"]!.GetValue<string>();
        var monitors = req["screens"] != null ? Monitors(req["screens"]) : null;
        var res = new JsonObject();
        int loadedVersion = Settings.ReadVersion(json);
        res["loaded_version"] = loadedVersion;
        Settings s;
        try
        {
            s = JsonSerializer.Deserialize<Settings>(json) ?? new Settings();
            s.Normalize();
        }
        catch (Exception ex)
        {
            res["parse_error"] = ex.GetType().Name;
            return res;
        }
        if (loadedVersion < Settings.CurrentSettingsVersion && monitors != null)
        {
            s.MigrateLayoutKeysToStable(monitors);
            s.SettingsVersion = Settings.CurrentSettingsVersion;
        }
        s.Normalize();
        s.SettingsVersion = Settings.CurrentSettingsVersion;
        res["repairable"] = s.HasRepairableImportErrors;
        res["json"] = JsonSerializer.Serialize(s, new JsonSerializerOptions { WriteIndented = true });
        res["texts"] = new JsonObject
        {
            ["hot"] = s.HotKeyText(), ["confine"] = s.ConfineHotKeyText(),
            ["pause"] = s.PauseHotKeyText(), ["side"] = s.SideHotKeyText(),
        };
        res["modifiers"] = new JsonArray(s.Modifiers(), s.ConfineModifiers(), s.PauseModifiers(), s.SideModifiers());
        if (monitors != null)
            res["sealed"] = new JsonArray(s.ValidateNoSeal(monitors).Select(Issue).ToArray());
        return res;
    }

    // ---------------------------------------------------------------- keys

    private static JsonObject KeyNames()
    {
        var names = new JsonObject();
        for (int vk = 0; vk < 256; vk++) names[vk.ToString()] = ((Keys)vk).ToString();
        return new JsonObject { ["names"] = names };
    }

    // ---------------------------------------------------------------- helpers

    private static JsonObject Helper(JsonObject req)
    {
        string fn = req["fn"]!.GetValue<string>();
        var a = req["args"]!;
        JsonNode result = fn switch
        {
            "MapLayout" => new JsonArray(GuardCore.MapLayout(
                    a["rects"]!.AsArray().Select(r => RectArr(r!)).ToList(),
                    a["w"]!.GetValue<int>(), a["h"]!.GetValue<int>(), a["pad"]!.GetValue<int>())
                .Select(r => (JsonNode)R(r)).ToArray()),
            "DisambiguateKeys" => Strs(GuardCore.DisambiguateKeys(a["items"]!.AsArray().Select(n =>
                (n!["key"]!.GetValue<string>(), n["left"]!.GetValue<int>(), n["top"]!.GetValue<int>(),
                 RawIdentityKey: n["raw"]!.GetValue<string>())).ToList())),
            "SanitizeExcluded" => SanitizeExcluded(a),
            "MigrateLayoutKeysToStable" => Migrate(a),
            "MonitorLabel" => GuardCore.MonitorLabel(a["ordinal"]!.GetValue<int>(), a["width"]!.GetValue<int>(),
                a["height"]!.GetValue<int>(), a["primary"]!.GetValue<bool>(), a["main_tag"]?.GetValue<string>(),
                a["name"]?.GetValue<string>()),
            "IsArrowRotationHotkey" => GuardCore.IsArrowRotationHotkey(a["vk"]!.GetValue<int>(), a["ctrl"]!.GetValue<bool>(),
                a["alt"]!.GetValue<bool>(), a["shift"]!.GetValue<bool>(), a["win"]!.GetValue<bool>()),
            "NormalizeMonitorName" => Settings.NormalizeMonitorName(a["value"]?.GetValue<string>()),
            "AntiTrap" => AntiTrap(a),
            "Adjacent" => GuardCore.Adjacent(RectArr(a["a"]!), RectArr(a["b"]!), Enum.Parse<Dir>(a["dir"]!.GetValue<string>())),
            "DeriveRoutes" => DeriveRoutes(a),
            "SortAndNumber" => SortAndNumber(a),
            "FreshIntel" => FreshIntel(),
            _ => throw new ArgumentException($"unknown helper '{fn}'"),
        };
        return new JsonObject { ["result"] = result };
    }

    private static JsonNode SanitizeExcluded(JsonNode a)
    {
        var present = a["present"]!.AsArray().Select(n => n!.GetValue<string>()).ToList();
        var primary = a["primary"]?.GetValue<string>();
        var requested = new HashSet<string>(a["requested"]!.AsArray().Select(n => n!.GetValue<string>()),
                                            StringComparer.OrdinalIgnoreCase);
        var excluded = GuardCore.SanitizeExcluded(present, primary, requested, out bool pruned);
        return new JsonObject { ["excluded"] = Strs(excluded), ["pruned"] = pruned };
    }

    private static JsonNode Migrate(JsonNode a)
    {
        var manual = a["manual"]!.AsArray().Select(n => n?.GetValue<string>()).ToList();
        var links = a["links"]!.AsArray().Select(n => (n![0]?.GetValue<string>(), n[1]?.GetValue<string>())).ToList();
        var map = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var pair in a["map"]!.AsArray()) map[pair![0]!.GetValue<string>()] = pair[1]!.GetValue<string>();
        var (m, l) = GuardCore.MigrateLayoutKeysToStable(manual, links, map);
        return new JsonObject
        {
            ["manual"] = Strs(m),
            ["links"] = new JsonArray(l.Select(x => (JsonNode)new JsonArray(x.From, x.To)).ToArray()),
        };
    }

    private static JsonNode AntiTrap(JsonNode a)
    {
        var mons = a["rects"]!.AsArray().Select(r => RectArr(r!)).ToList();
        var top = new HashSet<int>(a["top_idx"]!.AsArray().Select(n => n!.GetValue<int>()));
        var (warn, gates) = GuardCore.AntiTrap(mons, top);
        return new JsonObject
        {
            ["warn"] = new JsonArray(warn.Select(i => (JsonNode)i).ToArray()),
            ["gates"] = new JsonArray(gates.Select(g => (JsonNode)new JsonArray(g.Min, g.Max, g.OwnerT, g.OwnerB)).ToArray()),
        };
    }

    private static JsonNode DeriveRoutes(JsonNode a)
    {
        var links = a["links"]!.AsArray().Select(n => (n![0]!.GetValue<string>(), n[1]!.GetValue<string>())).ToList();
        var rects = new Dictionary<string, (int L, int T, int R, int B)>();
        foreach (var pair in a["rects"]!.AsArray()) rects[pair![0]!.GetValue<string>()] = RectArr(pair[1]!);
        var tops = new HashSet<string>(a["tops"]!.AsArray().Select(n => n!.GetValue<string>()));
        var routes = GuardCore.DeriveRoutes(links, rects, tops);
        return new JsonArray(routes.Select(r => (JsonNode)new JsonArray(R(r.Top), R(r.Landing))).ToArray());
    }

    private static JsonNode SortAndNumber(JsonNode a)
    {
        var list = new List<MonitorInfo>();
        foreach (var s in a["screens"]!.AsArray())
        {
            var r = Rect(s!);
            list.Add(new MonitorInfo
            {
                Device = s!["device"]!.GetValue<string>(), StableId = s["id"]!.GetValue<string>(),
                Bounds = new Native.RECT { Left = r.L, Top = r.T, Right = r.R, Bottom = r.B },
                Primary = s["primary"]!.GetValue<bool>(),
            });
        }
        MonitorInfo.SortAndNumber(list);
        return new JsonArray(list.Select(m => (JsonNode)new JsonObject
        {
            ["id"] = m.StableId, ["device"] = m.Device, ["index"] = m.Index,
        }).ToArray());
    }

    private static JsonNode FreshIntel()
    {
        var fresh = new Settings { SettingsVersion = Settings.CurrentSettingsVersion };
        fresh.AddShiftToArrowHotkeys();
        return new JsonObject
        {
            ["json"] = JsonSerializer.Serialize(fresh, new JsonSerializerOptions { WriteIndented = true }),
            ["intel_defaults_applied"] = fresh.IntelDefaultsApplied,
        };
    }

    // ---------------------------------------------------------------- Track B verbs (additive, 2026-10-13)
    // Semantics: crates/fence-trace/tests/config_reference_diff.rs. They only call the untouched 1.1.1 code and
    // the .NET runtime (Enum, StringComparer, StringInfo, string.Trim) whose behaviour the Rust port copies.

    /// <summary>Enum.GetNames/GetValues of WinForms Keys, in .NET's own order (by unsigned value).</summary>
    private static JsonObject KeysEnum()
    {
        var names = Enum.GetNames(typeof(Keys));
        var values = (Keys[])Enum.GetValues(typeof(Keys));
        return new JsonObject
        {
            ["names"] = Strs(names),
            ["values"] = new JsonArray(values.Select(v => (JsonNode)JsonValue.Create((int)v)).ToArray()),
        };
    }

    /// <summary>((Keys)value).ToString() for each value (Settings.HotKeyText uses it).</summary>
    private static JsonObject KeysText(JsonObject req) => new()
    {
        ["texts"] = Strs(req["values"]!.AsArray().Select(n => ((Keys)n!.GetValue<int>()).ToString())),
    };

    /// <summary>OrdinalIgnoreCase grouping (index of the first equal string) and compare signs for pairs.</summary>
    private static JsonObject CaseGroups(JsonObject req)
    {
        var strings = (req["strings"]?.AsArray() ?? new JsonArray()).Select(n => n!.GetValue<string>()).ToList();
        var first = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
        var groups = new JsonArray();
        for (int i = 0; i < strings.Count; i++)
        {
            if (!first.TryGetValue(strings[i], out int f)) { f = i; first[strings[i]] = i; }
            groups.Add(f);
        }
        var ignoreCase = new JsonArray();
        var ordinal = new JsonArray();
        foreach (var p in req["pairs"]?.AsArray() ?? new JsonArray())
        {
            string a = p![0]!.GetValue<string>(), b = p[1]!.GetValue<string>();
            ignoreCase.Add(Math.Sign(StringComparer.OrdinalIgnoreCase.Compare(a, b)));
            ordinal.Add(Math.Sign(StringComparer.Ordinal.Compare(a, b)));
        }
        return new JsonObject { ["groups"] = groups, ["ignore_case"] = ignoreCase, ["ordinal"] = ordinal };
    }

    /// <summary>Text-element starts (UTF-16 offsets), Trim(), IsNullOrWhiteSpace and NormalizeMonitorName.</summary>
    private static JsonObject TextElements(JsonObject req)
    {
        var values = req["values"]!.AsArray().Select(n => n?.GetValue<string>()).ToList();
        return new JsonObject
        {
            ["starts"] = new JsonArray(values.Select(v => (JsonNode)new JsonArray(
                System.Globalization.StringInfo.ParseCombiningCharacters(v ?? "").Select(i => (JsonNode)i).ToArray())).ToArray()),
            ["trimmed"] = Strs(values.Select(v => v?.Trim())),
            ["blank"] = new JsonArray(values.Select(v => (JsonNode)string.IsNullOrWhiteSpace(v)).ToArray()),
            ["normalized"] = Strs(values.Select(Settings.NormalizeMonitorName)),
        };
    }

    /// <summary>Deserialize + Normalize (as ReadSettings), then one Settings method; returns the serialized result
    /// without a further Normalize so the method's own effect is visible.</summary>
    private static JsonObject SettingsOp(JsonObject req)
    {
        string json = req["json"]!.GetValue<string>();
        string op = req["op"]!.GetValue<string>();
        var monitors = req["screens"] != null ? Monitors(req["screens"]) : new List<MonitorInfo>();
        var s = JsonSerializer.Deserialize<Settings>(json) ?? new Settings();
        s.Normalize();
        var res = new JsonObject();
        switch (op)
        {
            case "AddShiftToArrowHotkeys":
                s.AddShiftToArrowHotkeys();
                res["intel_defaults_applied"] = s.IntelDefaultsApplied;
                break;
            case "ResetLayoutConfig":
                s.ResetLayoutConfig();
                break;
            case "DeepClone":
                s = s.DeepClone();
                break;
            case "MonitorNameFor":
                res["name"] = Settings.MonitorNameFor(s.MonitorNames, req["stable_id"]?.GetValue<string>());
                break;
            case "Computed":
                res["crossing_path_mode"] = s.CrossingPathMode;
                res["up_links"] = new JsonArray(s.UpLinks.Select(l => (JsonNode)Strs(new[] { l.FromDevice, l.ToDevice })).ToArray());
                break;
            case "ValidateNoSeal":
                res["sealed"] = new JsonArray(s.ValidateNoSeal(monitors).Select(Issue).ToArray());
                break;
            default:
                throw new ArgumentException($"unknown settings op '{op}'");
        }
        res["repairable"] = s.HasRepairableImportErrors;
        res["json"] = JsonSerializer.Serialize(s, new JsonSerializerOptions { WriteIndented = true });
        return res;
    }

    /// <summary>Settings.Load() without files (Settings.cs:163-189): what 1.x keeps in memory, whether its migration
    /// ran, and what the migration's Save() would write. Save's seal check is reported as "sealed" (it is the
    /// boundary compiler's answer); "saved_json" is the content Save writes when that list is empty.</summary>
    private static JsonObject LoadSim(JsonObject req)
    {
        string json = req["json"]!.GetValue<string>();
        var monitors = req["screens"] != null ? Monitors(req["screens"]) : new List<MonitorInfo>();
        var res = new JsonObject();
        int loadedVersion = Settings.ReadVersion(json);
        res["loaded_version"] = loadedVersion;
        Settings settings;
        try
        {
            settings = JsonSerializer.Deserialize<Settings>(json) ?? new Settings();
            settings.Normalize();
        }
        catch (Exception ex)
        {
            res["parse_error"] = ex.GetType().Name;
            return res;
        }
        res["migration"] = "none";
        if (loadedVersion < Settings.CurrentSettingsVersion)
        {
            try
            {
                settings.MigrateLayoutKeysToStable(monitors);
                settings.SettingsVersion = Settings.CurrentSettingsVersion;
                // Save(): Normalize, stamp the version, refuse repairable errors, refuse a sealed layout, write.
                settings.Normalize();
                settings.SettingsVersion = Settings.CurrentSettingsVersion;
                if (settings.HasRepairableImportErrors)
                {
                    res["migration"] = "refused_repairable";
                }
                else
                {
                    res["migration"] = "save";
                    res["sealed"] = new JsonArray(settings.ValidateNoSeal(monitors).Select(Issue).ToArray());
                    res["saved_json"] = JsonSerializer.Serialize(settings, new JsonSerializerOptions { WriteIndented = true });
                }
            }
            catch (Exception ex)
            {
                res["migration"] = "threw:" + ex.GetType().Name;
            }
        }
        res["repairable"] = settings.HasRepairableImportErrors;
        res["json"] = JsonSerializer.Serialize(settings, new JsonSerializerOptions { WriteIndented = true });
        return res;
    }

    // ---------------------------------------------------------------- core (Track A, shared additive)

    /// <summary>A GuardCore built field by field, the way the 1.1.1 tests build it (no Configure), optionally
    /// with transitions from BoundaryGraph.Compile over raw settings and screens (no Normalize, as the tests'
    /// GraphCore), then a list of ops. Every move reports bx/by (the target itself for Pass).</summary>
    private static JsonObject CoreRun(JsonObject req)
    {
        var c = req["core"] ?? new JsonObject();
        List<(int L, int T, int R, int B)> Rects(string name) =>
            c[name]?.AsArray().Select(r => RectArr(r!)).ToList() ?? new List<(int L, int T, int R, int B)>();
        var core = new GuardCore
        {
            HasTop = c["has_top"]?.GetValue<bool>() ?? false,
            BarrierY = c["barrier_y"]?.GetValue<int>() ?? 0,
            Gates = c["gates"]?.AsArray().Select(g => (g![0]!.GetValue<int>(), g[1]!.GetValue<int>())).ToList()
                    ?? new List<(int Min, int Max)>(),
            SafetyGates = c["safety_gates"]?.AsArray().Select(g => (g![0]!.GetValue<int>(), g[1]!.GetValue<int>(),
                              g[2]!.GetValue<int>(), g[3]!.GetValue<int>())).ToList()
                          ?? new List<(int Min, int Max, int OwnerT, int OwnerB)>(),
            Monitors = Rects("monitors"),
            Forbidden = Rects("forbidden"),
            HasSides = c["has_sides"]?.GetValue<bool>() ?? false,
            DeliberateCross = c["deliberate_cross"]?.GetValue<bool>() ?? true,
            DescentRouting = c["descent_routing"]?.GetValue<bool>() ?? false,
            DescentRoutes = c["descent_routes"]?.AsArray().Select(p => (RectArr(p![0]!), RectArr(p[1]!))).ToList()
                            ?? new List<((int L, int T, int R, int B) Top, (int L, int T, int R, int B) Landing)>(),
            CrossMinUp = c["cross_min_up"]?.GetValue<int>() ?? 3,
            CrossSlack = c["cross_slack"]?.GetValue<int>() ?? 5,
            SideCrossMin = c["side_cross_min"]?.GetValue<int>() ?? 3,
            SideCrossSlack = c["side_cross_slack"]?.GetValue<int>() ?? 5,
            Confine = c["confine"]?.GetValue<bool>() ?? false,
        };
        if (c["main"] != null)
        {
            var m = RectArr(c["main"]!);
            core.MainL = m.L; core.MainT = m.T; core.MainR = m.R; core.MainB = m.B;
        }

        var res = new JsonObject();
        if (req["graph"] is JsonNode g)
        {
            var settings = g["settings"] == null
                ? new Settings()
                : JsonSerializer.Deserialize<Settings>(g["settings"]!.ToJsonString()) ?? new Settings();
            var screens = g["screens"]!.AsArray().Select(s => new BoundaryScreen
            {
                StableId = s!["id"]!.GetValue<string>(),
                Bounds = RectArr(s["rect"]!),
                Primary = s["primary"]?.GetValue<bool>() ?? false,
                Excluded = s["excluded"]?.GetValue<bool>() ?? false,
            }).ToList();
            var compiled = BoundaryGraph.Compile(settings, screens, g["safety"]?.GetValue<bool>() ?? true);
            core.Transitions = compiled.Transitions;
            res["transitions"] = new JsonArray(compiled.Transitions.Select(T).ToArray());
            res["sealed"] = new JsonArray(compiled.SealedScreens.Select(Issue).ToArray());
            res["has_safety_demotions"] = compiled.HasSafetyDemotions;
            res["validate"] = new JsonArray(BoundaryGraph.ValidateNoSeal(settings, screens).Select(Issue).ToArray());
        }
        core.Reset();
        res["needs_processing"] = core.NeedsProcessing;

        var results = new JsonArray();
        int i = 0;
        foreach (var op in req["ops"]?.AsArray() ?? new JsonArray())
        {
            string kind = op!["op"]!.GetValue<string>();
            string act = "Ok";
            int x = 0, y = 0;
            switch (kind)
            {
                case "move":
                    x = op["x"]!.GetValue<int>(); y = op["y"]!.GetValue<int>();
                    act = core.Decide(x, y, op["gate"]!.GetValue<bool>(), op["side"]!.GetValue<bool>(),
                                      out int bx, out int by) == GuardAction.Block ? "Block" : "Pass";
                    x = bx; y = by;
                    break;
                case "set_last":
                    core.LastX = op["x"]!.GetValue<int>(); core.LastY = op["y"]!.GetValue<int>(); core.HaveLast = true;
                    break;
                case "confine": core.Confine = op["on"]!.GetValue<bool>(); break;
                case "reset": core.Reset(); break;
                case "set": SetCoreFields(core, op["core"]!); break;
                default: throw new ArgumentException($"unknown core op '{kind}'");
            }
            results.Add(new JsonObject
            {
                ["i"] = i++, ["act"] = act, ["x"] = x, ["y"] = y, ["on_top"] = core.OnTop, ["have_last"] = core.HaveLast,
            });
        }
        res["results"] = results;
        return res;
    }

    /// <summary>A test assigning GuardCore fields between moves (no Reset): every field present in `c` is set,
    /// the compiled transitions and the cursor state stay.</summary>
    private static void SetCoreFields(GuardCore core, JsonNode c)
    {
        List<(int L, int T, int R, int B)> Rects(string name) => c[name]!.AsArray().Select(r => RectArr(r!)).ToList();
        if (c["has_top"] != null) core.HasTop = c["has_top"]!.GetValue<bool>();
        if (c["barrier_y"] != null) core.BarrierY = c["barrier_y"]!.GetValue<int>();
        if (c["gates"] != null)
            core.Gates = c["gates"]!.AsArray().Select(g => (g![0]!.GetValue<int>(), g[1]!.GetValue<int>())).ToList();
        if (c["safety_gates"] != null)
            core.SafetyGates = c["safety_gates"]!.AsArray().Select(g => (g![0]!.GetValue<int>(), g[1]!.GetValue<int>(),
                g[2]!.GetValue<int>(), g[3]!.GetValue<int>())).ToList();
        if (c["monitors"] != null) core.Monitors = Rects("monitors");
        if (c["forbidden"] != null) core.Forbidden = Rects("forbidden");
        if (c["has_sides"] != null) core.HasSides = c["has_sides"]!.GetValue<bool>();
        if (c["deliberate_cross"] != null) core.DeliberateCross = c["deliberate_cross"]!.GetValue<bool>();
        if (c["descent_routing"] != null) core.DescentRouting = c["descent_routing"]!.GetValue<bool>();
        if (c["descent_routes"] != null)
            core.DescentRoutes = c["descent_routes"]!.AsArray().Select(p => (RectArr(p![0]!), RectArr(p[1]!))).ToList();
        if (c["cross_min_up"] != null) core.CrossMinUp = c["cross_min_up"]!.GetValue<int>();
        if (c["cross_slack"] != null) core.CrossSlack = c["cross_slack"]!.GetValue<int>();
        if (c["side_cross_min"] != null) core.SideCrossMin = c["side_cross_min"]!.GetValue<int>();
        if (c["side_cross_slack"] != null) core.SideCrossSlack = c["side_cross_slack"]!.GetValue<int>();
        if (c["confine"] != null) core.Confine = c["confine"]!.GetValue<bool>();
        if (c["main"] != null)
        {
            var m = RectArr(c["main"]!);
            core.MainL = m.L; core.MainT = m.T; core.MainR = m.R; core.MainB = m.B;
        }
    }

    // ---------------------------------------------------------------- output helpers

    private static JsonArray R((int L, int T, int R, int B) r) => new JsonArray(r.L, r.T, r.R, r.B);

    private static JsonArray Strs(IEnumerable<string> items) =>
        new JsonArray(items.Select(s => (JsonNode)JsonValue.Create(s)).ToArray());

    private static JsonNode Issue(BoundaryValidationIssue i) => new JsonObject
    {
        ["id"] = i.StableId, ["focus_to"] = i.FocusToStableId,
        ["focus_dir"] = i.FocusDirection.ToString(), ["focus_width"] = i.FocusCorridorWidth,
    };

    private static JsonNode T(RuntimeTransition t) => new JsonObject
    {
        ["from"] = t.FromStableId, ["to"] = t.ToStableId, ["dir"] = t.Direction.ToString(),
        ["from_rect"] = R(t.From), ["to_rect"] = R(t.To),
        ["cs"] = t.CorridorStart, ["ce"] = t.CorridorEnd, ["gap"] = t.GapPx, ["tol"] = t.SeamTolerancePx,
        ["behavior"] = t.Behavior.ToString(), ["control"] = t.Control.ToString(),
        ["push_min"] = t.PushMin, ["push_slack"] = t.PushSlack, ["inset"] = t.PassageInsetPx,
        ["holdback"] = t.EdgeHoldbackPx, ["explicit"] = t.ExplicitRule,
        ["sole_up"] = t.SoleUpSafetyEscape, ["demoted"] = t.SafetyDemoted, ["sort"] = t.SortOrder,
        ["landings"] = new JsonArray(
            new JsonArray(t.AllowedLandings.Select(r => (JsonNode)R(r)).ToArray()),
            new JsonArray(t.AllowedLandingsSideOff.Select(r => (JsonNode)R(r)).ToArray()),
            new JsonArray(t.AllowedLandingsUpClosed.Select(r => (JsonNode)R(r)).ToArray()),
            new JsonArray(t.AllowedLandingsUpClosedSideOff.Select(r => (JsonNode)R(r)).ToArray())),
    };
}
