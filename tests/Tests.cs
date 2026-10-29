using MouseFence;
using System.Text.Json;

// Deterministic tests for the barrier decision logic, using the user's real monitor geometry:
//   DISPLAY3 left  X -1920..0    Y 14..1094
//   DISPLAY1 MAIN  X 0..2560     Y 0..1440   (primary; barrier line = Y 0)
//   DISPLAY2 right X 2560..4480  Y 0..1080
//   DISPLAY4 TOP   X -440..3000  Y -1440..0  (above; void above DISPLAY2 right part: X 3000..4480, Y<0)
// Gate = MAIN inset by 24 -> [24, 2536).

int fails = 0;
int passes = 0;
Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException);

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

// Wall-off only: no top monitors, no game mode — just forbidden rects. Proves (0) runs before the
// !HasTop && !Confine bail-out and that NeedsProcessing flips the hook on.
GuardCore NewForbidden(params (int L, int T, int R, int B)[] forbidden)
{
    var c = new GuardCore { HasTop = false, Confine = false, Forbidden = forbidden.ToList() };
    c.Reset();
    return c;
}

GuardCore NewMonitorAwareWall(
    IEnumerable<(int L, int T, int R, int B)> monitors,
    params (int L, int T, int R, int B)[] forbidden)
{
    var c = NewForbidden(forbidden);
    c.Monitors = monitors.ToList();
    return c;
}

bool InRect((int L, int T, int R, int B) rect, int x, int y) =>
    x >= rect.L && x < rect.R && y >= rect.T && y < rect.B;

bool IsLegalWallResult(GuardCore c, int x, int y) =>
    c.Monitors.Any(m => InRect(m, x, y)) && !c.Forbidden.Any(f => InRect(f, x, y));

IEnumerable<Control> DescendantsAndSelf(Control root)
{
    yield return root;
    foreach (Control child in root.Controls)
        foreach (var descendant in DescendantsAndSelf(child))
            yield return descendant;
}

void ShowOffscreen(Form form)
{
    form.StartPosition = FormStartPosition.Manual;
    form.Location = new Point(SystemInformation.VirtualScreen.Right + 2000, SystemInformation.VirtualScreen.Bottom + 2000);
    form.Opacity = 0;
    form.Show();
    Application.DoEvents();
    form.PerformLayout();
}

void SetLast(GuardCore c, int x, int y)
{
    c.LastX = x;
    c.LastY = y;
    c.HaveLast = true;
}

// side defaults to true (containment ON), but every non-side core has HasSides=false so the side branch never
// runs there regardless — existing scenarios are unaffected.
(GuardAction act, int by, bool onTop, int bx) Move(GuardCore c, int x, int y, bool gate, bool side = true)
{
    var a = c.Decide(x, y, gate, side, out int bx, out int by);
    return (a, by, c.OnTop, bx);
}

// MAIN = (0,0,2560,1440) with side screens present -> HasSides on. Up-barrier at Y 0, gate [24,2536] (as elsewhere).
// Monitors is populated (main + left/right + top) so every tested target has the same real layout as HasTop.
GuardCore NewSides()
{
    var c = new GuardCore { HasTop = true, BarrierY = 0, Gates = new List<(int Min, int Max)> { (24, 2536) },
        HasSides = true, MainL = 0, MainR = 2560, MainT = 0, MainB = 1440,
        Monitors = new List<(int L, int T, int R, int B)>
            { (0, 0, 2560, 1440), (-1920, 0, 0, 1080), (2560, 0, 4480, 1080), (-440, -1440, 3000, 0) } };
    c.Reset();
    return c;
}

void Check(string name, bool ok, string detail = "")
{
    Console.WriteLine($"{(ok ? "PASS" : "FAIL")}  {name}{(ok ? "" : "   <-- " + detail)}");
    if (!ok) fails++;
    else passes++;
}

Console.WriteLine("MouseFence barrier logic — scenario tests\n");
OldFeelScenarios.Regressions(Check);
OldFeelScenarios.Instantaneous(Check);

{
    using var first = new HotKeyWindow();
    using var second = new HotKeyWindow();
    uint modifiers = Native.MOD_CONTROL | Native.MOD_ALT | Native.MOD_SHIFT | Native.MOD_WIN;
    bool registered = first.Register(modifiers, (uint)Keys.F24);
    bool conflict = !second.Register(modifiers, (uint)Keys.F24);
    first.Unregister();
    bool reassigned = second.Register(modifiers, (uint)Keys.F24);
    second.Unregister();
    Check("logic audit: native hotkey conflict is reported and unregister permits re-registration",
        registered && conflict && reassigned);
}

{
    string folder = Path.Combine(AppContext.BaseDirectory, "diagnostic-audit-" + Guid.NewGuid().ToString("N"));
    CrashLogger.Write(new IOException(new string('ç', 180000)), fatal: false, directory: folder);
    CrashLogger.Write(new IOException("latest diagnostic marker"), fatal: true, directory: folder);
    string path = Path.Combine(folder, "error.log");
    byte[] bytes = File.ReadAllBytes(path);
    string text = new System.Text.UTF8Encoding(false, true).GetString(bytes);
    Check("logic audit: diagnostics stay bounded and retain valid UTF-8 plus the latest failure",
        bytes.Length > 0 && bytes.Length <= 256 * 1024 && text.Contains("FATAL") && text.Contains("latest diagnostic marker"));
    File.Delete(path); Directory.Delete(folder);
}

{
    using var form = new SettingsForm(new Settings { SettingsVersion = Settings.CurrentSettingsVersion }, Screenshot.DemoMonitors());
    var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
    form.KeyIsDown = key => key == Keys.RWin;
    typeof(SettingsForm).GetMethod("CaptureHotkey", flags).Invoke(form, new object[] { 0, new KeyEventArgs(Keys.Shift | Keys.T) });
    Check("logic audit: shortcut capture retains an already held Windows modifier",
        form.DraftForTests.HotKey == Keys.T && form.DraftForTests.ModWin && form.DraftForTests.ModShift);
}

foreach (var json in new[] { "{broken", "null", "{\"SettingsVersion\":999,\"Theme\":\"Dark\"}" })
{
    string path = Path.Combine(AppContext.BaseDirectory, "invalid-settings-" + Guid.NewGuid().ToString("N") + ".json");
    File.WriteAllText(path, json);
    bool refused = false;
    try { Settings.Load(path); }
    catch (InvalidDataException) { refused = true; }
    Check("logic audit: corrupt or future settings refuse a destructive default session: " + json,
        refused && File.ReadAllText(path) == json);
    File.Delete(path);
}

{
    string folder = Path.Combine(AppContext.BaseDirectory, "settings-audit-" + Guid.NewGuid().ToString("N"));
    Directory.CreateDirectory(folder);
    string path = Path.Combine(folder, "settings.json");
    const string original = "{\"SettingsVersion\":4,\"Theme\":\"Dark\"}";
    File.WriteAllText(path, original);
    var settings = new Settings { SettingsVersion = Settings.CurrentSettingsVersion, Theme = "Light" };
    bool saved = settings.Save(path, new List<MonitorInfo>(), (target, _) =>
    {
        File.WriteAllText(target, "{");
        throw new IOException("Simulated partial disk write");
    });
    Check("logic audit: failed settings write preserves previous bytes", !saved && File.ReadAllText(path) == original);
    Check("logic audit: successful settings replacement is readable", settings.Save(path, new List<MonitorInfo>())
        && JsonSerializer.Deserialize<Settings>(File.ReadAllText(path)).Theme == "Light");
    Check("logic audit: settings save cleans temporary siblings", Directory.GetFiles(folder).Length == 1);
    File.Delete(path); Directory.Delete(folder);
}

{
    var startup = new Settings { SettingsVersion = Settings.CurrentSettingsVersion,
        StartPaused = true, StartConfineOn = true, StartGateClosed = false, StartSideContainOn = false,
        HotKey = Keys.None, ConfineHotKey = Keys.None, PauseHotKey = Keys.None, SideHotKey = Keys.None };
    var flags = System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance;
    using var context = new TrayApplicationContext(startup);
    var guard = (MouseGuard)typeof(TrayApplicationContext).GetField("_guard", flags).GetValue(context);
    Check("logic audit: saved startup pause does not install a hook", !guard.Enabled && !guard.HookInstalled);
    Check("logic audit: saved startup confinement and ordinary toggles reach the real guard",
        guard.Confine && guard.GateOpen && !guard.SideContain);
    typeof(TrayApplicationContext).GetMethod("ExitApp", flags).Invoke(context, null);
}

// Logic audit 2026-10-22: inset pixels are a closed part of a Push gate, never a free bypass.
foreach (var direction in new[] { BoundaryDirection.Up, BoundaryDirection.Down, BoundaryDirection.Left, BoundaryDirection.Right })
{
    var screens = direction switch
    {
        BoundaryDirection.Up => GraphScreens(("a", -100, 0, 0, 100, true), ("b", -100, -100, 0, 0, false)),
        BoundaryDirection.Down => GraphScreens(("a", -100, -100, 0, 0, true), ("b", -100, 0, 0, 100, false)),
        BoundaryDirection.Left => GraphScreens(("a", 0, -100, 100, 0, true), ("b", -100, -100, 0, 0, false)),
        _ => GraphScreens(("a", -100, -100, 0, 0, true), ("b", 0, -100, 100, 0, false)),
    };
    var settings = new Settings { SettingsVersion = Settings.CurrentSettingsVersion, SafetyEscapes = false, PassageInsetPx = 24 };
    settings.BoundaryRules.Add(new BoundaryRule { FromStableId = "a", ToStableId = "b", Direction = direction,
        Behavior = BoundaryBehavior.Push, Control = BoundaryControl.Always });
    var c = GraphCore(settings, screens);
    var origin = direction switch
    {
        BoundaryDirection.Up => (-99, 0), BoundaryDirection.Down => (-99, -1),
        BoundaryDirection.Left => (0, -99), _ => (-1, -99),
    };
    var target = direction switch
    {
        BoundaryDirection.Up => (-99, -1), BoundaryDirection.Down => (-99, 0),
        BoundaryDirection.Left => (-1, -99), _ => (0, -99),
    };
    Move(c, origin.Item1, origin.Item2, true);
    var result = Move(c, target.Item1, target.Item2, true);
    Check($"logic audit: {direction} inset rejects slow drift at negative coordinates",
        result.act == GuardAction.Block && InRect(screens[0].Bounds, result.bx, result.by),
        $"act={result.act} target={result.bx},{result.by}");
}

// The public Free/Push/Blocked contracts must agree at both ends and the middle of every physical edge.
foreach (var direction in new[] { BoundaryDirection.Up, BoundaryDirection.Down, BoundaryDirection.Left, BoundaryDirection.Right })
foreach (var behavior in new[] { BoundaryBehavior.Free, BoundaryBehavior.Push, BoundaryBehavior.Blocked })
foreach (int inset in new[] { 0, 24, 500 })
foreach (int orthogonal in new[] { -99, -50, -1 })
foreach (int distance in new[] { 1, 10 })
{
    bool vertical = direction is BoundaryDirection.Up or BoundaryDirection.Down;
    var source = direction switch
    {
        BoundaryDirection.Up => (L: -100, T: 0, R: 0, B: 100),
        BoundaryDirection.Left => (L: 0, T: -100, R: 100, B: 0),
        _ => (L: -100, T: -100, R: 0, B: 0),
    };
    var destination = direction switch
    {
        BoundaryDirection.Up or BoundaryDirection.Left => (L: -100, T: -100, R: 0, B: 0),
        BoundaryDirection.Down => (L: -100, T: 0, R: 0, B: 100),
        _ => (L: 0, T: -100, R: 100, B: 0),
    };
    var screens = GraphScreens(("a", source.L, source.T, source.R, source.B, true),
        ("b", destination.L, destination.T, destination.R, destination.B, false));
    var settings = new Settings { SettingsVersion = Settings.CurrentSettingsVersion, SafetyEscapes = false, PassageInsetPx = inset };
    settings.BoundaryRules.Add(new BoundaryRule { FromStableId = "a", ToStableId = "b", Direction = direction,
        Behavior = behavior, Control = BoundaryControl.Always });
    var core = GraphCore(settings, screens);
    int originAxis = direction is BoundaryDirection.Up or BoundaryDirection.Left ? 0 : -1;
    int targetAxis = originAxis + (direction is BoundaryDirection.Up or BoundaryDirection.Left ? -distance : distance);
    Move(core, vertical ? orthogonal : originAxis, vertical ? originAxis : orthogonal, true);
    var result = Move(core, vertical ? orthogonal : targetAxis, vertical ? targetAxis : orthogonal, true);
    bool shouldCross = behavior == BoundaryBehavior.Free
        || behavior == BoundaryBehavior.Push && distance == 10 && (inset == 0 || orthogonal == -50);
    Check($"logic audit edge contract: {direction}/{behavior}/inset={inset}/position={orthogonal}/distance={distance}",
        (result.act == GuardAction.Pass) == shouldCross
        && InRect(shouldCross ? destination : source, result.bx, result.by));
}

// All disposable child windows must be released when a dynamic section is replaced.
{
    using var form = new SettingsForm(new Settings { SettingsVersion = Settings.CurrentSettingsVersion }, Screenshot.DemoMonitors());
    var flags = System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance;
    foreach (var section in new[] { ("_screenRows", "RebuildScreenRows"), ("_advancedPanel", "RebuildAdvancedContents") })
    {
        var panel = (Control)typeof(SettingsForm).GetField(section.Item1, flags).GetValue(form);
        var replaced = panel.Controls.Cast<Control>().Where(c => (c.Tag as string) != null || c is Label).First();
        _ = replaced.Handle;
        typeof(SettingsForm).GetMethod(section.Item2, flags).Invoke(form, null);
        Check($"logic audit: {section.Item1} disposes replaced controls", replaced.IsDisposed);
    }
    form.DraftForTests.ExcludedDevices.Add("temporarily-unplugged-monitor");
    typeof(SettingsForm).GetMethod("SanitizeExcluded", flags).Invoke(form, null);
    Check("logic audit: display refresh preserves absent excluded monitor identity",
        form.DraftForTests.ExcludedDevices.Contains("temporarily-unplugged-monitor"));
}

// 1) THE right-screen bug: at top of DISPLAY2, moving up-right into the VOID (X>3000) must be blocked
//    and clamped to Y=0 (not snapped onto the top monitor's far wall).
{
    var c = New(); Move(c, 3500, 0, true);
    var r = Move(c, 3600, -20, true);
    Check("right-screen void: up into void is blocked & clamped to Y=0",
        r.act == GuardAction.Block && r.by == 0 && !r.onTop, $"act={r.act} by={r.by} onTop={r.onTop}");
}

// M2 transaction: constructing/editing the dialog works on a true deep draft and cannot touch the live object.
{
    var live = new Settings
    {
        Mode = "Manual", ManualMonitors = new List<string> { "top" },
        MonitorNames = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase) { ["top"] = "Studio" },
        BoundaryRules = new List<BoundaryRule> { new() { FromStableId = "main", ToStableId = "top",
            Direction = BoundaryDirection.Up, Behavior = BoundaryBehavior.Push, Control = BoundaryControl.Automatic } },
    };
    var monitors = new List<MonitorInfo>
    {
        new() { StableId = "top", Device = @"\\.\DISPLAY2", Index = 1, Bounds = new Native.RECT { Left = 0, Top = -100, Right = 100, Bottom = 0 } },
        new() { StableId = "main", Device = @"\\.\DISPLAY1", Index = 2, Primary = true, Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
    };
    using var form = new SettingsForm(live, monitors);
    var draft = form.DraftForTests;
    draft.ManualMonitors.Clear(); draft.MonitorNames["top"] = "Changed"; draft.BoundaryRules[0].Behavior = BoundaryBehavior.Blocked;
    bool isolated = live.ManualMonitors.SequenceEqual(new[] { "top" }) && live.MonitorNames["top"] == "Studio"
        && live.BoundaryRules[0].Behavior == BoundaryBehavior.Push
        && !ReferenceEquals(live.ManualMonitors, draft.ManualMonitors)
        && !ReferenceEquals(live.MonitorNames, draft.MonitorNames)
        && !ReferenceEquals(live.BoundaryRules[0], draft.BoundaryRules[0]);
    Check("M2 settings transaction: form owns a nested deep draft and Cancel cannot mutate live settings", isolated);
}

// B2 footer regression: every footer child must receive the resolved theme, not its constructor-time light default.
{
    Strings.Use("en");
    using var form = new SettingsForm(new Settings { Language = "en", Theme = "Dark" }, new List<MonitorInfo>())
    {
        StartPosition = FormStartPosition.Manual,
        Location = new Point(SystemInformation.VirtualScreen.Right + 2000, SystemInformation.VirtualScreen.Bottom + 2000),
        Opacity = 0,
    };
    form.Show(); Application.DoEvents(); form.PerformLayout();
    var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
    var status = (Label)typeof(SettingsForm).GetField("_inlineStatus", flags).GetValue(form);
    var footer = status.Parent;
    var themedButtons = footer.Controls.OfType<ThemedButton>().ToList();
    var visibleButtons = themedButtons.Where(button => button.Visible).ToList();
    var inlineAction = (ThemedButton)typeof(SettingsForm).GetField("_inlineAction", flags).GetValue(form);
    var themeField = typeof(ThemedButton).GetField("_theme", flags);
    using var footerBitmap = new Bitmap(footer.Width, footer.Height);
    footer.DrawToBitmap(footerBitmap, footer.ClientRectangle);
    var paintedFills = visibleButtons.Select(button => footerBitmap.GetPixel(button.Left + 5, button.Top + button.Height / 2)).ToList();
    bool painted = paintedFills.All(color => color == Theme.Dark.Surface2 || color == Theme.Dark.DisabledSurface || color == Theme.Dark.Accent);
    bool enrolled = footer.BackColor == Theme.Dark.Surface && status.BackColor == Theme.Dark.Surface
        && themedButtons.Count == 3 && visibleButtons.Count == 2 && !inlineAction.Visible
        && themedButtons.All(button => (Theme)themeField.GetValue(button) == Theme.Dark) && painted;
    Check("B2 footer: dark surface, status, validation action, Cancel, and Save are enrolled in theming",
        enrolled,
        $"footer={footer.BackColor} status={status.BackColor} buttons={themedButtons.Count}/{visibleButtons.Count} actionVisible={inlineAction.Visible} fills=[{string.Join(",", paintedFills)}] themes=[{string.Join(",", themedButtons.Select(button => themeField.GetValue(button)))}]");
}

// Fable v2 acceptance: Advanced is a first-class page, its physical-pixel fold is deterministic,
// and the boundary editor is a full-width inline row with statically placed disclosures.
foreach (string language in new[] { "en", "tr" })
{
    Strings.Use(language);
    var monitors = new List<MonitorInfo>
    {
        new() { StableId = "top", Device = @"\\.\DISPLAY2", Index = 1, Bounds = new Native.RECT { Left = 0, Top = -900, Right = 1600, Bottom = 0 } },
        new() { StableId = "main", Device = @"\\.\DISPLAY1", Index = 2, Primary = true, Bounds = new Native.RECT { Left = 0, Top = 0, Right = 1920, Bottom = 1080 } },
        new() { StableId = "right", Device = @"\\.\DISPLAY3", Index = 3, Bounds = new Native.RECT { Left = 1920, Top = 0, Right = 3200, Bottom = 1024 } },
        new() { StableId = "left", Device = @"\\.\DISPLAY4", Index = 4, Bounds = new Native.RECT { Left = -1280, Top = 0, Right = 0, Bottom = 1024 } },
        new() { StableId = "far-top", Device = @"\\.\DISPLAY5", Index = 5, Bounds = new Native.RECT { Left = 1600, Top = -768, Right = 2624, Bottom = 0 } },
    };
    using var form = new SettingsForm(new Settings { Language = language, DownwardBoundaryDefault = BoundaryDefault.Blocked }, monitors)
    {
        StartPosition = FormStartPosition.Manual,
        Location = new Point(SystemInformation.VirtualScreen.Right + 2000, SystemInformation.VirtualScreen.Bottom + 2000),
        Opacity = 0,
    };
    form.Show(); Application.DoEvents();
    form.ClientSize = new Size(1064 * form.DeviceDpi / 96, 712 * form.DeviceDpi / 96);
    Application.DoEvents(); form.PerformLayout();

    var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
    var toggle = (Button)typeof(SettingsForm).GetField("_advancedToggle", flags).GetValue(form);
    var panel = (Control)typeof(SettingsForm).GetField("_advancedPanel", flags).GetValue(form);
    var tabs = (TabControl)typeof(SettingsForm).GetField("_tabs", flags).GetValue(form);
    var advancedPage = (TabPage)typeof(SettingsForm).GetField("_advancedPage", flags).GetValue(form);
    bool fourPages = tabs.TabCount == 4 && tabs.TabPages[1] == advancedPage;
    bool initialChevron = toggle.Text.StartsWith("›  ", StringComparison.Ordinal);
    form.SelectTab(1); Application.DoEvents(); form.PerformLayout();
    toggle.PerformClick(); Application.DoEvents(); form.PerformLayout();
    panel.PerformLayout();
    var numericGrid = panel.Controls.Cast<Control>().First(c => (c.Tag as string) == "numericGrid");
    bool revealed = tabs.SelectedIndex == 1 && panel.Visible && numericGrid.Visible && toggle.Text.StartsWith("⌄  ", StringComparison.Ordinal);

    form.OpenFirstBoundaryForPreview();
    Application.DoEvents(); form.PerformLayout(); panel.PerformLayout();
    var returnLine = (Label)typeof(SettingsForm).GetField("_returnLine", flags).GetValue(form);
    bool truthfulReturn = returnLine.Text == Strings.Ui("ComingDownBlocked");
    var drawerFlow = (FlowLayoutPanel)typeof(SettingsForm).GetField("_drawerFlow", flags).GetValue(form);
    var editorHost = (Panel)typeof(SettingsForm).GetField("_screenSplit", flags).GetValue(form);
    var body = (FlowLayoutPanel)typeof(SettingsForm).GetField("_bodyFlow", flags).GetValue(form);
    var whenLink = (LinkLabel)typeof(SettingsForm).GetField("_whenLink", flags).GetValue(form);
    var separateLink = (LinkLabel)typeof(SettingsForm).GetField("_separateLink", flags).GetValue(form);
    Rectangle whenInDrawer = drawerFlow.RectangleToClient(whenLink.RectangleToScreen(whenLink.ClientRectangle));
    bool drawerFits = editorHost.Visible && whenLink.Visible && separateLink.Visible && !separateLink.Enabled
        && whenInDrawer.Top >= 0 && whenInDrawer.Bottom <= drawerFlow.ClientSize.Height
        && editorHost.Width >= body.ClientSize.Width - body.Padding.Horizontal - SystemInformation.VerticalScrollBarWidth - 8;
    bool resetsContained = panel.Controls.Cast<Control>().SelectMany(DescendantsAndSelf)
        .OfType<Button>().Where(button => (button.Tag as string) == "numericReset")
        .All(button => button.Right <= button.Parent.ClientSize.Width + 1);
    bool contained = !advancedPage.HorizontalScroll.Visible
        && panel.Controls.Cast<Control>().All(c => c.Right <= panel.ClientSize.Width + 1)
        && resetsContained;
    form.SelectTab(1); Application.DoEvents();
    toggle.PerformClick(); Application.DoEvents();
    bool collapsed = panel.Visible && !numericGrid.Visible && toggle.Text.StartsWith("›  ", StringComparison.Ordinal);
    Check($"v2 shell ({language}): four-page nav, physical-pixel fold, full-width inline editor, static disclosures",
        fourPages && initialChevron && revealed && truthfulReturn && drawerFits && contained && collapsed,
        $"pages={fourPages} initial={initialChevron} revealed={revealed} truthfulReturn={truthfulReturn} drawerFits={drawerFits} contained={contained} resetsContained={resetsContained} collapsed={collapsed} panelVisible={panel.Visible} toggle='{toggle.Text}' " +
        $"bodyClient={body.ClientSize.Width} editor={editorHost.Width} panel={panel.Width}/{panel.ClientSize.Width} " +
        $"childMax={panel.Controls.Cast<Control>().DefaultIfEmpty().Max(c => c?.Right ?? 0)} hscroll={advancedPage.HorizontalScroll.Visible}");
}

// B2 FIX2: the inherited and active overall-behavior links must receive their full measured single-line width.
foreach (string language in new[] { "en", "tr" })
{
    Strings.Use(language);
    var monitors = new List<MonitorInfo>
    {
        new() { StableId = "left", Device = @"\\.\DISPLAY1", Index = 1, Primary = true, Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
        new() { StableId = "right", Device = @"\\.\DISPLAY2", Index = 2, Bounds = new Native.RECT { Left = 100, Top = 0, Right = 200, Bottom = 100 } },
    };
    using var form = new SettingsForm(new Settings { Language = language }, monitors)
    {
        StartPosition = FormStartPosition.Manual,
        Location = new Point(SystemInformation.VirtualScreen.Right + 2000, SystemInformation.VirtualScreen.Bottom + 2000),
        Opacity = 0,
    };
    form.Show(); Application.DoEvents(); form.ClientSize = new Size(1064, 712); form.PerformLayout();
    form.OpenFirstBoundaryForPreview(); Application.DoEvents(); form.PerformLayout();
    var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
    var overall = (LinkLabel)typeof(SettingsForm).GetField("_overallLink", flags).GetValue(form);
    int RequiredWidth() => Math.Max(
        TextRenderer.MeasureText(overall.Text, overall.Font, Size.Empty,
            TextFormatFlags.SingleLine | TextFormatFlags.NoPrefix | TextFormatFlags.NoPadding).Width + 4 * form.DeviceDpi / 96,
        overall.GetPreferredSize(Size.Empty).Width);
    int measured = RequiredWidth();
    Check($"B2 FIX2 ({language}): inherited overall-behavior link receives its full measured width",
        overall.Width >= measured,
        $"text='{overall.Text}' width={overall.Width} measured={measured} left={overall.Left} parent={overall.Parent.ClientSize.Width} dpi={form.DeviceDpi}");

    var freeTile = (Control)typeof(SettingsForm).GetField("_freeTile", flags).GetValue(form);
    freeTile.GetType().GetMethod("OnClick", flags).Invoke(freeTile, new object[] { EventArgs.Empty });
    Application.DoEvents(); form.PerformLayout(); measured = RequiredWidth();
    Check($"B2 FIX2 ({language}): active overall-behavior link receives its full measured width",
        overall.Text == Strings.Ui("UseOverallBehavior") && overall.Width >= measured,
        $"text='{overall.Text}' width={overall.Width} measured={measured} left={overall.Left} parent={overall.Parent.ClientSize.Width} dpi={form.DeviceDpi}");
}

// FIX4 restore/import transaction: the form owns a draft, and only a confirmed valid import produces Result.
{
    Strings.Use("en");
    string path = Path.Combine(Path.GetTempPath(), $"MouseFence-valid-restore-{Guid.NewGuid():N}.json");
    try
    {
        using (var source = new SettingsForm(new Settings { Language = "en", UpCrossMin = 20 }, new List<MonitorInfo>()))
            source.BackupToFileForTests(path);
        var live = new Settings { Language = "en", UpCrossMin = 3 };
        using var form = new SettingsForm(live, new List<MonitorInfo>());
        int confirmations = 0;
        form.RestorePathPickerForTests = () => path;
        form.RestoreConfirmForTests = () => { confirmations++; return true; };
        form.RestoreFromFileForTests();
        Check("B2 restore: a confirmed valid exported file produces the replacement Result without mutating live settings",
            confirmations == 1 && form.DialogResult == DialogResult.OK && form.Result?.UpCrossMin == 20 && live.UpCrossMin == 3,
            $"confirmations={confirmations} dialog={form.DialogResult} result={form.Result?.UpCrossMin} live={live.UpCrossMin}");
    }
    finally { if (File.Exists(path)) File.Delete(path); }
}
{
    Strings.Use("en");
    string path = Path.Combine(Path.GetTempPath(), $"MouseFence-repairable-restore-{Guid.NewGuid():N}.json");
    try
    {
        File.WriteAllText(path, "{\"SettingsVersion\":4,\"UpCrossMin\":60,\"UpwardBoundaryDefault\":\"Surprise\"}");
        var live = new Settings { Language = "en", UpCrossMin = 3, UpwardBoundaryDefault = BoundaryDefault.Automatic };
        using var form = new SettingsForm(live, new List<MonitorInfo>());
        int confirmations = 0;
        form.RestorePathPickerForTests = () => path;
        form.RestoreConfirmForTests = () => { confirmations++; return true; };
        form.RestoreFromFileForTests();
        Check("B2 restore: repairable-invalid import remains a draft and leaves live settings untouched",
            confirmations == 0 && form.Result == null && form.DraftForTests.HasRepairableImportErrors
            && form.DraftForTests.UpCrossMin == 60 && live.UpCrossMin == 3,
            $"confirmations={confirmations} result={form.Result != null} repairable={form.DraftForTests.HasRepairableImportErrors} draft={form.DraftForTests.UpCrossMin} live={live.UpCrossMin}");

        form.DraftForTests.UpwardBoundaryDefault = BoundaryDefault.Automatic;
        var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
        typeof(SettingsForm).GetMethod("Save_Click", flags).Invoke(form, new object[] { form, EventArgs.Empty });
        Check("B2 restore: repairing the imported draft permits a valid apply Result while the caller-owned live object stays isolated",
            form.DialogResult == DialogResult.OK && form.Result?.UpCrossMin == 60
            && !form.Result.HasRepairableImportErrors && live.UpCrossMin == 3,
            $"dialog={form.DialogResult} result={form.Result?.UpCrossMin} resultRepairable={form.Result?.HasRepairableImportErrors} live={live.UpCrossMin}");
    }
    finally { if (File.Exists(path)) File.Delete(path); }
}

// Fold-in A: differing horizontal rules have no false primary selection; a primary choice reunifies the pair.
{
    Strings.Use("en");
    var monitors = new List<MonitorInfo>
    {
        new() { StableId = "left", Index = 1, Primary = true, Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
        new() { StableId = "right", Index = 2, Bounds = new Native.RECT { Left = 100, Top = 0, Right = 200, Bottom = 100 } },
    };
    var settings = new Settings
    {
        BoundaryRules = new List<BoundaryRule>
        {
            new() { FromStableId = "left", ToStableId = "right", Direction = BoundaryDirection.Right, Behavior = BoundaryBehavior.Push, Control = BoundaryControl.Automatic },
            new() { FromStableId = "right", ToStableId = "left", Direction = BoundaryDirection.Left, Behavior = BoundaryBehavior.Free, Control = BoundaryControl.Automatic },
        },
    };
    using var form = new SettingsForm(settings, monitors);
    ShowOffscreen(form);
    form.OpenFirstBoundaryForPreview();
    var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
    var caption = (Label)typeof(SettingsForm).GetField("_drawerDirection", flags).GetValue(form);
    var tiles = new[] { "_freeTile", "_pushTile", "_blockedTile" }
        .Select(name => typeof(SettingsForm).GetField(name, flags).GetValue(form)).ToArray();
    bool selected(object tile) => (bool)tile.GetType().GetProperty("Selected").GetValue(tile);
    Check("B2 split-direction: differing pair shows Different by direction and no selected primary tile",
        caption.Text == Strings.Ui("DifferentByDirection") && tiles.All(tile => !selected(tile)),
        $"caption='{caption.Text}' selected=[{string.Join(",", tiles.Select(selected))}]");

    var blocked = tiles[2];
    blocked.GetType().GetMethod("OnClick", flags).Invoke(blocked, new object[] { EventArgs.Empty });
    var pairRules = form.DraftForTests.BoundaryRules.Where(rule =>
        (rule.FromStableId == "left" && rule.ToStableId == "right") || (rule.FromStableId == "right" && rule.ToStableId == "left")).ToList();
    Check("B2 split-direction: selecting a primary tile reunifies both directions",
        caption.Text == Strings.Ui("BothDirections") && selected(blocked) && pairRules.Count == 2
        && pairRules.All(rule => rule.Behavior == BoundaryBehavior.Blocked),
        $"caption='{caption.Text}' selected={selected(blocked)} rules=[{string.Join(",", pairRules.Select(rule => rule.Behavior))}]");
}

// Fold-in B: status text exists only when live runtime state changes the selected/inherited effective behavior.
{
    Strings.Use("en");
    var monitors = new List<MonitorInfo>
    {
        new() { StableId = "left", Index = 1, Primary = true, Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
        new() { StableId = "right", Index = 2, Bounds = new Native.RECT { Left = 100, Top = 0, Right = 200, Bottom = 100 } },
    };
    bool sideProtectionOn = true;
    using var form = new SettingsForm(new Settings(), monitors,
        () => new SettingsRuntimeState(UpwardPassageOpen: true, SideProtectionOn: sideProtectionOn, Paused: false));
    ShowOffscreen(form);
    form.OpenFirstBoundaryForPreview();
    var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
    var status = (Label)typeof(SettingsForm).GetField("_drawerStatus", flags).GetValue(form);
    var overall = (LinkLabel)typeof(SettingsForm).GetField("_overallLink", flags).GetValue(form);
    Check("B2 status slot: plain inherited behavior agreeing with live state is empty",
        overall.Text == Strings.Ui("UsingOverallBehavior") && status.Text.Length == 0,
        $"overall='{overall.Text}' status='{status.Text}'");
    sideProtectionOn = false;
    typeof(SettingsForm).GetMethod("UpdateLiveStatus", flags).Invoke(form, null);
    Check("B2 status slot: live result diverging from the selected tile is non-empty",
        status.Text == Strings.Ui("StatusSideOff"), $"status='{status.Text}'");
}

// Owner v2 refinement: a kept-out target is not Free/Push/Blocked. The editor keeps its geometry,
// clears every selection, explains the stronger screen-level rule, and offers one primary recovery action.
foreach (string language in new[] { "en", "tr" })
{
    Strings.Use(language);
    var monitors = new List<MonitorInfo>
    {
        new() { StableId = "left", Index = 1, Primary = true, Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
        new() { StableId = "right", Index = 2, Bounds = new Native.RECT { Left = 100, Top = 0, Right = 200, Bottom = 100 } },
    };
    using var form = new SettingsForm(new Settings
    {
        Language = language,
        ExcludedDevices = new List<string> { "right" },
        MonitorNames = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase) { ["right"] = "Studio" },
    }, monitors);
    ShowOffscreen(form);
    form.OpenFirstBoundaryForPreview();
    Application.DoEvents(); form.PerformLayout();

    var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
    var tiles = new[] { "_freeTile", "_pushTile", "_blockedTile" }
        .Select(name => (Control)typeof(SettingsForm).GetField(name, flags).GetValue(form)).ToArray();
    bool selected(Control tile) => (bool)tile.GetType().GetProperty("Selected").GetValue(tile);
    var status = (Label)typeof(SettingsForm).GetField("_drawerStatus", flags).GetValue(form);
    var caption = (Label)typeof(SettingsForm).GetField("_drawerDirection", flags).GetValue(form);
    var allow = (ThemedButton)typeof(SettingsForm).GetField("_allowScreen", flags).GetValue(form);
    string expected = string.Format(Strings.Ui("ExcludedTargetExplanation"), "Studio");
    bool truthfulExcluded = tiles.All(tile => !tile.Enabled && !selected(tile))
        && caption.Text == Strings.Ui("BlockedByKeepOut") && status.Text == expected
        && allow.Enabled && allow.Role == ThemedButtonRole.Primary
        && allow.AccessibleDescription == expected;

    allow.PerformClick(); Application.DoEvents(); form.PerformLayout();
    bool recovered = !form.DraftForTests.ExcludedDevices.Contains("right", StringComparer.OrdinalIgnoreCase)
        && tiles.All(tile => tile.Enabled) && tiles.Count(selected) == 1
        && !allow.Enabled && allow.Role == ThemedButtonRole.Quiet;
    Check($"owner excluded-target editor ({language}): no false selection, plain explanation, primary Allow path",
        truthfulExcluded && recovered,
        $"truthful={truthfulExcluded} recovered={recovered} selected=[{string.Join(",", tiles.Select(selected))}] status='{status.Text}' expected='{expected}' allow={allow.Enabled}/{allow.Role}");
}

// Required follow-up: vertical editors normalize the primary direction to Up, so the lower endpoint is
// `_primary.From`. Excluding that endpoint must suppress the crossing controls just like any other target.
{
    Strings.Use("en");
    var monitors = new List<MonitorInfo>
    {
        new() { StableId = "lower", Index = 1, Primary = true, Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
        new() { StableId = "upper", Index = 2, Bounds = new Native.RECT { Left = 0, Top = -100, Right = 100, Bottom = 0 } },
    };
    using var form = new SettingsForm(new Settings
    {
        ExcludedDevices = new List<string> { "lower" },
        MonitorNames = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase) { ["lower"] = "Lower" },
    }, monitors);
    ShowOffscreen(form);
    form.OpenFirstBoundaryForPreview();
    Application.DoEvents(); form.PerformLayout();

    var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
    var tiles = new[] { "_freeTile", "_pushTile", "_blockedTile" }
        .Select(name => (Control)typeof(SettingsForm).GetField(name, flags).GetValue(form)).ToArray();
    bool selected(Control tile) => (bool)tile.GetType().GetProperty("Selected").GetValue(tile);
    var status = (Label)typeof(SettingsForm).GetField("_drawerStatus", flags).GetValue(form);
    var allow = (ThemedButton)typeof(SettingsForm).GetField("_allowScreen", flags).GetValue(form);
    string expected = string.Format(Strings.Ui("ExcludedTargetExplanation"), "Lower");
    bool blocked = tiles.All(tile => !tile.Enabled && !selected(tile))
        && status.Text == expected && allow.Enabled && allow.Role == ThemedButtonRole.Primary;

    allow.PerformClick(); Application.DoEvents(); form.PerformLayout();
    bool recovered = !form.DraftForTests.ExcludedDevices.Contains("lower", StringComparer.OrdinalIgnoreCase)
        && tiles.All(tile => tile.Enabled) && tiles.Count(selected) == 1;
    Check("owner excluded-target editor: excluded lower endpoint blocks a vertical boundary and can be allowed",
        blocked && recovered,
        $"blocked={blocked} recovered={recovered} selected=[{string.Join(",", tiles.Select(selected))}] status='{status.Text}' expected='{expected}' allow={allow.Enabled}/{allow.Role}");
}

// Owner v2 refinement follow-up: surplus height belongs to the map. Both resting modes keep the final
// content close to the footer, and the larger acceptance viewport must produce a strictly taller map.
{
    Strings.Use("en");
    (int MapHeight, int BodyHeight, int ListBand, int DrawerBand, Padding BodyPadding, int MapWidth) Measure(Size clientSize)
    {
        using var form = new SettingsForm(Screenshot.DemoSettings("Light", "en"), Screenshot.DemoMonitors());
        ShowOffscreen(form);
        form.ClientSize = clientSize;
        Application.DoEvents(); form.PerformLayout();
        var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
        var body = (FlowLayoutPanel)typeof(SettingsForm).GetField("_bodyFlow", flags).GetValue(form);
        var map = (Control)typeof(SettingsForm).GetField("_map", flags).GetValue(form);
        var rows = (Control)typeof(SettingsForm).GetField("_screenRows", flags).GetValue(form);
        int listBottom = body.RectangleToClient(rows.RectangleToScreen(rows.ClientRectangle)).Bottom;
        int mapHeight = map.Height;

        form.OpenFirstBoundaryForPreview(hideScreenList: true);
        Application.DoEvents(); form.PerformLayout();
        var drawer = (Control)typeof(SettingsForm).GetField("_screenSplit", flags).GetValue(form);
        int drawerBottom = body.RectangleToClient(drawer.RectangleToScreen(drawer.ClientRectangle)).Bottom;
        return (mapHeight, body.ClientSize.Height,
            Math.Max(0, body.ClientSize.Height - listBottom), Math.Max(0, body.ClientSize.Height - drawerBottom),
            body.Padding, map.Width);
    }

    var compact = Measure(new Size(1600, 1040));
    var maximized = Measure(new Size(2560, 1352));
    bool compactBandsFit = compact.ListBand <= compact.BodyHeight * .12 + 2
        && compact.DrawerBand <= compact.BodyHeight * .12 + 2;
    bool maximizedBandsFit = maximized.ListBand <= maximized.BodyHeight * .12 + 2
        && maximized.DrawerBand <= maximized.BodyHeight * .12 + 2;
    bool centeredMeasure = compact.MapWidth <= 1240 + 1 && maximized.MapWidth <= 1240 + 1
        && Math.Abs(compact.BodyPadding.Left - compact.BodyPadding.Right) <= 2
        && Math.Abs(maximized.BodyPadding.Left - maximized.BodyPadding.Right) <= 2;
    Check("owner large-window composition: leftover map grows and list/drawer footer bands stay within 12%",
        compactBandsFit && maximizedBandsFit && maximized.MapHeight > compact.MapHeight && centeredMeasure,
        $"1600x1040 map={compact.MapHeight} body={compact.BodyHeight} bands={compact.ListBand}/{compact.DrawerBand} padding={compact.BodyPadding}; " +
        $"2560x1352 map={maximized.MapHeight} body={maximized.BodyHeight} bands={maximized.ListBand}/{maximized.DrawerBand} padding={maximized.BodyPadding}");
}

// Fold-in C/D: frozen numeric help copy is present in the actual grid, and every actual status candidate fits the reserve.
foreach (string language in new[] { "en", "tr" })
{
    Strings.Use(language);
    var monitors = new List<MonitorInfo>
    {
        new() { StableId = "left", Index = 1, Primary = true, Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
        new() { StableId = "right", Index = 2, Bounds = new Native.RECT { Left = 100, Top = 0, Right = 200, Bottom = 100 } },
    };
    using var form = new SettingsForm(new Settings { Language = language,
        MonitorNames = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase) { ["left"] = "Twenty Four Character Name" } }, monitors);
    ShowOffscreen(form);
    var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
    var advanced = (Control)typeof(SettingsForm).GetField("_advancedPanel", flags).GetValue(form);
    var numericGrid = advanced.Controls.Cast<Control>().First(control => (control.Tag as string) == "numericGrid");
    var numericText = DescendantsAndSelf(numericGrid).OfType<Label>().Select(label => label.Text).ToHashSet();
    string strength = language == "en" ? "3 = selected crossing speed. 6 = twice that speed. Kept for existing settings."
        : "3 = seçilen geçiş hızı. 6 = bu hızın iki katı. Mevcut ayarlar korunur.";
    string holdback = language == "en"
        ? "0 = exactly at the edge (default). Applies wherever the pointer is stopped — Never crosses and a failed firm push alike. These are physical pixels."
        : "0 = tam kenarda (varsayılan). İşaretçinin durdurulduğu her yerde uygulanır: hem Asla geçmez hem de başarısız sıkı itiş için. Bunlar fiziksel piksellerdir.";
    Check($"B2 numeric helpers ({language}): speed scaling and hold-back explanations are present in the Advanced grid",
        Strings.Ui("UpStrengthHelp") == strength && Strings.Ui("EdgeHoldbackHelpV2") == holdback
        && numericText.Contains(strength) && numericText.Contains(holdback));

    form.OpenFirstBoundaryForPreview(); form.PerformLayout();
    var drawerStatus = (Label)typeof(SettingsForm).GetField("_drawerStatus", flags).GetValue(form);
    int reserveWidth = Math.Max(220 * form.DeviceDpi / 96, drawerStatus.Width);
    var measured = form.DrawerStatusCandidatesForTests.Select(text => TextRenderer.MeasureText(text, drawerStatus.Font,
        new Size(reserveWidth, 0), TextFormatFlags.WordBreak | TextFormatFlags.NoPrefix).Height + 4 * form.DeviceDpi / 96).ToList();
    Check($"B2 status reserve ({language}): allocated height fits every locale/width candidate",
        measured.All(height => drawerStatus.Height >= height),
        $"reserved={drawerStatus.Height} measured=[{string.Join(",", measured)}] width={reserveWidth}");
}

// Fold-in E: duplicate resolution consistently marks the later-recorded row and names both actions in the footer.
{
    Strings.Use("en");
    var settings = new Settings
    {
        HotKey = Keys.Up, ModCtrl = true, ModAlt = true,
        PauseHotKey = Keys.Up, PauseModCtrl = true, PauseModAlt = true,
    };
    using var form = new SettingsForm(settings, new List<MonitorInfo>());
    var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
    var boxes = (TextBox[])typeof(SettingsForm).GetField("_hotkeyBoxes", flags).GetValue(form);
    var frames = boxes.Select(box => (ThemedControlFrame)box.Parent).ToArray();
    string expected = string.Format(Strings.Ui("DuplicateShortcutsNamed"), Strings.HotkeyHead, Strings.PauseHotkeyHead);
    Check("B2 duplicate chord: later-recorded row is flagged and footer names both conflicting actions",
        !frames[0].Error && frames[2].Error && frames.Where((_, index) => index != 2).All(frame => !frame.Error)
        && form.InlineStatusForTests == expected,
        $"errors=[{string.Join(",", frames.Select(frame => frame.Error))}] footer='{form.InlineStatusForTests}' expected='{expected}'");
}

{
    Strings.Use("en");
    var monitors = new List<MonitorInfo>
    {
        new() { StableId = "top", Device = @"\\.\DISPLAY2", Index = 1, Bounds = new Native.RECT { Left = 0, Top = -100, Right = 100, Bottom = 0 } },
        new() { StableId = "main", Device = @"\\.\DISPLAY1", Index = 2, Primary = true, Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
    };
    using var form = new SettingsForm(new Settings { Language = "en" }, monitors);
    string path = Path.Combine(Path.GetTempPath(), $"MouseFence-m2p2-{Guid.NewGuid():N}.json");
    try
    {
        bool wrote = form.BackupToFileForTests(path);
        var restored = File.Exists(path) ? JsonSerializer.Deserialize<Settings>(File.ReadAllText(path)) : null;
        Check("M2 polish backup: successful write persists the draft and posts localized success feedback",
            wrote && restored != null && form.InlineStatusForTests == Strings.Ui("BackupDone"),
            $"wrote={wrote} restored={restored != null} status='{form.InlineStatusForTests}'");
    }
    finally
    {
        if (File.Exists(path)) File.Delete(path);
    }
}

{
    string normalized = Settings.NormalizeMonitorName("  " + string.Concat(Enumerable.Repeat("e\u0301", 30)) + "\tStudio\r\n  ");
    int elements = System.Globalization.StringInfo.ParseCombiningCharacters(normalized).Length;
    Check("M2 screen names: tabs/newlines become one line and the limit is 24 Unicode text elements",
        elements == 24 && !normalized.Contains('\t') && !normalized.Contains('\r') && !normalized.Contains('\n'),
        $"elements={elements} value={normalized}");
}

// M2 map: the same compiler-backed pair is mouse/keyboard reachable and represented by virtual accessible children.
{
    var settings = new Settings();
    var monitors = new List<MonitorInfo>
    {
        new() { StableId = "top", Index = 1, Bounds = new Native.RECT { Left = 0, Top = -100, Right = 100, Bottom = 0 } },
        new() { StableId = "main", Index = 2, Primary = true, Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
    };
    using var map = new MonitorMapControl { Size = new Size(400, 180) };
    map.SetModel(monitors, settings);
    using var bmp = new Bitmap(map.Width, map.Height); map.DrawToBitmap(bmp, map.ClientRectangle);
    var boundary = map.Boundaries.Single();
    var accessible = map.AccessibilityObject;
    Check("M2 map: one tab stop exposes screens and boundary as accessible virtual children with 24x24 target",
        map.TabStop && boundary.HitTarget.Width >= 24 && boundary.HitTarget.Height >= 24
        && accessible.GetChildCount() == monitors.Count + 1
        && accessible.GetChild(monitors.Count).Name.Contains(Strings.Ui("DrawerTitle"), StringComparison.Ordinal));
}

// Owner's exact July 2026 layout with the 800x600 dummy DISCONNECTED. The right monitor starts at Y=58,
// while the ultrawide above ends at X=2952. Thus X>2952,Y<58 is a real desktop void. These are intentionally
// monitor-aware assertions: every returned cursor point (Pass target or Block clamp) must lie on a usable monitor.
{
    var monitors = new List<(int L, int T, int R, int B)>
    {
        (-1920, 71, 0, 1151), (-488, -1440, 2952, 0),
        (0, 0, 2560, 1440), (2560, 58, 4480, 1138),
    };
    var c = new GuardCore
    {
        HasTop = true, BarrierY = 0, Gates = new List<(int Min, int Max)> { (24, 2536) },
        HasSides = true, MainL = 0, MainR = 2560, MainT = 0, MainB = 1440,
        Monitors = monitors,
    };
    SetLast(c, 4478, 59);                              // on Samsung, one pixel below its top-right corner
    var r = Move(c, 4485, -10, true, true);            // up/right beyond it; old up-clamp was (4485,0), a void
    Check("void invariant: reporter right-corner up-block lands on a usable monitor",
        IsLegalWallResult(c, r.bx, r.by), $"act={r.act} point=({r.bx},{r.by})");
}
{
    var monitors = new List<(int L, int T, int R, int B)>
    {
        (-1920, 71, 0, 1151), (-488, -1440, 2952, 0),
        (0, 0, 2560, 1440), (2560, 58, 4480, 1138),
    };
    var c = new GuardCore { HasTop = true, BarrierY = 0, Monitors = monitors };
    SetLast(c, 4478, 59);
    var r = Move(c, 4479, 50, true);                   // shallow OS/easing target above Samsung, below global barrier
    Check("void invariant: reporter shallow right-corner Pass target is contained on-screen",
        IsLegalWallResult(c, r.bx, r.by), $"act={r.act} point=({r.bx},{r.by})");
}

// Exact reporter seam-void repro: free-roam across the top overhang, descend through X[2560,2952),Y[0,58),
// then try upward twice at the same outside-gate X. Descent must choose Samsung below, not bounce to TOP.Y=-1.
// The repeated up attempts are legitimately gated at X=2700; moving back into the gate must still allow re-entry,
// proving the cursor is not stranded above the barrier with stale OnTop=false.
{
    (int L, int T, int R, int B) top = (-488, -1440, 2952, 0);
    (int L, int T, int R, int B) main = (0, 0, 2560, 1440);
    (int L, int T, int R, int B) samsung = (2560, 58, 4480, 1138);
    var monitors = new List<(int L, int T, int R, int B)>
    {
        (-1920, 71, 0, 1151), top, main, samsung,
    };
    var c = new GuardCore
    {
        HasTop = true, BarrierY = 0, Gates = new List<(int Min, int Max)> { (24, 2536) },
        Monitors = monitors,
    };
    SetLast(c, 1000, 0);
    var crossUp = Move(c, 1000, -10, true);
    var roamRight = Move(c, 2700, -10, true);
    var descend = Move(c, 2700, 30, true);
    var upAgain = Move(c, 2700, -100, true);
    var upAgain2 = Move(c, 2700, -200, true);
    var returnToGate = Move(c, 2500, 58, true);
    var crossUpAgain = Move(c, 2500, -100, true);

    Check("seam-void descent: nearest legal landing is down on Samsung, never TOP.Y=-1",
        descend.act == GuardAction.Block && descend.bx == 2700 && descend.by == samsung.T && !descend.onTop
        && InRect(samsung, descend.bx, descend.by),
        $"act={descend.act} point=({descend.bx},{descend.by}) onTop={descend.onTop}");
    Check("seam-void descent: repeated outside-gate up attempts do not recreate the stale TOP.Y=-1 fixpoint",
        upAgain.act == GuardAction.Block && upAgain.bx == 2700 && upAgain.by == samsung.T && !upAgain.onTop
        && upAgain2.act == GuardAction.Block && upAgain2.bx == 2700 && upAgain2.by == samsung.T && !upAgain2.onTop,
        $"up1={upAgain.act}/({upAgain.bx},{upAgain.by})/{upAgain.onTop} "
        + $"up2={upAgain2.act}/({upAgain2.bx},{upAgain2.by})/{upAgain2.onTop}");
    Check("seam-void descent: cursor can return to the gate and regain consistent top free-roam",
        returnToGate.act == GuardAction.Pass && !returnToGate.onTop
        && crossUpAgain.act == GuardAction.Pass && crossUpAgain.bx == 2500 && crossUpAgain.by == -100
        && crossUpAgain.onTop && InRect(top, crossUpAgain.bx, crossUpAgain.by),
        $"return={returnToGate.act}/({returnToGate.bx},{returnToGate.by})/{returnToGate.onTop} "
        + $"cross={crossUpAgain.act}/({crossUpAgain.bx},{crossUpAgain.by})/{crossUpAgain.onTop}");
    var sequence = new[] { crossUp, roamRight, descend, upAgain, upAgain2, returnToGate, crossUpAgain };
    Check("seam-void descent: every result in the reproduced sequence lands on a usable monitor",
        sequence.All(r => IsLegalWallResult(c, r.bx, r.by)),
        string.Join("; ", sequence.Select(r => $"{r.act}/({r.bx},{r.by})/{r.onTop}")));
}

// The reporter's far-left BenQ edge was already healthy. A legal move onto its outermost pixel must remain an exact
// Pass (the universal guard must not create a gratuitous clamp just because this layout also has right-side voids).
{
    var monitors = new List<(int L, int T, int R, int B)>
    {
        (-1920, 71, 0, 1151), (-488, -1440, 2952, 0),
        (0, 0, 2560, 1440), (2560, 58, 4480, 1138),
    };
    var c = new GuardCore { HasTop = true, BarrierY = 0, Monitors = monitors };
    SetLast(c, -1919, 500);
    var r = Move(c, -1920, 500, true);
    Check("void invariant: reporter left-screen outer edge remains an unchanged legal Pass",
        r.act == GuardAction.Pass && r.bx == -1920 && r.by == 500 && IsLegalWallResult(c, r.bx, r.by),
        $"act={r.act} point=({r.bx},{r.by})");
}

// Generalized up-overhang: the lower display is wider and starts below the global barrier. Keeping X while clamping
// to BarrierY would land in the empty strip; entry-side resolution must return to that same lower display.
{
    var c = new GuardCore
    {
        HasTop = true, BarrierY = 0,
        Monitors = new List<(int L, int T, int R, int B)> { (0, -100, 100, 0), (-50, 20, 150, 120) },
    };
    SetLast(c, 140, 21);
    var r = Move(c, 140, -10, false);
    Check("void invariant: wider lower-monitor overhang up-block stays on its entry display",
        r.act == GuardAction.Block && r.bx == 140 && r.by == 20 && IsLegalWallResult(c, r.bx, r.by),
        $"act={r.act} point=({r.bx},{r.by})");
}

// Generalized side gap: the old side-in clamp is MainR (inside the gap). It must resolve back onto the side display.
{
    var c = new GuardCore
    {
        HasSides = true, MainL = 0, MainR = 100, MainT = 0, MainB = 100, SideCrossMin = 50,
        Monitors = new List<(int L, int T, int R, int B)> { (0, 0, 100, 100), (120, 0, 220, 100) },
    };
    SetLast(c, 120, 50);
    var r = Move(c, 99, 50, true, true);
    Check("void invariant: side-in containment across a gap lands on the origin side display",
        r.act == GuardAction.Block && r.bx == 120 && r.by == 50 && IsLegalWallResult(c, r.bx, r.by),
        $"act={r.act} point=({r.bx},{r.by})");
}

// One portrait monitor at a negative origin, no baseline: even a point beyond every screen resolves to its nearest
// usable pixel. This exercises the Pass-to-Block safety net without relying on any barrier branch.
{
    var c = new GuardCore
    {
        Monitors = new List<(int L, int T, int R, int B)> { (-300, -500, -100, 100) },
    };
    var r = Move(c, 900, 900, true);
    Check("void invariant: 1 portrait negative-origin monitor contains a corner beyond every screen",
        r.act == GuardAction.Block && r.bx == -101 && r.by == 99 && IsLegalWallResult(c, r.bx, r.by),
        $"act={r.act} point=({r.bx},{r.by})");
}

// Two monitors with a gap and negative coordinates: equal inputs must resolve identically even if enumeration order
// is reversed. Geometry, never list order or floating point, breaks ties.
{
    var monitors = new List<(int L, int T, int R, int B)> { (-300, -100, -200, 100), (0, -100, 100, 100) };
    (GuardAction act, int x, int y) Run(IEnumerable<(int L, int T, int R, int B)> layout)
    {
        var c = new GuardCore { Monitors = layout.ToList() };
        var r = Move(c, -100, 0, true);                    // centered in the 100px gap: exact distance tie
        return (r.act, r.bx, r.by);
    }
    var a = Run(monitors);
    var b = Run(monitors.AsEnumerable().Reverse());
    var checkCore = new GuardCore { Monitors = monitors };
    Check("void invariant: 2-monitor gap tie is legal and independent of monitor enumeration",
        a == b && a.act == GuardAction.Block && IsLegalWallResult(checkCore, a.x, a.y),
        $"forward={a} reverse={b}");
}

// Eight-monitor deterministic stress: portrait screens, negative origins, gaps, side containment, barrier blocks,
// valid passes, and far-off corners. Completing both 512-move runs proves bounded termination; every result is legal.
{
    var monitors = new List<(int L, int T, int R, int B)>
    {
        (-420, -300, -300, 100), (-220, -180, 100, 0), (0, 0, 200, 140),
        (-200, 20, 0, 140), (220, 30, 420, 150), (0, 160, 140, 360),
        (500, -80, 580, 180), (-520, 220, -260, 300),
    };
    List<(GuardAction act, int x, int y)> Run(IEnumerable<(int L, int T, int R, int B)> layout, out bool legal)
    {
        var c = new GuardCore
        {
            HasTop = true, BarrierY = 0, Gates = new List<(int Min, int Max)> { (20, 180) },
            HasSides = true, MainL = 0, MainR = 200, MainT = 0, MainB = 140,
            Monitors = layout.ToList(), SideCrossMin = 9, SideCrossSlack = 4,
        };
        SetLast(c, 100, 70);
        var output = new List<(GuardAction act, int x, int y)>();
        legal = true;
        for (int i = 0; i < 512; i++)
        {
            int x = -700 + (i * 137 % 1501);
            int y = -420 + (i * 83 % 901);
            var r = Move(c, x, y, (i & 1) == 0, (i % 3) != 0);
            legal &= IsLegalWallResult(c, r.bx, r.by);
            output.Add((r.act, r.bx, r.by));
        }
        return output;
    }
    var forward = Run(monitors, out bool forwardLegal);
    var reverse = Run(monitors.AsEnumerable().Reverse(), out bool reverseLegal);
    Check("void invariant: 8-monitor mixed-layout stress terminates, stays legal, and is deterministic",
        forwardLegal && reverseLegal && forward.SequenceEqual(reverse)
        && forward.Any(r => r.act == GuardAction.Block) && forward.Any(r => r.act == GuardAction.Pass),
        $"forwardLegal={forwardLegal} reverseLegal={reverseLegal} same={forward.SequenceEqual(reverse)}");
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
// A descent-routing target in a void still misses cond7, but the universal final guard must contain it on-screen.
// With no lower monitor under this overhang, the nearest real landing is TOP.Y=-1, so OnTop must be restored from
// that resolved landing; otherwise the next upward free-roam move would be treated as a fresh gated crossing.
{
    var c = OnTopCore((TOP, MAINrect), TOP, MAINrect);   // RIGHT deliberately absent -> X 2800 below TOP is a void
    Move(c, 2800, -100, true);
    var r = Move(c, 2800, 10, true);        // exit at (2800,10): not in MAIN, not in any monitor -> void
    Check("DescentRouting_Void_IsContainedOnScreen",
        r.act == GuardAction.Block && r.by == -1 && r.onTop && IsLegalWallResult(c, r.bx, r.by),
        $"act={r.act} point=({r.bx},{r.by}) onTop={r.onTop}");
    var up = Move(c, 2800, -100, true);
    Check("DescentRouting_Void_ResolvedAboveBarrier_KeepsTopFreeRoam",
        up.act == GuardAction.Pass && up.bx == 2800 && up.by == -100 && up.onTop,
        $"act={up.act} point=({up.bx},{up.by}) onTop={up.onTop}");
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

// ---- WALL-OFF (Forbidden-rect exclusion): the cursor may never enter an excluded display ----
// Dummy under test: (2560,0,3360,600) — an 800x600 headless screen to the right of MAIN.
var DUMMY = (2560, 0, 3360, 600);

// 1) ENTRY-SIDE large delta (binding fix #1): Last well inside MAIN, target deep on the FAR side of the dummy.
//    Nearest-edge would tunnel to bx=3360 (the void side) and re-lose the cursor; entry-side must exit LEFT.
{
    var c = NewForbidden(DUMMY);
    Move(c, 2500, 300, true);                     // last position inside MAIN, LEFT of the dummy
    var r = Move(c, 3340, 300, true);             // big flick deep into the dummy (near its far-right wall)
    Check("wall-off: large-delta entry from left clamps to LEFT edge (no far-side tunnel)",
        r.act == GuardAction.Block && r.bx == 2559 && r.by == 300,
        $"act={r.act} bx={r.bx} by={r.by}  (expected Block bx=2559)");
}

// 1a) DIAGONAL entry (binding fix #1, segment-entry): Last outside on BOTH axes. The exit edge is the wall the
//     MOVE SEGMENT crosses FIRST, NOT the larger-delta axis. forbidden=(100,100,200,200), Last=(0,99) (left+above),
//     target=(150,150). dx=150 dominates dy=51, but the segment reaches the TOP plane (t=0.0196) long before the
//     LEFT plane (t=0.667), so the cursor must exit the TOP edge: bx==150 (X kept), by==99 (T-1). Delta-dominance
//     would have wrongly exited LEFT (bx==99).
{
    var c = NewForbidden((100, 100, 200, 200));
    Move(c, 0, 99, true);                         // last position diagonally outside: LEFT and ABOVE
    var r = Move(c, 150, 150, true);              // dive into the rect; TOP edge is crossed first
    Check("wall-off diagonal: first-crossed edge is TOP (bx kept, by=T-1) despite larger dx",
        r.act == GuardAction.Block && r.bx == 150 && r.by == 99,
        $"act={r.act} bx={r.bx} by={r.by}  (expected Block bx=150 by=99)");
}

// 1b) DIAGONAL mirror "shallow-left then dive": Last=(99,0) (left+above) into the same rect with target (150,150).
//     Here dy=150 dominates dx=51, but the segment reaches the LEFT plane (t=0.0196) first, so the cursor exits LEFT:
//     bx==99 (L-1), by==150 (Y kept). Proves the choice is segment-entry, not delta-dominance, in the mirror case.
{
    var c = NewForbidden((100, 100, 200, 200));
    Move(c, 99, 0, true);                         // last position diagonally outside: LEFT and ABOVE
    var r = Move(c, 150, 150, true);              // dive in; LEFT edge is crossed first this time
    Check("wall-off diagonal mirror: first-crossed edge is LEFT (by kept, bx=L-1) despite larger dy",
        r.act == GuardAction.Block && r.bx == 99 && r.by == 150,
        $"act={r.act} bx={r.bx} by={r.by}  (expected Block bx=99 by=150)");
}

// 2) Blocks entry from the LEFT, keeping the orthogonal (Y) axis.
{
    var c = NewForbidden(DUMMY);
    Move(c, 2500, 250, true);
    var r = Move(c, 2600, 250, true);             // crosses the left seam into the dummy
    Check("wall-off: entry from left -> clamp X to L-1, keep Y",
        r.act == GuardAction.Block && r.bx == 2559 && r.by == 250, $"act={r.act} bx={r.bx} by={r.by}");
}

// 3) Blocks entry from the TOP, keeping the orthogonal (X) axis.
//    Place the dummy ABOVE so a downward move enters across its top edge.
{
    var TOPDUMMY = (2560, -600, 3360, 0);
    var c = NewForbidden(TOPDUMMY);
    Move(c, 2900, -650, true);                    // last position ABOVE the dummy
    var r = Move(c, 2900, -300, true);            // move down into the dummy across its top edge
    Check("wall-off: entry from top -> clamp Y to T-1, keep X",
        r.act == GuardAction.Block && r.by == -601 && r.bx == 2900, $"act={r.act} bx={r.bx} by={r.by}");
}

// 4) Slide ALONG the seam on the real side (just OUTSIDE) -> Pass, no clamp, no jitter.
{
    var c = NewForbidden(DUMMY);
    Move(c, 2559, 100, true);
    var a = Move(c, 2559, 200, true);
    var b = Move(c, 2559, 300, true);
    var d = Move(c, 2559, 400, true);
    Check("wall-off: sliding along the seam OUTSIDE passes untouched (no sticking)",
        a.act == GuardAction.Pass && b.act == GuardAction.Pass && d.act == GuardAction.Pass
        && a.bx == 2559 && d.by == 400, $"a={a.act} b={b.act} d={d.act}");
}

// 5) Empty Forbidden list is inert (true no-op / OFF safety regression guard).
{
    var c = NewForbidden();                       // no forbidden rects, no top, no confine
    var r = Move(c, 3000, 300, true);             // anywhere
    Check("wall-off: empty list is inert (Pass, no clamp)",
        r.act == GuardAction.Pass && r.bx == 3000 && r.by == 300, $"act={r.act} bx={r.bx} by={r.by}");
}

// 6) (0) runs BEFORE the barrier: with a top present, a move into the dummy is walled by step 0,
//    and a normal gate cross elsewhere still works.
{
    var c = new GuardCore { HasTop = true, BarrierY = 0,
        Gates = new List<(int Min, int Max)> { (24, 2536) },
        Forbidden = new List<(int, int, int, int)> { DUMMY } };
    c.Reset();
    Move(c, 2500, 300, true);
    var wall = Move(c, 2600, 300, true);          // into the dummy -> walled by (0), never reaches the barrier
    Check("wall-off: runs before the barrier (dummy entry is walled, not gated)",
        wall.act == GuardAction.Block && wall.bx == 2559 && wall.by == 300 && !wall.onTop,
        $"act={wall.act} bx={wall.bx} by={wall.by}");
    var c2 = new GuardCore { HasTop = true, BarrierY = 0,
        Gates = new List<(int Min, int Max)> { (24, 2536) },
        Forbidden = new List<(int, int, int, int)> { DUMMY } };
    c2.Reset();
    Move(c2, 1280, 0, true);
    var cross = Move(c2, 1280, -15, true);        // a normal deliberate gate cross still works
    Check("wall-off: a normal gate cross is unaffected by the wall",
        cross.act == GuardAction.Pass && cross.onTop, $"act={cross.act} onTop={cross.onTop}");
}

// 7) Composes with Confine: (0) runs before the confine branch, so a move toward the dummy is walled first.
{
    var c = new GuardCore { HasTop = false, Confine = true,
        Monitors = new List<(int, int, int, int)> { (0, 0, 2560, 1440) },
        Forbidden = new List<(int, int, int, int)> { DUMMY } };
    c.Reset();
    Move(c, 2500, 300, true);                     // baseline on MAIN
    var r = Move(c, 2600, 300, true);             // toward the dummy -> step 0 walls before confine
    Check("wall-off: composes with Confine (walled by step 0 first)",
        r.act == GuardAction.Block && r.bx == 2559 && r.by == 300, $"act={r.act} bx={r.bx} by={r.by}");
}

// 8) No top, no confine — still walls (proves (0) runs before the !HasTop && !Confine bail-out), and NeedsProcessing.
{
    var c = NewForbidden(DUMMY);
    Check("wall-off: NeedsProcessing is true with only Forbidden set", c.NeedsProcessing, "NeedsProcessing");
    Move(c, 2500, 300, true);
    var r = Move(c, 2600, 300, true);
    Check("wall-off: no top + no confine still walls the dummy",
        r.act == GuardAction.Block && r.bx == 2559, $"act={r.act} bx={r.bx}");
}
{
    var c = new GuardCore();                      // nothing configured
    Check("wall-off: NeedsProcessing is false when nothing is set (hook stays out)", !c.NeedsProcessing, "NeedsProcessing");
}

// 9) Adjacent/overlapping forbidden rects (binding fix #3): a single-rect exit could land inside a neighbour;
//    the bounded loop must leave the point OUTSIDE ALL forbidden rects. Two abutting dummies span X 2560..4160.
{
    var D1 = (2560, 0, 3360, 600);
    var D2 = (3360, 0, 4160, 600);                // shares the seam at x=3360 with D1
    var c = NewForbidden(D1, D2);
    Move(c, 2500, 300, true);                     // last inside MAIN, left of both
    var r = Move(c, 3400, 300, true);             // target inside D2; entry-side from the left -> exit left of D1
    bool outsideBoth = !(r.bx >= D1.Item1 && r.bx < D1.Item3 && r.by >= D1.Item2 && r.by < D1.Item4)
                    && !(r.bx >= D2.Item1 && r.bx < D2.Item3 && r.by >= D2.Item2 && r.by < D2.Item4);
    Check("wall-off: adjacent dummies — clamped point is outside ALL forbidden rects",
        r.act == GuardAction.Block && outsideBoth && r.bx == 2559 && r.by == 300,
        $"act={r.act} bx={r.bx} by={r.by} outsideBoth={outsideBoth}");
}

// 9-FIXA-1) ROUND-3 FIX A — cx repro: adjacent pair, NO HaveLast. D1=(0,0,100,100), D2=(100,0,200,100); target
//    (99,50) lands inside D1. The OLD per-rect nearest-edge fallback exited D1 RIGHT to x=100 (inside D2), then D2
//    LEFT back to x=99 (inside D1) and the bounded loop ended STILL INSIDE D2. The connected-region exit must march
//    the chosen direction past the WHOLE component (D1∪D2 = X[0,200)) -> x=200, OUTSIDE BOTH. (No Last => first move.)
{
    var D1 = (0, 0, 100, 100);
    var D2 = (100, 0, 200, 100);
    var c = NewForbidden(D1, D2);
    var r = Move(c, 99, 50, true);                // first move (no Last) lands inside D1, adjacent to D2
    bool outsideBoth = !(r.bx >= D1.Item1 && r.bx < D1.Item3 && r.by >= D1.Item2 && r.by < D1.Item4)
                    && !(r.bx >= D2.Item1 && r.bx < D2.Item3 && r.by >= D2.Item2 && r.by < D2.Item4);
    Check("wall-off FIX A: no-HaveLast into adjacent pair -> outside BOTH (cx repro: not stuck at x=100)",
        r.act == GuardAction.Block && outsideBoth && r.bx == 200 && r.by == 50,
        $"act={r.act} bx={r.bx} by={r.by} outsideBoth={outsideBoth}");
}

// 9-FIXA-2) ROUND-3 FIX A — Last INSIDE a neighbouring forbidden rect, move into the OTHER. Last=(50,50) inside D1,
//    target (150,50) inside D2. Last is inside the connected region (no usable entry direction) -> nearest-edge of the
//    HIT rect (D2: dl=50,dr=50,dt=50,db=50 -> tie LEFT) would re-enter D1; the region march to regL-1 = -1 leaves the
//    point OUTSIDE BOTH. Either way the GUARANTEE (outside every rect) must hold and it must terminate.
{
    var D1 = (0, 0, 100, 100);
    var D2 = (100, 0, 200, 100);
    var c = NewForbidden(D1, D2);
    Move(c, 50, 50, true);                        // Last inside D1 (a neighbour of D2)
    var r = Move(c, 150, 50, true);               // move into D2
    bool outsideBoth = !(r.bx >= D1.Item1 && r.bx < D1.Item3 && r.by >= D1.Item2 && r.by < D1.Item4)
                    && !(r.bx >= D2.Item1 && r.bx < D2.Item3 && r.by >= D2.Item2 && r.by < D2.Item4);
    Check("wall-off FIX A: Last inside neighbour, move into the other -> outside BOTH (no oscillation)",
        r.act == GuardAction.Block && outsideBoth, $"act={r.act} bx={r.bx} by={r.by} outsideBoth={outsideBoth}");
}

// 9-FIXA-3) ROUND-3 FIX A — three-rect chain, deep tunnel from the left. D1,D2,D3 abut to span X[0,300). Last left of
//    all, a fast flick deep into D3: entry-side exits LEFT to the whole region's regL-1 = -1, never re-captured by D2/D1.
{
    var D1 = (0, 0, 100, 100);
    var D2 = (100, 0, 200, 100);
    var D3 = (200, 0, 300, 100);
    var c = NewForbidden(D1, D2, D3);
    Move(c, -10, 50, true);                       // Last left of the whole chain
    var r = Move(c, 280, 50, true);               // deep flick into D3
    bool outsideAll = !(r.bx >= 0 && r.bx < 300 && r.by >= 0 && r.by < 100);
    Check("wall-off FIX A: three-rect chain, deep tunnel from left -> exits LEFT past the whole region",
        r.act == GuardAction.Block && outsideAll && r.bx == -1 && r.by == 50,
        $"act={r.act} bx={r.bx} by={r.by} outsideAll={outsideAll}");
}

// 9-FIXC-1) ROUND-3 FIX C — boundary corner pixel: Last sits EXACTLY ON the rect's top-left INSIDE corner (100,100),
//    which is INSIDE the half-open rect [100,200)x[100,200), so there is NO usable entry direction (Last not outside
//    the region) and the nearest-edge fallback stands. Target (150,150): dl=dr=dt=db=50 -> the documented tie order
//    Left>Right>Top>Bottom picks LEFT. Exit to regL-1 == 99, Y kept == 150. (Guards the t-degenerate corner case.)
{
    var c = NewForbidden((100, 100, 200, 200));
    Move(c, 100, 100, true);                      // Last ON the TL corner pixel -> INSIDE the half-open rect
    var r = Move(c, 150, 150, true);
    Check("wall-off FIX C: Last on TL corner pixel (inside) -> nearest-edge tie picks LEFT, exit L-1",
        r.act == GuardAction.Block && r.bx == 99 && r.by == 150, $"act={r.act} bx={r.bx} by={r.by}");
}

// 9-FIXC-2) ROUND-3 FIX C — segment-entry t==1 INCLUSIVE bound, both-axes branch. Last=(0,50) is LEFT+ABOVE the rect
//    [100,200)x[100,200). Target (150,100) lands exactly on the TOP row: the Y (top) wall is reached at tY==1.0 EXACTLY
//    (validY only because the bound is inclusive, tY<=1), while tX=(100-0)/150=0.667. With both valid, useX = tX<=tY =
//    0.667<=1.0 -> true -> exit the LEFT wall: bx == L-1 == 99, by kept == 100. (Pins that t==1 is treated as a valid
//    crossing, not rejected by an exclusive bound.)
{
    var c = NewForbidden((100, 100, 200, 200));
    Move(c, 0, 50, true);                         // diagonally outside: LEFT and ABOVE
    var r = Move(c, 150, 100, true);              // target on the top row -> tY == 1.0 exactly (inclusive bound)
    Check("wall-off FIX C: t==1 inclusive (target on top row) -> both valid, tie to earlier-t LEFT wall",
        r.act == GuardAction.Block && r.bx == 99 && r.by == 100, $"act={r.act} bx={r.bx} by={r.by}");
}

// 9-FIXC-3) ROUND-3 FIX C — segment-entry t==0 / boundary-grazing case, both-axes branch. Last=(99,99) is LEFT+ABOVE
//    by exactly ONE px (the diagonal corner just outside the TL of [100,200)x[100,200)). The TOP and LEFT planes are
//    reached at the smallest possible positive t: tX=(100-99)/(150-99)=1/51, tY=(100-99)/(150-99)=1/51 -> EQUAL, so the
//    tie picks X (useX = tX<=tY). Exit LEFT: bx == L-1 == 99, by kept == 150. Guards the near-zero-t / equal-t corner.
{
    var c = NewForbidden((100, 100, 200, 200));
    Move(c, 99, 99, true);                        // one px diagonally outside the TL corner
    var r = Move(c, 150, 150, true);              // dive in; tX == tY (equal earliest crossing) -> tie picks X
    Check("wall-off FIX C: equal-earliest-t corner entry -> tie picks X, exit L-1",
        r.act == GuardAction.Block && r.bx == 99 && r.by == 150, $"act={r.act} bx={r.bx} by={r.by}");
}

// 9-MONITOR-AWARE) A geometric exit is not sufficient: every wall-off result must also be a pixel on a USABLE
// monitor. These scenarios are deliberately shaped so the pre-fix clamp's entry/nearest edge lands in a desktop
// void. They are pure GuardCore checks; injected mouse input cannot exercise the low-level hook correctly.

// Reporter repro, exact live geometry. The pre-fix result was (4479,1400): outside the dummy but below monitor 4.
{
    var monitors = new List<(int L, int T, int R, int B)>
    {
        (-1920, 71, 0, 1151), (-488, -1440, 2952, 0),
        (0, 0, 2560, 1440), (2560, 58, 4480, 1138),
    };
    var monitor4 = monitors[3];
    var c = NewMonitorAwareWall(monitors, (4480, 845, 5280, 1445));
    SetLast(c, 4470, 1000);
    var r = Move(c, 4900, 1400, true);
    Check("wall-off monitor-aware: reporter teleport returns to monitor 4, never the void below it",
        r.act == GuardAction.Block && r.bx < 4480 && InRect(monitor4, r.bx, r.by) && IsLegalWallResult(c, r.bx, r.by),
        $"act={r.act} bx={r.bx} by={r.by} onM4={InRect(monitor4, r.bx, r.by)} legal={IsLegalWallResult(c, r.bx, r.by)}");
}

// Wall FEEL regression: hitting the dummy seam from monitor 4 must NOT jump the cursor vertically to the OS-eased
// target height — it must stop at monitor 4's right edge at ~the ENTRY height. Live-captured symptom this guards:
// (4476,290) -> (4479,902), a ~600px downward jump that read as "still broken" even though the cursor stayed on-screen.
{
    var monitors = new List<(int L, int T, int R, int B)>
    {
        (-1920, 71, 0, 1151), (-488, -1440, 2952, 0),
        (0, 0, 2560, 1440), (2560, 58, 4480, 1138),
    };
    var monitor4 = monitors[3];
    var c = NewMonitorAwareWall(monitors, (4480, 845, 5280, 1445));
    SetLast(c, 4476, 290);
    var r = Move(c, 4900, 902, true);   // Windows-eased deep target into the dummy
    Check("wall-off monitor-aware: seam hit keeps entry height (no ~600px vertical jump)",
        r.act == GuardAction.Block && r.bx < 4480 && InRect(monitor4, r.bx, r.by) && Math.Abs(r.by - 290) <= 5
            && IsLegalWallResult(c, r.bx, r.by),
        $"act={r.act} bx={r.bx} by={r.by} (want bx=4479, by~290, on monitor 4)");
}

// A taller dummy on each side of a smaller usable neighbour. Each deep target is beyond the neighbour on the
// orthogonal axis, so preserving that target coordinate would land in a void; the clamp must adjust it on-screen.
{
    var real = (L: 0, T: 0, R: 100, B: 100);
    var c = NewMonitorAwareWall(new[] { real }, (100, -50, 200, 150));
    SetLast(c, 90, 90);
    var r = Move(c, 190, 140, true);
    Check("wall-off monitor-aware: dummy RIGHT -> entry-side usable neighbour (orthogonal clamp)",
        r.act == GuardAction.Block && InRect(real, r.bx, r.by) && r.bx == 99 && IsLegalWallResult(c, r.bx, r.by),
        $"bx={r.bx} by={r.by} legal={IsLegalWallResult(c, r.bx, r.by)}");
}
{
    var real = (L: 100, T: 0, R: 200, B: 100);
    var c = NewMonitorAwareWall(new[] { real }, (0, -50, 100, 150));
    SetLast(c, 110, 90);
    var r = Move(c, 10, 140, true);
    Check("wall-off monitor-aware: dummy LEFT -> entry-side usable neighbour (orthogonal clamp)",
        r.act == GuardAction.Block && InRect(real, r.bx, r.by) && r.bx == 100 && IsLegalWallResult(c, r.bx, r.by),
        $"bx={r.bx} by={r.by} legal={IsLegalWallResult(c, r.bx, r.by)}");
}
{
    var real = (L: 0, T: 100, R: 100, B: 200);
    var c = NewMonitorAwareWall(new[] { real }, (-50, 0, 150, 100));
    SetLast(c, 90, 110);
    var r = Move(c, 140, 10, true);
    Check("wall-off monitor-aware: dummy ABOVE -> entry-side usable neighbour (orthogonal clamp)",
        r.act == GuardAction.Block && InRect(real, r.bx, r.by) && r.by == 100 && IsLegalWallResult(c, r.bx, r.by),
        $"bx={r.bx} by={r.by} legal={IsLegalWallResult(c, r.bx, r.by)}");
}
{
    var real = (L: 0, T: 0, R: 100, B: 100);
    var c = NewMonitorAwareWall(new[] { real }, (-50, 100, 150, 200));
    SetLast(c, 90, 90);
    var r = Move(c, 140, 190, true);
    Check("wall-off monitor-aware: dummy BELOW -> entry-side usable neighbour (orthogonal clamp)",
        r.act == GuardAction.Block && InRect(real, r.bx, r.by) && r.by == 99 && IsLegalWallResult(c, r.bx, r.by),
        $"bx={r.bx} by={r.by} legal={IsLegalWallResult(c, r.bx, r.by)}");
}

// A corner dummy touches two real monitors. The last point identifies the LEFT monitor as the entry-side owner;
// even though the landing is near the dummy's opposite corner, the clamp must return to that same usable screen.
{
    var left = (L: 0, T: 100, R: 100, B: 160);
    var above = (L: 100, T: 0, R: 160, B: 100);
    var c = NewMonitorAwareWall(new[] { left, above }, (100, 100, 200, 200));
    SetLast(c, 90, 150);
    var r = Move(c, 190, 190, true);
    Check("wall-off monitor-aware: corner dummy prefers the real monitor the move came from",
        r.act == GuardAction.Block && InRect(left, r.bx, r.by) && !InRect(above, r.bx, r.by)
        && IsLegalWallResult(c, r.bx, r.by),
        $"bx={r.bx} by={r.by} onLeft={InRect(left, r.bx, r.by)} onAbove={InRect(above, r.bx, r.by)}");
}

// The dummy is shorter than its neighbour but vertically offset, leaving a lower overhang. The crossing axis (X)
// clamps to the neighbour's right edge; the orthogonal axis keeps the ENTRY height (190, on the real screen), so the
// cursor slides along the wall rather than jumping down to the eased target Y. Entry height is always on-screen.
{
    var real = (L: 0, T: 0, R: 100, B: 200);
    var c = NewMonitorAwareWall(new[] { real }, (100, 150, 200, 250));
    SetLast(c, 90, 190);
    var r = Move(c, 190, 240, true);
    Check("wall-off monitor-aware: shorter offset dummy still returns to its taller real neighbour",
        r.act == GuardAction.Block && InRect(real, r.bx, r.by) && r.bx == 99 && r.by == 190
        && IsLegalWallResult(c, r.bx, r.by),
        $"bx={r.bx} by={r.by} legal={IsLegalWallResult(c, r.bx, r.by)}");
}

// Two adjacent dummies are one region. Entry is from the usable screen beyond the RIGHT edge; the crossing axis (X)
// clamps to that real screen's left edge (300), past both dummies, while the orthogonal axis keeps the ENTRY height
// (90, on the real screen) — the cursor stops at the wall at its own height, no vertical jump.
{
    var real = (L: 300, T: 0, R: 400, B: 100);
    var d1 = (L: 100, T: -50, R: 200, B: 150);
    var d2 = (L: 200, T: -50, R: 300, B: 150);
    var c = NewMonitorAwareWall(new[] { real }, d1, d2);
    SetLast(c, 310, 90);
    var r = Move(c, 110, 140, true);
    Check("wall-off monitor-aware: adjacent dummies march as one region onto the real screen beyond both",
        r.act == GuardAction.Block && InRect(real, r.bx, r.by) && r.bx == 300 && r.by == 90
        && IsLegalWallResult(c, r.bx, r.by),
        $"bx={r.bx} by={r.by} legal={IsLegalWallResult(c, r.bx, r.by)}");
}

// With no usable entry direction, every far-edge landing chooses the nearest usable monitor instead of accepting
// the naive geometric exit into a void.
{
    var real = (L: 0, T: 0, R: 50, B: 50);
    var targets = new[]
    {
        (Name: "LEFT", X: 101, Y: 150), (Name: "RIGHT", X: 199, Y: 150),
        (Name: "TOP", X: 150, Y: 101), (Name: "BOTTOM", X: 150, Y: 199),
    };
    foreach (var target in targets)
    {
        var c = NewMonitorAwareWall(new[] { real }, (100, 100, 200, 200));
        var r = Move(c, target.X, target.Y, true);       // no HaveLast
        Check($"wall-off monitor-aware: no-baseline deep landing near {target.Name} edge resolves on-screen",
            r.act == GuardAction.Block && InRect(real, r.bx, r.by) && IsLegalWallResult(c, r.bx, r.by),
            $"bx={r.bx} by={r.by} legal={IsLegalWallResult(c, r.bx, r.by)}");
    }
}

// Nearest-monitor fallback has a documented geometry tie-break. Rebuilding the same core twice must return the
// identical left-side pixel; explicitly putting Last inside the forbidden region uses the same fallback.
{
    var left = (L: 0, T: 100, R: 100, B: 140);
    var right = (L: 201, T: 100, R: 301, B: 140);
    (int X, int Y, bool Legal) Run(bool lastInside)
    {
        var c = NewMonitorAwareWall(new[] { right, left }, (100, 100, 201, 200)); // reverse input order intentionally
        if (lastInside) SetLast(c, 150, 150);
        var r = Move(c, 150, 150, true);
        return (r.bx, r.by, r.act == GuardAction.Block && IsLegalWallResult(c, r.bx, r.by));
    }
    var a = Run(false);
    var b = Run(false);
    var inside = Run(true);
    var repeatCore = NewMonitorAwareWall(new[] { right, left }, (100, 100, 201, 200));
    var firstOnSameCore = Move(repeatCore, 150, 150, true);
    var secondOnSameCore = Move(repeatCore, 150, 150, true);
    bool repeatsExactly = firstOnSameCore.bx == secondOnSameCore.bx
                       && firstOnSameCore.by == secondOnSameCore.by
                       && IsLegalWallResult(repeatCore, secondOnSameCore.bx, secondOnSameCore.by);
    Check("wall-off monitor-aware: nearest-monitor tie is deterministic and independent of monitor list order",
        a == b && a.Legal && a.X == 99 && a.Y == 139 && repeatsExactly,
        $"a={a} b={b} sameCore=({firstOnSameCore.bx},{firstOnSameCore.by})->({secondOnSameCore.bx},{secondOnSameCore.by})");
    Check("wall-off monitor-aware: Last inside forbidden region falls back to nearest legal monitor",
        inside.Legal && inside.X == 99 && inside.Y == 139,
        $"inside={inside}");
}

// Pathological connected ring: the usable monitor is a hole inside the component's bounding box. The old region
// extent march returns to an outer void; the monitor-aware fallback must find the legal centre and terminate.
{
    var centre = (L: 100, T: 100, R: 200, B: 200);
    var ring = new[]
    {
        (L: 0, T: 0, R: 100, B: 300), (L: 100, T: 0, R: 200, B: 100),
        (L: 200, T: 0, R: 300, B: 300), (L: 100, T: 200, R: 200, B: 300),
    };
    var c = NewMonitorAwareWall(new[] { centre }, ring);
    var r = Move(c, 250, 250, true);
    Check("wall-off monitor-aware: pathological dummy ring terminates on its usable centre",
        r.act == GuardAction.Block && InRect(centre, r.bx, r.by) && IsLegalWallResult(c, r.bx, r.by),
        $"bx={r.bx} by={r.by} legal={IsLegalWallResult(c, r.bx, r.by)}");
}

// 10) Topology: an excluded device is dropped from the AntiTrap inputs, so it can't become a spurious gate/owner.
//     Mirrors existing test #10 — feed the DUMMY-DROPPED rect list and assert no spurious gate for the right screen.
{
    // left, MAIN, right (edge-touching MAIN), top, and a DUMMY abutting the right screen (56px-style seam).
    // With the dummy present it would give the right screen a side neighbour; WALL-OFF drops the dummy from
    // topology so AntiTrap is computed over the real screens only — and the right screen stays non-isolated
    // via its real seam with MAIN, with NO gate spuriously created by the dummy.
    var withDummy = new List<(int, int, int, int)>
    {
        (-1920, 0, 0, 1080),     // 0 left
        (0, 0, 2560, 1440),      // 1 MAIN
        (2560, 0, 4480, 1080),   // 2 right (touches MAIN at x=2560)
        (-450, -1440, 2990, 0),  // 3 top
        (4480, 0, 5280, 600),    // 4 DUMMY abutting the right screen
    };
    // Drop the excluded dummy (index 4) before deriving topology — what Configure() does with the two-list split.
    var dropped = withDummy.Where((m, i) => i != 4).ToList();
    var topIdx = new HashSet<int> { 3 };
    var (warn, gates) = GuardCore.AntiTrap(dropped, topIdx);
    Check("wall-off topology: excluded device absent -> no spurious isolation/gate; right screen unaffected",
        warn.Count == 0 && gates.Count == 0, $"warn=[{string.Join(",", warn)}] gates={gates.Count}");
}

// 11) Primary-sanitization (binding fix #2 / FIX 3): the primary display can never be walled off, and never leave
//     zero usable monitors. The logic now lives in the SHARED pure helper GuardCore.SanitizeExcluded — Configure()
//     AND these tests call THE SAME method (no duplicated list math). Keys are STABLE ids (FIX 4).
{
    // monitors: MAIN (primary) + a dummy. User (or an imported file) asks to exclude BOTH.
    var present = new List<string> { "MONITOR\\MAIN\\{guid}", "MONITOR\\DUMMY\\{guid}" };
    var primary = "MONITOR\\MAIN\\{guid}";
    var requested = new HashSet<string> { "MONITOR\\MAIN\\{guid}", "MONITOR\\DUMMY\\{guid}" };
    var excluded = GuardCore.SanitizeExcluded(present, primary, requested, out bool pruned);
    Check("wall-off safety: primary is never excluded; one usable monitor remains; prune flagged",
        excluded.Count == 1 && excluded[0] == "MONITOR\\DUMMY\\{guid}" && !excluded.Contains(primary) && pruned,
        $"excluded=[{string.Join(",", excluded)}] pruned={pruned}");
}
{
    // Keep-one rule: a single-monitor PC where the user asks to exclude that one (non-primary path edge): never zero.
    var present = new List<string> { "MONITOR\\ONLY\\{guid}" };
    var primary = "MONITOR\\PRIMARY-ELSEWHERE";    // primary not present (e.g. stale import) -> keep-one must still fire
    var requested = new HashSet<string> { "MONITOR\\ONLY\\{guid}" };
    var excluded = GuardCore.SanitizeExcluded(present, primary, requested, out bool pruned);
    Check("wall-off safety: never leaves zero usable monitors (keep-one)",
        excluded.Count == 0 && pruned, $"excluded={excluded.Count} pruned={pruned}");
}
{
    // A requested key that is ABSENT from the live layout (imported/stale id) is dormant — NOT a prune, and the
    // present, requested non-primary monitor is still excluded normally.
    var present = new List<string> { "MONITOR\\MAIN\\{guid}", "MONITOR\\DUMMY\\{guid}" };
    var primary = "MONITOR\\MAIN\\{guid}";
    var requested = new HashSet<string> { "MONITOR\\DUMMY\\{guid}", "MONITOR\\GONE\\{guid}" };
    var excluded = GuardCore.SanitizeExcluded(present, primary, requested, out bool pruned);
    Check("wall-off safety: an absent (imported) id is dormant -> not a prune; present dummy still excluded",
        excluded.Count == 1 && excluded[0] == "MONITOR\\DUMMY\\{guid}" && !pruned,
        $"excluded=[{string.Join(",", excluded)}] pruned={pruned}");
}

// 12) STABLE-KEY identity (FIX 4): the exclude set keys on MonitorInfo.StableId, NOT \\.\DISPLAYn, so the SAME
//     monitor stays excluded after a layout re-arrange renumbers DISPLAYn. Simulate two device lists with the SAME
//     StableId but DIFFERENT \\.\DISPLAYn ordering; the sanitize/match must still pick the right monitor by StableId.
{
    // A small stand-in for MonitorInfo's (Device, StableId) pairing — the production match keys on StableId.
    var primaryStable = "MONITOR\\MAIN-EDID\\{guid}";
    var dummyStable = "MONITOR\\DUMMY-EDID\\{guid}";
    var requested = new HashSet<string> { dummyStable };   // the user excluded the dummy by its STABLE id

    // Layout #1: dummy is \\.\DISPLAY2.   Layout #2 (after re-arrange): the SAME dummy is now \\.\DISPLAY4.
    var layout1 = new List<(string Device, string Stable)>
        { ("\\\\.\\DISPLAY1", primaryStable), ("\\\\.\\DISPLAY2", dummyStable) };
    var layout2 = new List<(string Device, string Stable)>
        { ("\\\\.\\DISPLAY3", primaryStable), ("\\\\.\\DISPLAY4", dummyStable) };

    // Configure() matches by StableId: excluded = SanitizeExcluded(presentStableIds, primaryStable, requested) then
    // map back to the monitor whose StableId is in the result.
    (string Device, string Stable)? ExcludedMonitor(List<(string Device, string Stable)> layout)
    {
        var present = layout.Select(m => m.Stable).ToList();
        var ex = new HashSet<string>(GuardCore.SanitizeExcluded(present, primaryStable, requested, out _));
        var hit = layout.Where(m => ex.Contains(m.Stable)).ToList();
        return hit.Count == 1 ? hit[0] : null;
    }

    var e1 = ExcludedMonitor(layout1);
    var e2 = ExcludedMonitor(layout2);
    Check("wall-off stable-key: the SAME dummy is excluded under both DISPLAYn orderings (identity survives renumber)",
        e1.HasValue && e2.HasValue
        && e1.Value.Stable == dummyStable && e2.Value.Stable == dummyStable
        && e1.Value.Device == "\\\\.\\DISPLAY2" && e2.Value.Device == "\\\\.\\DISPLAY4",
        $"e1={e1?.Device}/{e1?.Stable} e2={e2?.Device}/{e2?.Stable}");
}

// 13) ROUND-3 FIX B — DUPLICATE StableId disambiguation: two PHYSICALLY-DISTINCT monitors with the SAME raw EDID key
//     (identical panels) must get DISTINCT StableIds so excluding ONE excludes ONLY that one. GuardCore.DisambiguateKeys
//     is the pure rule MonitorInfo.All() feeds; here we feed present=[PRIMARY, DUP, DUP] and assert the two DUPs become
//     distinct keys and excluding one leaves the other usable. (cx probe: present=[PRIMARY,DUP,DUP], exclude DUP.)
{
    var primaryRaw = "MONITOR\\PRIMARY-EDID\\{guid}";
    var dupRaw = "MONITOR\\DUP-EDID\\{guid}";       // SAME raw key on two physical panels (identical EDID)
    // present, in display order, with each monitor's stable geometric attribute (Left, Top) for the deterministic suffix.
    var present = new List<(string Key, int Left, int Top)>
    {
        (primaryRaw, 0, 0),        // 0 PRIMARY
        (dupRaw, 2560, 0),         // 1 DUP-A (left=2560)
        (dupRaw, 3360, 0),         // 2 DUP-B (left=3360)
    };
    var keys = GuardCore.DisambiguateKeys(present);

    // The two identical-panel monitors now have DISTINCT keys; the unique primary is left untouched (no suffix).
    bool distinct = keys[1] != keys[2];
    bool primaryUntouched = keys[0] == primaryRaw;
    // Deterministic ordinal by ascending Left: DUP-A (2560) -> #0, DUP-B (3360) -> #1.
    bool deterministicOrder = keys[1] == dupRaw + "#0" && keys[2] == dupRaw + "#1";
    Check("wall-off FIX B: identical-EDID panels get DISTINCT StableIds (deterministic by Left); primary untouched",
        distinct && primaryUntouched && deterministicOrder,
        $"keys=[{string.Join(",", keys)}]");

    // Now the user excludes ONE physical DUP (DUP-A's disambiguated key). Configure()'s per-monitor match must wall
    // EXACTLY that one and leave the other DUP usable (the OLD raw-key HashSet collapse would have excluded BOTH).
    var primaryKey = keys[0];
    var requested = new HashSet<string> { keys[1] };   // exclude only DUP-A
    var excluded = new HashSet<string>(
        GuardCore.SanitizeExcluded(keys, primaryKey, requested, out bool pruned));
    // Match per-monitor by the disambiguated key (exactly what TrayApplicationContext.Configure does).
    int excludedCount = present.Select((m, i) => keys[i]).Count(k => excluded.Contains(k));
    bool dupAExcluded = excluded.Contains(keys[1]);
    bool dupBUsable = !excluded.Contains(keys[2]);
    Check("wall-off FIX B: excluding ONE identical panel excludes ONLY that one (the other stays usable)",
        excludedCount == 1 && dupAExcluded && dupBUsable && !pruned,
        $"excludedCount={excludedCount} dupAExcluded={dupAExcluded} dupBUsable={dupBUsable} pruned={pruned}");
}

// Exact clones share every desktop coordinate. Excluding the non-primary clone must not make the surviving primary's
// same pixels forbidden; otherwise the first hook event is pushed outside the entire virtual desktop.
{
    var clone = (L: 0, T: 0, R: 100, B: 100);
    var c = NewMonitorAwareWall(new[] { clone }, clone);
    var r = Move(c, 50, 50, true);
    Check("wall-off clone overlap: usable clone geometry wins and the first event stays on-screen",
        r.act == GuardAction.Pass && r.bx == 50 && r.by == 50 && InRect(clone, r.bx, r.by),
        $"act={r.act} bx={r.bx} by={r.by}");
}

// 14) STABLE-KEY identity for manual top monitors and UpLinks: the SAME physical monitors keep their rules when
//     Windows renumbers \\.\DISPLAYn. Production now stores ManualMonitors/UpLinks by StableId, just like excludes.
{
    var bottomStable = "MONITOR\\BOTTOM\\{guid}";
    var topStable = "MONITOR\\TOP\\{guid}";
    var requestedManual = new HashSet<string> { topStable };
    var requestedLinks = new List<(string From, string To)> { (bottomStable, topStable) };

    var layout1 = new List<(string Device, string Stable)>
        { ("\\\\.\\DISPLAY1", bottomStable), ("\\\\.\\DISPLAY4", topStable) };
    var layout2 = new List<(string Device, string Stable)>
        { ("\\\\.\\DISPLAY3", bottomStable), ("\\\\.\\DISPLAY2", topStable) };

    (string FromDevice, string ToDevice)? ResolveLink(List<(string Device, string Stable)> layout)
    {
        var present = layout.ToDictionary(m => m.Stable, m => m.Device);
        var topSet = new HashSet<string>(layout.Where(m => requestedManual.Contains(m.Stable)).Select(m => m.Stable));
        foreach (var link in requestedLinks)
            if (topSet.Contains(link.To) && present.ContainsKey(link.From) && present.ContainsKey(link.To))
                return (present[link.From], present[link.To]);
        return null;
    }

    var l1 = ResolveLink(layout1);
    var l2 = ResolveLink(layout2);
    Check("layout rules stable-key: ManualMonitors/UpLinks survive DISPLAYn renumber and target the same physical pair",
        l1.HasValue && l2.HasValue
        && l1.Value.FromDevice == "\\\\.\\DISPLAY1" && l1.Value.ToDevice == "\\\\.\\DISPLAY4"
        && l2.Value.FromDevice == "\\\\.\\DISPLAY3" && l2.Value.ToDevice == "\\\\.\\DISPLAY2",
        $"l1={l1?.FromDevice}->{l1?.ToDevice} l2={l2?.FromDevice}->{l2?.ToDevice}");
}

// 15) Stale StableIds are inert: absent ManualMonitors/UpLinks/Excludes do not match a different live monitor.
{
    var present = new List<string> { "MONITOR\\MAIN\\{guid}", "MONITOR\\TOP\\{guid}" };
    var primary = "MONITOR\\MAIN\\{guid}";
    var stale = "MONITOR\\OLD-DUMMY\\{guid}";
    var excluded = GuardCore.SanitizeExcluded(present, primary, new HashSet<string> { stale }, out bool pruned);

    var liveByStable = present.ToDictionary(k => k, k => k);
    var manual = new HashSet<string> { stale };
    var links = new List<(string From, string To)> { (primary, stale), (stale, "MONITOR\\TOP\\{guid}") };
    bool manualMatched = present.Any(manual.Contains);
    bool anyLinkMatched = links.Any(l => liveByStable.ContainsKey(l.From) && liveByStable.ContainsKey(l.To));

    Check("layout stale refs: absent stable ids are inert and never retarget another monitor",
        excluded.Count == 0 && !pruned && !manualMatched && !anyLinkMatched,
        $"excluded={excluded.Count} pruned={pruned} manualMatched={manualMatched} anyLinkMatched={anyLinkMatched}");
}

// 16) Migration: old \\.\DISPLAY-keyed manual/rule settings convert through the live device map; stale DISPLAY refs drop.
{
    var map = new Dictionary<string, string>
    {
        ["\\\\.\\DISPLAY1"] = "MONITOR\\MAIN\\{guid}",
        ["\\\\.\\DISPLAY4"] = "MONITOR\\TOP\\{guid}",
    };
    var oldManual = new List<string> { "\\\\.\\DISPLAY4", "\\\\.\\DISPLAY9" };
    var oldLinks = new List<(string From, string To)>
    {
        ("\\\\.\\DISPLAY1", "\\\\.\\DISPLAY4"),
        ("\\\\.\\DISPLAY1", "\\\\.\\DISPLAY9"),
    };
    var migrated = GuardCore.MigrateLayoutKeysToStable(oldManual, oldLinks, map);

    Check("layout migration: present DISPLAY keys convert to StableId and stale DISPLAY keys are dropped",
        migrated.ManualKeys.SequenceEqual(new[] { "MONITOR\\TOP\\{guid}" })
        && migrated.Links.Count == 1
        && migrated.Links[0] == ("MONITOR\\MAIN\\{guid}", "MONITOR\\TOP\\{guid}"),
        $"manual=[{string.Join(",", migrated.ManualKeys)}] links={migrated.Links.Count}");
}

// 17) Reset current layout config: clear manual picks and explicit links, return to AutoTop defaults.
{
    var reset = GuardCore.ResetLayoutConfig();
    Check("layout reset: returns to AutoTop and clears manual monitors plus explicit crossing rules",
        reset.Mode == "AutoTop" && reset.ManualKeys.Count == 0 && reset.Links.Count == 0,
        $"mode={reset.Mode} manual={reset.ManualKeys.Count} links={reset.Links.Count}");
}

// 18) MapLayout: the same pure transform drives map painting and hit-testing across common and extreme layouts.
bool ValidMapLayout(string name, IReadOnlyList<(int L, int T, int R, int B)> source,
    int targetW, int targetH, int pad, out string detail)
{
    var mapped = GuardCore.MapLayout(source, targetW, targetH, pad);
    bool count = mapped.Count == source.Count;
    bool inside = mapped.All(r => r.L >= pad && r.T >= pad && r.R <= targetW - pad && r.B <= targetH - pad
                                  && r.R > r.L && r.B > r.T);
    bool aspect = count;
    for (int i = 0; i < source.Count && aspect; i++)
    {
        int sw = source[i].R - source[i].L, sh = source[i].B - source[i].T;
        int dw = mapped[i].R - mapped[i].L, dh = mapped[i].B - mapped[i].T;
        double scaleDifference = Math.Abs(dw / (double)sw - dh / (double)sh);
        aspect = scaleDifference <= 1.1 / sw + 1.1 / sh;
    }

    bool order = count;
    for (int i = 0; i < source.Count && order; i++)
        for (int j = 0; j < source.Count && order; j++)
        {
            if (source[i].R <= source[j].L && mapped[i].R > mapped[j].L) order = false;
            if (source[i].B <= source[j].T && mapped[i].B > mapped[j].T) order = false;
        }

    int minL = mapped.Count == 0 ? 0 : mapped.Min(r => r.L);
    int maxR = mapped.Count == 0 ? 0 : mapped.Max(r => r.R);
    int minT = mapped.Count == 0 ? 0 : mapped.Min(r => r.T);
    int maxB = mapped.Count == 0 ? 0 : mapped.Max(r => r.B);
    bool centered = mapped.Count > 0 && Math.Abs((minL + maxR) - targetW) <= 1
                                    && Math.Abs((minT + maxB) - targetH) <= 1;
    detail = $"{name}: count={mapped.Count} inside={inside} aspect={aspect} order={order} centered={centered} " +
             $"bounds={minL},{minT}-{maxR},{maxB}";
    return count && inside && aspect && order && centered;
}

{
    var source = new List<(int L, int T, int R, int B)> { (0, 0, 1920, 1080) };
    var mapped = GuardCore.MapLayout(source, 400, 300, 10);
    bool valid = ValidMapLayout("single", source, 400, 300, 10, out string detail);
    Check("map layout: single monitor scales and centers exactly",
        valid && mapped[0] == (10, 43, 390, 257), $"{detail} rect={mapped[0]}");
}
{
    var source = new List<(int L, int T, int R, int B)>
        { (0, 0, 1920, 1080), (1920, 0, 3840, 1080) };
    var mapped = GuardCore.MapLayout(source, 600, 240, 10);
    bool valid = ValidMapLayout("side-by-side", source, 600, 240, 10, out string detail);
    Check("map layout: two side-by-side monitors keep adjacency and order",
        valid && mapped[0].R == mapped[1].L && mapped[0].T == mapped[1].T, detail);
}
{
    var source = new List<(int L, int T, int R, int B)>
        { (0, 0, 1000, 1000), (1500, 0, 2500, 1000) };
    var mapped = GuardCore.MapLayout(source, 520, 240, 10);
    bool valid = ValidMapLayout("gap", source, 520, 240, 10, out string detail);
    Check("map layout: a real desktop gap remains visible",
        valid && mapped[1].L - mapped[0].R == 100, $"{detail} gap={mapped[1].L - mapped[0].R}");
}
{
    var source = new List<(int L, int T, int R, int B)>
        { (-2560, 0, 0, 1440), (0, 0, 1920, 1080) };
    var mapped = GuardCore.MapLayout(source, 700, 300, 12);
    bool valid = ValidMapLayout("negative-origin", source, 700, 300, 12, out string detail);
    Check("map layout: negative-origin left monitor stays left of Main",
        valid && mapped[0].R == mapped[1].L, detail);
}
{
    var source = new List<(int L, int T, int R, int B)>
        { (-1080, -400, 0, 1520), (0, 0, 1920, 1080) };
    bool valid = ValidMapLayout("portrait", source, 640, 420, 10, out string detail);
    var mapped = GuardCore.MapLayout(source, 640, 420, 10);
    Check("map layout: portrait geometry preserves its tall aspect",
        valid && (mapped[0].B - mapped[0].T) > (mapped[0].R - mapped[0].L), detail);
}
{
    var source = new List<(int L, int T, int R, int B)>
    {
        (-5120, -2160, -3200, -1080), (-3000, -2160, -1080, -1080), (0, -2160, 3840, 0),
        (-6000, 0, -4080, 1080), (-3840, 0, -1920, 1080), (0, 0, 2560, 1440),
        (2800, 200, 4720, 1280), (5000, 1600, 6080, 3520),
    };
    bool valid = ValidMapLayout("eight-monitor sprawl", source, 960, 540, 14, out string detail);
    Check("map layout: 8-monitor sprawl stays bounded, ordered, centered, and proportional", valid, detail);
}
{
    // Reporter geometry includes negative coordinates, a gap/offset, and an abutting dummy at m4.R == m5.L.
    var source = new List<(int L, int T, int R, int B)>
    {
        (-1920, 71, 0, 1151), (-488, -1440, 2952, 0), (0, 0, 2560, 1440),
        (2560, 58, 4480, 1138), (4480, 845, 5280, 1445),
    };
    var mapped = GuardCore.MapLayout(source, 960, 520, 14);
    var again = GuardCore.MapLayout(source, 960, 520, 14);
    var reversed = GuardCore.MapLayout(source.AsEnumerable().Reverse().ToList(), 960, 520, 14);
    bool valid = ValidMapLayout("reporter abutting dummy", source, 960, 520, 14, out string detail);
    bool sameByGeometry = mapped.SequenceEqual(again) && mapped.SequenceEqual(reversed.AsEnumerable().Reverse());
    bool abuts = mapped[3].R == mapped[4].L
              && Math.Min(mapped[3].B, mapped[4].B) > Math.Max(mapped[3].T, mapped[4].T);
    Check("map layout: reporter abutting dummy keeps its seam and is deterministic under input permutation",
        valid && abuts && sameByGeometry, $"{detail} abuts={abuts} sameByGeometry={sameByGeometry}");
}

// MouseFence numbers are Left,Top ordered. StableId/Device break exact-origin ties, so native enumeration order does
// not change labels or map hit order in clone/overlap layouts.
{
    MonitorInfo M(string stable, string device, int l, int t, int r, int b) => new()
    {
        StableId = stable, Device = device,
        Bounds = new Native.RECT { Left = l, Top = t, Right = r, Bottom = b },
    };
    var seed = new List<MonitorInfo>
    {
        M("stable-z", "DISPLAY-Z", 0, 0, 1920, 1080),
        M("stable-right", "DISPLAY-R", 2560, 58, 4480, 1138),
        M("stable-left", "DISPLAY-L", -1920, 71, 0, 1151),
        M("stable-a", "DISPLAY-A", 0, 0, 2560, 1440),
        M("stable-top", "DISPLAY-T", -488, -1440, 2952, 0),
    };
    List<(string Stable, int Index)> Run(IEnumerable<MonitorInfo> input)
    {
        var copy = input.Select(m => M(m.StableId, m.Device, m.Bounds.Left, m.Bounds.Top,
                                       m.Bounds.Right, m.Bounds.Bottom)).ToList();
        MonitorInfo.SortAndNumber(copy);
        return copy.Select(m => (m.StableId, m.Index)).ToList();
    }
    var forward = Run(seed);
    var reverse = Run(seed.AsEnumerable().Reverse());
    var expected = new[] { "stable-left", "stable-top", "stable-a", "stable-z", "stable-right" };
    Check("monitor identity: numbering is deterministic Left,Top order with stable exact-origin ties",
        forward.Select(x => x.Stable).SequenceEqual(expected)
        && forward.SequenceEqual(reverse)
        && forward.Select(x => x.Index).SequenceEqual(Enumerable.Range(1, expected.Length)),
        $"forward=[{string.Join(",", forward)}] reverse=[{string.Join(",", reverse)}]");
}

{
    var monitor = new MonitorInfo { Index = 1, Primary = true, StableId = "stable-main" };
    string named = IdentifyOverlay.DisplayText(monitor, "BenQ\r\nDesk");
    string fallback = IdentifyOverlay.DisplayText(monitor, null);
    Check("monitor naming: identify text is one line with number+name and number-only fallback",
        named == "1  ★ — BenQ  Desk" && fallback == "1  ★"
        && !named.Contains('\r') && !named.Contains('\n'), $"named={named} fallback={fallback}");
}

// 20) Deterministic schema-v4 migration and JSON round-trip. Legacy allowance is explicitly OverallFeel, never
// confused with an absent rule (which means inherit the orientation default).
{
    var migrated = JsonSerializer.Deserialize<Settings>("{\"SettingsVersion\":3,\"CrossingPathMode\":\"Default\",\"UpLinks\":[]}");
    migrated.Normalize();
    Check("schema v4 migration: v3 Default becomes Automatic with no explicit rules",
        migrated.SettingsVersion == 4 && migrated.UpwardBoundaryDefault == BoundaryDefault.Automatic
        && migrated.BoundaryRules.Count == 0 && migrated.EdgeHoldbackPx == 0,
        $"version={migrated.SettingsVersion} default={migrated.UpwardBoundaryDefault} rules={migrated.BoundaryRules.Count}");
}
{
    const string legacy = "{\"SettingsVersion\":3,\"CrossingPathMode\":\"Custom\",\"UpLinks\":[{\"FromDevice\":\"bottom\",\"ToDevice\":\"top\"}]}";
    var migrated = JsonSerializer.Deserialize<Settings>(legacy);
    migrated.Normalize();
    var rule = migrated.BoundaryRules.Single();
    Check("schema v4 migration: v3 Custom becomes Blocked default + explicit OverallFeel allowance",
        migrated.UpwardBoundaryDefault == BoundaryDefault.Blocked && rule.FromStableId == "bottom"
        && rule.ToStableId == "top" && rule.Direction == BoundaryDirection.Up
        && rule.Behavior == BoundaryBehavior.OverallFeel && rule.Control == BoundaryControl.Automatic,
        $"default={migrated.UpwardBoundaryDefault} behavior={rule.Behavior} control={rule.Control}");
}
{
    var migrated = JsonSerializer.Deserialize<Settings>(
        "{\"SettingsVersion\":3,\"Mode\":\"Manual\",\"ManualMonitors\":[],\"CrossingPathMode\":\"Custom\",\"UpLinks\":[]}");
    migrated.Normalize();
    Check("schema v4 migration: empty v3 Manual falls back to AutoTop while empty Custom remains explicit none",
        migrated.Mode == "AutoTop" && migrated.UpwardBoundaryDefault == BoundaryDefault.Blocked
        && migrated.BoundaryRules.Count == 0,
        $"mode={migrated.Mode} default={migrated.UpwardBoundaryDefault} rules={migrated.BoundaryRules.Count}");
}
{
    var exported = new Settings { UpwardBoundaryDefault = BoundaryDefault.Blocked };
    exported.BoundaryRules.Add(new BoundaryRule { FromStableId = "bottom", ToStableId = "top",
        Direction = BoundaryDirection.Up, Behavior = BoundaryBehavior.OverallFeel, Control = BoundaryControl.Automatic });
    string json = JsonSerializer.Serialize(exported);
    var imported = JsonSerializer.Deserialize<Settings>(json);
    imported.Normalize();
    Check("schema v4 JSON: rules round-trip and legacy CrossingPathMode/UpLinks stop emitting",
        imported.BoundaryRules.Count == 1 && imported.BoundaryRules[0].Behavior == BoundaryBehavior.OverallFeel
        && !json.Contains("CrossingPathMode") && !json.Contains("UpLinks"), json);
}

// StableId-keyed nicknames use the same JSON format as settings export/import. Dormant names are intentionally kept.
{
    var exported = new Settings();
    exported.MonitorNames["edid-benq"] = "BenQ";
    exported.MonitorNames["edid-absent-dummy"] = "Dummy";
    string exportJson = JsonSerializer.Serialize(exported);
    var imported = JsonSerializer.Deserialize<Settings>(exportJson);
    imported.Normalize();
    string reExportJson = JsonSerializer.Serialize(imported);
    var roundTrip = JsonSerializer.Deserialize<Settings>(reExportJson);
    roundTrip.Normalize();
    Check("settings JSON: MonitorNames persist through export/import including an absent StableId",
        roundTrip.MonitorNames.Count == 2
        && roundTrip.MonitorNames["edid-benq"] == "BenQ"
        && roundTrip.MonitorNames["edid-absent-dummy"] == "Dummy",
        reExportJson);
}
{
    var absent = JsonSerializer.Deserialize<Settings>("{}");
    var explicitNull = JsonSerializer.Deserialize<Settings>("{\"MonitorNames\":null}");
    absent.Normalize();
    explicitNull.Normalize();
    Check("settings JSON: absent/null MonitorNames are safe empty dictionaries",
        absent.MonitorNames != null && absent.MonitorNames.Count == 0
        && explicitNull.MonitorNames != null && explicitNull.MonitorNames.Count == 0);
}
{
    var settings = new Settings();
    settings.MonitorNames["stable-unplugged"] = "Old Dummy";
    settings.Normalize();
    string liveName = Settings.MonitorNameFor(settings.MonitorNames, "stable-live");
    string dormantName = Settings.MonitorNameFor(settings.MonitorNames, "stable-unplugged");
    Check("monitor naming: absent StableId stays inert and never retargets a live monitor",
        liveName == "" && dormantName == "Old Dummy" && settings.MonitorNames.Count == 1,
        $"live={liveName} dormant={dormantName}");
}
{
    var settings = new Settings();
    settings.MonitorNames["physical-samsung"] = "Samsung 24";
    settings.Normalize();
    var beforeLayout = new List<MonitorInfo>
    {
        new MonitorInfo { StableId = "other", Device = @"\\.\DISPLAY1", Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
        new MonitorInfo { StableId = "physical-samsung", Device = @"\\.\DISPLAY4", Bounds = new Native.RECT { Left = 100, Top = 0, Right = 200, Bottom = 100 } },
    };
    var afterLayout = new List<MonitorInfo>
    {
        new MonitorInfo { StableId = "physical-samsung", Device = @"\\.\DISPLAY1", Bounds = new Native.RECT { Left = -100, Top = 0, Right = 0, Bottom = 100 } },
        new MonitorInfo { StableId = "other", Device = @"\\.\DISPLAY4", Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
    };
    MonitorInfo.SortAndNumber(beforeLayout);
    MonitorInfo.SortAndNumber(afterLayout);
    var before = beforeLayout.Single(m => m.StableId == "physical-samsung");
    var after = afterLayout.Single(m => m.StableId == "physical-samsung");
    Check("monitor naming: nickname follows StableId across DISPLAYn renumbering and rearrangement",
        Settings.MonitorNameFor(settings.MonitorNames, before.StableId) == "Samsung 24"
        && Settings.MonitorNameFor(settings.MonitorNames, after.StableId) == "Samsung 24"
        && before.Index == 2 && after.Index == 1,
        $"before={before.Device}/{before.Index} after={after.Device}/{after.Index}");
}
{
    List<MonitorInfo> Layout() => new()
    {
        new MonitorInfo { StableId = "right", Device = @"\\.\DISPLAY9", Bounds = new Native.RECT { Left = 100, Top = 0, Right = 200, Bottom = 100 } },
        new MonitorInfo { StableId = "left", Device = @"\\.\DISPLAY2", Bounds = new Native.RECT { Left = -100, Top = 0, Right = 0, Bottom = 100 } },
        new MonitorInfo { StableId = "main", Device = @"\\.\DISPLAY5", Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
    };
    var unnamed = Layout();
    var named = Layout();
    var names = new Dictionary<string, string> { ["right"] = "Samsung", ["left"] = "BenQ" };
    MonitorInfo.SortAndNumber(unnamed);
    MonitorInfo.SortAndNumber(named);
    var unnamedIdentity = unnamed.Select(m => (m.StableId, m.Index)).ToList();
    var namedIdentity = named.Select(m => (m.StableId, m.Index)).ToList();
    bool namesResolveOnlyByStableId = named.All(m =>
        Settings.MonitorNameFor(names, m.StableId) == (m.StableId == "right" ? "Samsung" : m.StableId == "left" ? "BenQ" : ""));
    Check("monitor naming: display-only names do not affect identity or numbering determinism",
        unnamedIdentity.SequenceEqual(namedIdentity) && namesResolveOnlyByStableId,
        $"unnamed=[{string.Join(",", unnamedIdentity)}] named=[{string.Join(",", namedIdentity)}]");
}

// Every persisted setting has an editor, and every lean BoundaryRule field is covered.
{
    var persisted = typeof(Settings).GetProperties(System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.Public)
        .Where(p => p.Name != nameof(Settings.SettingsVersion)
                 && p.GetGetMethod() != null && p.GetSetMethod() != null
                 && p.GetCustomAttributes(typeof(System.Text.Json.Serialization.JsonExtensionDataAttribute), true).Length == 0
                 && p.GetCustomAttributes(typeof(System.Text.Json.Serialization.JsonIgnoreAttribute), true).Length == 0)
        .Select(p => p.Name)
        .ToHashSet();
    var editable = SettingsForm.GuiEditableSettingProperties.ToHashSet();
    var missing = persisted.Except(editable).OrderBy(x => x).ToList();
    var stale = editable.Except(persisted).OrderBy(x => x).ToList();
    var ruleFields = typeof(BoundaryRule).GetProperties().Select(p => p.Name).OrderBy(x => x).ToList();
    var expectedRuleFields = new[] { nameof(BoundaryRule.Behavior), nameof(BoundaryRule.Control),
        nameof(BoundaryRule.Direction), nameof(BoundaryRule.FromStableId), nameof(BoundaryRule.ToStableId) }.OrderBy(x => x).ToList();
    Check("settings GUI: every persisted field is edited; lean BoundaryRule reflection coverage",
        missing.Count == 0 && stale.Count == 0
        && ruleFields.SequenceEqual(expectedRuleFields) && editable.Contains(nameof(Settings.MonitorNames)),
        $"missing=[{string.Join(",", missing)}] stale=[{string.Join(",", stale)}] rules=[{string.Join(",", ruleFields)}]");
}

// ---- SIDE CONTAINMENT (soft barrier, horizontal mirror of the up-barrier) ----
// side=true => containment ON (a slow drift / steep diagonal off MAIN is blocked, but a deliberate horizontal push
// still crosses). side=false => containment OFF (the cursor leaves MAIN sideways FREELY, no restriction).
{
    // NeedsProcessing flips on with only HasSides set (the hook must install for side-only layouts).
    var c = new GuardCore { HasSides = true, MainL = 0, MainR = 2560, MainT = 0, MainB = 1440 };
    Check("side: NeedsProcessing is true with only HasSides set", c.NeedsProcessing, "NeedsProcessing");
}
{
    // ON + deliberate horizontal push to the RIGHT: the crossing is honoured (soft barrier lets intent through).
    var c = NewSides(); Move(c, 2000, 500, true, true);
    var r = Move(c, 2700, 500, true, true);
    Check("side on: deliberate push right crosses", r.act == GuardAction.Pass && r.bx == 2700, $"act={r.act} bx={r.bx}");
}
{
    // ON + deliberate horizontal push to the LEFT: crosses.
    var c = NewSides(); Move(c, 500, 500, true, true);
    var r = Move(c, -200, 500, true, true);
    Check("side on: deliberate push left crosses", r.act == GuardAction.Pass && r.bx == -200, $"act={r.act} bx={r.bx}");
}
{
    // ON + slow RIGHT drift (dx < CrossMinUp): accidental -> blocked, clamped to MainR-1, Y kept.
    var c = NewSides(); Move(c, 2559, 500, true, true);
    var r = Move(c, 2561, 500, true, true);                // dx=2 -> drift, not a deliberate push
    Check("side on: slow right drift blocked, clamped to 2559, Y kept",
        r.act == GuardAction.Block && r.bx == 2559 && r.by == 500 && !r.onTop, $"act={r.act} bx={r.bx} by={r.by}");
}
{
    // ON + slow LEFT drift: blocked, clamped to MainL=0.
    var c = NewSides(); Move(c, 1, 500, true, true);
    var r = Move(c, -1, 500, true, true);                  // dx=2 -> drift
    Check("side on: slow left drift blocked, clamped to 0, Y kept",
        r.act == GuardAction.Block && r.bx == 0 && r.by == 500, $"act={r.act} bx={r.bx} by={r.by}");
}
{
    // ON but a STEEP diagonal (mostly vertical) past the edge is NOT a deliberate side push -> blocked.
    var c = NewSides(); Move(c, 2555, 500, true, true);
    var r = Move(c, 2565, 600, true, true);                // dx=10, dy=100 -> not deliberate sideways
    Check("side on: steep diagonal (dx10,dy100) is not a side push -> blocked", r.act == GuardAction.Block && r.bx == 2559, $"act={r.act} bx={r.bx}");
}
{
    // ON (SYMMETRIC): a SLOW drift back from a side screen INTO main is now contained too — clamped JUST OUTSIDE main
    // on the origin (left) side, Y kept, so the cursor stays on the side screen it came from instead of slipping onto main.
    var c = NewSides(); Move(c, -1, 500, true, true);      // origin on the LEFT side screen, hugging main's left edge
    var r = Move(c, 1, 500, true, true);                   // dx=2 -> drift across main's left edge into MAIN
    Check("side on: slow side->main drift blocked, clamped to -1, Y kept",
        r.act == GuardAction.Block && r.bx == -1 && r.by == 500, $"act={r.act} bx={r.bx} by={r.by}");
}
{
    // ON (SYMMETRIC, mirror RIGHT): a slow drift from the right side screen back into main clamps to MainR=2560.
    var c = NewSides(); Move(c, 2561, 500, true, true);    // origin on the RIGHT side screen, hugging main's right edge
    var r = Move(c, 2559, 500, true, true);                // dx=2 -> drift left across main's right edge into MAIN
    Check("side on: slow side->main drift (right) blocked, clamped to 2560, Y kept",
        r.act == GuardAction.Block && r.bx == 2560 && r.by == 500, $"act={r.act} bx={r.bx} by={r.by}");
}
{
    // ON: a DELIBERATE push back into main from a side screen still crosses — the barrier breaks on real intent
    // (the user's "let it cross when I really want to" requirement), so the cursor is never trapped.
    var c = NewSides(); Move(c, -100, 500, true, true);    // origin on the LEFT side screen
    var r = Move(c, 100, 500, true, true);                 // dx=200 -> deliberate push into MAIN
    Check("side on: deliberate side->main push crosses", r.act == GuardAction.Pass && r.bx == 100, $"act={r.act} bx={r.bx}");
}
{
    // ON + DeliberateCross OFF: a side->main drift crosses freely too (no deliberate requirement, symmetric with OUT).
    var c = NewSides(); c.DeliberateCross = false;
    Move(c, -1, 500, true, true);
    var r = Move(c, 1, 500, true, true);
    Check("side on + deliberate OFF: side->main drift crosses", r.act == GuardAction.Pass, $"act={r.act}");
}
{
    // SENSITIVITY: raising SideCrossMin stiffens the side barrier — a push that crosses at the default (dx=4) is now
    // treated as drift and BLOCKED when the threshold is 8.
    var c = NewSides(); c.SideCrossMin = 8;
    Move(c, 2558, 500, true, true);
    var r = Move(c, 2562, 500, true, true);                // dx=4 < 8 -> not deliberate enough -> blocked, clamped to 2559
    Check("side sensitivity: dx=4 blocked when SideCrossMin=8",
        r.act == GuardAction.Block && r.bx == 2559, $"act={r.act} bx={r.bx}");
}
{
    // SENSITIVITY: the SAME dx=4 move CROSSES at the default threshold (3) — proving the knob actually changes the feel.
    var c = NewSides();                                    // SideCrossMin defaults to 3
    Move(c, 2558, 500, true, true);
    var r = Move(c, 2562, 500, true, true);                // dx=4 >= 3 -> deliberate -> crosses
    Check("side sensitivity: dx=4 crosses at default SideCrossMin=3",
        r.act == GuardAction.Pass && r.bx == 2562, $"act={r.act} bx={r.bx}");
}
{
    // ON: moving further out while ALREADY on a side screen is free (only main->side is contained).
    var c = NewSides(); Move(c, -100, 500, true, true);
    var r = Move(c, -300, 500, true, true);
    Check("side on: roaming within a side screen is free", r.act == GuardAction.Pass, $"act={r.act}");
}
{
    // ON: a slow up-left corner DRIFT is contained on BOTH axes — clamp X into main AND clamp Y to the up-barrier,
    // so the cursor can't escape at the corner.
    var c = NewSides(); Move(c, 1, 1, true, true);
    var r = Move(c, -1, -1, true, true);                   // dx=2,dy=2 -> drift crossing left & up
    Check("side on: up-left corner drift contained on both axes (bx=0, by=0)",
        r.act == GuardAction.Block && r.bx == 0 && r.by == 0, $"act={r.act} bx={r.bx} by={r.by}");
}
{
    // ON must NOT interfere with a legitimate up-crossing through the top gate (straight up the centre).
    var c = NewSides(); Move(c, 1280, 0, true, true);
    var r = Move(c, 1280, -15, true, true);
    Check("side on: a centre up-gate crossing still works (no interference)", r.act == GuardAction.Pass && r.onTop, $"act={r.act} onTop={r.onTop}");
}
{
    // ON + DeliberateCross OFF: any horizontal exit crosses (no deliberate requirement).
    var c = NewSides(); c.DeliberateCross = false;
    Move(c, 2000, 500, true, true);
    var r = Move(c, 2700, 510, true, true);
    Check("side on + deliberate OFF: any sideways move crosses", r.act == GuardAction.Pass, $"act={r.act}");
}
{
    // ON: a purely vertical move inside main (no horizontal exit) is untouched by side containment.
    var c = NewSides(); Move(c, 1280, 500, true, true);
    var r = Move(c, 1280, 800, true, true);
    Check("side on: a purely vertical move inside main passes", r.act == GuardAction.Pass, $"act={r.act}");
}
{
    // COUNCIL FIX (cx): a side screen TALLER than main. A steep diagonal drift whose Y lands BELOW main's bottom
    // would, with a naive `by=y`, leave the cursor in the void (no monitor) at the clamped X. The void-check
    // must pull Y back onto the main band so the cursor lands on a real pixel ON main.
    var c = new GuardCore { HasTop = false, BarrierY = 0, HasSides = true,
        MainL = 0, MainR = 1000, MainT = 0, MainB = 800,
        Monitors = new List<(int L, int T, int R, int B)> { (0, 0, 1000, 800), (1000, 0, 2000, 1000) } };
    c.Reset();
    Move(c, 990, 790, true, true);                  // origin on main, near its bottom-right corner
    var r = Move(c, 1010, 900, true, true);         // dx=20,dy=110 steep -> blocked; Y=900 (> main bottom 800) is void
    Check("side on: taller side screen -> blocked drift stays on main (void corrected)",
        r.act == GuardAction.Block && r.bx == 999 && r.by == 799, $"act={r.act} bx={r.bx} by={r.by}");
}
{
    // COUNCIL CRITIC REGRESSION GUARD: a screen stacked BELOW main. A steep down-right diagonal drift must keep its Y
    // (a legal descent into the below screen) — the void-check must NOT yank Y up to main's bottom, because
    // the clamped-X point IS on the below screen.
    var c = new GuardCore { HasTop = false, BarrierY = 0, HasSides = true,
        MainL = 0, MainR = 2560, MainT = 0, MainB = 1440,
        Monitors = new List<(int L, int T, int R, int B)> { (0, 0, 2560, 1440), (0, 1440, 2560, 2880) } };
    c.Reset();
    Move(c, 2500, 1400, true, true);                // origin on main, near bottom-right
    var r = Move(c, 2565, 1700, true, true);        // dx=65,dy=300 steep -> blocked; X exits, Y descends into below
    Check("side on: descent into a below-main screen keeps Y (no over-clamp regression)",
        r.act == GuardAction.Block && r.bx == 2559 && r.by == 1700, $"act={r.act} bx={r.bx} by={r.by}");
}
{
    // COUNCIL GAP (Opus #4): an up-cross at the INSIDE edge of the gate (x stays within main) is NOT swallowed
    // by the side block even with containment ON — proving side containment doesn't eat a legitimate up-crossing.
    var c = NewSides(); Move(c, 30, 0, true, true);    // x=30 is inside the gate [24,2536] AND inside main
    var r = Move(c, 30, -15, true, true);
    Check("side on: a near-edge up-gate crossing (x stays in main) still crosses", r.act == GuardAction.Pass && r.onTop, $"act={r.act} onTop={r.onTop}");
}
{
    // COUNCIL GAP (cx): game-mode Confine takes precedence over side containment (its branch returns first).
    var c = new GuardCore { HasTop = false, Confine = true, HasSides = true,
        MainL = 0, MainR = 2560, MainT = 0, MainB = 1440,
        Monitors = new List<(int L, int T, int R, int B)> { (0, 0, 2560, 1440), (2560, 0, 4480, 1080) } };
    c.Reset();
    Move(c, 1000, 500, true, true);                 // baseline on main
    var r = Move(c, 3000, 500, true, true);         // leave main -> Confine blocks (clamps to main's right edge)
    Check("confine precedence: game mode handles the block before side containment", r.act == GuardAction.Block, $"act={r.act}");
}
{
    // OFF: with containment disabled the cursor leaves MAIN sideways FREELY — even a slow drift that WOULD be
    // blocked when ON now passes with no clamp.
    var c = NewSides(); Move(c, 2559, 500, true, false);
    var r = Move(c, 2561, 500, true, false);               // dx=2 drift -> free because containment is OFF
    Check("side off: sideways drift passes freely (no containment)",
        r.act == GuardAction.Pass && r.bx == 2561, $"act={r.act} bx={r.bx}");
}

// ---- INTEL ROTATION HOTKEY CLASH (Ctrl+Alt+Arrow): pure detection used to warn / pick a safe default ----
// VK codes: Left 0x25, Up 0x26, Right 0x27, Down 0x28; a non-arrow like 'G' = 0x47.
{
    bool up    = GuardCore.IsArrowRotationHotkey(0x26, ctrl: true, alt: true, shift: false, win: false);
    bool down  = GuardCore.IsArrowRotationHotkey(0x28, true, true, false, false);
    bool left  = GuardCore.IsArrowRotationHotkey(0x25, true, true, false, false);
    bool right = GuardCore.IsArrowRotationHotkey(0x27, true, true, false, false);
    Check("intel: Ctrl+Alt+{Up,Down,Left,Right} all detected as rotation clashes", up && down && left && right,
        $"up={up} down={down} left={left} right={right}");
}
{
    // Adding Shift (the safe default) or Win clears the clash — that's exactly how the fix dodges rotation.
    Check("intel: Ctrl+Alt+Shift+Right is NOT a clash (the safe remap)",
        !GuardCore.IsArrowRotationHotkey(0x27, true, true, shift: true, win: false));
    Check("intel: Ctrl+Alt+Win+Right is NOT a clash",
        !GuardCore.IsArrowRotationHotkey(0x27, true, true, false, win: true));
}
{
    // A non-arrow key, or missing Ctrl/Alt, is never an Intel rotation clash.
    Check("intel: a non-arrow key (G) is not a clash", !GuardCore.IsArrowRotationHotkey(0x47, true, true, false, false));
    Check("intel: missing Ctrl is not a clash", !GuardCore.IsArrowRotationHotkey(0x26, false, true, false, false));
    Check("intel: missing Alt is not a clash", !GuardCore.IsArrowRotationHotkey(0x26, true, false, false, false));
}

// ---- M1 SCHEMA-v4 BOUNDARY GRAPH + COMPILED ENGINE ----
List<BoundaryScreen> GraphScreens(params (string Id, int L, int T, int R, int B, bool Primary)[] items) =>
    items.Select(x => new BoundaryScreen { StableId = x.Id, Bounds = (x.L, x.T, x.R, x.B), Primary = x.Primary }).ToList();

GuardCore GraphCore(Settings settings, List<BoundaryScreen> screens, bool hasTop = false, int barrierY = 0)
{
    var compiled = BoundaryGraph.Compile(settings, screens);
    var core = new GuardCore { HasTop = hasTop, BarrierY = barrierY, Transitions = compiled.Transitions,
        Monitors = screens.Where(s => !s.Excluded).Select(s => s.Bounds).ToList() };
    core.Reset();
    return core;
}

{
    var s = new Settings { SideBoundaryDefault = BoundaryDefault.Free, PassageInsetPx = 0 };
    var screens = GraphScreens(("bottom", 0, 200, 300, 300, true), ("top", 0, 0, 300, 100, false),
                               ("occluder", 100, 100, 200, 200, false));
    var a = BoundaryGraph.Compile(s, screens).Transitions
        .Where(t => t.FromStableId == "bottom" && t.ToStableId == "top" && t.Direction == BoundaryDirection.Up).ToList();
    var b = BoundaryGraph.Compile(s, screens.AsEnumerable().Reverse().ToList()).Transitions
        .Where(t => t.FromStableId == "bottom" && t.ToStableId == "top" && t.Direction == BoundaryDirection.Up).ToList();
    Check("M1 compiler: third-screen occlusion splits a corridor and sort is input-order independent",
        a.Count == 2 && a[0].CorridorStart == 0 && a[0].CorridorEnd == 100
        && a[1].CorridorStart == 200 && a[1].CorridorEnd == 300
        && a.Select(t => (t.CorridorStart, t.CorridorEnd)).SequenceEqual(b.Select(t => (t.CorridorStart, t.CorridorEnd))));
}
{
    var s = new Settings { SideBoundaryDefault = BoundaryDefault.Free, PassageInsetPx = 0 };
    s.BoundaryRules.Add(new BoundaryRule { FromStableId = "a", ToStableId = "b", Direction = BoundaryDirection.Right,
        Behavior = BoundaryBehavior.Blocked, Control = BoundaryControl.Always });
    s.BoundaryRules.Add(new BoundaryRule { FromStableId = "b", ToStableId = "a", Direction = BoundaryDirection.Left,
        Behavior = BoundaryBehavior.Blocked, Control = BoundaryControl.Always });
    var screens = GraphScreens(("a", 0, 0, 100, 100, true), ("b", 100, 0, 200, 100, false));
    var issues = BoundaryGraph.ValidateNoSeal(s, screens);
    var healed = BoundaryGraph.Compile(s, screens);
    Check("M1 validator: an authored zero-exit configuration is rejected deterministically",
        issues.Length == 2 && issues[0].FocusCorridorWidth == 100, $"issues={issues.Length}");
    Check("M1 compiler: geometry-seal backstop demotes widest exits to Push and flags them",
        healed.HasSafetyDemotions && healed.Transitions.Count(t => t.SafetyDemoted) == 2
        && healed.Transitions.Where(t => t.SafetyDemoted).All(t => t.Behavior == BoundaryBehavior.Push));
}
{
    var s = new Settings { SideBoundaryDefault = BoundaryDefault.Free, PassageInsetPx = 0 };
    s.BoundaryRules.Add(new BoundaryRule { FromStableId = "b", ToStableId = "c", Direction = BoundaryDirection.Right,
        Behavior = BoundaryBehavior.Blocked, Control = BoundaryControl.Always });
    var screens = GraphScreens(("a", 0, 0, 100, 100, true), ("b", 100, 0, 200, 100, false),
                               ("c", 200, 0, 300, 100, false));
    var c = GraphCore(s, screens); Move(c, 90, 50, true, false);
    var r = Move(c, 250, 50, true, false); // OS/ease-style fast move crosses two seams in one event
    Check("M1 engine: authorized landing stops a fast multi-seam teleport before a blocked second transition",
        r.act == GuardAction.Block && r.bx == 199 && r.by == 50, $"act={r.act} bx={r.bx} by={r.by}");

    s.BoundaryRules.Clear();
    c = GraphCore(s, screens); Move(c, 90, 50, true, false); r = Move(c, 250, 50, true, false);
    Check("M1 engine: fast multi-seam move reaches a transitively authorized free monitor",
        r.act == GuardAction.Pass && r.bx == 250, $"act={r.act} bx={r.bx}");
}
{
    var s = new Settings { SideBoundaryDefault = BoundaryDefault.Free, PassageInsetPx = 0 };
    var screens = GraphScreens(("a", 0, 0, 100, 100, true), ("b", 100, 0, 200, 100, false));
    var c = GraphCore(s, screens); Move(c, 90, 50, true, false);
    var r = Move(c, 250, -40, true, false); // crosses the seam then attempts a desktop void
    Check("M1 engine: compiled Free crossing into void is corrected only onto an authorized real screen",
        r.act == GuardAction.Block && screens.Any(m => InRect(m.Bounds, r.bx, r.by)),
        $"act={r.act} bx={r.bx} by={r.by}");
}
{
    var s = new Settings { SideBoundaryDefault = BoundaryDefault.Free, PassageInsetPx = 0, EdgeHoldbackPx = 5,
        SafetyEscapes = false };
    s.BoundaryRules.Add(new BoundaryRule { FromStableId = "a", ToStableId = "dummy", Direction = BoundaryDirection.Right,
        Behavior = BoundaryBehavior.Blocked, Control = BoundaryControl.Always });
    var screens = GraphScreens(("a", 0, 0, 100, 100, true), ("dummy", 100, 0, 200, 100, false));
    var c = GraphCore(s, screens); Move(c, 90, 22, true, true);
    var r = Move(c, 180, 78, true, true); // eased target changes height substantially
    Check("M1 engine: failed/Blocked Push holds back on source and never lands on the blocked monitor",
        r.act == GuardAction.Block && r.bx == 94 && r.by == 78 && !InRect(screens[1].Bounds, r.bx, r.by),
        $"bx={r.bx} by={r.by}");
}
{
    var s = new Settings { SideBoundaryDefault = BoundaryDefault.Free, PassageInsetPx = 0 };
    var screens = GraphScreens(("a", 0, 0, 100, 100, true), ("dummy", 100, 0, 200, 100, false));
    screens[1].Excluded = true;
    var compiled = BoundaryGraph.Compile(s, screens);
    Check("M1 compiler: excluded screens are absent from transitions and every allowed-landing set",
        compiled.Transitions.Length == 0 && compiled.Transitions.All(t => t.AllowedLandings.All(r => r != screens[1].Bounds)));
}
{
    var s = new Settings { DownwardBoundaryDefault = BoundaryDefault.Free, PassageInsetPx = 0, SafetyEscapes = false };
    s.BoundaryRules.Add(new BoundaryRule { FromStableId = "top", ToStableId = "bottom", Direction = BoundaryDirection.Down,
        Behavior = BoundaryBehavior.Blocked, Control = BoundaryControl.Always });
    var screens = GraphScreens(("top", 0, -100, 100, 0, false), ("bottom", 0, 0, 100, 100, true));
    var c = GraphCore(s, screens, hasTop: true, barrierY: 0);
    Move(c, 50, -10, true); var r = Move(c, 50, 20, true);
    Check("M1 engine: Down-Blocked runs before OnTop legacy descent and state follows the source landing",
        r.act == GuardAction.Block && r.by == -1 && r.onTop, $"act={r.act} by={r.by} onTop={r.onTop}");
}
{
    var s = new Settings { DownwardBoundaryDefault = BoundaryDefault.Free, PassageInsetPx = 0 };
    var screens = GraphScreens(("top", 0, -100, 200, 0, false), ("landing", 0, 0, 100, 100, true),
                               ("wrong", 100, 0, 200, 100, false));
    var c = GraphCore(s, screens, true, 0);
    c.DescentRouting = true;
    c.DescentRoutes.Add((screens[0].Bounds, screens[1].Bounds));
    Move(c, 150, -10, true); var r = Move(c, 150, 10, true);
    Check("M1 engine: compiled Down-Free applies global descent guidance after transition resolution",
        r.act == GuardAction.Block && r.bx == 99 && r.by == 10 && !r.onTop,
        $"act={r.act} bx={r.bx} by={r.by} onTop={r.onTop}");
}
{
    // The target is already beyond Top.R, but the movement segment crosses Top.B at x=150, inside the top. Routing
    // must use that intersection, not the endpoint X, or a fast diagonal tick lands on the wrong lower screen.
    var s = new Settings { DownwardBoundaryDefault = BoundaryDefault.Free, PassageInsetPx = 0 };
    var screens = GraphScreens(("top", 0, -100, 200, 0, false), ("landing", 0, 0, 100, 100, true),
                               ("wrong", 100, 0, 300, 100, false));
    var c = GraphCore(s, screens, hasTop: true);
    c.DescentRouting = true;
    c.DescentRoutes.Add((screens[0].Bounds, screens[1].Bounds));
    Move(c, 50, -50, true);
    var r = Move(c, 250, 50, true);
    Check("M1 descent routing: fast diagonal uses the local-edge intersection after endpoint leaves the top X-span",
        r.act == GuardAction.Block && r.bx == 99 && r.by == 50 && !r.onTop,
        $"act={r.act} bx={r.bx} by={r.by} onTop={r.onTop}");
}
{
    var automatic = new Settings { UpwardBoundaryDefault = BoundaryDefault.Blocked, PassageInsetPx = 0 };
    automatic.BoundaryRules.Add(new BoundaryRule { FromStableId = "bottom", ToStableId = "top", Direction = BoundaryDirection.Up,
        Behavior = BoundaryBehavior.Free, Control = BoundaryControl.Automatic });
    // A side neighbour makes bottom non-isolated, so this remains a pure Automatic-vs-Always toggle test.
    var screens = GraphScreens(("top", 0, -100, 100, 0, false), ("bottom", 0, 0, 100, 100, true),
                               ("side", 100, 0, 200, 100, false));
    var c = GraphCore(automatic, screens, true, 0); Move(c, 50, 10, false); var blocked = Move(c, 50, -10, false);
    automatic.BoundaryRules[0].Control = BoundaryControl.Always;
    c = GraphCore(automatic, screens, true, 0); Move(c, 50, 10, false); var allowed = Move(c, 50, -10, false);
    Check("M1 engine: Automatic follows the live Up switch while Always ignores it",
        blocked.act == GuardAction.Block && allowed.act == GuardAction.Pass,
        $"automatic={blocked.act} always={allowed.act}");
}
{
    // The authorized-landing resolver must use the same live Automatic state as transition matching. A persisted
    // Blocked side default is free while side protection is OFF; a static compile-time landing set must not undo it.
    var s = new Settings { SideBoundaryDefault = BoundaryDefault.Blocked, PassageInsetPx = 0,
        SafetyEscapes = false };
    var screens = GraphScreens(("a", 0, 0, 100, 100, true), ("b", 100, 0, 200, 100, false));
    var c = GraphCore(s, screens); Move(c, 50, 50, true, side: false);
    var protectionOff = Move(c, 150, 50, true, side: false);
    c = GraphCore(s, screens); Move(c, 50, 50, true, side: true);
    var protectionOn = Move(c, 150, 50, true, side: true);
    Check("M1 engine: Automatic Blocked side rule becomes end-to-end Free only while side protection is off",
        protectionOff.act == GuardAction.Pass && protectionOff.bx == 150
        && protectionOn.act == GuardAction.Block && InRect(screens[0].Bounds, protectionOn.bx, protectionOn.by),
        $"off=({protectionOff.act},{protectionOff.bx},{protectionOff.by}) on=({protectionOn.act},{protectionOn.bx},{protectionOn.by})");
}
{
    // A fast diagonal enters the right screen and the top-right screen at their shared corner in one event. The first
    // Right edge is Free, but its transitive landing set must not walk through B's Automatic Up edge while Up is closed.
    var s = new Settings { SideBoundaryDefault = BoundaryDefault.Free,
        UpwardBoundaryDefault = BoundaryDefault.Automatic, PassageInsetPx = 0, SafetyEscapes = false };
    var screens = GraphScreens(("a", 0, 0, 100, 100, true), ("b", 100, 0, 200, 100, false),
                               ("top", 100, -100, 200, 0, false));
    var c = GraphCore(s, screens, hasTop: true); Move(c, 50, 50, gate: false, side: false);
    var closed = Move(c, 150, -50, gate: false, side: false);
    c = GraphCore(s, screens, hasTop: true); Move(c, 50, 50, gate: true, side: false);
    var open = Move(c, 150, -50, gate: true, side: false);
    Check("M1 engine: transitive authorized landing honors the live Up toggle on downstream edges",
        closed.act == GuardAction.Block && InRect(screens[1].Bounds, closed.bx, closed.by)
        && open.act == GuardAction.Pass && InRect(screens[2].Bounds, open.bx, open.by),
        $"closed=({closed.act},{closed.bx},{closed.by}) open=({open.act},{open.bx},{open.by})");
}
{
    var dormant = new Settings { SettingsVersion = Settings.CurrentSettingsVersion, SafetyEscapes = false };
    dormant.BoundaryRules.Add(new BoundaryRule { FromStableId = "dock-old-a", ToStableId = "dock-old-b",
        Direction = BoundaryDirection.Right, Behavior = BoundaryBehavior.Blocked, Control = BoundaryControl.Always });
    dormant.Normalize();
    var liveOther = GraphScreens(("live-a", 0, 0, 100, 100, true), ("live-b", 100, 0, 200, 100, false));
    bool inert = BoundaryGraph.Compile(dormant, liveOther).Transitions.All(t => !t.ExplicitRule);
    string json = JsonSerializer.Serialize(dormant); var restored = JsonSerializer.Deserialize<Settings>(json); restored.Normalize();
    var oldReturns = GraphScreens(("dock-old-a", 0, 0, 100, 100, true), ("dock-old-b", 100, 0, 200, 100, false));
    bool restoredRule = BoundaryGraph.Compile(restored, oldReturns).Transitions.Any(t => t.ExplicitRule && t.Behavior == BoundaryBehavior.Blocked);
    Check("M1 identity dormancy: absent IDs stay inert, persist, and reactivate only on exact-identity return",
        inert && restoredRule && restored.BoundaryRules.Count == 1,
        $"inert={inert} restoredRule={restoredRule} rules={restored.BoundaryRules.Count} json={json}");
}
{
    var s = new Settings { SideBoundaryDefault = BoundaryDefault.Free, PassageInsetPx = 0,
        SeamTolerancePx = 2, SafetyEscapes = false };
    s.BoundaryRules.Add(new BoundaryRule { FromStableId = "a", ToStableId = "b", Direction = BoundaryDirection.Right,
        Behavior = BoundaryBehavior.Blocked, Control = BoundaryControl.Always });
    var screens = GraphScreens(("a", 0, 0, 100, 100, true), ("b", 200, 0, 300, 100, false));
    var c = GraphCore(s, screens); Move(c, 90, 50, true, false);
    var intoGap = Move(c, 150, 50, true, false);
    c = GraphCore(s, screens); Move(c, 90, 50, true, false);
    var established = Move(c, 220, 50, true, false);
    Check("M1 gap semantics: void attempt stays source-authorized; OS landing on To activates the saved gap rule",
        intoGap.act == GuardAction.Block && intoGap.bx == 99
        && established.act == GuardAction.Block && established.bx == 99,
        $"gap=({intoGap.act},{intoGap.bx}) established=({established.act},{established.bx})");
}
{
    // A->diagonal has zero orthogonal overlap. The segment reaches the shared corner of Up and Right at the same t;
    // half-open corridors deliberately fire neither ordered edge, independent of compiler input order.
    var s = new Settings { SideBoundaryDefault = BoundaryDefault.Blocked, UpwardBoundaryDefault = BoundaryDefault.Blocked,
        PassageInsetPx = 0, SafetyEscapes = false };
    var screens = GraphScreens(("a", 0, 0, 100, 100, true), ("up", 0, -100, 100, 0, false),
        ("right", 100, 0, 200, 100, false), ("diagonal", 100, -100, 200, 0, false));
    var c1 = GraphCore(s, screens); Move(c1, 50, 50, true, true); var r1 = Move(c1, 150, -50, true, true);
    var c2 = GraphCore(s, screens.AsEnumerable().Reverse().ToList()); Move(c2, 50, 50, true, true); var r2 = Move(c2, 150, -50, true, true);
    bool noDiagonalEdge = BoundaryGraph.Compile(s, screens).Transitions.All(t => t.ToStableId != "diagonal" || t.FromStableId != "a");
    Check("M1 simultaneous-intersection tie: true corner fires no edge; authorized repair is deterministic",
        noDiagonalEdge && r1.act == GuardAction.Block && r2.act == GuardAction.Block
        && r1.bx == r2.bx && r1.by == r2.by && InRect(screens[0].Bounds, r1.bx, r1.by),
        $"first=({r1.act},{r1.bx},{r1.by}) second=({r2.act},{r2.bx},{r2.by})");
}
{
    var s = new Settings { Mode = "Manual", UpwardBoundaryDefault = BoundaryDefault.Automatic, PassageInsetPx = 0 };
    var screens = GraphScreens(("top", 0, -100, 100, 0, false), ("main", 0, 0, 100, 100, true));
    var up = BoundaryGraph.Compile(s, screens).Transitions.Single(t => t.FromStableId == "main" && t.Direction == BoundaryDirection.Up);
    Check("M1 schema: a newly empty v4 Manual upper group is literal none (Automatic Up resolves Free)",
        up.Behavior == BoundaryBehavior.Free, $"behavior={up.Behavior}");
}
{
    // Legacy anti-trap invariant: the lower screen has no down/left/right neighbour, so its only real exit is Up.
    // Closing the normal Up gate at launch must not strand the cursor when SafetyEscapes is enabled.
    var s = new Settings { PassageInsetPx = 0, SafetyEscapes = true };
    var screens = GraphScreens(("top", 0, -100, 100, 0, false), ("isolated-lower", 0, 0, 100, 100, true));
    var compiled = BoundaryGraph.Compile(s, screens);
    var safetyExit = compiled.Transitions.Single(t => t.FromStableId == "isolated-lower"
        && t.ToStableId == "top" && t.Direction == BoundaryDirection.Up);
    var c = GraphCore(s, screens, hasTop: true); Move(c, 50, 50, gate: false);
    var escaped = Move(c, 50, -10, gate: false);
    Check("M1 anti-trap: sole-up exit remains a deliberate Push while the normal Up gate is closed",
        safetyExit.SoleUpSafetyEscape && escaped.act == GuardAction.Pass && escaped.by == -10
        && InRect(screens[0].Bounds, escaped.bx, escaped.by),
        $"safety={safetyExit.SoleUpSafetyEscape} act={escaped.act} bx={escaped.bx} by={escaped.by}");

    s.SafetyEscapes = false;
    c = GraphCore(s, screens, hasTop: true); Move(c, 50, 50, gate: false);
    var sealedByExpert = Move(c, 50, -10, gate: false);
    BoundaryValidationIssue[] validation = null;
    Exception validationError = null;
    try { validation = BoundaryGraph.ValidateNoSeal(s, screens); }
    catch (Exception ex) { validationError = ex; }
    Check("M1 anti-trap: SafetyEscapes off permits the expert runtime seal and validator remains total",
        sealedByExpert.act == GuardAction.Block && sealedByExpert.by == 0 && validationError == null,
        $"act={sealedByExpert.act} bx={sealedByExpert.bx} by={sealedByExpert.by} issues={validation?.Length} error={validationError?.GetType().Name}");
}
{
    // Geometry safety is broader than the legacy sole-up escape. This lower screen has a real side neighbour, so
    // AntiTrap does not classify it as isolated; nevertheless every authored exit is Blocked and Compile must keep
    // the widest demoted exit usable even while the ordinary Up toggle is closed.
    var s = new Settings { UpwardBoundaryDefault = BoundaryDefault.Blocked,
        SideBoundaryDefault = BoundaryDefault.Blocked, PassageInsetPx = 0, SafetyEscapes = true };
    var screens = GraphScreens(("top", 0, -100, 200, 0, false), ("lower", 0, 0, 200, 100, true),
                               ("side", 200, 0, 250, 100, false));
    var compiled = BoundaryGraph.Compile(s, screens);
    var safetyExit = compiled.Transitions.Single(t => t.FromStableId == "lower"
        && t.ToStableId == "top" && t.Direction == BoundaryDirection.Up);
    var c = GraphCore(s, screens, hasTop: true); Move(c, 100, 50, gate: false);
    var escaped = Move(c, 100, -20, gate: false);
    Check("M1 anti-trap: geometry-demoted Up exit remains Push while the ordinary Up toggle is closed",
        safetyExit.SafetyDemoted && !safetyExit.SoleUpSafetyEscape
        && escaped.act == GuardAction.Pass && InRect(screens[0].Bounds, escaped.bx, escaped.by),
        $"demoted={safetyExit.SafetyDemoted} sole={safetyExit.SoleUpSafetyEscape} act={escaped.act} bx={escaped.bx} by={escaped.by}");
}
{
    // Persisted Free makes the graph look unsealed, but Automatic Up becomes Blocked when the live toggle closes.
    // The side neighbour prevents legacy sole-up classification and its own Always rule is Blocked, so geometry safety
    // must also analyze the restrictive live-toggle state and preserve one exit.
    var s = new Settings { UpwardBoundaryDefault = BoundaryDefault.Blocked,
        SideBoundaryDefault = BoundaryDefault.Blocked, PassageInsetPx = 0, SafetyEscapes = true };
    s.BoundaryRules.Add(new BoundaryRule { FromStableId = "lower", ToStableId = "top",
        Direction = BoundaryDirection.Up, Behavior = BoundaryBehavior.Free, Control = BoundaryControl.Automatic });
    s.BoundaryRules.Add(new BoundaryRule { FromStableId = "lower", ToStableId = "side",
        Direction = BoundaryDirection.Right, Behavior = BoundaryBehavior.Blocked, Control = BoundaryControl.Always });
    var screens = GraphScreens(("top", 0, -100, 200, 0, false), ("lower", 0, 0, 200, 100, true),
                               ("side", 200, 0, 250, 100, false));
    var compiled = BoundaryGraph.Compile(s, screens);
    var safetyExit = compiled.Transitions.Single(t => t.FromStableId == "lower"
        && t.ToStableId == "top" && t.Direction == BoundaryDirection.Up);
    var c = GraphCore(s, screens, hasTop: true); Move(c, 100, 50, gate: false);
    var escaped = Move(c, 100, -20, gate: false);
    Check("M1 anti-trap: live Up closure cannot seal an otherwise-Free Automatic exit",
        safetyExit.SafetyDemoted && !safetyExit.SoleUpSafetyEscape
        && escaped.act == GuardAction.Pass && InRect(screens[0].Bounds, escaped.bx, escaped.by),
        $"demoted={safetyExit.SafetyDemoted} sole={safetyExit.SoleUpSafetyEscape} act={escaped.act} bx={escaped.bx} by={escaped.by}");
}
{
    const string fieldlessLegacy =
        "{\"CrossingPathMode\":\"Custom\",\"UpLinks\":[{\"FromDevice\":\"bottom\",\"ToDevice\":\"top\"}]}";
    var migrated = JsonSerializer.Deserialize<Settings>(fieldlessLegacy);
    migrated.Normalize();
    var rule = migrated.BoundaryRules.SingleOrDefault();
    Check("schema v4 migration: field-absent legacy JSON migrates as v0 and preserves custom UpLinks",
        migrated.SettingsVersion == Settings.CurrentSettingsVersion
        && migrated.UpwardBoundaryDefault == BoundaryDefault.Blocked
        && rule?.FromStableId == "bottom" && rule.ToStableId == "top"
        && rule.Direction == BoundaryDirection.Up && rule.Behavior == BoundaryBehavior.OverallFeel,
        $"version={migrated.SettingsVersion} default={migrated.UpwardBoundaryDefault} rules={migrated.BoundaryRules.Count}");
}
{
    const string importedJson =
        "{\"SettingsVersion\":4,\"SafetyEscapes\":false,\"ExcludedDevices\":[\"primary\"],"
        + "\"BoundaryRules\":["
        + "{\"FromStableId\":\"primary\",\"ToStableId\":\"side\",\"Direction\":\"Right\",\"Behavior\":\"Blocked\",\"Control\":\"Always\"},"
        + "{\"FromStableId\":\"side\",\"ToStableId\":\"primary\",\"Direction\":\"Left\",\"Behavior\":\"Blocked\",\"Control\":\"Always\"}]}";
    var imported = JsonSerializer.Deserialize<Settings>(importedJson);
    var monitors = new List<MonitorInfo>
    {
        new() { StableId = "primary", Primary = true, Bounds = new Native.RECT { Left = 0, Top = 0, Right = 100, Bottom = 100 } },
        new() { StableId = "side", Bounds = new Native.RECT { Left = 100, Top = 0, Right = 200, Bottom = 100 } },
    };
    var runtimeExcluded = new HashSet<string>(GuardCore.SanitizeExcluded(
        monitors.Select(m => m.StableId).ToList(), "primary",
        new HashSet<string>(imported.ExcludedDevices, StringComparer.OrdinalIgnoreCase), out _),
        StringComparer.OrdinalIgnoreCase);
    var runtimeIssues = BoundaryGraph.ValidateNoSeal(imported, BoundaryGraph.FromMonitors(monitors, runtimeExcluded));
    var validatorIssues = imported.ValidateNoSeal(monitors);
    Check("M1 validator: imported primary exclusion is sanitized identically to runtime topology",
        runtimeExcluded.Count == 0 && validatorIssues.Select(x => x.StableId).SequenceEqual(runtimeIssues.Select(x => x.StableId))
        && validatorIssues.Length == 2,
        $"excluded={runtimeExcluded.Count} validator=[{string.Join(",", validatorIssues.Select(x => x.StableId))}] runtime=[{string.Join(",", runtimeIssues.Select(x => x.StableId))}]");
}
{
    const string unknown = "{\"SettingsVersion\":4,\"UpwardBoundaryDefault\":\"Surprise\",\"BoundaryRules\":[{\"FromStableId\":\"a\",\"ToStableId\":\"b\",\"Direction\":\"Up\",\"Behavior\":\"Mystery\",\"Control\":\"Automatic\"}]}";
    var draft = JsonSerializer.Deserialize<Settings>(unknown); draft.Normalize();
    Check("M1 import: unknown enum values remain a repairable draft and never silently collapse",
        draft.HasRepairableImportErrors && draft.UpwardBoundaryDefault == BoundaryDefault.Unknown
        && draft.BoundaryRules.Single().Behavior == BoundaryBehavior.Unknown);
}
{
    var projected = GuardCore.MapLayout(new List<(int L, int T, int R, int B)>
        { (-32768, 0, -32128, 480), (30000, 0, 32767, 2160) }, 400, 160, 10);
    Check("A-FIX-1: extreme-span 640x480 dummy retains a minimum 12x12 logical footprint",
        projected[0].R - projected[0].L >= 12 && projected[0].B - projected[0].T >= 12,
        $"dummy={projected[0]}");
}
{
    var input = new List<(string Key, int Left, int Top, string RawIdentityKey)>
        { ("clone", 0, 0, "port-z"), ("clone", 0, 0, "port-a") };
    var first = GuardCore.DisambiguateKeys(input);
    var reversed = GuardCore.DisambiguateKeys(input.AsEnumerable().Reverse().ToList());
    var byRawFirst = input.Select((x, i) => (x.RawIdentityKey, Key: first[i])).ToDictionary(x => x.RawIdentityKey, x => x.Key);
    var reversedInput = input.AsEnumerable().Reverse().ToList();
    var byRawSecond = reversedInput.Select((x, i) => (x.RawIdentityKey, Key: reversed[i])).ToDictionary(x => x.RawIdentityKey, x => x.Key);
    Check("A-FIX-2: exact-clone suffix tie breaks by ordinal raw identity, not enumeration order",
        byRawFirst["port-a"] == "clone#0" && byRawFirst["port-z"] == "clone#1"
        && byRawFirst.OrderBy(x => x.Key).SequenceEqual(byRawSecond.OrderBy(x => x.Key)));
}

// ---- public docs: deterministic synthetic layout + faithful off-screen capture ----
foreach (var preview in new[] { (Tab: 0, Mode: "list"), (Tab: -1, Mode: "drawer"),
                               (Tab: 1, Mode: "advanced"), (Tab: 2, Mode: "shortcuts"),
                               (Tab: 3, Mode: "general"), (Tab: -2, Mode: "advanced") })
{
    using var form = new SettingsForm(Screenshot.DemoSettings("Dark", "en"), Screenshot.DemoMonitors());
    ShowOffscreen(form);
    if (preview.Tab == -2) form.OpenAdvancedForPreview();
    else if (preview.Tab == -1) form.OpenFirstBoundaryForPreview(hideScreenList: true);
    else if (preview.Tab == 1) form.OpenAdvancedOverviewForPreview();
    else form.SelectTab(preview.Tab);
    Application.DoEvents(); form.PerformLayout();
    try
    {
        var metrics = form.DisplayLayoutMetrics();
        bool displayPage = preview.Tab is 0 or -1;
        Check($"docs metrics: preview {preview.Tab} reports the visible page",
            metrics.Mode == preview.Mode && metrics.BodyHeight > 0
            && (displayPage ? metrics.MapHeight > 0 : metrics.MapHeight == 0 && metrics.BandHeight == 0),
            $"metrics={metrics}");
    }
    catch (InvalidOperationException ex)
    {
        Check($"docs metrics: preview {preview.Tab} reports the visible page", false, ex.Message);
    }
}

{
    var demo = Screenshot.DemoMonitors();
    var demoSettings = Screenshot.DemoSettings("Dark", "en");
    Check("docs demo: fixed three-screen geometry and neutral identities never depend on live monitor enumeration",
        demo.Count == 3
        && demo.Select(m => m.StableId).SequenceEqual(new[] { "demo-main", "demo-top", "demo-laptop" })
        && demo.Select(m => m.Index).SequenceEqual(new[] { 1, 2, 3 })
        && demo[0].Primary && demo[0].Bounds.Width == 2560 && demo[0].Bounds.Height == 1440
        && demo[1].Bounds.Bottom == demo[0].Bounds.Top
        && demo[2].Bounds.Left == demo[0].Bounds.Right
        && demoSettings.MonitorNames["demo-main"] == "Main"
        && demoSettings.MonitorNames["demo-top"] == "Top"
        && demoSettings.MonitorNames["demo-laptop"] == "Laptop"
        && demoSettings.ExcludedDevices.SequenceEqual(new[] { "demo-laptop" }));

    string path = Path.Combine(Path.GetTempPath(), $"MouseFence-demo-capture-{Guid.NewGuid():N}.png");
    try
    {
        Screenshot.CaptureSettings(path, "Dark", -1, "en", 1f, useDemoLayout: true);
        using var capture = new Bitmap(path);
        int bright = 0, sampled = 0;
        for (int y = Math.Max(0, capture.Height - 58); y < capture.Height - 10; y += 2)
        for (int x = 18; x < capture.Width - 18; x += 2)
        {
            var pixel = capture.GetPixel(x, y);
            if (pixel.R >= 245 && pixel.G >= 245 && pixel.B >= 245) bright++;
            sampled++;
        }
        int brandInk = 0;
        for (int y = 12; y < Math.Min(56, capture.Height); y++)
        for (int x = 40; x < Math.Min(160, capture.Width); x++)
        {
            var pixel = capture.GetPixel(x, y);
            if (pixel.R >= 120 && pixel.G >= 120 && pixel.B >= 120) brandInk++;
        }
        Check("docs capture: synthetic dark client is aligned and owner-drawn controls stay themed",
            bright < sampled / 100 && brandInk >= 40,
            $"bright={bright}/{sampled} brandInk={brandInk} path={path}");
    }
    finally
    {
        try { File.Delete(path); } catch { }
    }
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

// ===== BUGHUNT (owner's REAL captured geometry) — the VOID BELOW the right monitor + the EXCLUDED dummy =====
// Physical px: left(-1920,71,0,1151) top(-488,-1440,2952,0) main*(0,0,2560,1440) right(2560,58,4480,1138).
// right is 58px down-offset AND 1080 tall next to main's 1440 -> a ~1920x302 VOID sits directly below it
// (X 2560..4480, Y 1138..1440) = the historical "cursor disappears bottom-right" region. dummy(4480,845,
// 5280,1445) is EXCLUDED (Forbidden). The existing "reporter" tests cover the top-right corner; these cover
// the bottom void + the dummy, which they don't. Every decided landing must be a usable, non-forbidden pixel.
{
    var usable = new List<(int L, int T, int R, int B)>
    {
        (-1920, 71, 0, 1151), (-488, -1440, 2952, 0), (0, 0, 2560, 1440), (2560, 58, 4480, 1138),
    };
    GuardCore Rig()
    {
        var c = new GuardCore
        {
            HasTop = true, BarrierY = 0, Gates = new List<(int Min, int Max)> { (24, 2536) },
            HasSides = true, MainL = 0, MainR = 2560, MainT = 0, MainB = 1440,
            Monitors = usable, Forbidden = new List<(int L, int T, int R, int B)> { (4480, 845, 5280, 1445) },
        };
        c.Reset();
        return c;
    }
    { var c = Rig(); SetLast(c, 3500, 1130); var r = Move(c, 3500, 1300, true, true);
      Check("bughunt real-geo: drift down off right's bottom into the void lands on a usable monitor (never disappears)",
            IsLegalWallResult(c, r.bx, r.by), $"act={r.act} point=({r.bx},{r.by})"); }
    { var c = Rig(); SetLast(c, 3500, 1130); var r = Move(c, 3600, 1439, true, true);
      Check("bughunt real-geo: deep eased flick to the void's bottom lands legal",
            IsLegalWallResult(c, r.bx, r.by), $"act={r.act} point=({r.bx},{r.by})"); }
    { var c = Rig(); SetLast(c, 2000, 700); var r = Move(c, 3500, 1400, true, true);
      Check("bughunt real-geo: ease-teleport main -> void below right never lands in the void",
            IsLegalWallResult(c, r.bx, r.by), $"act={r.act} point=({r.bx},{r.by})"); }
    { var c = Rig(); SetLast(c, 4400, 1000); var r = Move(c, 4900, 1100, true, true);
      Check("bughunt real-geo: move into the EXCLUDED dummy is kept out (usable, not dummy)",
            IsLegalWallResult(c, r.bx, r.by), $"act={r.act} point=({r.bx},{r.by})"); }
    { var c = Rig(); SetLast(c, 4400, 1100); var r = Move(c, 4900, 1440, true, true);
      Check("bughunt real-geo: teleport deep into the dummy's bottom is kept out, lands legal",
            IsLegalWallResult(c, r.bx, r.by), $"act={r.act} point=({r.bx},{r.by})"); }
    { var c = Rig(); SetLast(c, 2559, 1200); var r = Move(c, 2700, 1300, true, true);
      Check("bughunt real-geo: the main/right/void corner move lands legal",
            IsLegalWallResult(c, r.bx, r.by), $"act={r.act} point=({r.bx},{r.by})"); }
    { var c = Rig(); SetLast(c, 3500, 60); var r = Move(c, 3500, 20, true, true);
      Check("bughunt real-geo: thin void above right (Y 0..58) lands legal",
            IsLegalWallResult(c, r.bx, r.by), $"act={r.act} point=({r.bx},{r.by})"); }
}

// ---------------------------------------------------------------------------------------------------
// HOOK LIVENESS — Windows silently unhooks a WH_MOUSE_LL hook whose callback overruns LowLevelHooksTimeout
// and leaves our handle intact, so "handle != 0" is not evidence the fence is up. HookHealth turns the only
// real evidence (hook traffic vs. cursor movement between two probes) into a verdict. Dead => reinstall, so a
// false Dead would tear down a healthy hook: an idle probe must stay Inconclusive.
// ---------------------------------------------------------------------------------------------------
Console.WriteLine("\n-- hook liveness (HookHealth) --");
{
    Check("hook health: traffic since the last probe => Alive",
          HookHealth.Evaluate(cursorMoved: true, callbacksSincePreviousProbe: 12) == HookHealthVerdict.Alive);

    Check("hook health: traffic without cursor movement (clicks/wheel only) is still Alive",
          HookHealth.Evaluate(cursorMoved: false, callbacksSincePreviousProbe: 3) == HookHealthVerdict.Alive);

    Check("hook health: the cursor moved but NOT ONE callback arrived => Dead (silently unhooked)",
          HookHealth.Evaluate(cursorMoved: true, callbacksSincePreviousProbe: 0) == HookHealthVerdict.Dead);

    Check("hook health: nothing moved and nothing arrived => Inconclusive, never a reinstall",
          HookHealth.Evaluate(cursorMoved: false, callbacksSincePreviousProbe: 0) == HookHealthVerdict.Inconclusive);

    // A probe counter is a running total, so the caller passes a delta; a zero delta on a busy hook (the
    // counter simply didn't advance in this window) must read exactly like an idle window, not a failure.
    long total = 4321;
    long previous = 4321;
    Check("hook health: a zero delta on a non-zero counter is not evidence of death",
          HookHealth.Evaluate(cursorMoved: false, callbacksSincePreviousProbe: total - previous) == HookHealthVerdict.Inconclusive);
}

foreach (var language in new[] { "en", "tr" })
{
    // UI refresh 2026-10-22 (M2): on a live off-screen window every shortcut field and Clear button keeps one
    // field height, sits vertically centred inside its own row, and the footnote below starts after the last row.
    using var form = new SettingsForm(new Settings { Language = language, Theme = "Dark" }, Screenshot.DemoMonitors())
    {
        StartPosition = FormStartPosition.Manual,
        Location = new Point(SystemInformation.VirtualScreen.Right + 2000, SystemInformation.VirtualScreen.Bottom + 2000),
        Opacity = 0,
    };
    form.Show(); Application.DoEvents();
    form.ClientSize = new Size(1064 * form.DeviceDpi / 96, 820 * form.DeviceDpi / 96);
    form.SelectTab(2); Application.DoEvents(); form.PerformLayout(); Application.DoEvents();
    var all = DescendantsAndSelf(form).ToList();
    var rows = all.OfType<TableLayoutPanel>().Where(row => (row.Tag as string) == "listRow"
        && row.Controls.Cast<Control>().Any(c => (c.Tag as string) == "shortcutFrame")).ToList();
    int field = 36 * form.DeviceDpi / 96;
    var details = new List<string>();
    bool shaped = rows.Count == 4;
    foreach (var row in rows)
    {
        var frame = row.Controls.Cast<Control>().First(c => (c.Tag as string) == "shortcutFrame");
        var clear = row.Controls.Cast<Control>().First(c => c is ThemedButton);
        int frameCentre = frame.Top + frame.Height / 2, rowCentre = row.ClientSize.Height / 2;
        bool ok = Math.Abs(frame.Height - field) <= 2 && Math.Abs(clear.Height - field) <= 2
            && frame.Top >= 0 && frame.Bottom <= row.ClientSize.Height && clear.Bottom <= row.ClientSize.Height
            && Math.Abs(frameCentre - rowCentre) <= 3;
        shaped &= ok;
        details.Add($"row={row.ClientSize.Height} frame={frame.Top}+{frame.Height} clear={clear.Top}+{clear.Height}");
    }
    var lastRow = rows.LastOrDefault();
    var footnote = all.OfType<Label>().FirstOrDefault(label => label.Text == Strings.Ui("ShortcutsHealthy"));
    bool footClear = lastRow != null && footnote != null
        && footnote.Parent.RectangleToScreen(footnote.Bounds).Top >= lastRow.RectangleToScreen(lastRow.ClientRectangle).Bottom;
    Check($"UI refresh M2 ({language}): shortcut rows keep one centred field height and the footnote clears the last row",
        shaped && footClear, $"rows={rows.Count} field={field} footClear={footClear} " + string.Join("; ", details));
}

foreach (var language in new[] { "en", "tr" })
{
    // UI refresh 2026-10-22 (M1): the tray menu follows the theme, uses the Settings names, shows state only as a
    // tick and the shortcut on the right; paused keeps the same enabled states as before the refresh.
    var settings = Screenshot.DemoSettings("Dark", language);
    Strings.Use(settings.Language);
    int clicked = 0;
    using var menu = new TrayMenu(() => clicked |= 1, () => clicked |= 2, () => clicked |= 4, () => clicked |= 8, () => clicked |= 16, () => clicked |= 32);
    menu.Apply(settings, new TrayMenuState(Enabled: true, GateOpen: false, SideOn: true, Confine: false));
    bool themed = menu.Menu.Renderer is ThemedMenuRenderer renderer && renderer.Theme == Theme.Dark;
    bool names = menu.Gate.Text == Strings.HotkeyHead && menu.Side.Text == Strings.SideHotkeyHead
        && menu.Game.Text == Strings.MenuGame && menu.Pause.Text == Strings.MenuPause
        && !menu.Gate.Text.Contains('—') && !menu.Side.Text.Contains('—');
    bool ticks = !menu.Gate.Checked && menu.Side.Checked && !menu.Game.Checked && !menu.Pause.Checked;
    bool shortcuts = menu.Gate.ShortcutKeyDisplayString == settings.HotKeyText()
        && menu.Pause.ShortcutKeyDisplayString == settings.PauseHotKeyText();
    settings.SideHotKey = Keys.None;
    menu.Apply(settings, new TrayMenuState(Enabled: false, GateOpen: false, SideOn: true, Confine: false));
    bool paused = !menu.Gate.Enabled && !menu.Side.Enabled && !menu.Game.Enabled && menu.Pause.Enabled && menu.Pause.Checked
        && menu.Gate.Text == Strings.GatePausedMenu && menu.Side.ShortcutKeyDisplayString == "";
    foreach (var item in new[] { menu.Gate, menu.Side, menu.Game, menu.Pause, menu.SettingsItem, menu.Exit }) item.PerformClick();
    bool actions = clicked == (8 | 16 | 32);   // disabled items do not fire, exactly as before
    menu.Apply(Screenshot.DemoSettings("Light", language), new TrayMenuState(true, true, false, true));
    bool relight = menu.Menu.Renderer is ThemedMenuRenderer light && light.Theme == Theme.Light && menu.Gate.Checked && menu.Game.Checked;
    Check($"UI refresh M1 ({language}): themed tray menu with Settings names, ticks, shortcuts and unchanged paused states",
        themed && names && ticks && shortcuts && paused && actions && relight,
        $"themed={themed} names={names} ticks={ticks} shortcuts={shortcuts} paused={paused} actions={clicked} relight={relight}");
}

foreach (var language in new[] { "en", "tr" })
{
    // UI refresh 2026-10-22 (M3): every radio button is themed like the checkboxes, and one choosable screen
    // reads in the singular.
    using var form = new SettingsForm(Screenshot.DemoSettings("Dark", language), Screenshot.DemoMonitors())
    {
        StartPosition = FormStartPosition.Manual,
        Location = new Point(SystemInformation.VirtualScreen.Right + 2000, SystemInformation.VirtualScreen.Bottom + 2000),
        Opacity = 0,
    };
    form.Show(); Application.DoEvents(); form.SelectTab(1); Application.DoEvents();
    var radios = DescendantsAndSelf(form).OfType<RadioButton>().ToList();
    bool themed = radios.Count >= 4 && radios.All(r => r is ThemedRadioButton t && t.CurrentTheme == Theme.Dark);
    bool singular = radios.Any(r => r.Text == Strings.Ui("UpperManualOne")) && !radios.Any(r => r.Text.Contains("1 screens"));
    Check($"UI refresh M3 ({language}): radio buttons are themed and one choosable screen reads in the singular",
        themed && singular, $"radios={radios.Count} types={string.Join(",", radios.Select(r => r.GetType().Name))} texts={string.Join(" | ", radios.Select(r => r.Text))}");
}

foreach (var theme in new[] { "Light", "Dark" })
{
    // UI refresh 2026-10-22 (M6): the documentation capture completes in Light as well as Dark; a hidden footer
    // button used to be sampled and failed every Light capture off-screen.
    string folder = Path.Combine(AppContext.BaseDirectory, "m6-captures");
    Directory.CreateDirectory(folder);
    var results = new List<string>();
    bool all = true;
    foreach (int view in new[] { -1, 2 })
    {
        string path = Path.Combine(folder, $"{theme}-{view}.png");
        if (File.Exists(path)) File.Delete(path);
        try
        {
            Screenshot.CaptureSettings(path, theme, view, "en", 1f, useDemoLayout: true);
            bool ok = File.Exists(path) && new FileInfo(path).Length > 10_000;
            all &= ok; results.Add($"{view}:{ok}");
        }
        catch (Exception ex) { all = false; results.Add($"{view}:{ex.Message}"); }
    }
    Check($"UI refresh M6 ({theme}): documentation captures complete off-screen", all, string.Join("; ", results));
}

{
    // UI refresh 2026-10-22 (M5): the sidebar brand shows the shield app icon beside a text-only wordmark.
    using var form = new SettingsForm(Screenshot.DemoSettings("Dark", "en"), Screenshot.DemoMonitors())
    {
        StartPosition = FormStartPosition.Manual,
        Location = new Point(SystemInformation.VirtualScreen.Right + 2000, SystemInformation.VirtualScreen.Bottom + 2000),
        Opacity = 0,
    };
    form.Show(); Application.DoEvents();
    var all = DescendantsAndSelf(form).ToList();
    var icon = all.OfType<PictureBox>().FirstOrDefault(p => (p.Tag as string) == "brandIcon");
    var word = all.OfType<Label>().FirstOrDefault(l => l.Text == "MouseFence");
    bool ok = icon?.Image != null && icon.Image.Width >= 20 && icon.BackColor == icon.Parent.BackColor
        && word != null && word.Left >= icon.Right && !all.OfType<Label>().Any(l => l.Text.Contains('▣'));
    Check("UI refresh M5: sidebar brand uses the shield app icon beside a text-only wordmark", ok,
        $"icon={(icon?.Image == null ? "none" : icon.Image.Width.ToString())} word={word?.Left} iconRight={icon?.Right}");
}

{
    // UI refresh 2026-10-22 (M4): a gate pill low in a screen leaves the name readable above it; a pill near the
    // top of a screen still hides the name, as before.
    var label = new Rectangle(408, 140, 192, 79);
    var lowPill = new Rectangle(465, 199, 78, 22);
    var highPill = new Rectangle(465, 145, 78, 22);
    var above = MonitorMapControl.NameAbovePill(label, lowPill);
    bool ok = above is Rectangle r && r.Top == label.Top && r.Bottom == lowPill.Top - 4 && r.Width == label.Width
        && MonitorMapControl.NameAbovePill(label, highPill) == null;
    Check("UI refresh M4: a screen name moves above a low gate pill instead of disappearing", ok, $"above={above}");
}

Console.WriteLine();
Console.WriteLine($"Passed: {passes}; Failed: {fails}");
Console.WriteLine(fails == 0 && passes > 0 ? "ALL TESTS PASSED" : $"{fails} TEST(S) FAILED");
Environment.Exit(fails == 0 && passes > 0 ? 0 : 1);
