using System.Text.Json;
using System.Text.Json.Serialization;

namespace MouseFence;

public sealed class Settings
{
    public const int CurrentSettingsVersion = 4;
    public const string DefaultCrossingPathMode = "Default";
    public const string CustomCrossingPathMode = "Custom";
    public const int DefaultUpCrossMin = 3;
    public const int DefaultUpCrossSlack = 5;
    public const int DefaultPassageInsetPx = 24;
    public const int DefaultEdgeHoldbackPx = 0;
    public const int DefaultSeamTolerancePx = 2;

    private int _settingsVersion = CurrentSettingsVersion;
    // A missing JSON field is legacy v0. Fresh/schema-v4 construction sites set SettingsVersion through the setter;
    // deserialization does the same only when the property is genuinely present.
    private bool _settingsVersionWasSpecified;
    public int SettingsVersion
    {
        get => _settingsVersion;
        set { _settingsVersion = value; _settingsVersionWasSpecified = true; }
    }

    // hotkey
    public Keys HotKey { get; set; } = Keys.Up;
    public bool ModCtrl { get; set; } = true;
    public bool ModAlt { get; set; } = true;
    public bool ModShift { get; set; } = false;
    public bool ModWin { get; set; } = false;

    // game-mode (confine cursor to its monitor) hotkey
    public Keys ConfineHotKey { get; set; } = Keys.G;
    public bool ConfineModCtrl { get; set; } = true;
    public bool ConfineModAlt { get; set; } = true;
    public bool ConfineModShift { get; set; } = false;
    public bool ConfineModWin { get; set; } = false;

    // global pause/resume hotkey — a universal keyboard escape if the cursor ever gets trapped
    public Keys PauseHotKey { get; set; } = Keys.P;
    public bool PauseModCtrl { get; set; } = true;
    public bool PauseModAlt { get; set; } = true;
    public bool PauseModShift { get; set; } = false;
    public bool PauseModWin { get; set; } = false;

    // side-containment hotkey — toggles blocking the cursor from leaving the MAIN screen LEFT/RIGHT into a
    // side screen (the horizontal mirror of the up-barrier). Default Ctrl+Alt+Right.
    public Keys SideHotKey { get; set; } = Keys.Right;
    public bool SideModCtrl { get; set; } = true;
    public bool SideModAlt { get; set; } = true;
    public bool SideModShift { get; set; } = false;
    public bool SideModWin { get; set; } = false;

    // behaviour
    public bool StartGateClosed { get; set; } = true;  // main->top crossing blocked at launch
    public bool StartConfineOn { get; set; } = false;
    public bool StartPaused { get; set; } = false;
    // Default ON (true): side containment is a SOFT barrier — it only blocks accidental drift out of the main screen
    // sideways; a deliberate horizontal push always crosses, so shipping it on costs the user nothing but stops the
    // pixel-creep onto a side monitor. An absent JSON key deserializes to this default; toggle off via the tray menu
    // or the Ctrl+Alt+Right hotkey for fully free side movement.
    public bool StartSideContainOn { get; set; } = true;  // start with the soft side barrier engaged?
    public bool AutoStart { get; set; } = false;       // launch with Windows
    public bool DeliberateCross { get; set; } = true;  // true: deliberate push to cross; false: any upward move
    public bool DescentRouting { get; set; } = false;  // opt-in: clamp a descent back onto the linked bottom screen

    // Schema v4 boundary policy. Numeric crossing feel is global by design; BoundaryRule is intentionally lean.
    [JsonConverter(typeof(TolerantEnumConverter<BoundaryDefault>))]
    public BoundaryDefault UpwardBoundaryDefault { get; set; } = BoundaryDefault.Automatic;
    [JsonConverter(typeof(TolerantEnumConverter<BoundaryDefault>))]
    public BoundaryDefault SideBoundaryDefault { get; set; } = BoundaryDefault.Automatic;
    [JsonConverter(typeof(TolerantEnumConverter<BoundaryDefault>))]
    public BoundaryDefault DownwardBoundaryDefault { get; set; } = BoundaryDefault.Free;
    public List<BoundaryRule> BoundaryRules { get; set; } = new();
    public int UpCrossMin { get; set; } = DefaultUpCrossMin;
    public int UpCrossSlack { get; set; } = DefaultUpCrossSlack;
    public int PassageInsetPx { get; set; } = DefaultPassageInsetPx;
    public int EdgeHoldbackPx { get; set; } = DefaultEdgeHoldbackPx;
    public int SeamTolerancePx { get; set; } = DefaultSeamTolerancePx;
    public bool SafetyEscapes { get; set; } = true;
    public bool BoundaryTipSeen { get; set; } = false;

    // Side-barrier sensitivity (independent of the up barrier): the min horizontal px in one move that counts as a
    // DELIBERATE side crossing, and the vertical slack allowed beyond it. Higher SideCrossMin = a firmer/stiffer side
    // barrier (a faster horizontal flick is needed to cross, so accidental drift onto/off a side screen is caught more
    // aggressively). Defaults match the up barrier's feel; an absent JSON key deserializes to these.
    public const int DefaultSideCrossMin = 3;   // shared so the UI reset buttons and persisted defaults agree
    public const int DefaultSideCrossSlack = 5;
    public int SideCrossMin { get; set; } = DefaultSideCrossMin;
    public int SideCrossSlack { get; set; } = DefaultSideCrossSlack;

    // which monitors to block. Layout-dependent monitor keys are stable MonitorInfo.StableId values.
    public string Mode { get; set; } = "AutoTop";   // "AutoTop" | "Manual"
    public List<string> ManualMonitors { get; set; } = new();

    // Displays to WALL OFF: the cursor can never enter these (headless/dummy screens it would get lost in).
    // Keyed on the STABLE per-monitor id (MonitorInfo.StableId — the EDID-derived instance path) so the exclusion
    // SURVIVES \\.\DISPLAYn renumbering when the layout changes (FIX 4). Still machine-specific, so an imported list
    // from another PC simply lies dormant — a missing id is skipped in Configure(). Empty default => feature inert,
    // backward-compatible (an existing settings.json lacking the key deserializes to empty; no migration needed —
    // no ExcludedDevices has shipped yet).
    public List<string> ExcludedDevices { get; set; } = new();

    // Optional user nicknames, keyed by the same stable physical-monitor identity used by exclusions and layout rules.
    // A name for an unplugged/absent StableId remains dormant and can never attach to a different live monitor.
    public Dictionary<string, string> MonitorNames { get; set; } = new(StringComparer.OrdinalIgnoreCase);

    // appearance / localization
    public string Language { get; set; } = "auto";   // "auto" | "en" | "tr"
    public string Theme { get; set; } = "System";    // "System" | "Light" | "Dark"

    // per-screen crossing rules: which bottom monitor may cross UP into which top monitor.
    // FromDevice/ToDevice store stable MonitorInfo.StableId values; the property names stay for JSON compatibility.
    // Default mode deliberately persists an empty list; Custom mode treats this as an explicit "allow none" list.
    private string _crossingPathMode = DefaultCrossingPathMode;
    private bool _crossingPathModeWasSpecified;
    [JsonPropertyName("CrossingPathMode")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string LegacyCrossingPathMode
    {
        get => SettingsVersion < CurrentSettingsVersion ? _crossingPathMode : null;
        set
        {
            _crossingPathMode = value;
            _crossingPathModeWasSpecified = true;
        }
    }
    private List<UpLink> _legacyUpLinks = new();
    [JsonPropertyName("UpLinks")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public List<UpLink> LegacyUpLinks
    {
        get => SettingsVersion < CurrentSettingsVersion ? _legacyUpLinks : null;
        set => _legacyUpLinks = value ?? new List<UpLink>();
    }

    // M1 compatibility surface for the old textual editor. These are never persisted by schema v4.
    [JsonIgnore]
    public string CrossingPathMode => UpwardBoundaryDefault == BoundaryDefault.Blocked
        ? CustomCrossingPathMode : DefaultCrossingPathMode;
    [JsonIgnore]
    public List<UpLink> UpLinks => BoundaryRules
        .Where(r => r.Direction == BoundaryDirection.Up && r.Behavior == BoundaryBehavior.OverallFeel)
        .Select(r => new UpLink { FromDevice = r.FromStableId, ToDevice = r.ToStableId }).ToList();

    [JsonIgnore]
    public bool HasRepairableImportErrors { get; private set; }

    // Transient (not persisted): set when a fresh install on an Intel GPU had Shift added to its arrow hotkeys,
    // so the tray can show an info balloon explaining the safe defaults.
    [JsonIgnore]
    public bool IntelDefaultsApplied { get; private set; }

    [JsonIgnore]
    public static string Dir =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "MouseFence");

    [JsonIgnore]
    public static string FilePath => Path.Combine(Dir, "settings.json");

    public static Settings Load() => Load(FilePath);

    internal static Settings Load(string path)
    {
        try
        {
            // Read directly: File.Exists also returns false for access errors, which are not a fresh install.
            {
                var json = File.ReadAllText(path);
                int loadedVersion = ReadVersion(json);
                if (loadedVersion > CurrentSettingsVersion)
                    throw new InvalidDataException("The settings file requires a newer MouseFence version.");
                var settings = JsonSerializer.Deserialize<Settings>(json)
                    ?? throw new InvalidDataException("The settings file must contain a settings object.");
                settings.Normalize();
                if (loadedVersion < CurrentSettingsVersion)
                {
                    try
                    {
                        settings.MigrateLayoutKeysToStable(MonitorInfo.All());
                        settings.SettingsVersion = CurrentSettingsVersion;
                        settings.Save(path, MonitorInfo.All());
                    }
                    catch
                    {
                        // Keep the loaded settings rather than falling back to defaults; migration can retry next load.
                    }
                }
                return settings;
            }
        }
        catch (FileNotFoundException) { /* missing settings are a fresh install */ }
        catch (DirectoryNotFoundException) { /* missing settings directory is a fresh install */ }
        catch (Exception ex) { throw new InvalidDataException($"Could not load MouseFence settings: {path}", ex); }

        // Fresh install (no settings file yet): on an Intel GPU, the Ctrl+Alt+Arrow defaults clash with the driver's
        // screen-rotation hotkeys, so add Shift to keep the directional arrow but dodge rotation.
        var fresh = new Settings { SettingsVersion = CurrentSettingsVersion };
        try { if (Native.HasIntelGpu()) fresh.AddShiftToArrowHotkeys(); }
        catch { /* detection is best-effort */ }
        return fresh;
    }

    /// <summary>Creates a transaction-safe copy, including every nested collection and boundary rule.</summary>
    public Settings DeepClone()
    {
        var json = JsonSerializer.Serialize(this);
        var clone = JsonSerializer.Deserialize<Settings>(json) ?? new Settings();
        clone.Normalize();
        return clone;
    }

    /// <summary>
    /// Normalize deserialized/imported data and migrate the pre-v3 implicit crossing-path meaning. When the persisted
    /// field is absent, an old non-empty UpLinks list means Custom; an old empty list means Default.
    /// </summary>
    public void Normalize()
    {
        ManualMonitors ??= new List<string>();
        ExcludedDevices ??= new List<string>();
        _legacyUpLinks ??= new List<UpLink>();
        BoundaryRules ??= new List<BoundaryRule>();

        // Deserialization creates a comparer-neutral Dictionary, and hand-edited/imported JSON may contain blank,
        // duplicate-by-case, or multi-line values. Rebuild deterministically as a case-insensitive, single-line map.
        var normalizedNames = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var pair in (MonitorNames ?? new Dictionary<string, string>())
                     .OrderBy(p => p.Key, StringComparer.Ordinal))
        {
            string stableId = pair.Key?.Trim();
            string name = NormalizeMonitorName(pair.Value);
            if (!string.IsNullOrEmpty(stableId) && !string.IsNullOrEmpty(name)
                && !normalizedNames.ContainsKey(stableId))
                normalizedNames.Add(stableId, name);
        }
        MonitorNames = normalizedNames;

        int loadedVersion = _settingsVersionWasSpecified ? _settingsVersion : 0;
        if (loadedVersion < CurrentSettingsVersion)
        {
            bool valid = _crossingPathMode is DefaultCrossingPathMode or CustomCrossingPathMode;
            if (!_crossingPathModeWasSpecified || !valid)
                _crossingPathMode = _legacyUpLinks.Count > 0 ? CustomCrossingPathMode : DefaultCrossingPathMode;
            if (_crossingPathMode == CustomCrossingPathMode)
            {
                UpwardBoundaryDefault = BoundaryDefault.Blocked;
                BoundaryRules = _legacyUpLinks.Where(l => l != null)
                    .Select(l => new BoundaryRule
                    {
                        FromStableId = l.FromDevice ?? "", ToStableId = l.ToDevice ?? "",
                        Direction = BoundaryDirection.Up, Behavior = BoundaryBehavior.OverallFeel,
                        Control = BoundaryControl.Automatic,
                    }).ToList();
            }
            else
            {
                UpwardBoundaryDefault = BoundaryDefault.Automatic;
                BoundaryRules.Clear();
            }
            SideBoundaryDefault = BoundaryDefault.Automatic;
            DownwardBoundaryDefault = BoundaryDefault.Free;
            EdgeHoldbackPx = DefaultEdgeHoldbackPx;
            StartConfineOn = false;
            StartPaused = false;
            BoundaryTipSeen = false;
            if (Mode == "Manual" && ManualMonitors.Count == 0) Mode = "AutoTop";
            _settingsVersion = CurrentSettingsVersion;
            _settingsVersionWasSpecified = true;
        }

        UpCrossMin = Math.Clamp(UpCrossMin, 1, 200);
        UpCrossSlack = Math.Clamp(UpCrossSlack, 0, 200);
        SideCrossMin = Math.Clamp(SideCrossMin, 1, 200);
        SideCrossSlack = Math.Clamp(SideCrossSlack, 0, 200);
        PassageInsetPx = Math.Clamp(PassageInsetPx, 0, 500);
        EdgeHoldbackPx = Math.Clamp(EdgeHoldbackPx, 0, 200);
        SeamTolerancePx = Math.Clamp(SeamTolerancePx, 0, 200);

        HasRepairableImportErrors = UpwardBoundaryDefault == BoundaryDefault.Unknown
            || SideBoundaryDefault == BoundaryDefault.Unknown || DownwardBoundaryDefault == BoundaryDefault.Unknown;
        var dedup = new Dictionary<string, BoundaryRule>(StringComparer.OrdinalIgnoreCase);
        var invalid = new List<BoundaryRule>();
        foreach (var rule in BoundaryRules)
        {
            if (rule == null) continue;
            rule.FromStableId = rule.FromStableId?.Trim() ?? "";
            rule.ToStableId = rule.ToStableId?.Trim() ?? "";
            bool enumInvalid = rule.Direction == BoundaryDirection.Unknown || rule.Behavior == BoundaryBehavior.Unknown
                || rule.Control == BoundaryControl.Unknown;
            if (enumInvalid) { HasRepairableImportErrors = true; invalid.Add(rule); continue; }
            if (rule.FromStableId.Length == 0 || rule.ToStableId.Length == 0
                || string.Equals(rule.FromStableId, rule.ToStableId, StringComparison.OrdinalIgnoreCase))
            { HasRepairableImportErrors = true; invalid.Add(rule); continue; }
            dedup[RuleIdentity(rule)] = rule; // import contract: last-valid-wins
        }
        BoundaryRules = dedup.Values.OrderBy(r => r.FromStableId, StringComparer.Ordinal)
            .ThenBy(r => r.ToStableId, StringComparer.Ordinal).ThenBy(r => r.Direction).Concat(invalid).ToList();

        if (Mode is not ("AutoTop" or "Manual")) Mode = "AutoTop";
    }

    internal static string NormalizeMonitorName(string value)
    {
        string clean = value?.Replace('\t', ' ').Replace('\r', ' ').Replace('\n', ' ').Trim() ?? "";
        int[] elements = System.Globalization.StringInfo.ParseCombiningCharacters(clean);
        return elements.Length <= 24 ? clean : clean[..elements[24]];
    }

    internal static string MonitorNameFor(IReadOnlyDictionary<string, string> names, string stableId)
    {
        if (names == null || string.IsNullOrEmpty(stableId) || !names.TryGetValue(stableId, out var value)) return "";
        return NormalizeMonitorName(value);
    }

    /// <summary>On an Intel GPU, the Ctrl+Alt+Arrow defaults (up-barrier + side containment) collide with the driver's
    /// screen-rotation hotkeys. Add Shift to any arrow-rotation hotkey so it keeps the arrow but no longer rotates the
    /// screen. Sets <see cref="IntelDefaultsApplied"/> if anything changed (for the tray's info balloon).</summary>
    public void AddShiftToArrowHotkeys()
    {
        bool changed = false;
        if (GuardCore.IsArrowRotationHotkey((int)HotKey, ModCtrl, ModAlt, ModShift, ModWin)) { ModShift = true; changed = true; }
        if (GuardCore.IsArrowRotationHotkey((int)SideHotKey, SideModCtrl, SideModAlt, SideModShift, SideModWin)) { SideModShift = true; changed = true; }
        IntelDefaultsApplied = changed;
    }

    public static int ReadVersion(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            return doc.RootElement.TryGetProperty(nameof(SettingsVersion), out var v) && v.TryGetInt32(out int n) ? n : 0;
        }
        catch
        {
            return 0;
        }
    }

    public void MigrateLayoutKeysToStable(List<MonitorInfo> monitors)
    {
        var map = monitors
            .GroupBy(m => m.Device)
            .ToDictionary(g => g.Key, g => g.First().StableId, StringComparer.OrdinalIgnoreCase);
        var migrated = GuardCore.MigrateLayoutKeysToStable(
            ManualMonitors,
            _legacyUpLinks.Select(lk => (lk.FromDevice, lk.ToDevice)),
            map);
        ManualMonitors = migrated.ManualKeys;
        _legacyUpLinks = migrated.Links
            .Select(lk => new UpLink { FromDevice = lk.From, ToDevice = lk.To })
            .ToList();
        foreach (var rule in BoundaryRules)
        {
            if (map.TryGetValue(rule.FromStableId, out var from)) rule.FromStableId = from;
            else if (rule.FromStableId?.StartsWith(@"\\.\DISPLAY", StringComparison.OrdinalIgnoreCase) == true) rule.FromStableId = "";
            if (map.TryGetValue(rule.ToStableId, out var to)) rule.ToStableId = to;
            else if (rule.ToStableId?.StartsWith(@"\\.\DISPLAY", StringComparison.OrdinalIgnoreCase) == true) rule.ToStableId = "";
        }
        Normalize();
    }

    public void ResetLayoutConfig()
    {
        var reset = GuardCore.ResetLayoutConfig();
        Mode = reset.Mode;
        ManualMonitors = reset.ManualKeys;
        UpwardBoundaryDefault = BoundaryDefault.Automatic;
        SideBoundaryDefault = BoundaryDefault.Automatic;
        DownwardBoundaryDefault = BoundaryDefault.Free;
        BoundaryRules.Clear();
        UpCrossMin = DefaultUpCrossMin; UpCrossSlack = DefaultUpCrossSlack;
        SideCrossMin = DefaultSideCrossMin; SideCrossSlack = DefaultSideCrossSlack;
        PassageInsetPx = DefaultPassageInsetPx; EdgeHoldbackPx = DefaultEdgeHoldbackPx;
        SeamTolerancePx = DefaultSeamTolerancePx; SafetyEscapes = true;
    }

    public bool Save() => Save(FilePath, MonitorInfo.All());

    internal bool Save(string path, List<MonitorInfo> monitors, Action<string, string> write = null)
    {
        string temporary = Path.GetFullPath(path) + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            Normalize();
            if (SettingsVersion > CurrentSettingsVersion) return false;
            SettingsVersion = CurrentSettingsVersion;
            if (HasRepairableImportErrors) return false;
            var excluded = SanitizedExcluded(monitors);
            if (BoundaryGraph.ValidateNoSeal(this, BoundaryGraph.FromMonitors(monitors, excluded)).Length > 0)
                return false;
            Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(path)));
            (write ?? File.WriteAllText)(temporary, JsonSerializer.Serialize(this, new JsonSerializerOptions { WriteIndented = true }));
            using (var file = new FileStream(temporary, FileMode.Open, FileAccess.ReadWrite, FileShare.None)) file.Flush(true);
            File.Move(temporary, path, overwrite: true);
            return true;
        }
        catch { return false; }
        finally { try { File.Delete(temporary); } catch { } }
    }

    public BoundaryValidationIssue[] ValidateNoSeal(IEnumerable<MonitorInfo> monitors)
    {
        Normalize();
        var monitorList = (monitors ?? Enumerable.Empty<MonitorInfo>()).ToList();
        var excluded = SanitizedExcluded(monitorList);
        return BoundaryGraph.ValidateNoSeal(this, BoundaryGraph.FromMonitors(monitorList, excluded));
    }

    private HashSet<string> SanitizedExcluded(IReadOnlyList<MonitorInfo> monitors)
    {
        var present = monitors.Select(m => m.StableId).ToList();
        string primary = (monitors.FirstOrDefault(m => m.Primary) ?? monitors.FirstOrDefault())?.StableId;
        var requested = new HashSet<string>(ExcludedDevices ?? new List<string>(), StringComparer.OrdinalIgnoreCase);
        return new HashSet<string>(GuardCore.SanitizeExcluded(present, primary, requested, out _),
                                   StringComparer.OrdinalIgnoreCase);
    }

    private static string RuleIdentity(BoundaryRule rule) =>
        rule.FromStableId + "\n" + rule.ToStableId + "\n" + rule.Direction;

    public uint Modifiers()
    {
        uint m = 0;
        if (ModCtrl) m |= Native.MOD_CONTROL;
        if (ModAlt) m |= Native.MOD_ALT;
        if (ModShift) m |= Native.MOD_SHIFT;
        if (ModWin) m |= Native.MOD_WIN;
        return m;
    }

    public string HotKeyText()
    {
        var parts = new List<string>();
        if (ModCtrl) parts.Add("Ctrl");
        if (ModAlt) parts.Add("Alt");
        if (ModShift) parts.Add("Shift");
        if (ModWin) parts.Add("Win");
        if (HotKey != Keys.None) parts.Add(HotKey.ToString());
        return parts.Count == 0 ? "(none)" : string.Join("+", parts);
    }

    public uint ConfineModifiers()
    {
        uint m = 0;
        if (ConfineModCtrl) m |= Native.MOD_CONTROL;
        if (ConfineModAlt) m |= Native.MOD_ALT;
        if (ConfineModShift) m |= Native.MOD_SHIFT;
        if (ConfineModWin) m |= Native.MOD_WIN;
        return m;
    }

    public string ConfineHotKeyText()
    {
        var parts = new List<string>();
        if (ConfineModCtrl) parts.Add("Ctrl");
        if (ConfineModAlt) parts.Add("Alt");
        if (ConfineModShift) parts.Add("Shift");
        if (ConfineModWin) parts.Add("Win");
        if (ConfineHotKey != Keys.None) parts.Add(ConfineHotKey.ToString());
        return parts.Count == 0 ? "(none)" : string.Join("+", parts);
    }

    public uint PauseModifiers()
    {
        uint m = 0;
        if (PauseModCtrl) m |= Native.MOD_CONTROL;
        if (PauseModAlt) m |= Native.MOD_ALT;
        if (PauseModShift) m |= Native.MOD_SHIFT;
        if (PauseModWin) m |= Native.MOD_WIN;
        return m;
    }

    public string PauseHotKeyText()
    {
        var parts = new List<string>();
        if (PauseModCtrl) parts.Add("Ctrl");
        if (PauseModAlt) parts.Add("Alt");
        if (PauseModShift) parts.Add("Shift");
        if (PauseModWin) parts.Add("Win");
        if (PauseHotKey != Keys.None) parts.Add(PauseHotKey.ToString());
        return parts.Count == 0 ? "(none)" : string.Join("+", parts);
    }

    public uint SideModifiers()
    {
        uint m = 0;
        if (SideModCtrl) m |= Native.MOD_CONTROL;
        if (SideModAlt) m |= Native.MOD_ALT;
        if (SideModShift) m |= Native.MOD_SHIFT;
        if (SideModWin) m |= Native.MOD_WIN;
        return m;
    }

    public string SideHotKeyText()
    {
        var parts = new List<string>();
        if (SideModCtrl) parts.Add("Ctrl");
        if (SideModAlt) parts.Add("Alt");
        if (SideModShift) parts.Add("Shift");
        if (SideModWin) parts.Add("Win");
        if (SideHotKey != Keys.None) parts.Add(SideHotKey.ToString());
        return parts.Count == 0 ? "(none)" : string.Join("+", parts);
    }
}

/// <summary>An allowed upward crossing: the cursor on <see cref="FromDevice"/> may go up into <see cref="ToDevice"/>.</summary>
public sealed class UpLink
{
    public string FromDevice { get; set; } = "";
    public string ToDevice { get; set; } = "";
}

/// <summary>String enum converter that preserves unknown import values as the enum's Unknown sentinel. This lets the
/// settings dialog keep a repairable draft instead of throwing or silently choosing a different behavior.</summary>
public sealed class TolerantEnumConverter<T> : JsonConverter<T> where T : struct, Enum
{
    public override T Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.String
            && Enum.TryParse<T>(reader.GetString(), ignoreCase: true, out var parsed)
            && Enum.IsDefined(parsed))
            return parsed;
        if (reader.TokenType == JsonTokenType.Number && reader.TryGetInt32(out int raw)
            && Enum.IsDefined(typeof(T), raw))
            return (T)Enum.ToObject(typeof(T), raw);
        return Enum.TryParse<T>("Unknown", out var unknown) ? unknown : default;
    }

    public override void Write(Utf8JsonWriter writer, T value, JsonSerializerOptions options) =>
        writer.WriteStringValue(value.ToString());
}
