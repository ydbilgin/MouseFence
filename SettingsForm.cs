using System.Drawing;
using System.Text.Json;
using Microsoft.Win32;

namespace MouseFence;

/// <summary>Settings dialog: General (hotkey, startup), Appearance (language, theme), Monitors (top monitors + per-screen crossing rules).</summary>
public sealed class SettingsForm : Form
{
    private readonly Settings _orig;
    private readonly List<MonitorInfo> _monitors = new();
    private readonly List<MonitorInfo> _tops = new();
    private readonly List<MonitorInfo> _bottoms = new();

    private readonly TextBox _hotkeyBox;
    private readonly CheckBox _startActive;
    private readonly CheckBox _autoStart;
    private readonly ComboBox _langCombo;
    private readonly ComboBox _themeCombo;
    private RadioButton _modeAuto;
    private RadioButton _modeManual;
    private GlyphCheckedListBox _monitorList;
    private RadioButton _crossingDefault;
    private RadioButton _crossingCustom;
    private ComboBox _fromCombo;
    private CheckedListBox _topsCheck;
    private MonitorMapControl _map;
    private GlyphCheckedListBox _excludeList;
    private Panel _displayBody;
    private Panel _displayBodyContent;
    private Panel _advancedPanel;
    private Panel _customPathsPanel;
    private Panel _crossingModesPanel;
    private Button _advancedToggle;
    private Label _behaviorHead;
    private Button _resetCrossing;
    private Label _layoutChangedBanner;
    private readonly System.Windows.Forms.Timer _displayRefreshTimer = new() { Interval = 650 };

    private readonly TabControl _tabs;
    private readonly TextBox _gameHotkeyBox;
    private readonly TextBox _pauseHotkeyBox;
    private readonly TextBox _sideHotkeyBox;
    private readonly CheckBox _startSideActive;
    private readonly NumericUpDown _sideSensNum;
    private CheckBox _deliberateCheck;
    private CheckBox _descentCheck;
    private readonly ToolTip _tips = new();
    private readonly HashSet<string> _excluded = new();
    private readonly HashSet<string> _manualTops = new();
    private readonly Dictionary<string, HashSet<string>> _rules = new();
    private bool _rebuildingMonitorUi;
    private bool _advancedExpanded;
    private string _selectedStableId;
    private bool _mc, _ma, _ms, _mw;
    private Keys _key;
    private bool _gc, _ga, _gs, _gw;
    private Keys _gkey;
    private bool _pc, _pa, _ps, _pw;
    private Keys _pkey;
    private bool _sc, _sa, _ss, _sw;
    private Keys _skey;

    public Settings Result { get; private set; }

    /// <summary>Dev helper for docs screenshots: select a tab by index.</summary>
    public void SelectTab(int i) { if (i >= 0 && i < _tabs.TabPages.Count) _tabs.SelectedIndex = i; }

    public SettingsForm(Settings s, List<MonitorInfo> monitors)
    {
        s.Normalize();
        _orig = s;
        _monitors.AddRange(monitors ?? new List<MonitorInfo>());
        _mc = s.ModCtrl; _ma = s.ModAlt; _ms = s.ModShift; _mw = s.ModWin; _key = s.HotKey;
        _gc = s.ConfineModCtrl; _ga = s.ConfineModAlt; _gs = s.ConfineModShift; _gw = s.ConfineModWin; _gkey = s.ConfineHotKey;
        _pc = s.PauseModCtrl; _pa = s.PauseModAlt; _ps = s.PauseModShift; _pw = s.PauseModWin; _pkey = s.PauseHotKey;
        _sc = s.SideModCtrl; _sa = s.SideModAlt; _ss = s.SideModShift; _sw = s.SideModWin; _skey = s.SideHotKey;

        foreach (var id in s.ManualMonitors) _manualTops.Add(id);
        _tops.AddRange(s.Mode == "Manual" && _manualTops.Any(id => _monitors.Any(m => m.StableId == id && MonitorInfo.IsAbovePrimary(m)))
            ? _monitors.Where(m => _manualTops.Contains(m.StableId) && MonitorInfo.IsAbovePrimary(m))
            : _monitors.Where(MonitorInfo.IsAbovePrimary));
        var topSet = new HashSet<string>(_tops.Select(t => t.StableId));
        _bottoms.AddRange(_monitors.Where(m => !topSet.Contains(m.StableId)));
        foreach (var lk in s.UpLinks)
        {
            if (!_rules.TryGetValue(lk.FromDevice, out var set)) { set = new HashSet<string>(); _rules[lk.FromDevice] = set; }
            set.Add(lk.ToDevice);
        }
        // The exclude list keys on the STABLE per-monitor id (FIX 4), so an exclusion survives \\.\DISPLAYn
        // renumbering. Never seed the primary as excluded (it can't be walled).
        var primaryStable = (_monitors.FirstOrDefault(m => m.Primary) ?? _monitors.FirstOrDefault())?.StableId;
        foreach (var id in s.ExcludedDevices)
            if (id != primaryStable) _excluded.Add(id);

        Text = Strings.SettingsTitle;
        FormBorderStyle = FormBorderStyle.Sizable;
        MaximizeBox = true;
        MinimizeBox = false;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.CenterScreen;
        Font = new Font("Segoe UI", 9f);
        ClientSize = new Size(520, 720);
        MinimumSize = new Size(480, 600);

        var headFont = new Font("Segoe UI Semibold", 9f);

        _tabs = new TabControl
        {
            Left = 12, Top = 12, Width = ClientSize.Width - 24, Height = ClientSize.Height - 66,
            DrawMode = TabDrawMode.OwnerDrawFixed, SizeMode = TabSizeMode.Fixed, ItemSize = new Size(147, 28),
            Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right,
        };
        _tabs.DrawItem += DrawTab;
        var tabGeneral = new TabPage(Strings.TabGeneral) { Padding = new Padding(10), AutoScroll = true };
        var tabAppearance = new TabPage(Strings.TabAppearance) { Padding = new Padding(10) };
        var tabMonitors = new TabPage(Strings.DisplaysTab) { Padding = new Padding(10) };
        _tabs.TabPages.AddRange(new[] { tabGeneral, tabAppearance, tabMonitors });

        // ---- General ----
        int y = 12;
        tabGeneral.Controls.Add(new Label { Text = Strings.HotkeyHead, AutoSize = true, Left = 12, Top = y, Font = headFont, Tag = "head" });
        y += 20;
        tabGeneral.Controls.Add(new Label { Text = Strings.HotkeyHint, AutoSize = true, Left = 12, Top = y, Tag = "subtle" });
        y += 22;
        _hotkeyBox = new TextBox
        {
            Left = 12, Top = y, Width = 300, Height = 26, ReadOnly = true, Cursor = Cursors.Hand,
            TextAlign = HorizontalAlignment.Center, Font = new Font("Segoe UI Semibold", 9.5f), BorderStyle = BorderStyle.FixedSingle
        };
        _hotkeyBox.KeyDown += HotkeyBox_KeyDown;
        var btnClear = new Button { Text = Strings.Clear, Left = 320, Top = y - 1, Width = 96, Height = 28, FlatStyle = FlatStyle.Flat };
        btnClear.Click += (a, b) => { _mc = _ma = _ms = _mw = false; _key = Keys.None; UpdateHotkeyText(); };
        tabGeneral.Controls.Add(_hotkeyBox);
        tabGeneral.Controls.Add(btnClear);
        y += 44;
        tabGeneral.Controls.Add(new Label { Text = Strings.StartupHead, AutoSize = true, Left = 12, Top = y, Font = headFont, Tag = "head" });
        y += 22;
        _startActive = new CheckBox { Text = Strings.StartGateClosed, Left = 12, Top = y, Width = 400, Height = 22, Checked = s.StartGateClosed };
        tabGeneral.Controls.Add(_startActive);
        y += 26;
        _autoStart = new CheckBox { Text = Strings.StartWithWindows, Left = 12, Top = y, Width = 400, Height = 22, Checked = s.AutoStart };
        tabGeneral.Controls.Add(_autoStart);
        y += 34;
        tabGeneral.Controls.Add(new Label { Text = Strings.GameHotkeyHead, AutoSize = true, Left = 12, Top = y, Font = headFont, Tag = "head" });
        y += 20;
        tabGeneral.Controls.Add(new Label { Text = Strings.GameHotkeyHint, Left = 12, Top = y, Width = 412, Height = 30, Tag = "subtle" });
        y += 32;
        _gameHotkeyBox = new TextBox
        {
            Left = 12, Top = y, Width = 300, Height = 26, ReadOnly = true, Cursor = Cursors.Hand,
            TextAlign = HorizontalAlignment.Center, Font = new Font("Segoe UI Semibold", 9.5f), BorderStyle = BorderStyle.FixedSingle
        };
        _gameHotkeyBox.KeyDown += GameHotkeyBox_KeyDown;
        var btnClearGame = new Button { Text = Strings.Clear, Left = 320, Top = y - 1, Width = 96, Height = 28, FlatStyle = FlatStyle.Flat };
        btnClearGame.Click += (a, b) => { _gc = _ga = _gs = _gw = false; _gkey = Keys.None; UpdateGameHotkeyText(); };
        tabGeneral.Controls.Add(_gameHotkeyBox);
        tabGeneral.Controls.Add(btnClearGame);
        y += 44;
        tabGeneral.Controls.Add(new Label { Text = Strings.PauseHotkeyHead, AutoSize = true, Left = 12, Top = y, Font = headFont, Tag = "head" });
        y += 20;
        tabGeneral.Controls.Add(new Label { Text = Strings.PauseHotkeyHint, Left = 12, Top = y, Width = 412, Height = 30, Tag = "subtle" });
        y += 32;
        _pauseHotkeyBox = new TextBox
        {
            Left = 12, Top = y, Width = 300, Height = 26, ReadOnly = true, Cursor = Cursors.Hand,
            TextAlign = HorizontalAlignment.Center, Font = new Font("Segoe UI Semibold", 9.5f), BorderStyle = BorderStyle.FixedSingle
        };
        _pauseHotkeyBox.KeyDown += PauseHotkeyBox_KeyDown;
        var btnClearPause = new Button { Text = Strings.Clear, Left = 320, Top = y - 1, Width = 96, Height = 28, FlatStyle = FlatStyle.Flat };
        btnClearPause.Click += (a, b) => { _pc = _pa = _ps = _pw = false; _pkey = Keys.None; UpdatePauseHotkeyText(); };
        tabGeneral.Controls.Add(_pauseHotkeyBox);
        tabGeneral.Controls.Add(btnClearPause);
        y += 44;
        tabGeneral.Controls.Add(new Label { Text = Strings.SideHotkeyHead, AutoSize = true, Left = 12, Top = y, Font = headFont, Tag = "head" });
        y += 20;
        tabGeneral.Controls.Add(new Label { Text = Strings.SideHotkeyHint, Left = 12, Top = y, Width = 412, Height = 30, Tag = "subtle" });
        y += 32;
        _sideHotkeyBox = new TextBox
        {
            Left = 12, Top = y, Width = 300, Height = 26, ReadOnly = true, Cursor = Cursors.Hand,
            TextAlign = HorizontalAlignment.Center, Font = new Font("Segoe UI Semibold", 9.5f), BorderStyle = BorderStyle.FixedSingle
        };
        _sideHotkeyBox.KeyDown += SideHotkeyBox_KeyDown;
        var btnClearSide = new Button { Text = Strings.Clear, Left = 320, Top = y - 1, Width = 96, Height = 28, FlatStyle = FlatStyle.Flat };
        btnClearSide.Click += (a, b) => { _sc = _sa = _ss = _sw = false; _skey = Keys.None; UpdateSideHotkeyText(); };
        tabGeneral.Controls.Add(_sideHotkeyBox);
        tabGeneral.Controls.Add(btnClearSide);
        y += 36;
        _startSideActive = new CheckBox { Text = Strings.StartSideContainOn, Left = 12, Top = y, Width = 412, Height = 22, Checked = s.StartSideContainOn };
        tabGeneral.Controls.Add(_startSideActive);
        y += 28;
        tabGeneral.Controls.Add(new Label { Text = Strings.SideSensitivityLabel, AutoSize = true, Left = 12, Top = y + 4, Tag = "subtle" });
        // The min horizontal px/move that counts as a deliberate side push — higher = a firmer barrier. Clamp the
        // seeded value into the control's range so an out-of-range imported setting can't throw on construction.
        _sideSensNum = new NumericUpDown { Left = 150, Top = y, Width = 70, Height = 26, Minimum = 1, Maximum = 200, Value = Math.Clamp(s.SideCrossMin, 1, 200) };
        tabGeneral.Controls.Add(_sideSensNum);
        // One-click reset back to the shipped default (3), so a user who over-tightens the barrier can always recover it.
        var btnDefaultSide = new Button { Text = Strings.DefaultLabel, Left = 228, Top = y - 1, Width = 110, Height = 28, FlatStyle = FlatStyle.Flat };
        btnDefaultSide.Click += (a, b) => _sideSensNum.Value = Settings.DefaultSideCrossMin;
        tabGeneral.Controls.Add(btnDefaultSide);
        y += 30;
        tabGeneral.Controls.Add(new Label { Text = Strings.SideSensitivityHint, Left = 12, Top = y, Width = 412, Height = 44, Tag = "subtle" });

        // ---- Appearance ----
        tabAppearance.Controls.Add(new Label { Text = Strings.LanguageLabel, AutoSize = true, Left = 12, Top = 14, Tag = "subtle" });
        tabAppearance.Controls.Add(new Label { Text = Strings.ThemeLabel, AutoSize = true, Left = 220, Top = 14, Tag = "subtle" });
        _langCombo = new ComboBox { Left = 12, Top = 34, Width = 190, Height = 24, DropDownStyle = ComboBoxStyle.DropDownList };
        _langCombo.Items.AddRange(new object[] { Strings.LangAuto, "English", "Türkçe" });
        _langCombo.SelectedIndex = s.Language switch { "en" => 1, "tr" => 2, _ => 0 };
        _themeCombo = new ComboBox { Left = 220, Top = 34, Width = 190, Height = 24, DropDownStyle = ComboBoxStyle.DropDownList };
        _themeCombo.Items.AddRange(new object[] { Strings.ThemeSystem, Strings.ThemeLight, Strings.ThemeDark });
        _themeCombo.SelectedIndex = s.Theme switch { "Light" => 1, "Dark" => 2, _ => 0 };
        _themeCombo.SelectedIndexChanged += (a, b) => ApplyThemeLive();
        tabAppearance.Controls.Add(_langCombo);
        tabAppearance.Controls.Add(_themeCombo);

        // ---- Displays: pinned map header + independently scrolling simple/advanced body ----
        BuildDisplaysTab(tabMonitors, headFont, s);

        // ---- buttons ----
        int buttonTop = ClientSize.Height - 42;
        var cancel = new Button { Text = Strings.Cancel, Left = ClientSize.Width - 12 - 96, Top = buttonTop, Width = 96, Height = 30, FlatStyle = FlatStyle.Flat, DialogResult = DialogResult.Cancel, Anchor = AnchorStyles.Bottom | AnchorStyles.Right };
        var ok = new Button { Text = Strings.Save, Left = cancel.Left - 104, Top = buttonTop, Width = 96, Height = 30, FlatStyle = FlatStyle.Flat, Tag = "primary", DialogResult = DialogResult.OK, Anchor = AnchorStyles.Bottom | AnchorStyles.Right };
        ok.Click += Ok_Click;

        // back up / restore the whole configuration as a single settings.json-format file (e.g. before a format/reinstall)
        var export = new Button { Text = Strings.ExportLabel, Left = 12, Top = buttonTop, Width = 110, Height = 30, FlatStyle = FlatStyle.Flat, Anchor = AnchorStyles.Bottom | AnchorStyles.Left };
        export.Click += Export_Click;
        var import = new Button { Text = Strings.ImportLabel, Left = export.Left + 116, Top = buttonTop, Width = 110, Height = 30, FlatStyle = FlatStyle.Flat, Anchor = AnchorStyles.Bottom | AnchorStyles.Left };
        import.Click += Import_Click;

        Controls.Add(_tabs);
        Controls.Add(export);
        Controls.Add(import);
        Controls.Add(ok);
        Controls.Add(cancel);
        AcceptButton = ok;
        CancelButton = cancel;

        Result = s;
        UpdateHotkeyText();
        UpdateGameHotkeyText();
        UpdatePauseHotkeyText();
        UpdateSideHotkeyText();
        _displayRefreshTimer.Tick += DisplayRefreshTimer_Tick;
        ApplyThemeLive();
        SystemEvents.DisplaySettingsChanged += DisplaySettingsChanged;
    }

    private void BuildDisplaysTab(TabPage tab, Font headFont, Settings s)
    {
        var header = new Panel { Dock = DockStyle.Fill, Margin = Padding.Empty };
        var identifyAll = new Button
        {
            Text = Strings.IdentifyAll, Top = 2, Width = 116, Height = 28,
            FlatStyle = FlatStyle.Flat, Anchor = AnchorStyles.Top | AnchorStyles.Right,
        };
        identifyAll.Click += (a, b) => IdentifyOverlay.Flash(_monitors, SelectedTheme(), this);
        _tips.SetToolTip(identifyAll, Strings.IdentifyHint);

        _layoutChangedBanner = new Label
        {
            Text = Strings.LayoutChangedBanner, Left = 2, Top = 7, Height = 22,
            AutoEllipsis = true, Visible = false, Tag = "head",
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _map = new MonitorMapControl
        {
            Left = 0, Top = 36, Height = 140,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _map.MonitorClicked += MapMonitorClicked;
        _map.MonitorHovered += MapMonitorHovered;

        var legend = new Label
        {
            Text = $"{Strings.LegendMain}        {Strings.LegendKeptOut}",
            Left = 2, Top = 181, Height = 18, AutoEllipsis = true, Tag = "subtle",
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        var hint = new Label
        {
            Text = Strings.MapHint, Left = 2, Top = 201, Height = 20,
            AutoEllipsis = true, Tag = "subtle",
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        header.Controls.AddRange(new Control[] { identifyAll, _layoutChangedBanner, _map, legend, hint });
        header.Resize += (a, b) =>
        {
            identifyAll.Left = Math.Max(0, header.ClientSize.Width - identifyAll.Width);
            _layoutChangedBanner.Width = Math.Max(0, identifyAll.Left - 10);
            _map.Width = header.ClientSize.Width;
            legend.Width = hint.Width = header.ClientSize.Width - 4;
        };

        _displayBody = new Panel { Dock = DockStyle.Fill, AutoScroll = true, Margin = Padding.Empty };
        _displayBodyContent = new Panel { Left = 0, Top = 0, Height = 190 };
        _displayBody.Controls.Add(_displayBodyContent);

        var keepOutHead = new Label
        {
            Text = Strings.KeepOutHead, Left = 2, Top = 10, Height = 22,
            AutoSize = true, Font = headFont, Tag = "head",
        };
        _excludeList = new GlyphCheckedListBox
        {
            Left = 2, Top = 36, Height = 130, IntegralHeight = false,
            BorderStyle = BorderStyle.FixedSingle, DrawMode = DrawMode.OwnerDrawFixed,
            ItemHeight = 24, ToggleAllowed = ExcludeCanToggle,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _excludeList.DrawItem += (a, e) => DrawExcludeItem(e);
        _excludeList.ItemCheck += ExcludeList_ItemCheck;
        _excludeList.RowClicked += ExcludeRowClicked;
        _excludeList.MouseMove += (a, e) => HoverMapFromList(_excludeList, _monitors, e.Location);
        _excludeList.MouseLeave += (a, e) => _map.ExternalHoveredStableId = null;

        _advancedToggle = new Button
        {
            Left = 2, Top = 178, Height = 32, FlatStyle = FlatStyle.Flat,
            TextAlign = ContentAlignment.MiddleLeft,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _advancedToggle.Click += (a, b) => SetAdvancedExpanded(!_advancedExpanded);

        _advancedPanel = new Panel
        {
            Left = 2, Top = 222, Height = 590, Visible = false,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };

        var topHead = new Label
        {
            Text = Strings.TopSelectionHead, Left = 0, Top = 0, AutoSize = true,
            Font = headFont, Tag = "head",
        };
        var topModes = new Panel
        {
            Left = 0, Top = 20, Height = 160,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _modeAuto = new RadioButton
        {
            Text = Strings.TopModeAuto, Left = 0, Top = 4, Height = 22,
            Checked = s.Mode != "Manual", Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _modeManual = new RadioButton
        {
            Text = Strings.TopModeManual, Left = 0, Top = 29, Height = 22,
            Checked = s.Mode == "Manual", Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _monitorList = new GlyphCheckedListBox
        {
            Left = 0, Top = 56, Height = 102, IntegralHeight = false,
            BorderStyle = BorderStyle.FixedSingle, DrawMode = DrawMode.OwnerDrawFixed, ItemHeight = 24,
            ToggleAllowed = ManualCanToggle, ToggleAnywhere = true,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _monitorList.DrawItem += (a, e) => DrawManualTopItem(e);
        _monitorList.ItemCheck += ManualTop_ItemCheck;
        _monitorList.MouseMove += (a, e) => HoverMapFromList(_monitorList, _monitors, e.Location);
        _monitorList.MouseLeave += (a, e) => _map.ExternalHoveredStableId = null;
        _modeAuto.CheckedChanged += TopMode_CheckedChanged;
        _modeManual.CheckedChanged += TopMode_CheckedChanged;
        topModes.Controls.AddRange(new Control[] { _modeAuto, _modeManual, _monitorList });

        var crossingHead = new Label
        {
            Text = Strings.CrossingPathsHead, Left = 0, Top = 192, AutoSize = true,
            Font = headFont, Tag = "head",
        };
        _crossingModesPanel = new Panel
        {
            Left = 0, Top = 212, Height = 212,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _crossingDefault = new RadioButton
        {
            Text = Strings.CrossingDefault, Left = 0, Top = 4, Height = 34,
            Checked = s.CrossingPathMode != Settings.CustomCrossingPathMode,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _crossingCustom = new RadioButton
        {
            Text = Strings.CrossingCustom, Left = 0, Top = 40, Height = 22,
            Checked = s.CrossingPathMode == Settings.CustomCrossingPathMode,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _crossingDefault.CheckedChanged += CrossingMode_CheckedChanged;
        _crossingCustom.CheckedChanged += CrossingMode_CheckedChanged;

        _customPathsPanel = new Panel
        {
            Left = 0, Top = 66, Height = 146,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        var fromLabel = new Label { Text = Strings.FromScreenLabel, Left = 0, Top = 0, AutoSize = true, Tag = "subtle" };
        _fromCombo = new ComboBox
        {
            Left = 0, Top = 20, Height = 24, DropDownStyle = ComboBoxStyle.DropDownList,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _fromCombo.SelectedIndexChanged += (a, b) =>
        {
            if (_rebuildingMonitorUi) return;
            LoadTopsForSelectedFrom();
            SyncMapState();
        };
        var allowedLabel = new Label { Text = Strings.AllowedTopsLabel, Left = 0, Top = 53, AutoSize = true, Tag = "subtle" };
        _topsCheck = new CheckedListBox
        {
            Left = 0, Top = 74, Height = 70, CheckOnClick = true, IntegralHeight = false,
            BorderStyle = BorderStyle.FixedSingle,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _topsCheck.ItemCheck += TopsCheck_ItemCheck;
        _topsCheck.MouseMove += (a, e) => HoverMapFromList(_topsCheck, _tops, e.Location);
        _topsCheck.MouseLeave += (a, e) => _map.ExternalHoveredStableId = null;
        _customPathsPanel.Controls.AddRange(new Control[] { fromLabel, _fromCombo, allowedLabel, _topsCheck });
        _crossingModesPanel.Controls.AddRange(new Control[] { _crossingDefault, _crossingCustom, _customPathsPanel });

        _behaviorHead = new Label
        {
            Text = Strings.BehaviorHead, Left = 0, Top = 436, AutoSize = true,
            Font = headFont, Tag = "head",
        };
        _deliberateCheck = new CheckBox
        {
            Text = Strings.DeliberateCrossLabel, Left = 0, Top = 460, Height = 38, Checked = s.DeliberateCross,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _descentCheck = new CheckBox
        {
            Text = Strings.DescentRoutingLabel, Left = 0, Top = 502, Height = 28, Checked = s.DescentRouting,
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
        };
        _resetCrossing = new Button
        {
            Text = Strings.ResetCrossing, Left = 0, Top = 544, Width = 180, Height = 30,
            FlatStyle = FlatStyle.Flat,
        };
        _resetCrossing.Click += ResetLayout_Click;

        _advancedPanel.Controls.AddRange(new Control[]
        {
            topHead, topModes, crossingHead, _crossingModesPanel,
            _behaviorHead, _deliberateCheck, _descentCheck, _resetCrossing,
        });
        _displayBodyContent.Controls.AddRange(new Control[] { keepOutHead, _excludeList, _advancedToggle, _advancedPanel });

        void LayoutBody()
        {
            int width = Math.Max(240, _displayBody.ClientSize.Width - 2);
            _displayBodyContent.Width = width;
            _excludeList.Width = _advancedToggle.Width = Math.Max(0, width - 4);
            _advancedPanel.Width = Math.Max(0, width - 4);
            int advancedWidth = _advancedPanel.ClientSize.Width;
            topModes.Width = _crossingModesPanel.Width = advancedWidth;
            _modeAuto.Width = _modeManual.Width = _monitorList.Width = topModes.ClientSize.Width;
            _crossingDefault.Width = _crossingCustom.Width = _crossingModesPanel.ClientSize.Width;
            _customPathsPanel.Width = _crossingModesPanel.ClientSize.Width;
            _fromCombo.Width = _topsCheck.Width = advancedWidth;
            _deliberateCheck.Width = _descentCheck.Width = advancedWidth;
        }
        _displayBody.Resize += (a, b) => LayoutBody();

        var zones = new TableLayoutPanel
        {
            Dock = DockStyle.Fill, ColumnCount = 1, RowCount = 2,
            Margin = Padding.Empty, Padding = Padding.Empty,
        };
        zones.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100f));
        zones.RowStyles.Add(new RowStyle(SizeType.Absolute, 224f));
        zones.RowStyles.Add(new RowStyle(SizeType.Percent, 100f));
        zones.Controls.Add(header, 0, 0);
        zones.Controls.Add(_displayBody, 0, 1);
        tab.Controls.Add(zones);
        LayoutBody();
        SetAdvancedExpanded(false);
        RebuildMonitorUi(null, (_monitors.FirstOrDefault(m => m.Primary) ?? _monitors.FirstOrDefault())?.StableId);
    }

    private string MonitorLabel(MonitorInfo m)
    {
        return GuardCore.MonitorLabel(m.Index, m.Bounds.Width, m.Bounds.Height, m.Primary, Strings.MainTag);
    }

    private string SelectedFromKey() =>
        _fromCombo.SelectedIndex >= 0 && _fromCombo.SelectedIndex < _bottoms.Count ? _bottoms[_fromCombo.SelectedIndex].StableId : null;

    private void SetAdvancedExpanded(bool expanded)
    {
        _advancedExpanded = expanded;
        _advancedPanel.Visible = expanded;
        _advancedToggle.Text = (expanded ? "▼ " : "▶ ") + Strings.AdvancedHead;
        _displayBodyContent.Height = expanded ? _advancedPanel.Bottom + 16 : _advancedToggle.Bottom + 4;
        _displayBody.AutoScrollMinSize = new Size(0, _displayBodyContent.Height);
    }

    private void TopMode_CheckedChanged(object sender, EventArgs e)
    {
        if (_rebuildingMonitorUi || sender is RadioButton radio && !radio.Checked) return;
        RebuildMonitorUi(SelectedFromKey(), _selectedStableId);
    }

    private void CrossingMode_CheckedChanged(object sender, EventArgs e)
    {
        if (_rebuildingMonitorUi || sender is RadioButton radio && !radio.Checked) return;
        UpdateCrossingModeUi();
        SyncMapState();
    }

    private void UpdateCrossingModeUi()
    {
        bool custom = _crossingCustom.Checked;
        _customPathsPanel.Visible = custom;
        _crossingModesPanel.Height = custom ? 212 : 66;
        _fromCombo.Enabled = custom && _bottoms.Count > 0 && _tops.Count > 0;
        _topsCheck.Enabled = custom && _fromCombo.SelectedIndex >= 0 && _tops.Count > 0;

        int behaviorTop = _crossingModesPanel.Bottom + 12;
        _behaviorHead.Top = behaviorTop;
        _deliberateCheck.Top = behaviorTop + 24;
        _descentCheck.Top = behaviorTop + 66;
        _resetCrossing.Top = behaviorTop + 108;
        _advancedPanel.Height = _resetCrossing.Bottom + 16;
        SetAdvancedExpanded(_advancedExpanded);
    }

    private void ManualTop_ItemCheck(object sender, ItemCheckEventArgs e)
    {
        if (_rebuildingMonitorUi || e.Index < 0 || e.Index >= _monitors.Count) return;
        var monitor = _monitors[e.Index];
        if (!_modeManual.Checked || !MonitorInfo.IsAbovePrimary(monitor))
        {
            e.NewValue = e.CurrentValue;
            return;
        }

        if (e.NewValue == CheckState.Checked) _manualTops.Add(monitor.StableId);
        else _manualTops.Remove(monitor.StableId);

        if (IsHandleCreated)
            BeginInvoke((Action)(() => RebuildMonitorUi(SelectedFromKey(), _selectedStableId)));
    }

    private bool ManualCanToggle(int index) =>
        _modeManual.Checked && index >= 0 && index < _monitors.Count && MonitorInfo.IsAbovePrimary(_monitors[index]);

    private bool ExcludeCanToggle(int index)
    {
        if (index < 0 || index >= _monitors.Count) return false;
        var primary = _monitors.FirstOrDefault(m => m.Primary) ?? _monitors.FirstOrDefault();
        return primary != null && !SameId(_monitors[index].StableId, primary.StableId);
    }

    private void ExcludeList_ItemCheck(object sender, ItemCheckEventArgs e)
    {
        if (e.Index < 0 || e.Index >= _monitors.Count) return;
        var monitor = _monitors[e.Index];
        if (!ExcludeCanToggle(e.Index))
        {
            e.NewValue = CheckState.Unchecked;
            return;
        }
        if (_rebuildingMonitorUi) return;

        if (e.NewValue == CheckState.Checked) _excluded.Add(monitor.StableId);
        else _excluded.Remove(monitor.StableId);
        SyncMapState();
    }

    private void ExcludeRowClicked(int index)
    {
        if (index < 0 || index >= _monitors.Count) return;
        SelectMonitor(_monitors[index], flash: true, selectAsFrom: false);
    }

    private void MapMonitorClicked(MonitorInfo monitor) =>
        SelectMonitor(monitor, flash: true, selectAsFrom: _crossingCustom.Checked);

    private void SelectMonitor(MonitorInfo monitor, bool flash, bool selectAsFrom)
    {
        if (monitor == null) return;
        _selectedStableId = monitor.StableId;
        _map.SelectedStableId = monitor.StableId;

        int allIndex = _monitors.FindIndex(m => SameId(m.StableId, monitor.StableId));
        if (allIndex >= 0)
        {
            _excludeList.SelectedIndex = allIndex;
            if (_advancedExpanded) _monitorList.SelectedIndex = allIndex;
        }

        if (selectAsFrom)
        {
            int fromIndex = _bottoms.FindIndex(m => SameId(m.StableId, monitor.StableId));
            if (fromIndex >= 0) _fromCombo.SelectedIndex = fromIndex;
        }

        if (flash) IdentifyOverlay.Flash(monitor, SelectedTheme(), this);
    }

    private void MapMonitorHovered(MonitorInfo monitor)
    {
        if (monitor == null) return;
        int allIndex = _monitors.FindIndex(m => SameId(m.StableId, monitor.StableId));
        if (allIndex >= 0)
        {
            _excludeList.SelectedIndex = allIndex;
            if (_advancedExpanded) _monitorList.SelectedIndex = allIndex;
        }
        int topIndex = _tops.FindIndex(m => SameId(m.StableId, monitor.StableId));
        if (_advancedExpanded && _crossingCustom.Checked && topIndex >= 0)
            _topsCheck.SelectedIndex = topIndex;
    }

    private void HoverMapFromList(ListBox list, IReadOnlyList<MonitorInfo> source, Point point)
    {
        int index = list.IndexFromPoint(point);
        _map.ExternalHoveredStableId = index >= 0 && index < source.Count ? source[index].StableId : null;
    }

    private void LoadTopsForSelectedFrom()
    {
        string from = SelectedFromKey();
        _rules.TryGetValue(from ?? "", out var allowed);
        bool wasRebuilding = _rebuildingMonitorUi;
        _rebuildingMonitorUi = true;
        try
        {
            for (int i = 0; i < _topsCheck.Items.Count && i < _tops.Count; i++)
            {
                bool on = allowed != null && allowed.Contains(_tops[i].StableId);
                _topsCheck.SetItemChecked(i, on);
            }
        }
        finally { _rebuildingMonitorUi = wasRebuilding; }
    }

    private void TopsCheck_ItemCheck(object sender, ItemCheckEventArgs e)
    {
        if (_rebuildingMonitorUi || e.Index < 0 || e.Index >= _tops.Count) return;
        string from = SelectedFromKey();
        if (from == null) return;
        if (!_rules.TryGetValue(from, out var set)) { set = new HashSet<string>(); _rules[from] = set; }
        string to = _tops[e.Index].StableId;
        if (e.NewValue == CheckState.Checked) set.Add(to); else set.Remove(to);
        SyncMapState();
    }

    private void ResetLayout_Click(object sender, EventArgs e)
    {
        _rebuildingMonitorUi = true;
        _manualTops.Clear();
        _rules.Clear();
        _modeAuto.Checked = true;
        _modeManual.Checked = false;
        _crossingDefault.Checked = true;
        _crossingCustom.Checked = false;
        _rebuildingMonitorUi = false;
        RebuildMonitorUi(null, _selectedStableId);
    }

    private void RecomputeTopology()
    {
        _tops.Clear();
        var manualLive = _monitors
            .Where(m => MonitorInfo.IsAbovePrimary(m) && _manualTops.Contains(m.StableId))
            .ToList();
        _tops.AddRange(_modeManual.Checked && manualLive.Count > 0
            ? manualLive
            : _monitors.Where(MonitorInfo.IsAbovePrimary));
        var topSet = new HashSet<string>(_tops.Select(m => m.StableId));
        _bottoms.Clear();
        _bottoms.AddRange(_monitors.Where(m => !topSet.Contains(m.StableId)));
    }

    private void RebuildMonitorUi(string preserveFromStableId, string preserveSelectedStableId)
    {
        _rebuildingMonitorUi = true;
        _displayBodyContent.SuspendLayout();
        _excludeList.BeginUpdate();
        _monitorList.BeginUpdate();
        _topsCheck.BeginUpdate();
        try
        {
            RecomputeTopology();
            var primary = _monitors.FirstOrDefault(m => m.Primary) ?? _monitors.FirstOrDefault();
            if (primary != null) _excluded.Remove(primary.StableId);

            _excludeList.ClearItems();
            _monitorList.ClearItems();
            foreach (var monitor in _monitors)
            {
                int excludeIndex = _excludeList.Items.Add(MonitorLabel(monitor));
                _excludeList.SetItemChecked(excludeIndex, !monitor.Primary && _excluded.Contains(monitor.StableId));

                int manualIndex = _monitorList.Items.Add(MonitorLabel(monitor));
                _monitorList.SetItemChecked(manualIndex,
                    MonitorInfo.IsAbovePrimary(monitor) && _manualTops.Contains(monitor.StableId));
            }

            _fromCombo.Items.Clear();
            foreach (var bottom in _bottoms) _fromCombo.Items.Add(MonitorLabel(bottom));
            _topsCheck.Items.Clear();
            foreach (var top in _tops) _topsCheck.Items.Add(MonitorLabel(top));

            _monitorList.Enabled = _modeManual.Checked;
            int fromIndex = _bottoms.FindIndex(m => SameId(m.StableId, preserveFromStableId));
            if (fromIndex < 0 && primary != null)
                fromIndex = _bottoms.FindIndex(m => SameId(m.StableId, primary.StableId));
            if (fromIndex < 0 && _bottoms.Count > 0) fromIndex = 0;
            _fromCombo.SelectedIndex = fromIndex;

            var selected = _monitors.FirstOrDefault(m => SameId(m.StableId, preserveSelectedStableId)) ?? primary;
            _selectedStableId = selected?.StableId;
            _excludeList.SelectedIndex = selected == null ? -1 : _monitors.FindIndex(m => SameId(m.StableId, selected.StableId));
            _monitorList.SelectedIndex = _excludeList.SelectedIndex;

            _map.SetMonitors(_monitors);
            _map.SelectedStableId = _selectedStableId;
        }
        finally
        {
            _topsCheck.EndUpdate();
            _monitorList.EndUpdate();
            _excludeList.EndUpdate();
            _displayBodyContent.ResumeLayout();
            _rebuildingMonitorUi = false;
        }

        LoadTopsForSelectedFrom();
        UpdateCrossingModeUi();
        SyncMapState();
        _excludeList.Invalidate();
        _monitorList.Invalidate();
    }

    private void SyncMapState()
    {
        string from = _crossingCustom.Checked ? SelectedFromKey() : null;
        _rules.TryGetValue(from ?? "", out var allowed);
        _map.SetState(_excluded, _tops.Select(m => m.StableId), from,
            _crossingCustom.Checked ? allowed : null, _crossingCustom.Checked);
        _map.SelectedStableId = _selectedStableId;
    }

    private void DrawExcludeItem(DrawItemEventArgs e)
    {
        if (e.Index < 0 || e.Index >= _monitors.Count) return;
        var theme = SelectedTheme();
        var monitor = _monitors[e.Index];
        var primary = _monitors.FirstOrDefault(m => m.Primary) ?? _monitors.FirstOrDefault();
        bool isPrimary = primary != null && SameId(monitor.StableId, primary.StableId);
        bool selected = (e.State & DrawItemState.Selected) != 0;

        Color background = selected ? (isPrimary ? theme.Border : theme.Accent) : theme.Surface;
        using (var brush = new SolidBrush(background)) e.Graphics.FillRectangle(brush, e.Bounds);

        int box = 14;
        int boxY = e.Bounds.Top + (e.Bounds.Height - box) / 2;
        var boxRect = new Rectangle(e.Bounds.Left + 4, boxY, box, box);
        if (isPrimary)
        {
            using var dash = new Pen(theme.Subtle, 2f);
            e.Graphics.DrawLine(dash, boxRect.Left + 2, boxRect.Top + box / 2, boxRect.Right - 2, boxRect.Top + box / 2);
        }
        else
        {
            var state = _excludeList.GetItemChecked(e.Index)
                ? System.Windows.Forms.VisualStyles.CheckBoxState.CheckedNormal
                : System.Windows.Forms.VisualStyles.CheckBoxState.UncheckedNormal;
            CheckBoxRenderer.DrawCheckBox(e.Graphics, boxRect.Location, state);
        }

        string label = MonitorLabel(monitor) + (isPrimary ? $" — {Strings.MainAlwaysAvailable}" : "");
        Color foreground = isPrimary ? theme.Subtle : (selected ? Color.White : theme.Text);
        var textRect = new Rectangle(boxRect.Right + 6, e.Bounds.Top,
            Math.Max(0, e.Bounds.Right - boxRect.Right - 8), e.Bounds.Height);
        TextRenderer.DrawText(e.Graphics, label, _excludeList.Font, textRect, foreground,
            TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.SingleLine);
        e.DrawFocusRectangle();
    }

    private void DrawManualTopItem(DrawItemEventArgs e)
    {
        if (e.Index < 0 || e.Index >= _monitors.Count) return;
        var theme = SelectedTheme();
        var monitor = _monitors[e.Index];
        bool eligible = MonitorInfo.IsAbovePrimary(monitor);
        bool interactive = eligible && _modeManual.Checked;
        bool selected = interactive && (e.State & DrawItemState.Selected) != 0;

        using (var brush = new SolidBrush(selected ? theme.Accent : theme.Surface))
            e.Graphics.FillRectangle(brush, e.Bounds);

        int box = 14;
        int boxY = e.Bounds.Top + (e.Bounds.Height - box) / 2;
        var boxRect = new Rectangle(e.Bounds.Left + 4, boxY, box, box);
        bool isChecked = _monitorList.GetItemChecked(e.Index);
        var state = interactive
            ? (isChecked ? System.Windows.Forms.VisualStyles.CheckBoxState.CheckedNormal : System.Windows.Forms.VisualStyles.CheckBoxState.UncheckedNormal)
            : (isChecked ? System.Windows.Forms.VisualStyles.CheckBoxState.CheckedDisabled : System.Windows.Forms.VisualStyles.CheckBoxState.UncheckedDisabled);
        CheckBoxRenderer.DrawCheckBox(e.Graphics, boxRect.Location, state);

        string label = MonitorLabel(monitor) + (eligible ? "" : $" — {Strings.NotAbovePrimary}");
        Color foreground = interactive ? (selected ? Color.White : theme.Text) : theme.Subtle;
        var textRect = new Rectangle(boxRect.Right + 6, e.Bounds.Top,
            Math.Max(0, e.Bounds.Right - boxRect.Right - 8), e.Bounds.Height);
        TextRenderer.DrawText(e.Graphics, label, _monitorList.Font, textRect, foreground,
            TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.SingleLine);
        e.DrawFocusRectangle();
    }

    private void DisplaySettingsChanged(object sender, EventArgs e)
    {
        if (IsDisposed || Disposing || !IsHandleCreated) return;
        void Debounce()
        {
            if (IsDisposed || Disposing) return;
            _displayRefreshTimer.Stop();
            _displayRefreshTimer.Start();
        }
        try
        {
            if (InvokeRequired) BeginInvoke((Action)Debounce);
            else Debounce();
        }
        catch (InvalidOperationException) { }
    }

    private void DisplayRefreshTimer_Tick(object sender, EventArgs e)
    {
        _displayRefreshTimer.Stop();
        List<MonitorInfo> fresh;
        try { fresh = MonitorInfo.All(); }
        catch { return; }

        string preserveFrom = SelectedFromKey();
        string preserveSelected = _selectedStableId;
        _monitors.Clear();
        _monitors.AddRange(fresh);
        RebuildMonitorUi(preserveFrom, preserveSelected);
        _layoutChangedBanner.Visible = true;
        _layoutChangedBanner.BringToFront();
    }

    private static bool SameId(string a, string b) =>
        string.Equals(a, b, StringComparison.OrdinalIgnoreCase);

    private Theme SelectedTheme() => Theming.Resolve(_themeCombo.SelectedIndex switch { 1 => "Light", 2 => "Dark", _ => "System" });

    private void DrawTab(object sender, DrawItemEventArgs e)
    {
        var t = SelectedTheme();
        var rect = _tabs.GetTabRect(e.Index);
        bool sel = e.Index == _tabs.SelectedIndex;
        using (var bg = new SolidBrush(sel ? t.Surface : t.Back))
            e.Graphics.FillRectangle(bg, rect);
        if (sel)
            using (var bar = new SolidBrush(t.Accent))
                e.Graphics.FillRectangle(bar, rect.X, rect.Bottom - 2, rect.Width, 2);
        TextRenderer.DrawText(e.Graphics, _tabs.TabPages[e.Index].Text, _tabs.Font, rect,
            sel ? t.Accent : t.Subtle, TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis);
    }

    private void ApplyThemeLive()
    {
        var theme = SelectedTheme();
        Theming.Apply(this, theme);
        _map?.SetTheme(theme);
        _tabs.Invalidate();
        Invalidate(true);
    }

    protected override void OnShown(EventArgs e)
    {
        base.OnShown(e);
        Theming.DarkTitleBar(this, SelectedTheme().IsDark);
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        SystemEvents.DisplaySettingsChanged -= DisplaySettingsChanged;
        _displayRefreshTimer.Stop();
        _displayRefreshTimer.Dispose();
        base.OnFormClosed(e);
    }

    private void HotkeyBox_KeyDown(object sender, KeyEventArgs e)
    {
        e.SuppressKeyPress = true;
        e.Handled = true;
        _mc = e.Control; _ma = e.Alt; _ms = e.Shift;
        var kc = e.KeyCode;
        if (kc is Keys.ControlKey or Keys.Menu or Keys.ShiftKey) { _key = Keys.None; UpdateHotkeyText(); return; }
        if (kc is Keys.LWin or Keys.RWin) { _mw = true; _key = Keys.None; UpdateHotkeyText(); return; }
        _key = kc;
        UpdateHotkeyText();
    }

    private void UpdateHotkeyText()
    {
        var parts = new List<string>();
        if (_mc) parts.Add("Ctrl");
        if (_ma) parts.Add("Alt");
        if (_ms) parts.Add("Shift");
        if (_mw) parts.Add("Win");
        if (_key != Keys.None) parts.Add(_key.ToString());
        _hotkeyBox.Text = parts.Count == 0 ? Strings.HotkeyPressKeys : string.Join(" + ", parts);
    }

    private void GameHotkeyBox_KeyDown(object sender, KeyEventArgs e)
    {
        e.SuppressKeyPress = true;
        e.Handled = true;
        _gc = e.Control; _ga = e.Alt; _gs = e.Shift;
        var kc = e.KeyCode;
        if (kc is Keys.ControlKey or Keys.Menu or Keys.ShiftKey) { _gkey = Keys.None; UpdateGameHotkeyText(); return; }
        if (kc is Keys.LWin or Keys.RWin) { _gw = true; _gkey = Keys.None; UpdateGameHotkeyText(); return; }
        _gkey = kc;
        UpdateGameHotkeyText();
    }

    private void UpdateGameHotkeyText()
    {
        var parts = new List<string>();
        if (_gc) parts.Add("Ctrl");
        if (_ga) parts.Add("Alt");
        if (_gs) parts.Add("Shift");
        if (_gw) parts.Add("Win");
        if (_gkey != Keys.None) parts.Add(_gkey.ToString());
        _gameHotkeyBox.Text = parts.Count == 0 ? Strings.HotkeyPressKeys : string.Join(" + ", parts);
    }

    private void PauseHotkeyBox_KeyDown(object sender, KeyEventArgs e)
    {
        e.SuppressKeyPress = true;
        e.Handled = true;
        _pc = e.Control; _pa = e.Alt; _ps = e.Shift;
        var kc = e.KeyCode;
        if (kc is Keys.ControlKey or Keys.Menu or Keys.ShiftKey) { _pkey = Keys.None; UpdatePauseHotkeyText(); return; }
        if (kc is Keys.LWin or Keys.RWin) { _pw = true; _pkey = Keys.None; UpdatePauseHotkeyText(); return; }
        _pkey = kc;
        UpdatePauseHotkeyText();
    }

    private void UpdatePauseHotkeyText()
    {
        var parts = new List<string>();
        if (_pc) parts.Add("Ctrl");
        if (_pa) parts.Add("Alt");
        if (_ps) parts.Add("Shift");
        if (_pw) parts.Add("Win");
        if (_pkey != Keys.None) parts.Add(_pkey.ToString());
        _pauseHotkeyBox.Text = parts.Count == 0 ? Strings.HotkeyPressKeys : string.Join(" + ", parts);
    }

    private void SideHotkeyBox_KeyDown(object sender, KeyEventArgs e)
    {
        e.SuppressKeyPress = true;
        e.Handled = true;
        _sc = e.Control; _sa = e.Alt; _ss = e.Shift;
        var kc = e.KeyCode;
        if (kc is Keys.ControlKey or Keys.Menu or Keys.ShiftKey) { _skey = Keys.None; UpdateSideHotkeyText(); return; }
        if (kc is Keys.LWin or Keys.RWin) { _sw = true; _skey = Keys.None; UpdateSideHotkeyText(); return; }
        _skey = kc;
        UpdateSideHotkeyText();
    }

    private void UpdateSideHotkeyText()
    {
        var parts = new List<string>();
        if (_sc) parts.Add("Ctrl");
        if (_sa) parts.Add("Alt");
        if (_ss) parts.Add("Shift");
        if (_sw) parts.Add("Win");
        if (_skey != Keys.None) parts.Add(_skey.ToString());
        _sideHotkeyBox.Text = parts.Count == 0 ? Strings.HotkeyPressKeys : string.Join(" + ", parts);
    }

    private void Ok_Click(object sender, EventArgs e)
    {
        bool anyMod = _mc || _ma || _ms || _mw;
        if (_key == Keys.None || !anyMod)
        {
            MessageBox.Show(Strings.ValidationPickModifier, "MouseFence", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            DialogResult = DialogResult.None;
            return;
        }

        Result = BuildResult();
    }

    /// <summary>Snapshot the current dialog selections into a <see cref="Settings"/> (shared by Save and Export).</summary>
    private Settings BuildResult()
    {
        var res = new Settings
        {
            ModCtrl = _mc, ModAlt = _ma, ModShift = _ms, ModWin = _mw, HotKey = _key,
            StartGateClosed = _startActive.Checked,
            AutoStart = _autoStart.Checked,
            Mode = _modeManual.Checked ? "Manual" : "AutoTop",
            CrossingPathMode = _crossingCustom.Checked ? Settings.CustomCrossingPathMode : Settings.DefaultCrossingPathMode,
            Language = _langCombo.SelectedIndex switch { 1 => "en", 2 => "tr", _ => "auto" },
            Theme = _themeCombo.SelectedIndex switch { 1 => "Light", 2 => "Dark", _ => "System" },
            ConfineModCtrl = _gc, ConfineModAlt = _ga, ConfineModShift = _gs, ConfineModWin = _gw, ConfineHotKey = _gkey,
            PauseModCtrl = _pc, PauseModAlt = _pa, PauseModShift = _ps, PauseModWin = _pw, PauseHotKey = _pkey,
            SideModCtrl = _sc, SideModAlt = _sa, SideModShift = _ss, SideModWin = _sw, SideHotKey = _skey,
            StartSideContainOn = _startSideActive.Checked,
            SideCrossMin = (int)_sideSensNum.Value,
            SideCrossSlack = _orig.SideCrossSlack,   // not surfaced in the UI — preserve whatever was loaded/imported
            DeliberateCross = _deliberateCheck.Checked,
            DescentRouting = _descentCheck.Checked,
        };

        if (res.Mode == "Manual")
        {
            foreach (var id in _manualTops.OrderBy(x => x, StringComparer.OrdinalIgnoreCase))
            {
                var live = _monitors.FirstOrDefault(m => SameId(m.StableId, id));
                if (live == null || MonitorInfo.IsAbovePrimary(live))
                    res.ManualMonitors.Add(id);   // keep unplugged edits dormant; never apply a live ineligible row
            }
        }

        if (res.CrossingPathMode == Settings.CustomCrossingPathMode)
            foreach (var kv in _rules.OrderBy(kv => kv.Key, StringComparer.OrdinalIgnoreCase))
                foreach (var to in kv.Value.OrderBy(x => x, StringComparer.OrdinalIgnoreCase))
                    res.UpLinks.Add(new UpLink { FromDevice = kv.Key, ToDevice = to });

        // Carry the walled-off displays so a Save/Export doesn't silently drop them (binding fix #5).
        res.ExcludedDevices.AddRange(_excluded.OrderBy(x => x, StringComparer.OrdinalIgnoreCase));

        return res;
    }

    // Export the current dialog state to a settings.json-format file the user picks.
    private void Export_Click(object sender, EventArgs e)
    {
        using var dlg = new SaveFileDialog
        {
            Title = Strings.ExportTitle,
            Filter = Strings.SettingsFileFilter,
            FileName = "MouseFence-settings.json",
            DefaultExt = "json",
            AddExtension = true,
        };
        if (dlg.ShowDialog(this) != DialogResult.OK) return;
        try
        {
            var json = JsonSerializer.Serialize(BuildResult(), new JsonSerializerOptions { WriteIndented = true });
            File.WriteAllText(dlg.FileName, json);
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, ex.Message, "MouseFence", MessageBoxButtons.OK, MessageBoxIcon.Warning);
        }
    }

    // Import a settings.json-format file: validate, confirm, then hand it back through the normal Save path
    // (tray persists it and reconfigures the barrier). Same format as Export -> a round-trip just works.
    private void Import_Click(object sender, EventArgs e)
    {
        using var dlg = new OpenFileDialog
        {
            Title = Strings.ImportTitle,
            Filter = Strings.SettingsFileFilter,
            CheckFileExists = true,
        };
        if (dlg.ShowDialog(this) != DialogResult.OK) return;

        Settings imported;
        string json;
        try
        {
            json = File.ReadAllText(dlg.FileName);
            imported = JsonSerializer.Deserialize<Settings>(json);
        }
        catch
        {
            json = "";
            imported = null;
        }
        if (imported == null)
        {
            MessageBox.Show(this, Strings.ImportInvalid, "MouseFence", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return;
        }
        if (MessageBox.Show(this, Strings.ImportConfirm, "MouseFence", MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes)
            return;

        int importedVersion = Settings.ReadVersion(json);
        imported.Normalize();
        if (importedVersion < Settings.CurrentSettingsVersion)
        {
            imported.MigrateLayoutKeysToStable(_monitors);
            imported.SettingsVersion = Settings.CurrentSettingsVersion;
        }
        Result = imported;
        DialogResult = DialogResult.OK;   // closes the dialog; the tray saves + reconfigures via the existing path
    }

    /// <summary>
    /// Checked list whose mouse behavior is intentionally conservative: a row click selects/identifies, and only a
    /// click in the checkbox column (or Space) changes the check state.
    /// </summary>
    private sealed class GlyphCheckedListBox : ListBox
    {
        private readonly List<CheckState> _states = new();

        [System.ComponentModel.Browsable(false)]
        [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
        public Func<int, bool> ToggleAllowed { get; set; }
        public bool ToggleAnywhere;
        public event Action<int> RowClicked;
        public event ItemCheckEventHandler ItemCheck;

        public void ClearItems()
        {
            Items.Clear();
            _states.Clear();
        }

        public bool GetItemChecked(int index) =>
            index >= 0 && index < _states.Count && _states[index] == CheckState.Checked;

        public void SetItemChecked(int index, bool value)
        {
            while (_states.Count < Items.Count) _states.Add(CheckState.Unchecked);
            if (index < 0 || index >= Items.Count) throw new ArgumentOutOfRangeException(nameof(index));
            var current = _states[index];
            var requested = value ? CheckState.Checked : CheckState.Unchecked;
            if (current == requested) return;
            var args = new ItemCheckEventArgs(index, requested, current);
            ItemCheck?.Invoke(this, args);
            _states[index] = args.NewValue;
            Invalidate(GetItemRectangle(index));
        }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            Focus();
            int index = IndexFromPoint(e.Location);
            if (index < 0) return;
            SelectedIndex = index;
            RowClicked?.Invoke(index);
            if (e.Button == MouseButtons.Left && (ToggleAnywhere || e.X <= 24) && (ToggleAllowed?.Invoke(index) ?? true))
                SetItemChecked(index, !GetItemChecked(index));
            Invalidate(GetItemRectangle(index));
        }

        protected override void OnKeyDown(KeyEventArgs e)
        {
            if (e.KeyCode == Keys.Space && SelectedIndex >= 0)
            {
                e.Handled = true;
                e.SuppressKeyPress = true;
                if (ToggleAllowed?.Invoke(SelectedIndex) ?? true)
                    SetItemChecked(SelectedIndex, !GetItemChecked(SelectedIndex));
                return;
            }
            base.OnKeyDown(e);
        }
    }
}
