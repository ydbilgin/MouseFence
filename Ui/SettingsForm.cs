using System.Drawing;
using System.Drawing.Drawing2D;
using System.Text.Json;
using Microsoft.Win32;

namespace MouseFence;

public readonly record struct SettingsRuntimeState(bool UpwardPassageOpen, bool SideProtectionOn, bool Paused);

/// <summary>The transactional, native WinForms settings experience.</summary>
public sealed class SettingsForm : Form
{
    internal static IReadOnlyCollection<string> GuiEditableSettingProperties { get; } = new HashSet<string>
    {
        nameof(Settings.HotKey), nameof(Settings.ModCtrl), nameof(Settings.ModAlt), nameof(Settings.ModShift), nameof(Settings.ModWin),
        nameof(Settings.ConfineHotKey), nameof(Settings.ConfineModCtrl), nameof(Settings.ConfineModAlt),
        nameof(Settings.ConfineModShift), nameof(Settings.ConfineModWin),
        nameof(Settings.PauseHotKey), nameof(Settings.PauseModCtrl), nameof(Settings.PauseModAlt),
        nameof(Settings.PauseModShift), nameof(Settings.PauseModWin),
        nameof(Settings.SideHotKey), nameof(Settings.SideModCtrl), nameof(Settings.SideModAlt),
        nameof(Settings.SideModShift), nameof(Settings.SideModWin),
        nameof(Settings.StartGateClosed), nameof(Settings.StartSideContainOn), nameof(Settings.AutoStart),
        nameof(Settings.StartConfineOn), nameof(Settings.StartPaused),
        nameof(Settings.DeliberateCross), nameof(Settings.DescentRouting), nameof(Settings.Mode),
        nameof(Settings.ManualMonitors), nameof(Settings.ExcludedDevices), nameof(Settings.MonitorNames),
        nameof(Settings.Language), nameof(Settings.Theme),
        nameof(Settings.UpwardBoundaryDefault), nameof(Settings.SideBoundaryDefault), nameof(Settings.DownwardBoundaryDefault),
        nameof(Settings.BoundaryRules), nameof(Settings.UpCrossMin), nameof(Settings.UpCrossSlack),
        nameof(Settings.SideCrossMin), nameof(Settings.SideCrossSlack), nameof(Settings.PassageInsetPx),
        nameof(Settings.EdgeHoldbackPx), nameof(Settings.SeamTolerancePx), nameof(Settings.SafetyEscapes),
        nameof(Settings.BoundaryTipSeen),
    };

    internal static IReadOnlyCollection<string> M1DeferredGuiSettingProperties { get; } = new HashSet<string>();

    private readonly Settings _draft;
    private readonly Func<SettingsRuntimeState> _runtimeState;
    private readonly bool _showWithoutActivation;
    private readonly List<MonitorInfo> _monitors = new();
    private readonly ToolTip _tips = new();
    private readonly System.Windows.Forms.Timer _runtimeTimer = new() { Interval = 400 };
    private readonly System.Windows.Forms.Timer _displayRefreshTimer = new() { Interval = 700 };

    private readonly FlatTabControl _tabs;
    private readonly TabPage _advancedPage;
    private readonly TabPage _shortcutsPage;
    private readonly TabPage _generalPage;
    private readonly Panel _tabStrip;
    private readonly SidebarButton[] _tabItems;
    private readonly MonitorMapControl _map;
    private readonly MapLegend _mapLegend;
    private readonly TableLayoutPanel _screenRows;
    private readonly Panel _screenSplit;
    private readonly Panel _drawer;
    private readonly FlowLayoutPanel _drawerFlow;
    private readonly FlowLayoutPanel _bodyFlow;
    private readonly Label _mapHint;
    private readonly Panel _screenSection;
    private readonly Label _screenSectionTitle;
    private readonly Label _screenSectionHelp;
    private readonly TableLayoutPanel _footer;
    private readonly Label _inlineStatus;
    private readonly Button _inlineAction;
    private readonly Button _endpointFrom;
    private readonly Button _endpointTo;
    private readonly Label _pairSeparator;
    private readonly Label _drawerTitle;
    private readonly Label _drawerMeta;
    private readonly Label _drawerBinding;
    private readonly Label _drawerStatus;
    private readonly Label _drawerDirection;
    private readonly Label _returnLine;
    private readonly LinkLabel _separateLink;
    private readonly LinkLabel _overallLink;
    private readonly LinkLabel _whenLink;
    private readonly ThemedButton _allowScreen;
    private readonly FlowLayoutPanel _modeTiles;
    private readonly TableLayoutPanel _separatePanel;
    private readonly Panel _whenPanel;
    private readonly RadioButton _automaticControl;
    private readonly RadioButton _alwaysControl;
    private readonly ModeTile _freeTile;
    private readonly ModeTile _pushTile;
    private readonly ModeTile _blockedTile;
    private readonly FlowLayoutPanel _advancedPanel;
    private readonly Button _advancedToggle;

    private readonly TextBox[] _hotkeyBoxes = new TextBox[4];
    private readonly ComboBox _langCombo;
    private readonly ComboBox _themeCombo;
    private readonly CheckBox _startGate;
    private readonly CheckBox _startSide;
    private readonly CheckBox _autoStart;
    private readonly CheckBox _startConfine;
    private readonly CheckBox _startPaused;

    private BoundaryView _boundary;
    private BoundaryEndpoint _primary;
    private BoundaryEndpoint _reverse;
    private bool _separateExpanded;
    private bool _whenExpanded;
    private bool _advancedExpanded;
    private bool _rebuilding;
    private bool _inlineStatusIsError;
    private int _advancedWidth = 520;
    private Theme _theme;

    // Keep the preferred 1064 px composition unchanged, then give large windows a deliberate
    // centered measure instead of stretching task controls across the whole desktop.
    private const int WideContentMeasure = 1240;
    private const double PreferredMapHeightRatio = .39;
    private const double MaximumMapHeightRatio = .59;

    private sealed record BoundaryEndpoint(MonitorInfo From, MonitorInfo To, BoundaryDirection Direction, RuntimeTransition Runtime);

    public Settings Result { get; private set; }
    internal Settings DraftForTests => _draft;
    internal bool BoundaryEditorVisibleForTests => _screenSplit.Height > 0 && _screenSplit.Visible;
    internal string InlineStatusForTests => _inlineStatus?.Text ?? "";
    internal IReadOnlyList<string> DrawerStatusCandidatesForTests => DrawerStatusCandidates();
    internal (string Mode, int BodyHeight, int MapHeight, int BandHeight) DisplayLayoutMetrics()
    {
        var last = new[] { _screenSplit, _screenSection, _screenRows }
            .Where(control => control?.Visible == true)
            .Select(control => (Control: control,
                Bounds: _bodyFlow.RectangleToClient(control.RectangleToScreen(control.ClientRectangle))))
            .OrderByDescending(item => item.Bounds.Bottom)
            .First();
        return (_screenSplit.Visible && !_screenRows.Visible ? "drawer" : "list",
            _bodyFlow.ClientSize.Height, _map.Height,
            Math.Max(0, _bodyFlow.ClientSize.Height - last.Bounds.Bottom));
    }
    // Test hooks (fields, not auto-properties, so the WinForms WFO1000 designer-serialization analyzer stays quiet).
    internal Func<string> RestorePathPickerForTests = null;
    internal Func<bool> RestoreConfirmForTests = null;
    // Drives the backup path (validate + write + inline success) to a fixed path without a file dialog, for tests.
    internal bool BackupToFileForTests(string path) => ValidateDraftForBackup() && WriteBackupFile(path);
    internal void RestoreFromFileForTests() => Import_Click(this, EventArgs.Empty);

    public SettingsForm(Settings settings, List<MonitorInfo> monitors,
                        Func<SettingsRuntimeState> runtimeState = null, bool showWithoutActivation = false)
    {
        // Never normalize or edit the live instance. The draft owns all nested lists, dictionaries, and rule objects.
        _draft = (settings ?? new Settings()).DeepClone();
        _runtimeState = runtimeState;
        _showWithoutActivation = showWithoutActivation;
        _monitors.AddRange(monitors ?? new List<MonitorInfo>());
        Strings.Use(_draft.Language);
        _theme = Theming.Resolve(_draft.Theme);

        Text = Strings.SettingsTitle;
        Font = new Font("Segoe UI", 9f);
        FormBorderStyle = FormBorderStyle.Sizable;
        MaximizeBox = true;
        MinimizeBox = false;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.CenterScreen;
        AutoScaleMode = AutoScaleMode.Dpi;
        ClientSize = new Size(1064, 712);
        MinimumSize = new Size(720, 560);

        var root = new TableLayoutPanel
        {
            Dock = DockStyle.Fill, ColumnCount = 2, RowCount = 2, Margin = Padding.Empty, Padding = Padding.Empty,
        };
        root.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        root.RowStyles.Add(new RowStyle(SizeType.Absolute, 64));
        root.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 168));
        root.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

        _tabs = new FlatTabControl
        {
            Dock = DockStyle.Fill, Appearance = TabAppearance.FlatButtons, SizeMode = TabSizeMode.Fixed,
            ItemSize = new Size(0, 1), Padding = Point.Empty, Margin = Padding.Empty,
        };
        var screens = new TabPage(Strings.Ui("TabScreens"))
            { Padding = new Padding(0), UseVisualStyleBackColor = false };
        _advancedPage = new TabPage(Strings.Ui("AdvancedM2"))
            { Padding = new Padding(28, 23, 28, 32), AutoScroll = true, UseVisualStyleBackColor = false };
        _shortcutsPage = new TabPage(Strings.Ui("TabShortcuts"))
            { Padding = new Padding(28, 23, 28, 32), AutoScroll = true, UseVisualStyleBackColor = false };
        _generalPage = new TabPage(Strings.TabGeneral)
            { Padding = new Padding(28, 23, 28, 32), AutoScroll = true, UseVisualStyleBackColor = false };
        _tabs.TabPages.AddRange(new[] { screens, _advancedPage, _shortcutsPage, _generalPage });
        _tabStrip = new Panel { Dock = DockStyle.Fill, Margin = Padding.Empty, Tag = "sidebar", Padding = new Padding(12, 18, 12, 12) };
        _tabStrip.Paint += (_, e) =>
        {
            using var border = new Pen(_theme.Border);
            e.Graphics.DrawLine(border, _tabStrip.Width - 1, 0, _tabStrip.Width - 1, _tabStrip.Height);
        };
        var product = new Label { Text = "▣  MouseFence", Left = 12, Top = 21, Width = 144, Height = 25,
            Font = new Font("Segoe UI Semibold", 10.5f), Tag = "head" };
        var settingsLabel = new Label { Text = Strings.Ui("SettingsNavLabel"), Left = 12, Top = 66, Width = 144, Height = 18,
            Font = new Font("Segoe UI Semibold", 7.5f), Tag = "faint" };
        _tabStrip.Controls.Add(product); _tabStrip.Controls.Add(settingsLabel);
        _tabItems = new[]
        {
            new SidebarButton(Strings.Ui("NavDisplays"), 0),
            new SidebarButton(Strings.Ui("NavAdvanced"), 1),
            new SidebarButton(Strings.Ui("NavShortcuts"), 2),
            new SidebarButton(Strings.Ui("NavGeneral"), 3),
        };
        int tabTop = 92;
        foreach (var item in _tabItems)
        {
            item.Left = 12; item.Top = tabTop; item.Width = 144; item.Height = 42;
            item.Click += (_, _) => _tabs.SelectedIndex = item.Index;
            item.SelectionRequested += SelectRelativeTab;
            _tabStrip.Controls.Add(item);
            tabTop += 46;
        }
        _tabs.SelectedIndexChanged += (_, _) =>
        {
            UpdateTabStrip();
            UpdateBoundaryTipVisibility();
        };
        root.SetRowSpan(_tabStrip, 2);
        root.Controls.Add(_tabStrip, 0, 0);
        root.Controls.Add(_tabs, 1, 0);

        _bodyFlow = new FlowLayoutPanel { Dock = DockStyle.Fill, AutoScroll = true, FlowDirection = FlowDirection.TopDown,
            WrapContents = false, Padding = new Padding(28, 23, 28, 32), Margin = Padding.Empty };
        _bodyFlow.HandleCreated += (_, _) => ThemeScrollbars(_bodyFlow);
        _bodyFlow.SizeChanged += (_, _) => ResizeV2Layout();
        var mapHeader = new TableLayoutPanel
        {
            Width = 820, Height = 72, MinimumSize = new Size(0, 72), ColumnCount = 2, RowCount = 1,
            Margin = new Padding(0, 0, 0, 10), Tag = "mapHeader",
        };
        mapHeader.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        mapHeader.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        mapHeader.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        var headingStack = new Panel { Dock = DockStyle.Fill };
        headingStack.Controls.Add(new Label { Text = Strings.Ui("DisplaysTitle"), Left = 0, Top = 0, Width = 520, Height = 28,
            Font = new Font("Segoe UI Semibold", 18f), Tag = "head" });
        headingStack.Controls.Add(new Label { Text = Strings.Ui("DisplaysSubtitle"), Left = 0, Top = 31, Width = 650, Height = 23, Tag = "subtle" });
        var identifyAll = new ThemedButton { Text = Strings.Ui("IdentifyAllScreensM2"), Width = 154, Height = 36,
            Anchor = AnchorStyles.Right, Role = ThemedButtonRole.Secondary, Tag = "identifyAll" };
        identifyAll.Click += (_, _) => IdentifyOverlay.Flash(
            _monitors.Where(m => !_draft.ExcludedDevices.Contains(m.StableId, StringComparer.OrdinalIgnoreCase)),
            _draft.MonitorNames, _theme, this);
        mapHeader.Controls.Add(headingStack, 0, 0); mapHeader.Controls.Add(identifyAll, 1, 0);
        _bodyFlow.Controls.Add(mapHeader);
        _map = new MonitorMapControl { Width = 820, Height = 258, Margin = Padding.Empty };
        _map.SetTheme(_theme);
        _map.MonitorClicked += MonitorChosen;
        _map.MonitorHovered += m => HoverRow(m?.StableId);
        _map.BoundaryClicked += OpenBoundary;
        _map.BoundaryEscape += CloseDrawer;
        _bodyFlow.Controls.Add(_map);
        _mapLegend = new MapLegend { Visible = false };
        _mapLegend.SetTheme(_theme);
        _mapHint = new Label
        {
            Width = 820, Height = 38, TextAlign = ContentAlignment.MiddleLeft, Tag = "subtle", Margin = Padding.Empty,
            Text = _monitors.Count(m => !_draft.ExcludedDevices.Contains(m.StableId, StringComparer.OrdinalIgnoreCase)) <= 1
                ? Strings.Ui("MapOnlyOne") : Strings.Ui("MapHintV2"),
        };
        _bodyFlow.Controls.Add(_mapHint);

        _screenSplit = new Panel { Width = 820, Height = 0, Visible = false, Margin = Padding.Empty };

        _drawer = new Panel { Dock = DockStyle.Fill, Tag = "surface2", Margin = Padding.Empty, Padding = new Padding(14, 12, 14, 12) };
        _drawer.Paint += PaintDrawerCard;
        _drawerFlow = new FlowLayoutPanel
        {
            Dock = DockStyle.Fill, AutoScroll = false,
            FlowDirection = FlowDirection.TopDown, WrapContents = false, Padding = Padding.Empty, Tag = "surface2",
        };
        var drawerFlow = _drawerFlow;
        var drawerHeader = new Panel { Width = 790, Height = 51, Tag = "surface2", Margin = Padding.Empty };
        _drawerTitle = new Label
        {
            Text = Strings.Ui("DrawerTitle"), Left = 0, Top = 0, Width = 10, Height = 1, Visible = false,
        };
        var close = new DrawerCloseButton { Left = 758, Top = 0, Size = new Size(31, 31), AccessibleName = Strings.Ui("CloseDrawer") };
        close.Click += (_, _) => CloseDrawer();
        _endpointFrom = new ThemedButton { Width = 135, Height = 31, Role = ThemedButtonRole.Secondary };
        _endpointTo = new ThemedButton { Left = 165, Width = 135, Height = 31, Role = ThemedButtonRole.Secondary };
        _pairSeparator = new Label { Text = "↔", Left = 136, Top = 0, Width = 28, Height = 31, TextAlign = ContentAlignment.MiddleCenter, Tag = "subtle" };
        // Endpoint chips are static labels of the two screens in this boundary — no click-to-identify
        // (owner request 2026-07-15; identification is the "Identify all screens" button's job only).
        _endpointFrom.Cursor = Cursors.Default;
        _endpointTo.Cursor = Cursors.Default;
        _drawerMeta = new Label { Left = 316, Top = 0, Width = 430, Height = 20, Tag = "head", Font = new Font("Segoe UI Semibold", 8.5f) };
        _drawerBinding = new Label { Text = Strings.Ui("DrawerBinding"), Left = 316, Top = 21, Width = 430, Height = 25, Tag = "subtle" };
        drawerHeader.Controls.AddRange(new Control[] { _drawerTitle, _endpointFrom, _pairSeparator, _endpointTo, _drawerMeta, _drawerBinding, close });
        drawerFlow.Controls.Add(drawerHeader);
        var question = new Panel { Width = 790, Height = 29, Margin = Padding.Empty, Tag = "surface2" };
        _drawerDirection = new Label { Left = 0, Top = 4, Width = 520, Height = 22, Font = new Font("Segoe UI Semibold", 10.5f), TextAlign = ContentAlignment.MiddleLeft };
        _overallLink = new FlatLinkLabel { Text = Strings.Ui("UseOverallBehavior"), Left = 570, Top = 4, Width = 215, Height = 20, TextAlign = ContentAlignment.MiddleRight };
        _overallLink.LinkClicked += (_, _) => UseOverall(); question.Controls.Add(_drawerDirection); question.Controls.Add(_overallLink);
        drawerFlow.Controls.Add(question);
        _modeTiles = new FlowLayoutPanel { Width = 790, Height = 84, FlowDirection = FlowDirection.LeftToRight, WrapContents = false, Margin = Padding.Empty, Tag = "surface2" };
        _freeTile = NewModeTile(BoundaryBehavior.Free, Strings.Ui("CrossesFreely"), Strings.Ui("CrossesFreelyHelp"));
        _pushTile = NewModeTile(BoundaryBehavior.Push, Strings.Ui("NeedsFirmPush"), Strings.Ui("NeedsFirmPushHelp"));
        _blockedTile = NewModeTile(BoundaryBehavior.Blocked, Strings.Ui("NeverCrosses"), Strings.Ui("NeverCrossesHelp"));
        _modeTiles.Controls.AddRange(new Control[] { _freeTile, _pushTile, _blockedTile });
        drawerFlow.Controls.Add(_modeTiles);
        _returnLine = new Label { Width = 790, Height = 20, Tag = "subtle", Margin = Padding.Empty };
        drawerFlow.Controls.Add(_returnLine);
        _drawerStatus = new Label { Width = 790, Height = 21, Tag = "warning", Font = new Font("Segoe UI", 8.5f, FontStyle.Italic), Margin = Padding.Empty };
        drawerFlow.Controls.Add(_drawerStatus);
        var folds = new FlowLayoutPanel { Width = 790, Height = 31, FlowDirection = FlowDirection.LeftToRight, WrapContents = false, Margin = Padding.Empty, Tag = "surface2" };
        _separateLink = new FlatLinkLabel { Text = Strings.Ui("SetDirectionsSeparately"), Width = 390, Height = 31, LinkColor = _theme.Accent, Margin = Padding.Empty, TextAlign = ContentAlignment.MiddleLeft };
        _separateLink.LinkClicked += (_, _) => ToggleSeparate();
        _whenLink = new FlatLinkLabel { Text = Strings.Ui("WhenThisApplies"), Width = 390, Height = 31, LinkColor = _theme.Accent, Margin = Padding.Empty, TextAlign = ContentAlignment.MiddleLeft };
        _whenLink.LinkClicked += (_, _) => ToggleWhen();
        folds.Controls.Add(_separateLink); folds.Controls.Add(_whenLink); drawerFlow.Controls.Add(folds);
        _separatePanel = new TableLayoutPanel { Width = 790, AutoSize = true, ColumnCount = 1, Visible = false, Margin = Padding.Empty, Tag = "surface2" };
        drawerFlow.Controls.Add(_separatePanel);
        _whenPanel = new Panel { Width = 790, Height = 94, Visible = false, Margin = Padding.Empty, Tag = "surface2" };
        _automaticControl = new RadioButton { Text = Strings.Ui("AutomaticControl"), Left = 0, Top = 0, Width = 760, Height = 24 };
        var autoHelp = new Label { Text = Strings.Ui("AutomaticControlHelp"), Left = 24, Top = 24, Width = 302, Height = 24, Tag = "subtle" };
        _alwaysControl = new RadioButton { Text = Strings.Ui("AlwaysControl"), Left = 0, Top = 48, Width = 760, Height = 24 };
        var alwaysHelp = new Label { Text = Strings.Ui("AlwaysControlHelp"), Left = 24, Top = 76, Width = 302, Height = 24, Tag = "subtle" };
        _automaticControl.CheckedChanged += (_, _) => { if (!_rebuilding && _automaticControl.Checked) SetControl(BoundaryControl.Automatic); };
        _alwaysControl.CheckedChanged += (_, _) => { if (!_rebuilding && _alwaysControl.Checked) SetControl(BoundaryControl.Always); };
        _whenPanel.Controls.AddRange(new Control[] { _automaticControl, autoHelp, _alwaysControl, alwaysHelp });
        drawerFlow.Controls.Add(_whenPanel);
        _allowScreen = new ThemedButton { Text = Strings.Ui("AllowThisScreen"), Width = 190, Height = 32, Role = ThemedButtonRole.Quiet }; _allowScreen.Visible = true; _allowScreen.Enabled = false;
        _allowScreen.Click += (_, _) => AllowBoundaryScreen(); drawerFlow.Controls.Add(_allowScreen);
        _drawer.Controls.Add(drawerFlow);
        _screenSplit.Controls.Add(_drawer); _bodyFlow.Controls.Add(_screenSplit);

        _screenSection = new Panel { Width = 820, Height = 48, Margin = new Padding(0, 5, 0, 0) };
        _screenSectionTitle = new Label { Text = Strings.Ui("ScreensSectionTitle"), Left = 0, Top = 8, Height = 24, Font = new Font("Segoe UI Semibold", 11f) };
        _screenSectionHelp = new Label { Text = Strings.Ui("ScreensSectionHelp"), Top = 10, Height = 22, Tag = "subtle", TextAlign = ContentAlignment.MiddleRight };
        _screenSection.Controls.Add(_screenSectionTitle);
        _screenSection.Controls.Add(_screenSectionHelp);
        _bodyFlow.Controls.Add(_screenSection);
        _screenRows = new TableLayoutPanel { AutoSize = false, ColumnCount = 1, Width = 820, Height = 1, Margin = Padding.Empty };
        _screenRows.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100)); _bodyFlow.Controls.Add(_screenRows);
        screens.Controls.Add(_bodyFlow);

        _advancedToggle = new ThemedButton { Text = Strings.Ui("FineTunePixels"), Width = 800, Height = 36, Role = ThemedButtonRole.Quiet, TextAlign = ContentAlignment.MiddleLeft };
        _advancedToggle.Click += (_, _) => ToggleAdvanced();
        _advancedPanel = BuildAdvancedPanel();
        _advancedPage.Controls.Add(_advancedPanel);
        _advancedPage.SizeChanged += (_, _) => ResizeV2Layout();
        _shortcutsPage.SizeChanged += (_, _) => ResizeV2Layout();
        _generalPage.SizeChanged += (_, _) => ResizeV2Layout();
        ToggleAdvanced(force: false);

        _shortcutsPage.HandleCreated += (_, _) => ThemeScrollbars(_shortcutsPage);
        _generalPage.HandleCreated += (_, _) => ThemeScrollbars(_generalPage);
        BuildShortcutsTab(_shortcutsPage);
        var generalControls = BuildGeneralTab(_generalPage);
        _langCombo = generalControls.Language;
        _themeCombo = generalControls.Theme;
        _startGate = generalControls.StartGate;
        _startSide = generalControls.StartSide;
        _autoStart = generalControls.AutoStart;
        _startConfine = generalControls.StartConfine;
        _startPaused = generalControls.StartPaused;

        _footer = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 4, Padding = new Padding(22, 13, 22, 13), Tag = "surface" };
        _footer.Paint += (_, e) => { using var p = new Pen(_theme.Border); e.Graphics.DrawLine(p, 0, 0, _footer.Width, 0); };
        _footer.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100)); _footer.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        _footer.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize)); _footer.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        _inlineStatus = new Label
        {
            AutoSize = false, Dock = DockStyle.Fill, Height = 36, Anchor = AnchorStyles.Left,
            TextAlign = ContentAlignment.MiddleLeft, AutoEllipsis = true, Tag = "faint",
            Text = !_draft.BoundaryTipSeen && !_draft.BoundaryRules.Any() ? Strings.Ui("BoundaryTip") : Strings.Ui("UnsavedFooter"),
        };
        _inlineAction = new ThemedButton { Text = "", Width = 145, Height = 36, Role = ThemedButtonRole.Quiet, Visible = false, Enabled = false }; _inlineAction.Click += OpenValidationBoundary;
        var cancel = new ThemedButton { Text = Strings.Cancel, Width = 94, Height = 36, Role = ThemedButtonRole.Secondary, DialogResult = DialogResult.Cancel, Margin = new Padding(10, 0, 0, 0) };
        var save = new ThemedButton { Text = Strings.Save, Width = 94, Height = 36, Role = ThemedButtonRole.Primary, Margin = new Padding(10, 0, 0, 0) }; save.Click += Save_Click;
        _footer.Controls.Add(_inlineStatus, 0, 0); _footer.Controls.Add(_inlineAction, 1, 0);
        _footer.Controls.Add(cancel, 2, 0); _footer.Controls.Add(save, 3, 0);
        root.Controls.Add(_footer, 1, 1);
        Controls.Add(root);
        AcceptButton = save; CancelButton = cancel;

        RebuildScreenRows();
        RefreshMap();
        UpdateShortcutErrors();
        _runtimeTimer.Tick += (_, _) => UpdateLiveStatus();
        _runtimeTimer.Start();
        _displayRefreshTimer.Tick += DisplayRefreshTimer_Tick;
        SystemEvents.DisplaySettingsChanged += DisplaySettingsChanged;
        Resize += (_, _) => ResizeV2Layout();
        HandleCreated += (_, _) => Theming.DarkTitleBar(this, _theme);
        UpdateTabStrip();
        ApplyTheme();
        ResizeV2Layout();
    }

    protected override bool ShowWithoutActivation => _showWithoutActivation;

    public void SelectTab(int i) { if (i >= 0 && i < _tabs.TabCount) _tabs.SelectedIndex = i; }
    internal void OpenFirstBoundaryForPreview(bool hideScreenList = false)
    {
        var first = _map.Boundaries.FirstOrDefault();
        if (first != null)
        {
            _tabs.SelectedIndex = 0;
            OpenBoundary(first);
            if (hideScreenList)
            {
                _screenSection.Visible = false;
                _screenRows.Visible = false;
                BeginInvoke((Action)(() =>
                {
                    _bodyFlow.AutoScrollPosition = Point.Empty;
                    ResizeV2Layout();
                }));
            }
        }
    }
    internal void OpenAdvancedForPreview()
    {
        _tabs.SelectedIndex = 1;
        ToggleAdvanced(force: true);
        var numericGrid = _advancedPanel.Controls.Cast<Control>().FirstOrDefault(c => (c.Tag as string) == "numericGrid");
        if (numericGrid != null) BeginInvoke((Action)(() => _advancedPage.ScrollControlIntoView(numericGrid)));
    }
    internal void OpenAdvancedOverviewForPreview()
    {
        _tabs.SelectedIndex = 1;
        // The overview capture ends after the automatic/manual choice. Hide the long manual list and reset
        // section so the fixed footer never cuts through the next row in public documentation.
        bool afterOverview = false;
        foreach (Control child in _advancedPanel.Controls)
        {
            if (child is FlowLayoutPanel) afterOverview = true;
            if (afterOverview) child.Visible = false;
        }
    }

    private void RebuildScreenRows()
    {
        _rebuilding = true;
        _screenRows.SuspendLayout(); _screenRows.Controls.Clear(); _screenRows.RowStyles.Clear(); _screenRows.RowCount = 0;
        _screenRows.ColumnCount = 1;
        int headerRow = _screenRows.RowCount++;
        _screenRows.RowStyles.Add(new RowStyle(SizeType.Absolute, 31));
        var header = ScreenListGrid(31);
        header.Tag = "listHeader";
        header.Controls.Add(ListHeader(Strings.Ui("ScreenColumn")), 0, 0);
        header.Controls.Add(ListHeader(Strings.Ui("NameColumn")), 1, 0);
        header.Controls.Add(ListHeader(Strings.Ui("ResolutionColumn")), 2, 0);
        header.Controls.Add(ListHeader(Strings.Ui("PointerAccessColumn")), 3, 0);
        _screenRows.Controls.Add(header, 0, headerRow);
        foreach (var monitor in _monitors.OrderBy(m => m.Index))
        {
            int row = _screenRows.RowCount++;
            _screenRows.RowStyles.Add(new RowStyle(SizeType.Absolute, 53));
            var panel = ScreenListGrid(53); panel.Tag = "screenRow"; panel.AccessibleName = ScreenName(monitor); panel.Paint += PaintListSeparator;
            var screenCell = new Panel { Dock = DockStyle.Fill };
            var chip = new NumberChip { Number = monitor.Index, Accent = monitor.Primary, Left = 0, Top = 10, Size = new Size(26, 26) };
            screenCell.Controls.Add(chip);
            var name = new TextBox { Text = Settings.MonitorNameFor(_draft.MonitorNames, monitor.StableId), BorderStyle = BorderStyle.None };
            name.HandleCreated += (_, _) => Native.SendMessage(name.Handle, Native.EM_SETCUEBANNER, (IntPtr)1, Strings.Ui("OptionalName"));
            name.TextChanged += (_, _) =>
            {
                if (_rebuilding) return;
                string clean = Settings.NormalizeMonitorName(name.Text);
                if (clean.Length == 0) _draft.MonitorNames.Remove(monitor.StableId); else _draft.MonitorNames[monitor.StableId] = clean;
                RefreshMap();
            };
            var nameFrame = new ThemedControlFrame { Dock = DockStyle.Fill, Height = 36, Margin = new Padding(0, 8, 12, 8) }; nameFrame.Host(name);
            var resolution = new Label { Dock = DockStyle.Fill, Text = $"{monitor.Bounds.Width} × {monitor.Bounds.Height}", Tag = "subtle", TextAlign = ContentAlignment.MiddleLeft };
            Control keep;
            if (monitor.Primary)
                keep = new Label { Text = Strings.Ui("MainAlwaysV2"), Dock = DockStyle.Fill, Tag = "accent", TextAlign = ContentAlignment.MiddleLeft };
            else
            {
                var check = new ThemedCheckBox { Text = Strings.Ui("KeepPointerOff"), Dock = DockStyle.Fill, Height = 32,
                    Checked = _draft.ExcludedDevices.Contains(monitor.StableId, StringComparer.OrdinalIgnoreCase) };
                check.CheckedChanged += (_, _) =>
                {
                    if (_rebuilding) return;
                    if (check.Checked) _draft.ExcludedDevices.Add(monitor.StableId);
                    else _draft.ExcludedDevices.RemoveAll(id => Same(id, monitor.StableId));
                    SanitizeExcluded(); RefreshMap(); RebuildScreenRows(); UpdateDrawer();
                };
                keep = check;
            }
            panel.Controls.Add(screenCell, 0, 0); panel.Controls.Add(nameFrame, 1, 0); panel.Controls.Add(resolution, 2, 0); panel.Controls.Add(keep, 3, 0);
            panel.Click += (_, _) => MonitorChosen(monitor);
            panel.MouseEnter += (_, _) => _map.ExternalHoveredStableId = monitor.StableId;
            panel.MouseLeave += (_, _) => _map.ExternalHoveredStableId = null;
            _screenRows.Controls.Add(panel, 0, row);
        }
        _screenRows.ResumeLayout(); _rebuilding = false;
        _screenRows.Height = Math.Max(1, 31 + Math.Max(0, _screenRows.RowCount - 1) * 53);
        ApplyTheme();
    }

    private void UpdateScreenRowPitch(bool narrow)
    {
        if (_screenRows.IsDisposed || _rebuilding) return;
        int pitch = DpiPx(narrow ? 72 : 53);
        if (_screenRows.RowStyles.Count < 2 || Math.Abs(_screenRows.RowStyles[1].Height - pitch) < 0.1f) return;
        for (int i = 1; i < _screenRows.RowStyles.Count; i++) _screenRows.RowStyles[i].Height = pitch;
        _screenRows.Height = Math.Max(1, DpiPx(31) + Math.Max(0, _screenRows.RowCount - 1) * pitch);
    }

    private FlowLayoutPanel BuildAdvancedPanel()
    {
        var flow = new FlowLayoutPanel
        {
            Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Width = 800,
            FlowDirection = FlowDirection.TopDown, WrapContents = false,
            Padding = Padding.Empty, Margin = Padding.Empty,
        };
        flow.Controls.Add(PageHeader(Strings.Ui("NavAdvanced"), Strings.Ui("AdvancedSubtitle"), 800));
        AddSectionHeading(flow, Strings.Ui("DefaultBoundaryHead"));
        flow.Controls.Add(DefaultRow(Strings.Ui("UpwardDefault"), _draft.UpwardBoundaryDefault, v => _draft.UpwardBoundaryDefault = v, true));
        flow.Controls.Add(DefaultRow(Strings.Ui("SideDefault"), _draft.SideBoundaryDefault, v => _draft.SideBoundaryDefault = v, true));
        flow.Controls.Add(DefaultRow(Strings.Ui("DownDefault"), _draft.DownwardBoundaryDefault, v => _draft.DownwardBoundaryDefault = v, false));

        AddSectionHeading(flow, Strings.Ui("CrossingFeelHead"));
        var deliberate = new ThemedCheckBox { Text = Strings.Ui("FirmPushRecommended"), Width = 790, Height = 32, Checked = _draft.DeliberateCross };
        deliberate.CheckedChanged += (_, _) => _draft.DeliberateCross = deliberate.Checked; flow.Controls.Add(deliberate);
        flow.Controls.Add(HelpLabel(Strings.Ui("CrossingFeelSectionHelp"), 790, 34));
        flow.Controls.Add(_advancedToggle);
        var numericGrid = new TableLayoutPanel { Width = 790, AutoSize = true, ColumnCount = 2, Tag = "numericGrid", Margin = Padding.Empty };
        numericGrid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50)); numericGrid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        var numericRows = new[]
        {
            NumericRow(Strings.Ui("UpStrength"), Strings.Ui("UpStrengthHelp"), 1, 200, _draft.UpCrossMin, Settings.DefaultUpCrossMin, v => _draft.UpCrossMin = v),
            NumericRow(Strings.Ui("UpSlack"), Strings.Ui("UpSlackHelp"), 0, 200, _draft.UpCrossSlack, Settings.DefaultUpCrossSlack, v => _draft.UpCrossSlack = v),
            NumericRow(Strings.Ui("SideStrength"), Strings.Ui("SideStrengthHelp"), 1, 200, _draft.SideCrossMin, Settings.DefaultSideCrossMin, v => _draft.SideCrossMin = v),
            NumericRow(Strings.Ui("SideSlackM2"), Strings.Ui("SideSlackHelpM2"), 0, 200, _draft.SideCrossSlack, Settings.DefaultSideCrossSlack, v => _draft.SideCrossSlack = v),
            NumericRow(Strings.Ui("PassageInset"), Strings.Ui("PassageInsetHelp"), 0, 500, _draft.PassageInsetPx, Settings.DefaultPassageInsetPx, v => _draft.PassageInsetPx = v),
            NumericRow(Strings.Ui("EdgeHoldback"), Strings.Ui("EdgeHoldbackHelpV2"), 0, 200, _draft.EdgeHoldbackPx, Settings.DefaultEdgeHoldbackPx, v => _draft.EdgeHoldbackPx = v),
            NumericRow(Strings.Ui("SeamTolerance"), Strings.Ui("SeamToleranceHelp"), 0, 200, _draft.SeamTolerancePx, Settings.DefaultSeamTolerancePx, v => _draft.SeamTolerancePx = v),
        };
        for (int i = 0; i < numericRows.Length; i++) { numericGrid.RowStyles.Add(new RowStyle(SizeType.AutoSize)); numericGrid.Controls.Add(numericRows[i], i % 2, i / 2); }
        flow.Controls.Add(numericGrid);

        AddSectionHeading(flow, Strings.Ui("SafetyHead"));
        var safety = new ThemedCheckBox { Text = Strings.Ui("SafetyEscapes"), Width = 790, Height = 32, Checked = _draft.SafetyEscapes };
        safety.CheckedChanged += (_, _) => { _draft.SafetyEscapes = safety.Checked; RefreshMap(); };
        flow.Controls.Add(safety); flow.Controls.Add(HelpLabel(Strings.Ui("SafetyEscapesHelp"), 790));
        var descent = new ThemedCheckBox { Text = Strings.Ui("GuideDownward"), Width = 790, Height = 32, Checked = _draft.DescentRouting };
        descent.CheckedChanged += (_, _) => _draft.DescentRouting = descent.Checked;
        flow.Controls.Add(descent); flow.Controls.Add(HelpLabel(Strings.Ui("GuideDownwardHelp"), 790));

        AddSectionHeading(flow, Strings.Ui("UpperGroupHead"));
        var auto = new RadioButton { Text = Strings.Ui("UpperAutomatic"), Width = 790, Height = 28, Checked = _draft.Mode != "Manual" };
        int chooseCount = _monitors.Count(m => !m.Primary && !_draft.ExcludedDevices.Contains(m.StableId, StringComparer.OrdinalIgnoreCase));
        var manual = new RadioButton { Text = string.Format(Strings.Ui("UpperManualV2"), chooseCount), Width = 790, Height = 28, Checked = _draft.Mode == "Manual" };
        flow.Controls.Add(auto); flow.Controls.Add(manual);
        var manualList = new FlowLayoutPanel { Width = 790, AutoSize = true, FlowDirection = FlowDirection.TopDown, WrapContents = false, Padding = new Padding(24, 0, 0, 8) };
        foreach (var monitor in _monitors.Where(m => !m.Primary))
        {
            bool unusual = !MonitorInfo.IsAbovePrimary(monitor);
            var check = new ThemedCheckBox { Width = 750, Height = 32, Checked = _draft.ManualMonitors.Contains(monitor.StableId, StringComparer.OrdinalIgnoreCase),
                Enabled = !_draft.ExcludedDevices.Contains(monitor.StableId, StringComparer.OrdinalIgnoreCase),
                Text = ScreenName(monitor) + (unusual ? " · " + Strings.Ui("UnusualUpper") : "") };
            check.CheckedChanged += (_, _) => { if (check.Checked) AddUnique(_draft.ManualMonitors, monitor.StableId); else _draft.ManualMonitors.RemoveAll(x => Same(x, monitor.StableId)); RefreshMap(); };
            manualList.Controls.Add(check);
        }
        auto.CheckedChanged += (_, _) => { if (auto.Checked) { _draft.Mode = "AutoTop"; manualList.Enabled = false; RefreshMap(); } };
        manual.CheckedChanged += (_, _) => { if (manual.Checked) { _draft.Mode = "Manual"; manualList.Enabled = true; RefreshMap(); } };
        manualList.Enabled = manual.Checked; flow.Controls.Add(manualList);

        AddSectionHeading(flow, Strings.Ui("ResetBehavior"));
        flow.Controls.Add(HelpLabel(Strings.Ui("ResetBehaviorHelp"), 790, 52));
        var reset = new ThemedButton { Text = Strings.Ui("ResetBehavior"), Width = 286, Height = 36, Role = ThemedButtonRole.DangerText }; reset.Click += (_, _) =>
        {
            _draft.ResetLayoutConfig(); SetInlineStatus(Strings.Ui("ResetDone")); RefreshMap(); RebuildAdvancedContents();
        };
        flow.Controls.Add(reset);
        return flow;
    }

    private void ResizeAdvanced(int width)
    {
        if (_advancedPanel == null) return;
        _advancedWidth = Math.Max(DpiPx(420), width);
        _advancedPanel.Width = _advancedWidth;
        foreach (Control child in _advancedPanel.Controls)
        {
            child.Margin = new Padding(0, child.Margin.Top, 0, child.Margin.Bottom);
            child.Width = _advancedWidth;
            if (child is FlowLayoutPanel manualList)
            {
                foreach (Control option in manualList.Controls)
                    option.Width = Math.Max(DpiPx(200), _advancedWidth - manualList.Padding.Horizontal);
            }
        }
        _advancedPanel.PerformLayout();
    }

    private void BuildShortcutsTab(TabPage page)
    {
        var flow = PageFlow(page);
        flow.Controls.Add(PageHeader(Strings.Ui("NavShortcuts"), Strings.Ui("ShortcutSubtitle"), 800));
        var header = new TableLayoutPanel { Width = 800, Height = 32, ColumnCount = 3, Margin = Padding.Empty, Tag = "listHeader" };
        header.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100)); header.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 238)); header.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 72));
        header.Controls.Add(new Label { Text = Strings.Ui("ShortcutActionColumn"), Dock = DockStyle.Fill, Tag = "faint", TextAlign = ContentAlignment.MiddleLeft }, 0, 0);
        header.Controls.Add(new Label { Text = Strings.Ui("ShortcutColumn"), Dock = DockStyle.Fill, Tag = "faint", TextAlign = ContentAlignment.MiddleLeft }, 1, 0);
        header.Controls.Add(new Label { Text = Strings.Clear, Dock = DockStyle.Fill, Tag = "faint", TextAlign = ContentAlignment.MiddleCenter }, 2, 0);
        flow.Controls.Add(header);
        var specs = new[]
        {
            (Strings.HotkeyHead, Strings.HotkeyHint), (Strings.GameHotkeyHead, Strings.GameHotkeyHint),
            (Strings.PauseHotkeyHead, Strings.PauseHotkeyHint), (Strings.SideHotkeyHead, Strings.SideHotkeyHint),
        };
        for (int i = 0; i < specs.Length; i++)
        {
            int index = i;
            var row = new TableLayoutPanel { Width = 800, Height = 83, ColumnCount = 3, Margin = Padding.Empty, Tag = "listRow", Padding = new Padding(0, 8, 0, 8) };
            row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 238)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 72));
            row.Paint += PaintListSeparator;
            var copy = new Panel { Dock = DockStyle.Fill };
            var required = i == 0 ? Strings.Ui("RequiredStatus") : Strings.Ui("OptionalStatus");
            var head = new Label { Text = specs[i].Item1 + "  ·  " + required, Left = 0, Top = 4, Width = 440, Height = 22, Font = new Font("Segoe UI Semibold", 9.5f) };
            var help = new Label { Text = specs[i].Item2 + (i == 1 ? " " + Strings.Ui("GameModeSuffix") : ""), Left = 0, Top = 27, Width = 445, Height = 36, Tag = "subtle" };
            copy.Controls.Add(head); copy.Controls.Add(help);
            var box = new TextBox { ReadOnly = true, TextAlign = HorizontalAlignment.Center, Cursor = Cursors.Hand, Font = new Font("Segoe UI Semibold", 9f), BorderStyle = BorderStyle.None };
            box.Text = HotkeyText(index); box.KeyDown += (_, e) => CaptureHotkey(index, e);
            var frame = new ThemedControlFrame { Dock = DockStyle.Fill, Height = 36, Margin = new Padding(0, 14, 8, 14), Tag = "shortcutFrame" }; frame.Host(box);
            var clear = new ThemedButton { Text = Strings.Clear, Dock = DockStyle.Fill, Height = 36, Role = ThemedButtonRole.Quiet, Margin = new Padding(4, 14, 0, 14) };
            clear.Click += (_, _) => { ClearHotkey(index); box.Text = HotkeyText(index); UpdateShortcutErrors(); };
            if (i == 0) { clear.Enabled = false; clear.AccessibleName = Strings.Ui("RequiredClearReason"); _tips.SetToolTip(clear, Strings.Ui("RequiredClearReason")); }
            row.Controls.Add(copy, 0, 0); row.Controls.Add(frame, 1, 0); row.Controls.Add(clear, 2, 0);
            flow.Controls.Add(row); _hotkeyBoxes[i] = box;
        }
        flow.Controls.Add(HelpLabel(Strings.Ui("ShortcutsHealthy"), 800, 32));
    }

    private (ComboBox Language, ComboBox Theme, CheckBox StartGate, CheckBox StartSide, CheckBox AutoStart, CheckBox StartConfine, CheckBox StartPaused) BuildGeneralTab(TabPage page)
    {
        var flow = PageFlow(page); flow.Controls.Add(PageHeader(Strings.Ui("NavGeneral"), Strings.Ui("GeneralSubtitle"), 800));
        AddSectionHeading(flow, Strings.StartupHead);
        var gate = new ThemedCheckBox { Text = Strings.StartGateClosed, Width = 800, Height = 32, Checked = _draft.StartGateClosed };
        var side = new ThemedCheckBox { Text = Strings.StartSideContainOn, Width = 800, Height = 32, Checked = _draft.StartSideContainOn };
        var auto = new ThemedCheckBox { Text = Strings.StartWithWindows, Width = 800, Height = 32, Checked = _draft.AutoStart };
        flow.Controls.Add(gate); flow.Controls.Add(side); flow.Controls.Add(auto);
        _tips.SetToolTip(gate, Strings.StartGateClosedTip); _tips.SetToolTip(side, Strings.StartSideContainTip); _tips.SetToolTip(auto, Strings.StartWithWindowsTip);
        var more = new ThemedButton { Text = "›  " + Strings.Ui("MoreStartupChoices"), Width = 800, Height = 36, Role = ThemedButtonRole.Quiet, TextAlign = ContentAlignment.MiddleLeft };
        var morePanel = new Panel { Width = 800, Height = 206, Visible = false, Tag = "surface" };
        var confine = new ThemedCheckBox { Text = Strings.Ui("StartConfineOn"), Left = 0, Top = 0, Width = 790, Height = 32, Checked = _draft.StartConfineOn };
        var confineHelp = new Label { Text = Strings.Ui("StartConfineHelp"), Left = 28, Top = 31, Width = 760, Height = 30, Tag = "subtle" };
        var paused = new ThemedCheckBox { Text = Strings.Ui("StartPaused"), Left = 0, Top = 64, Width = 790, Height = 32, Checked = _draft.StartPaused };
        var pausedHelp = new Label { Text = Strings.Ui("StartPausedHelp"), Left = 28, Top = 95, Width = 760, Height = 38, Tag = "subtle" };
        var warning = new Label { Left = 0, Top = 138, Width = 790, Height = 64, Tag = "warning", Text = Strings.Ui("StartupConfineWarning") };
        void UpdateWarning() { bool active = confine.Checked && auto.Checked; warning.Enabled = active; warning.Text = active ? Strings.Ui("StartupConfineWarning") : ""; }
        confine.CheckedChanged += (_, _) => UpdateWarning(); auto.CheckedChanged += (_, _) => UpdateWarning(); UpdateWarning();
        morePanel.Controls.Add(confine); morePanel.Controls.Add(confineHelp); morePanel.Controls.Add(paused); morePanel.Controls.Add(pausedHelp); morePanel.Controls.Add(warning);
        more.Click += (_, _) => { morePanel.Visible = !morePanel.Visible; more.Text = (morePanel.Visible ? "⌄  " : "›  ") + Strings.Ui("MoreStartupChoices"); };
        flow.Controls.Add(more); flow.Controls.Add(morePanel);
        AddSectionHeading(flow, Strings.AppearanceHead);
        var row = new TableLayoutPanel { Width = 800, Height = 78, ColumnCount = 2, Tag = "appearanceGrid" };
        row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        var langLabel = new Label { Text = Strings.LanguageLabel, Left = 0, Top = 0, Width = 310, Height = 24, Tag = "subtle" };
        var themeLabel = new Label { Text = Strings.ThemeLabel, Left = 0, Top = 0, Width = 310, Height = 24, Tag = "subtle" };
        var lang = new ThemedComboBox { Left = 0, Top = 28, Width = 370, Height = 36 };
        lang.Items.AddRange(new object[] { Strings.LangAuto, Strings.LangEnglish, Strings.LangTurkish }); lang.SelectedIndex = _draft.Language switch { "en" => 1, "tr" => 2, _ => 0 };
        var theme = new ThemedComboBox { Left = 0, Top = 28, Width = 370, Height = 36 };
        theme.Items.AddRange(new object[] { Strings.ThemeSystem, Strings.ThemeLight, Strings.ThemeDark }); theme.SelectedIndex = _draft.Theme switch { "Light" => 1, "Dark" => 2, _ => 0 };
        var left = new Panel { Dock = DockStyle.Fill }; left.Controls.Add(lang); left.Controls.Add(langLabel);
        var right = new Panel { Dock = DockStyle.Fill }; right.Controls.Add(theme); right.Controls.Add(themeLabel);
        row.Controls.Add(left, 0, 0); row.Controls.Add(right, 1, 0); flow.Controls.Add(row);
        theme.SelectedIndexChanged += (_, _) => { _draft.Theme = theme.SelectedIndex switch { 1 => "Light", 2 => "Dark", _ => "System" }; _theme = Theming.Resolve(_draft.Theme); ApplyTheme(); };
        AddSectionHeading(flow, Strings.Ui("BackupHead"));
        flow.Controls.Add(HelpLabel(Strings.Ui("BackupHelpV2"), 800, 44));
        var backupRow = new FlowLayoutPanel { Width = 800, Height = 48, FlowDirection = FlowDirection.LeftToRight };
        var export = new ThemedButton { Text = Strings.ExportLabel, Width = 170, Height = 36, Role = ThemedButtonRole.Secondary }; export.Click += Export_Click;
        var import = new ThemedButton { Text = Strings.ImportLabel, Width = 170, Height = 36, Role = ThemedButtonRole.Secondary }; import.Click += Import_Click;
        backupRow.Controls.Add(export); backupRow.Controls.Add(import); flow.Controls.Add(backupRow);
        return (lang, theme, gate, side, auto, confine, paused);
    }

    private FlowLayoutPanel PageFlow(TabPage page)
    {
        var flow = new FlowLayoutPanel { Dock = DockStyle.Top, AutoSize = true, FlowDirection = FlowDirection.TopDown, WrapContents = false, Padding = Padding.Empty };
        page.Controls.Add(flow); return flow;
    }

    private Control DefaultRow(string label, BoundaryDefault value, Action<BoundaryDefault> set, bool automatic)
    {
        var row = new TableLayoutPanel { Width = 790, Height = 44, ColumnCount = 2, Margin = Padding.Empty };
        row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 250));
        row.Controls.Add(new Label { Text = label, Dock = DockStyle.Fill, TextAlign = ContentAlignment.MiddleLeft }, 0, 0);
        var combo = new ThemedComboBox { Dock = DockStyle.Fill, Height = 36, Margin = new Padding(0, 4, 0, 4) };
        if (automatic) combo.Items.Add(Strings.Ui("DefaultAutomatic"));
        combo.Items.AddRange(new object[] { Strings.Ui("DefaultFree"), Strings.Ui("DefaultPush"), Strings.Ui("DefaultBlocked") });
        var values = automatic ? new[] { BoundaryDefault.Automatic, BoundaryDefault.Free, BoundaryDefault.Push, BoundaryDefault.Blocked }
                               : new[] { BoundaryDefault.Free, BoundaryDefault.Push, BoundaryDefault.Blocked };
        combo.SelectedIndex = Math.Max(0, Array.IndexOf(values, value));
        combo.SelectedIndexChanged += (_, _) => { if (combo.SelectedIndex >= 0) { set(values[combo.SelectedIndex]); RefreshMap(); } };
        row.Controls.Add(combo, 1, 0); return row;
    }

    private Control NumericRow(string label, string help, int min, int max, int value, int defaultValue, Action<int> set)
    {
        int helpHeight = Math.Max(36, TextRenderer.MeasureText(help, Font, new Size(335, 0), TextFormatFlags.WordBreak).Height + 4);
        const int labelWidth = 190;
        int labelHeight = Math.Max(28, TextRenderer.MeasureText(label, Font, new Size(labelWidth, 0),
            TextFormatFlags.WordBreak | TextFormatFlags.NoPrefix | TextFormatFlags.NoPadding).Height + 4);
        int helpTop = Math.Max(42, 5 + labelHeight + 4);
        var panel = new Panel { Width = 385, Height = helpTop + helpHeight, Tag = "advancedNumeric", Margin = new Padding(0, 0, 10, 8) };
        panel.Controls.Add(new Label { Text = label, Left = 0, Top = 5, Width = labelWidth, Height = labelHeight });
        var number = new ThemedNumericUpDown { Dock = DockStyle.Fill, Minimum = min, Maximum = max, Value = Math.Clamp(value, min, max) };
        number.ValueChanged += (_, _) => { set((int)number.Value); RefreshMap(); };
        var numberHost = new Panel { Left = 194, Top = 2, Width = 82, Height = 36, Tag = "surface2" };
        numberHost.Controls.Add(number);
        var spinnerRail = new ThemedSpinnerRail { Owner = number, Dock = DockStyle.Right, Width = 20 };
        numberHost.Controls.Add(spinnerRail); spinnerRail.BringToFront();
        var reset = new ThemedButton { Text = Strings.Ui("UseDefault"), Width = 98, Height = 32, Left = 282, Top = 2, Role = ThemedButtonRole.Quiet, Tag = "numericReset" }; reset.Click += (_, _) => number.Value = defaultValue;
        panel.Controls.Add(numberHost); panel.Controls.Add(reset);
        panel.Controls.Add(new Label { Text = help, Left = 0, Top = helpTop, Width = 375, Height = helpHeight, Tag = "subtle" });
        return panel;
    }

    private void OpenBoundary(BoundaryView boundary)
    {
        if (boundary == null) return;
        _boundary = boundary; _map.SelectedBoundary = boundary;
        ConfigureEndpoints();
        _draft.BoundaryTipSeen = true;
        if (_inlineStatus.Text == Strings.Ui("BoundaryTip")) SetInlineStatus(Strings.Ui("UnsavedFooter"));
        _screenSplit.Visible = true;
        LayoutBoundaryDrawer();
        UpdateDrawer();
        BeginInvoke((Action)(() =>
        {
            _bodyFlow.ScrollControlIntoView(_screenSplit);
            Rectangle hint = _bodyFlow.RectangleToClient(_mapHint.RectangleToScreen(_mapHint.ClientRectangle));
            if (hint.Top < 0) _bodyFlow.VerticalScroll.Value = Math.Max(0, _bodyFlow.VerticalScroll.Value + hint.Top);
        }));
    }

    private void ConfigureEndpoints()
    {
        if (_boundary.VerticalConnection)
        {
            bool firstUp = _boundary.FirstToSecondDirection == BoundaryDirection.Up;
            _primary = firstUp
                ? new(_boundary.First, _boundary.Second, BoundaryDirection.Up, _boundary.FirstToSecond)
                : new(_boundary.Second, _boundary.First, BoundaryDirection.Up, _boundary.SecondToFirst);
            _reverse = firstUp
                ? new(_boundary.Second, _boundary.First, BoundaryDirection.Down, _boundary.SecondToFirst)
                : new(_boundary.First, _boundary.Second, BoundaryDirection.Down, _boundary.FirstToSecond);
        }
        else
        {
            _primary = new(_boundary.First, _boundary.Second, _boundary.FirstToSecondDirection, _boundary.FirstToSecond);
            _reverse = new(_boundary.Second, _boundary.First, _boundary.SecondToFirstDirection, _boundary.SecondToFirst);
        }
    }

    private void UpdateDrawer()
    {
        if (_boundary == null) return;
        _rebuilding = true;
        _endpointFrom.Text = ChipText(_primary.From);
        _endpointTo.Text = ChipText(_primary.To);
        _pairSeparator.Text = _boundary.VerticalConnection ? "↕" : "↔";
        _drawerTitle.Text = Strings.Ui("DrawerTitle");
        _drawerMeta.Text = _boundary.GapPx > _draft.SeamTolerancePx
            ? string.Format(Strings.Ui("EditorMeetGap"), _boundary.GapPx) : Strings.Ui("EditorMeetTouching");
        bool horizontal = !_boundary.VerticalConnection;
        bool splitTruth = horizontal && DirectionsDiffer();
        if (splitTruth) _separateExpanded = true;
        _drawerDirection.Text = splitTruth ? Strings.Ui("DifferentByDirection") : horizontal ? Strings.Ui("BothDirections") : string.Format(Strings.Ui("GoingUpInto"), ScreenName(_primary.To));
        _returnLine.Text = horizontal ? "" : ResolvedBehavior(_reverse) switch
        {
            BoundaryBehavior.Blocked => Strings.Ui("ComingDownBlocked"),
            BoundaryBehavior.Push => Strings.Ui("ComingDownPush"),
            _ => Strings.Ui("ComingDownFree"),
        };
        _separateLink.Enabled = horizontal;
        if (!horizontal) _separateExpanded = false;
        _separateLink.Text = (_separateExpanded ? "⌄  " : "›  ") + (splitTruth ? Strings.Ui("DifferentByDirection") : Strings.Ui("SetDirectionsSeparately"));

        var excludedTarget = ExcludedBoundaryTarget();
        bool excluded = excludedTarget != null;
        _allowScreen.Enabled = excluded;
        _allowScreen.Role = excluded ? ThemedButtonRole.Primary : ThemedButtonRole.Quiet;
        _allowScreen.AccessibleDescription = excluded
            ? string.Format(Strings.Ui("ExcludedTargetExplanation"), ScreenName(excludedTarget)) : "";
        _allowScreen.Invalidate();
        _modeTiles.Enabled = !excluded;
        _overallLink.Enabled = !excluded;
        _separateLink.Enabled = horizontal && !excluded;
        _separatePanel.Enabled = !excluded;
        _whenLink.Enabled = !excluded;
        _automaticControl.Enabled = !excluded;
        _alwaysControl.Enabled = !excluded;
        _returnLine.Enabled = !excluded;
        if (excluded) _drawerDirection.Text = Strings.Ui("BlockedByKeepOut");

        var selected = ResolvedBehavior(_primary);
        _freeTile.Selected = !excluded && !splitTruth && selected == BoundaryBehavior.Free;
        _pushTile.Selected = !excluded && !splitTruth && selected is BoundaryBehavior.Push or BoundaryBehavior.OverallFeel;
        _blockedTile.Selected = !excluded && !splitTruth && selected == BoundaryBehavior.Blocked;
        var primaryRule = FindRule(_primary);
        _overallLink.Text = primaryRule == null ? Strings.Ui("UsingOverallBehavior") : Strings.Ui("UseOverallBehavior");
        _overallLink.Enabled = primaryRule != null && !excluded;
        var control = primaryRule?.Control ?? BoundaryControl.Automatic;
        _automaticControl.Checked = control == BoundaryControl.Automatic;
        _alwaysControl.Checked = control == BoundaryControl.Always;
        BuildSeparateRows(excluded);
        UpdateLiveStatus();
        ReserveDrawerStatusHeight();
        _rebuilding = false;
        LayoutBoundaryDrawer();
        ApplyTheme();
    }

    private void BuildSeparateRows(bool suppressSelection = false)
    {
        _separatePanel.SuspendLayout(); _separatePanel.Controls.Clear(); _separatePanel.RowStyles.Clear(); _separatePanel.RowCount = 0;
        if (!_separateExpanded) { _separatePanel.Visible = false; _separatePanel.ResumeLayout(); return; }
        _separatePanel.Visible = true;
        AddDirectionRow(_primary, suppressSelection); AddDirectionRow(_reverse, suppressSelection);
        _separatePanel.ResumeLayout();
    }

    private void AddDirectionRow(BoundaryEndpoint endpoint, bool suppressSelection)
    {
        int row = _separatePanel.RowCount++;
        _separatePanel.RowStyles.Add(new RowStyle(SizeType.Absolute, 78));
        var panel = new Panel { Dock = DockStyle.Fill, Tag = "surface2" };
        panel.Controls.Add(new Label { Text = string.Format(Strings.Ui("DirectionFromTo"), ScreenName(endpoint.From), ScreenName(endpoint.To)), Left = 0, Top = 0, Width = 190, Height = 72, Font = new Font("Segoe UI Semibold", 8.75f), TextAlign = ContentAlignment.MiddleLeft });
        var modes = new[] { BoundaryBehavior.Free, BoundaryBehavior.Push, BoundaryBehavior.Blocked };
        for (int i = 0; i < modes.Length; i++)
        {
            var behavior = modes[i];
            var tile = NewModeTile(behavior, BehaviorTitle(behavior), "", true);
            tile.Left = 198 + i * 195; tile.Top = 5; tile.Width = 188; tile.Height = 66;
            tile.Selected = !suppressSelection && ResolvedBehavior(endpoint) == behavior;
            tile.Click += (_, _) => { SetEndpointBehavior(endpoint, behavior); RefreshMap(); UpdateDrawer(); };
            panel.Controls.Add(tile);
        }
        _separatePanel.Controls.Add(panel, 0, row);
    }

    private ModeTile NewModeTile(BoundaryBehavior behavior, string title, string help, bool compact = false)
    {
        var tile = new ModeTile
        {
            Behavior = behavior, Title = title, Help = help,
            Width = compact ? 188 : 254, Height = compact ? 66 : 78,
            Margin = compact ? new Padding(0, 0, 7, 0) : new Padding(0, 0, 8, 0),
        };
        tile.SetTheme(_theme); tile.Click += (_, _) => { if (!compact) ChoosePrimaryBehavior(behavior); }; return tile;
    }

    private void ChoosePrimaryBehavior(BoundaryBehavior behavior)
    {
        if (_boundary == null) return;
        SetEndpointBehavior(_primary, behavior);
        if (!_boundary.VerticalConnection) { SetEndpointBehavior(_reverse, behavior); _separateExpanded = false; }
        else if (_boundary.VerticalConnection && FindRule(_reverse) == null) SetEndpointBehavior(_reverse, BoundaryBehavior.Free);
        RefreshMap(); UpdateDrawer();
    }

    private void SetEndpointBehavior(BoundaryEndpoint endpoint, BoundaryBehavior behavior)
    {
        var rule = FindRule(endpoint);
        if (rule == null)
        {
            rule = new BoundaryRule { FromStableId = endpoint.From.StableId, ToStableId = endpoint.To.StableId,
                Direction = endpoint.Direction, Control = BoundaryControl.Automatic };
            _draft.BoundaryRules.Add(rule);
        }
        rule.Behavior = behavior;
    }

    private void SetControl(BoundaryControl control)
    {
        var targets = !_boundary.VerticalConnection && !_separateExpanded ? new[] { _primary, _reverse } : new[] { _primary };
        foreach (var endpoint in targets)
        {
            var rule = FindRule(endpoint);
            if (rule != null) rule.Control = control;
        }
        RefreshMap(); UpdateLiveStatus();
    }

    private void UseOverall()
    {
        RemoveRule(_primary);
        if (!_boundary.VerticalConnection && !_separateExpanded) RemoveRule(_reverse);
        RefreshMap(); UpdateDrawer();
    }

    private void ToggleSeparate()
    {
        if (_boundary == null || _boundary.VerticalConnection) return;
        _separateExpanded = !_separateExpanded; BuildSeparateRows();
    }

    private void ToggleWhen()
    {
        _whenExpanded = !_whenExpanded; _whenPanel.Visible = _whenExpanded;
    }

    private void AllowBoundaryScreen()
    {
        if (_boundary == null) return;
        _draft.ExcludedDevices.RemoveAll(id => Same(id, _primary.To.StableId) || Same(id, _primary.From.StableId));
        RebuildScreenRows(); RefreshMap(); UpdateDrawer();
    }

    private void UpdateLiveStatus()
    {
        if (_boundary == null || _drawerStatus == null) return;
        string status = "";
        var excludedTarget = ExcludedBoundaryTarget();
        if (excludedTarget != null)
            status = string.Format(Strings.Ui("ExcludedTargetExplanation"), ScreenName(excludedTarget));
        else if (_primary.Runtime?.SafetyDemoted == true) status = string.Format(Strings.Ui("StatusSafety"), ScreenName(_primary.From));
        else if (_runtimeState != null)
        {
            var state = _runtimeState();
            var rule = FindRule(_primary);
            var chosen = ResolvedBehavior(_primary);
            if (rule?.Control != BoundaryControl.Always && _primary.Direction == BoundaryDirection.Up && !state.UpwardPassageOpen && chosen != BoundaryBehavior.Blocked)
                status = Strings.Ui("StatusUpClosed");
            else if (rule?.Control != BoundaryControl.Always && _primary.Direction is BoundaryDirection.Left or BoundaryDirection.Right && !state.SideProtectionOn && chosen != BoundaryBehavior.Free)
                status = Strings.Ui("StatusSideOff");
        }
        _drawerStatus.Text = status;
        _drawerStatus.AccessibleName = status;
    }

    private MonitorInfo ExcludedBoundaryTarget()
    {
        if (_boundary == null || _primary == null) return null;
        if (_draft.ExcludedDevices.Contains(_primary.To.StableId, StringComparer.OrdinalIgnoreCase))
            return _primary.To;
        if (_draft.ExcludedDevices.Contains(_primary.From.StableId, StringComparer.OrdinalIgnoreCase))
            return _primary.From;
        return null;
    }

    private void RefreshMap()
    {
        _map?.SetModel(_monitors, _draft, _draft.MonitorNames);
        if (_mapHint != null && _monitors.Count(m => !_draft.ExcludedDevices.Contains(m.StableId, StringComparer.OrdinalIgnoreCase)) > 1)
        {
            _mapHint.Text = Strings.Ui("MapHintV2");
            UpdateBoundaryTipVisibility();
        }
        if (_boundary != null)
        {
            _boundary = _map.FindBoundary(_primary.From.StableId, _primary.To.StableId, _primary.Direction);
            if (_boundary != null) { ConfigureEndpoints(); _map.SelectedBoundary = _boundary; }
        }
    }

    private void CloseDrawer()
    {
        if (!_screenSplit.Visible) return;
        _screenSplit.Visible = false; _screenSplit.Height = 0; _boundary = null; _map.SelectedBoundary = null; _map.Focus();
    }

    private void LayoutBoundaryDrawer()
    {
        if (_screenSplit == null || !_screenSplit.Visible || _screenSplit.Width <= 0) return;
        int contentWidth = Math.Max(DpiPx(420), _bodyFlow.ClientSize.Width - _bodyFlow.Padding.Horizontal - SystemInformation.VerticalScrollBarWidth - DpiPx(4));
        PrepareBoundaryDrawerWidth(contentWidth);
        int width = Math.Max(DpiPx(430), _screenSplit.ClientSize.Width - DpiPx(28));
        int preferred = _drawerFlow.GetPreferredSize(new Size(width, 0)).Height + _drawer.Padding.Vertical + DpiPx(2);
        _screenSplit.Height = Math.Max(DpiPx(245), preferred);
    }

    private void PrepareBoundaryDrawerWidth(int contentWidth)
    {
        _screenSplit.Width = contentWidth;
        int width = Math.Max(DpiPx(430), _screenSplit.ClientSize.Width - DpiPx(28));
        foreach (Control child in _drawerFlow.Controls) child.Width = width;
        LayoutDrawerQuestionText();
        _modeTiles.Width = width;
        int tileWidth = Math.Max(DpiPx(150), (width - DpiPx(16)) / 3);
        foreach (ModeTile tile in _modeTiles.Controls) tile.Width = tileWidth;
        _separatePanel.Width = width; _whenPanel.Width = width;
    }

    private int RestingBoundaryDrawerHeight(int contentWidth)
    {
        PrepareBoundaryDrawerWidth(contentWidth);
        ReserveDrawerStatusHeight();
        int height = _drawer.Padding.Vertical + DpiPx(2);
        foreach (Control child in _drawerFlow.Controls)
        {
            if (child == _separatePanel || child == _whenPanel) continue;
            height += child.Height + child.Margin.Vertical;
        }
        return Math.Max(DpiPx(245), height);
    }

    private int DpiPx(int logicalPixels) => (int)Math.Round(logicalPixels * DeviceDpi / 96f);

    private void MonitorChosen(MonitorInfo monitor)
    {
        if (monitor == null) return;
        // Clicking a screen only SELECTS/highlights it — identification (the on-screen overlay) is
        // triggered exclusively by the "Identify all screens" button (owner request 2026-07-15).
        _map.SelectedStableId = monitor.StableId;
    }

    private void HoverRow(string stableId)
    {
        foreach (Control panel in _screenRows.Controls) panel.Tag = "screenRow";
    }

    private void ToggleAdvanced(bool? force = null)
    {
        _advancedExpanded = force ?? !_advancedExpanded;
        var numericGrid = _advancedPanel?.Controls.Cast<Control>().FirstOrDefault(c => (c.Tag as string) == "numericGrid");
        if (numericGrid != null) numericGrid.Visible = _advancedExpanded;
        _advancedToggle.Text = (_advancedExpanded ? "⌄  " : "›  ") + Strings.Ui("FineTunePixels");
        if (_advancedExpanded)
        {
            _advancedPanel.PerformLayout();
            _advancedPage.ScrollControlIntoView(_advancedToggle);
        }
    }

    private void Save_Click(object sender, EventArgs e)
    {
        SyncSimpleControls();
        _draft.Normalize();
        if (!ValidPrimaryHotkey()) { ShowInline(Strings.ValidationPickModifier); return; }
        if (HasDuplicateHotkeys()) { UpdateShortcutErrors(); return; }
        var issues = _draft.ValidateNoSeal(_monitors);
        if (_draft.HasRepairableImportErrors || issues.Length > 0)
        {
            if (issues.Length > 0)
            {
                _pendingIssue = issues[0];
                ShowInline(string.Format(Strings.Ui("InlineSealError"), ScreenName(issues[0].StableId)), true);
            }
            else ShowInline(Strings.ImportInvalid);
            return;
        }
        Result = _draft.DeepClone();
        DialogResult = DialogResult.OK;
    }

    private BoundaryValidationIssue _pendingIssue;
    private void OpenValidationBoundary(object sender, EventArgs e)
    {
        if (_pendingIssue == null) return;
        var boundary = _map.FindBoundary(_pendingIssue.StableId, _pendingIssue.FocusToStableId, _pendingIssue.FocusDirection);
        if (boundary != null) { _tabs.SelectedIndex = 0; OpenBoundary(boundary); }
    }

    private void ShowInline(string text, bool action = false)
    {
        SetInlineStatus(text, true, action);
    }

    private void SetInlineStatus(string text, bool error = false, bool action = false)
    {
        if (_inlineStatus == null) return;
        _inlineStatusIsError = error;
        _inlineStatus.Text = text ?? "";
        _inlineStatus.ForeColor = error ? _theme.Danger : _theme.Faint;
        _inlineAction.Enabled = action;
        _inlineAction.Visible = action;
        _inlineAction.Text = action ? Strings.Ui("OpenBoundary") : "";
    }

    private void UpdateBoundaryTipVisibility()
    {
        if (_inlineStatus == null || _tabs == null) return;
        bool showingTip = _inlineStatus.Text == Strings.Ui("BoundaryTip");
        if (_tabs.SelectedIndex != 0)
        {
            if (showingTip) SetInlineStatus("");
            return;
        }
        if (_inlineStatus.Text.Length == 0 && !_draft.BoundaryTipSeen && !_draft.BoundaryRules.Any() &&
            _monitors.Count(m => !_draft.ExcludedDevices.Contains(m.StableId, StringComparer.OrdinalIgnoreCase)) > 1)
            SetInlineStatus(Strings.Ui("BoundaryTip"));
    }

    private void SyncSimpleControls()
    {
        _draft.StartGateClosed = _startGate.Checked; _draft.StartSideContainOn = _startSide.Checked; _draft.AutoStart = _autoStart.Checked;
        _draft.StartConfineOn = _startConfine.Checked; _draft.StartPaused = _startPaused.Checked;
        _draft.Language = _langCombo.SelectedIndex switch { 1 => "en", 2 => "tr", _ => "auto" };
        _draft.Theme = _themeCombo.SelectedIndex switch { 1 => "Light", 2 => "Dark", _ => "System" };
    }

    private void Export_Click(object sender, EventArgs e)
    {
        SyncSimpleControls();
        if (!ValidateDraftForBackup()) return;
        string path = PickBackupPath();
        if (!string.IsNullOrEmpty(path)) WriteBackupFile(path);
    }

    private string PickBackupPath()
    {
        using var dialog = new SaveFileDialog { Title = Strings.ExportTitle, Filter = Strings.SettingsFileFilter,
            FileName = "MouseFence-settings.json", DefaultExt = "json", AddExtension = true };
        return dialog.ShowDialog(this) == DialogResult.OK ? dialog.FileName : null;
    }

    private bool ValidateDraftForBackup()
    {
        var issues = _draft.ValidateNoSeal(_monitors);
        if (issues.Length == 0) return true;
        _pendingIssue = issues[0];
        ShowInline(string.Format(Strings.Ui("InlineSealError"), ScreenName(issues[0].StableId)), true);
        return false;
    }

    private bool WriteBackupFile(string path)
    {
        try
        {
            File.WriteAllText(path, JsonSerializer.Serialize(_draft, new JsonSerializerOptions { WriteIndented = true }));
            SetInlineStatus(Strings.Ui("BackupDone"));
            return true;
        }
        catch { ShowInline(Strings.Ui("ExportFailed")); return false; }
    }

    private void Import_Click(object sender, EventArgs e)
    {
        string path = PickRestorePath();
        if (string.IsNullOrEmpty(path)) return;
        try
        {
            string json = File.ReadAllText(path);
            var imported = JsonSerializer.Deserialize<Settings>(json);
            if (imported == null) { ShowInline(Strings.ImportInvalid); return; }
            int version = Settings.ReadVersion(json); imported.Normalize();
            if (version < Settings.CurrentSettingsVersion) imported.MigrateLayoutKeysToStable(_monitors);
            var issues = imported.ValidateNoSeal(_monitors);
            if (imported.HasRepairableImportErrors || issues.Length > 0)
            {
                CopySettings(imported, _draft);
                LoadDraftIntoControls();
                if (issues.Length > 0) { _pendingIssue = issues[0]; ShowInline(string.Format(Strings.Ui("InlineSealError"), ScreenName(issues[0].StableId)), true); }
                else ShowInline(Strings.ImportInvalid);
                return;
            }
            bool confirmed = RestoreConfirmForTests?.Invoke() ?? MessageBox.Show(this, Strings.ImportConfirm, Strings.SettingsTitle,
                MessageBoxButtons.YesNo, MessageBoxIcon.Question) == DialogResult.Yes;
            if (!confirmed) return;
            CopySettings(imported, _draft);
            Result = _draft.DeepClone(); DialogResult = DialogResult.OK;
        }
        catch { ShowInline(Strings.ImportInvalid); }
    }

    private string PickRestorePath()
    {
        if (RestorePathPickerForTests != null) return RestorePathPickerForTests();
        using var dialog = new OpenFileDialog { Title = Strings.ImportTitle, Filter = Strings.SettingsFileFilter, CheckFileExists = true };
        return dialog.ShowDialog(this) == DialogResult.OK ? dialog.FileName : null;
    }

    private static void CopySettings(Settings source, Settings target)
    {
        var json = JsonSerializer.Serialize(source);
        var copy = JsonSerializer.Deserialize<Settings>(json);
        foreach (var property in typeof(Settings).GetProperties().Where(p => p.CanRead && p.CanWrite && p.SetMethod?.IsPublic == true))
            property.SetValue(target, property.GetValue(copy));
        target.Normalize();
    }

    private void LoadDraftIntoControls()
    {
        _rebuilding = true;
        _startGate.Checked = _draft.StartGateClosed; _startSide.Checked = _draft.StartSideContainOn; _autoStart.Checked = _draft.AutoStart;
        _startConfine.Checked = _draft.StartConfineOn; _startPaused.Checked = _draft.StartPaused;
        _langCombo.SelectedIndex = _draft.Language switch { "en" => 1, "tr" => 2, _ => 0 };
        _themeCombo.SelectedIndex = _draft.Theme switch { "Light" => 1, "Dark" => 2, _ => 0 };
        for (int i = 0; i < _hotkeyBoxes.Length; i++) _hotkeyBoxes[i].Text = HotkeyText(i);
        _rebuilding = false;
        RebuildScreenRows(); RebuildAdvancedContents(); RefreshMap(); CloseDrawer();
    }

    private void CaptureHotkey(int index, KeyEventArgs e)
    {
        e.SuppressKeyPress = true; e.Handled = true;
        Keys key = e.KeyCode is Keys.ControlKey or Keys.Menu or Keys.ShiftKey or Keys.LWin or Keys.RWin ? Keys.None : e.KeyCode;
        bool win = e.KeyCode is Keys.LWin or Keys.RWin;
        SetHotkey(index, key, e.Control, e.Alt, e.Shift, win);
        _hotkeyBoxes[index].Text = HotkeyText(index);
        UpdateShortcutErrors();
    }

    private void SetHotkey(int i, Keys key, bool ctrl, bool alt, bool shift, bool win)
    {
        switch (i)
        {
            case 0: _draft.HotKey = key; _draft.ModCtrl = ctrl; _draft.ModAlt = alt; _draft.ModShift = shift; _draft.ModWin = win; break;
            case 1: _draft.ConfineHotKey = key; _draft.ConfineModCtrl = ctrl; _draft.ConfineModAlt = alt; _draft.ConfineModShift = shift; _draft.ConfineModWin = win; break;
            case 2: _draft.PauseHotKey = key; _draft.PauseModCtrl = ctrl; _draft.PauseModAlt = alt; _draft.PauseModShift = shift; _draft.PauseModWin = win; break;
            case 3: _draft.SideHotKey = key; _draft.SideModCtrl = ctrl; _draft.SideModAlt = alt; _draft.SideModShift = shift; _draft.SideModWin = win; break;
        }
    }

    private void ClearHotkey(int i) => SetHotkey(i, Keys.None, false, false, false, false);
    private string HotkeyText(int i)
    {
        (Keys key, bool c, bool a, bool s, bool w) = i switch
        {
            0 => (_draft.HotKey, _draft.ModCtrl, _draft.ModAlt, _draft.ModShift, _draft.ModWin),
            1 => (_draft.ConfineHotKey, _draft.ConfineModCtrl, _draft.ConfineModAlt, _draft.ConfineModShift, _draft.ConfineModWin),
            2 => (_draft.PauseHotKey, _draft.PauseModCtrl, _draft.PauseModAlt, _draft.PauseModShift, _draft.PauseModWin),
            _ => (_draft.SideHotKey, _draft.SideModCtrl, _draft.SideModAlt, _draft.SideModShift, _draft.SideModWin),
        };
        var parts = new List<string>(); if (c) parts.Add("Ctrl"); if (a) parts.Add("Alt"); if (s) parts.Add("Shift"); if (w) parts.Add("Win"); if (key != Keys.None) parts.Add(key.ToString());
        return parts.Count == 0 ? Strings.HotkeyPressKeys : string.Join(" + ", parts);
    }

    private bool ValidPrimaryHotkey() => _draft.HotKey != Keys.None && (_draft.ModCtrl || _draft.ModAlt || _draft.ModShift || _draft.ModWin);

    private bool HasDuplicateHotkeys()
    {
        var chords = new[]
        {
            (_draft.HotKey, _draft.ModCtrl, _draft.ModAlt, _draft.ModShift, _draft.ModWin),
            (_draft.ConfineHotKey, _draft.ConfineModCtrl, _draft.ConfineModAlt, _draft.ConfineModShift, _draft.ConfineModWin),
            (_draft.PauseHotKey, _draft.PauseModCtrl, _draft.PauseModAlt, _draft.PauseModShift, _draft.PauseModWin),
            (_draft.SideHotKey, _draft.SideModCtrl, _draft.SideModAlt, _draft.SideModShift, _draft.SideModWin),
        }.Where(x => x.Item1 != Keys.None).ToList();
        return chords.Count != chords.Distinct().Count();
    }

    private void RebuildAdvancedContents()
    {
        bool visible = _advancedPanel.Visible;
        using var fresh = BuildAdvancedPanel();
        _advancedPanel.SuspendLayout(); _advancedPanel.Controls.Clear();
        while (fresh.Controls.Count > 0)
        {
            var child = fresh.Controls[0]; fresh.Controls.Remove(child); _advancedPanel.Controls.Add(child);
        }
        _advancedPanel.Visible = visible; _advancedPanel.ResumeLayout(true); ResizeAdvanced(_advancedWidth); ApplyTheme();
    }

    private void SanitizeExcluded()
    {
        var ids = _monitors.Select(m => m.StableId).ToList();
        string primary = (_monitors.FirstOrDefault(m => m.Primary) ?? _monitors.FirstOrDefault())?.StableId;
        _draft.ExcludedDevices = GuardCore.SanitizeExcluded(ids, primary,
            new HashSet<string>(_draft.ExcludedDevices, StringComparer.OrdinalIgnoreCase), out _);
    }

    private BoundaryRule FindRule(BoundaryEndpoint e) => _draft.BoundaryRules.LastOrDefault(r => Same(r.FromStableId, e.From.StableId) && Same(r.ToStableId, e.To.StableId) && r.Direction == e.Direction);
    private void RemoveRule(BoundaryEndpoint e) => _draft.BoundaryRules.RemoveAll(r => Same(r.FromStableId, e.From.StableId) && Same(r.ToStableId, e.To.StableId) && r.Direction == e.Direction);
    private BoundaryBehavior ResolvedBehavior(BoundaryEndpoint e)
    {
        var saved = FindRule(e)?.Behavior;
        if (saved == BoundaryBehavior.OverallFeel) return BoundaryBehavior.Push;
        if (saved is BoundaryBehavior.Free or BoundaryBehavior.Push or BoundaryBehavior.Blocked) return saved.Value;
        return e.Runtime?.Behavior ?? BoundaryBehavior.Free;
    }
    private bool DirectionsDiffer() => ResolvedBehavior(_primary) != ResolvedBehavior(_reverse) || FindRule(_primary)?.Control != FindRule(_reverse)?.Control;
    private string BehaviorTitle(BoundaryBehavior b) => b switch { BoundaryBehavior.Blocked => Strings.Ui("NeverCrosses"), BoundaryBehavior.Push => Strings.Ui("NeedsFirmPush"), _ => Strings.Ui("CrossesFreely") };
    private string ScreenName(string id) => ScreenName(_monitors.FirstOrDefault(m => Same(m.StableId, id)));
    private string ScreenName(MonitorInfo m)
    {
        if (m == null) return "";
        string name = Settings.MonitorNameFor(_draft.MonitorNames, m.StableId);
        return name.Length > 0 ? name : (m.Primary ? Strings.Ui("MainWord") : string.Format(Strings.Ui("DefaultScreenName"), m.Index));
    }
    private string ChipText(MonitorInfo m) => $"{m.Index} · {ScreenName(m)}";

    private void SelectRelativeTab(int current, int delta)
    {
        int next = (current + delta + _tabItems.Length) % _tabItems.Length;
        _tabs.SelectedIndex = next;
        _tabItems[next].Focus();
    }

    private void UpdateTabStrip()
    {
        if (_tabItems == null) return;
        foreach (var item in _tabItems) item.SetState(_theme, item.Index == _tabs.SelectedIndex);
        _tabStrip?.Invalidate();
    }

    private void PaintRoundedCard(object sender, PaintEventArgs e)
    {
        if (sender is not Control card || card.Width < 3 || card.Height < 3) return;
        e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
        using var path = Rounded(new Rectangle(0, 0, card.Width - 1, card.Height - 1), 8);
        using var fill = new SolidBrush(_theme.Surface);
        using var border = new Pen(_theme.Border);
        e.Graphics.FillPath(fill, path);
        e.Graphics.DrawPath(border, path);
    }

    private void PaintDrawerCard(object sender, PaintEventArgs e)
    {
        if (_drawer.Width < 2 || _drawer.Height < 2) return;
        using var border = new Pen(_theme.AccentLine);
        e.Graphics.DrawRectangle(border, 0, 0, _drawer.Width - 1, _drawer.Height - 1);
    }

    private void ApplyTheme()
    {
        if (IsDisposed) return;
        Theming.Apply(this, _theme); _map?.SetTheme(_theme);
        _tips.BackColor = _theme.Surface; _tips.ForeColor = _theme.Text;
        foreach (var tile in AllControls(this).OfType<ModeTile>()) tile.SetTheme(_theme);
        foreach (var chip in AllControls(this).OfType<NumberChip>()) chip.SetTheme(_theme);
        foreach (var close in AllControls(this).OfType<DrawerCloseButton>()) close.SetTheme(_theme);
        foreach (var combo in AllControls(this).OfType<ThemedComboBox>()) combo.SetTheme(_theme);
        foreach (var link in AllControls(this).OfType<LinkLabel>())
        {
            link.LinkColor = _theme.Accent;
            link.ActiveLinkColor = _theme.Accent;
            link.VisitedLinkColor = _theme.Accent;
            link.DisabledLinkColor = _theme.Subtle;
        }
        if (_advancedToggle != null)
        {
            _advancedToggle.ForeColor = _theme.Accent;
            _advancedToggle.FlatAppearance.MouseOverBackColor = _theme.AccentWeak;
        }
        ThemeScrollbars(_bodyFlow);
        ThemeScrollbars(_drawerFlow);
        ThemeScrollbars(_shortcutsPage);
        ThemeScrollbars(_generalPage);
        _mapLegend?.SetTheme(_theme);
        if (_inlineStatus != null) _inlineStatus.ForeColor = _inlineStatusIsError ? _theme.Danger : _theme.Faint;
        UpdateTabStrip();
        _tabs.Invalidate(); Invalidate(true);
    }

    private void ThemeScrollbars(Control control)
    {
        if (control?.IsHandleCreated == true)
            _ = Native.SetWindowTheme(control.Handle, _theme.IsDark ? "DarkMode_Explorer" : "Explorer", null);
    }

    private static IEnumerable<Control> AllControls(Control root)
    {
        foreach (Control c in root.Controls) { yield return c; foreach (var child in AllControls(c)) yield return child; }
    }

    protected override void OnShown(EventArgs e)
    {
        base.OnShown(e);
        // Native child handles now exist. Reapply the complete walk so persistent chrome such as the footer
        // cannot retain the light visual-style surfaces it received while its handles were being realized.
        ApplyTheme();
        var virtualScreen = SystemInformation.VirtualScreen;
        if (Left >= virtualScreen.Right || Top >= virtualScreen.Bottom)
        {
            Theming.DarkTitleBar(this, _theme);
            return;
        }
        var area = Screen.FromControl(this).WorkingArea;
        Size = new Size(Math.Min(Width, (int)(area.Width * .9)), Math.Min(Height, (int)(area.Height * .9)));
        Location = new Point(Math.Max(area.Left, Math.Min(Left, area.Right - Width)), Math.Max(area.Top, Math.Min(Top, area.Bottom - Height)));
        Theming.DarkTitleBar(this, _theme);
    }

    protected override bool ProcessCmdKey(ref Message msg, Keys keyData)
    {
        if (keyData == Keys.Escape && _screenSplit.Visible) { CloseDrawer(); return true; }
        return base.ProcessCmdKey(ref msg, keyData);
    }

    private bool _resourcesDisposed;
    protected override void Dispose(bool disposing)
    {
        if (disposing && !_resourcesDisposed)
        {
            _resourcesDisposed = true;
            SystemEvents.DisplaySettingsChanged -= DisplaySettingsChanged;
            _runtimeTimer.Stop(); _runtimeTimer.Dispose();
            _displayRefreshTimer.Stop(); _displayRefreshTimer.Dispose();
            _tips.Dispose();
        }
        base.Dispose(disposing);
    }

    private void DisplaySettingsChanged(object sender, EventArgs e)
    {
        if (!IsHandleCreated || IsDisposed) return;
        BeginInvoke((Action)(() => { _displayRefreshTimer.Stop(); _displayRefreshTimer.Start(); }));
    }

    private void DisplayRefreshTimer_Tick(object sender, EventArgs e)
    {
        _displayRefreshTimer.Stop(); _monitors.Clear(); _monitors.AddRange(MonitorInfo.All()); SanitizeExcluded(); RebuildScreenRows(); RefreshMap(); CloseDrawer();
        SetInlineStatus(Strings.LayoutChangedBanner);
    }

    private bool _inResizeLayout;
    private void ResizeV2Layout()
    {
        if (_bodyFlow == null || _bodyFlow.IsDisposed) return;
        // Re-entrancy guard: this method sets child .Width/.Height, which re-fires the _bodyFlow/page
        // SizeChanged handlers that call back into here. Without this, one window-resize tick triggers a
        // storm of full relayouts (each doing whole-tree LINQ walks + grid rebuilds) → heavy drag lag.
        if (_inResizeLayout) return;
        _inResizeLayout = true;
        SuspendLayout();
        try
        {
        bool narrow = ClientSize.Width < DpiPx(900);
        int horizontal = DpiPx(narrow ? 18 : 28);
        int bodyViewport = Math.Max(DpiPx(420), _bodyFlow.ClientSize.Width - SystemInformation.VerticalScrollBarWidth - DpiPx(4));
        int width = Math.Max(DpiPx(420), Math.Min(DpiPx(WideContentMeasure), bodyViewport - horizontal * 2));
        int bodySpare = Math.Max(0, bodyViewport - width);
        int bodyLeft = Math.Max(horizontal, bodySpare / 2);
        int bodyRight = Math.Max(horizontal, bodySpare - bodyLeft);
        _bodyFlow.Padding = new Padding(bodyLeft, DpiPx(23), bodyRight, DpiPx(32));
        foreach (Control child in _bodyFlow.Controls) child.Width = width;
        LayoutScreenSectionText();
        _screenRows.Width = width;
        UpdateScreenRowPitch(narrow);
        ReflowScreenRows(narrow);
        foreach (var page in new[] { _advancedPage, _shortcutsPage, _generalPage })
        {
            int pageViewport = Math.Max(DpiPx(420), page.ClientSize.Width - SystemInformation.VerticalScrollBarWidth - DpiPx(4));
            int pageWidth = Math.Max(DpiPx(420), Math.Min(DpiPx(WideContentMeasure), pageViewport - horizontal * 2));
            int pageSpare = Math.Max(0, pageViewport - pageWidth);
            int pageLeft = Math.Max(horizontal, pageSpare / 2);
            int pageRight = Math.Max(horizontal, pageSpare - pageLeft);
            page.Padding = new Padding(pageLeft, DpiPx(23), pageRight, DpiPx(32));
            foreach (Control child in page.Controls) child.Width = pageWidth;
            if (page != _advancedPage)
            {
                var flow = page.Controls.OfType<FlowLayoutPanel>().FirstOrDefault();
                if (flow != null) foreach (Control child in flow.Controls) child.Width = pageWidth;
            }
        }
        int advancedContentWidth = Math.Max(DpiPx(420), _advancedPage.ClientSize.Width - _advancedPage.Padding.Horizontal - SystemInformation.VerticalScrollBarWidth - DpiPx(4));
        ResizeAdvanced(advancedContentWidth);
        var numericGrid = _advancedPanel.Controls.Cast<Control>().OfType<TableLayoutPanel>().FirstOrDefault(c => (c.Tag as string) == "numericGrid");
        if (numericGrid != null) ReflowGrid(numericGrid, narrow, Math.Max(DpiPx(420), _advancedWidth));
        var appearanceGrid = _generalPage.Controls.Cast<Control>().SelectMany(AllControls).OfType<TableLayoutPanel>().FirstOrDefault(c => (c.Tag as string) == "appearanceGrid");
        if (appearanceGrid != null) ReflowGrid(appearanceGrid, narrow,
            Math.Max(DpiPx(420), _generalPage.ClientSize.Width - _generalPage.Padding.Horizontal - SystemInformation.VerticalScrollBarWidth - DpiPx(4)));
        if (narrow)
        {
            _modeTiles.FlowDirection = FlowDirection.TopDown; _modeTiles.Height = DpiPx(250);
            foreach (ModeTile tile in _modeTiles.Controls) { tile.Width = Math.Max(DpiPx(220), width - DpiPx(28)); tile.Height = DpiPx(78); }
        }
        else
        {
            _modeTiles.FlowDirection = FlowDirection.LeftToRight; _modeTiles.Height = DpiPx(84);
        }
        int drawerHeight = RestingBoundaryDrawerHeight(width);
        int listHeight = _screenSection.Height + _screenSection.Margin.Vertical
            + _screenRows.Height + _screenRows.Margin.Vertical;
        var mapHeader = _bodyFlow.Controls.Cast<Control>().First(control => (control.Tag as string) == "mapHeader");
        int nonMapHeight = _bodyFlow.Padding.Vertical
            + mapHeader.Height + mapHeader.Margin.Vertical
            + _mapHint.Height + _mapHint.Margin.Vertical
            + Math.Max(drawerHeight, listHeight);
        int bodyHeight = _bodyFlow.ClientSize.Height;
        int leftoverHeight = bodyHeight - nonMapHeight;
        int preferredFloor = Math.Max(DpiPx(220), (int)Math.Round(bodyHeight * PreferredMapHeightRatio));
        int proportionalCeiling = Math.Max(preferredFloor, (int)Math.Round(bodyHeight * MaximumMapHeightRatio));
        _map.Height = Math.Clamp(leftoverHeight, preferredFloor, proportionalCeiling);
        if (_screenSplit.Visible) LayoutBoundaryDrawer();
        }
        finally { ResumeLayout(true); _inResizeLayout = false; }
    }

    private void LayoutScreenSectionText()
    {
        if (_screenSection == null || _screenSectionHelp == null) return;
        int gap = DpiPx(12), right = DpiPx(10);
        int titleWidth = MeasureSingleLineWidth(_screenSectionTitle);
        _screenSectionTitle.Width = Math.Min(titleWidth, Math.Max(DpiPx(80), _screenSection.ClientSize.Width / 2));
        int minimumLeft = _screenSectionTitle.Right + gap;
        FitRightAlignedText(_screenSectionHelp, minimumLeft, right);
    }

    private void LayoutDrawerQuestionText()
    {
        if (_overallLink?.Parent == null) return;
        int gap = DpiPx(12), right = DpiPx(4);
        FitRightAlignedText(_overallLink, Math.Min(DpiPx(260), _overallLink.Parent.ClientSize.Width / 2), right);
        _drawerDirection.Width = Math.Max(DpiPx(120), _overallLink.Left - gap - _drawerDirection.Left);
    }

    private void FitRightAlignedText(Label label, int minimumLeft, int rightPadding)
    {
        int available = Math.Max(DpiPx(80), label.Parent.ClientSize.Width - minimumLeft - rightPadding);
        label.Width = Math.Min(MeasureSingleLineWidth(label), available);
        label.Left = Math.Max(minimumLeft, label.Parent.ClientSize.Width - rightPadding - label.Width);
        int measuredHeight = TextRenderer.MeasureText(label.Text, label.Font, new Size(label.Width, 0),
            TextFormatFlags.WordBreak | TextFormatFlags.NoPrefix | TextFormatFlags.NoPadding).Height;
        label.Height = Math.Max(DpiPx(20), measuredHeight + DpiPx(2));
    }

    private int MeasureSingleLineWidth(Label label)
    {
        int rendered = TextRenderer.MeasureText(label.Text, label.Font, Size.Empty,
            TextFormatFlags.SingleLine | TextFormatFlags.NoPrefix | TextFormatFlags.NoPadding).Width + DpiPx(4);
        return Math.Max(rendered, label.GetPreferredSize(Size.Empty).Width);
    }

    private void ReflowScreenRows(bool narrow)
    {
        var rows = _screenRows.Controls.OfType<TableLayoutPanel>().ToList();
        if (rows.Count == 0) return;
        rows[0].Visible = !narrow;
        foreach (var row in rows.Skip(1))
        {
            var screen = row.GetControlFromPosition(0, 0);
            var name = row.GetControlFromPosition(1, 0);
            var resolution = row.GetControlFromPosition(2, 0) ?? row.GetControlFromPosition(1, 1);
            var access = row.GetControlFromPosition(3, 0);
            if (screen == null || name == null || resolution == null || access == null) continue;
            row.SuspendLayout(); row.RowStyles.Clear(); row.ColumnStyles.Clear();
            if (narrow)
            {
                row.RowCount = 2; row.RowStyles.Add(new RowStyle(SizeType.Percent, 58)); row.RowStyles.Add(new RowStyle(SizeType.Percent, 42));
                row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 62)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
                row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 0)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 176));
                row.SetCellPosition(screen, new TableLayoutPanelCellPosition(0, 0)); row.SetRowSpan(screen, 2);
                row.SetCellPosition(name, new TableLayoutPanelCellPosition(1, 0));
                row.SetCellPosition(resolution, new TableLayoutPanelCellPosition(1, 1));
                row.SetCellPosition(access, new TableLayoutPanelCellPosition(3, 0)); row.SetRowSpan(access, 2);
            }
            else
            {
                row.RowCount = 1; row.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
                row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 70)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
                row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 128)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 190));
                row.SetRowSpan(screen, 1); row.SetRowSpan(access, 1);
                row.SetCellPosition(screen, new TableLayoutPanelCellPosition(0, 0)); row.SetCellPosition(name, new TableLayoutPanelCellPosition(1, 0));
                row.SetCellPosition(resolution, new TableLayoutPanelCellPosition(2, 0)); row.SetCellPosition(access, new TableLayoutPanelCellPosition(3, 0));
            }
            row.ResumeLayout(true);
        }
    }

    private static void ReflowGrid(TableLayoutPanel grid, bool narrow, int width)
    {
        var controls = grid.Controls.Cast<Control>().ToList();
        grid.SuspendLayout(); grid.ColumnStyles.Clear(); grid.RowStyles.Clear(); grid.Width = width;
        grid.ColumnCount = narrow ? 1 : 2;
        for (int i = 0; i < grid.ColumnCount; i++) grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100f / grid.ColumnCount));
        grid.RowCount = narrow ? controls.Count : (controls.Count + 1) / 2;
        if ((grid.Tag as string) == "appearanceGrid") grid.Height = narrow ? 156 : 78;
        for (int i = 0; i < controls.Count; i++) grid.SetCellPosition(controls[i], new TableLayoutPanelCellPosition(narrow ? 0 : i % 2, narrow ? i : i / 2));
        grid.ResumeLayout(true);
    }

    private void ReserveDrawerStatusHeight()
    {
        if (_drawerStatus == null) return;
        int width = Math.Max(DpiPx(220), _drawerStatus.Width);
        _drawerStatus.Height = DrawerStatusCandidates().Max(text => TextRenderer.MeasureText(text, _drawerStatus.Font,
            new Size(width, 0), TextFormatFlags.WordBreak | TextFormatFlags.NoPrefix).Height) + DpiPx(4);
    }

    private string[] DrawerStatusCandidates()
    {
        string longestName = _monitors.Select(ScreenName).OrderByDescending(n => n?.Length ?? 0).FirstOrDefault() ?? "";
        return new[]
        {
            Strings.Ui("StatusUpClosed"), Strings.Ui("StatusSideOff"),
            string.Format(Strings.Ui("StatusSafety"), longestName), Strings.Ui("BlockedByKeepOut"),
            string.Format(Strings.Ui("ExcludedTargetExplanation"), longestName),
            Strings.Ui("SavedInactive"), " "
        };
    }

    private void UpdateShortcutErrors()
    {
        foreach (var frame in _hotkeyBoxes.Where(box => box != null).Select(box => box.Parent).OfType<ThemedControlFrame>())
        {
            frame.Error = false; frame.AccessibleDescription = ""; frame.Invalidate();
        }
        var chordRows = Enumerable.Range(0, 4).Select(i => (Index: i, Text: HotkeyText(i), Name: ShortcutName(i)))
            .Where(item => !string.Equals(item.Text, Strings.HotkeyPressKeys, StringComparison.Ordinal)).ToList();
        var duplicate = chordRows.GroupBy(item => item.Text, StringComparer.OrdinalIgnoreCase)
            .Where(group => group.Count() > 1)
            .Select(group => (First: group.MinBy(item => item.Index), Later: group.MaxBy(item => item.Index)))
            .OrderBy(pair => pair.Later.Index).FirstOrDefault();
        if (duplicate.Later.Text != null)
        {
            string message = string.Format(Strings.Ui("DuplicateShortcutsNamed"), duplicate.First.Name, duplicate.Later.Name);
            if (_hotkeyBoxes[duplicate.Later.Index]?.Parent is ThemedControlFrame frame)
            {
                frame.Error = true; frame.AccessibleDescription = message; frame.Invalidate();
            }
            SetInlineStatus(message, true);
        }
    }

    private string ShortcutName(int index) => index switch
    {
        0 => Strings.HotkeyHead,
        1 => Strings.GameHotkeyHead,
        2 => Strings.PauseHotkeyHead,
        _ => Strings.SideHotkeyHead,
    };

    private static TableLayoutPanel ScreenListGrid(int height)
    {
        var grid = new TableLayoutPanel { Dock = DockStyle.Fill, Height = height, ColumnCount = 4, Margin = Padding.Empty, Padding = new Padding(8, 0, 8, 0) };
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 70));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 128));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 190));
        return grid;
    }

    private static Label ListHeader(string text) => new() { Text = text, Dock = DockStyle.Fill, Tag = "faint", TextAlign = ContentAlignment.MiddleLeft, Font = new Font("Segoe UI Semibold", 7.5f) };

    private void PaintListSeparator(object sender, PaintEventArgs e)
    {
        if (sender is not Control control) return;
        using var pen = new Pen(_theme.Border); e.Graphics.DrawLine(pen, 0, 0, control.Width, 0);
    }

    private static Panel PageHeader(string title, string subtitle, int width)
    {
        var panel = new Panel { Width = width, Height = 70, Margin = Padding.Empty };
        panel.Controls.Add(new Label { Text = title, Left = 0, Top = 0, Width = width, Height = 29, Font = new Font("Segoe UI Semibold", 18f), Tag = "head" });
        panel.Controls.Add(new Label { Text = subtitle, Left = 0, Top = 34, Width = width, Height = 28, Tag = "subtle" });
        return panel;
    }

    private static void AddSectionHeading(Control flow, string text) => flow.Controls.Add(new Label { Text = text, Width = 790, Height = 46, Padding = new Padding(0, 16, 0, 0), Font = new Font("Segoe UI Semibold", 10.5f), Tag = "head" });
    private static Label HelpLabel(string text, int width, int height = 30) => new() { Text = text, Width = width, Height = height, Tag = "subtle" };
    private static Button FlatButton(string text, int width) => new() { Text = text, Width = width, Height = 32, FlatStyle = FlatStyle.Flat, Margin = new Padding(4, 0, 4, 0) };
    private static Button LinkButton(string text, int width, int height) => new() { Text = text, Width = width, Height = height, FlatStyle = FlatStyle.Flat, FlatAppearance = { BorderSize = 0 }, Cursor = Cursors.Hand };
    private static void AddUnique(List<string> list, string value) { if (!list.Contains(value, StringComparer.OrdinalIgnoreCase)) list.Add(value); }
    private static bool Same(string a, string b) => string.Equals(a, b, StringComparison.OrdinalIgnoreCase);

    private sealed class SidebarButton : Button
    {
        private Theme _theme = Theme.Light;
        private bool _selected;
        private bool _hovered;
        public int Index { get; }
        public event Action<int, int> SelectionRequested;

        public SidebarButton(string text, int index)
        {
            Text = text;
            Index = index;
            TabStop = true;
            Cursor = Cursors.Hand;
            AccessibleRole = AccessibleRole.PageTab;
            AccessibleName = text;
            FlatStyle = FlatStyle.Flat; FlatAppearance.BorderSize = 0;
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer, true);
        }

        public void SetState(Theme theme, bool selected)
        {
            _theme = theme;
            _selected = selected;
            AccessibleDefaultActionDescription = selected ? "" : Text;
            Invalidate();
        }

        protected override void OnMouseEnter(EventArgs e) { _hovered = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hovered = false; Invalidate(); base.OnMouseLeave(e); }
        protected override bool IsInputKey(Keys keyData) =>
            (keyData & Keys.KeyCode) is Keys.Up or Keys.Down or Keys.Left or Keys.Right || base.IsInputKey(keyData);
        protected override void OnKeyDown(KeyEventArgs e)
        {
            if (e.KeyCode is Keys.Left or Keys.Right or Keys.Up or Keys.Down)
            {
                SelectionRequested?.Invoke(Index, e.KeyCode is Keys.Left or Keys.Up ? -1 : 1);
                e.Handled = true;
            }
            else if (e.KeyCode is Keys.Enter or Keys.Space)
            {
                OnClick(EventArgs.Empty);
                e.Handled = true;
            }
            base.OnKeyDown(e);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            e.Graphics.Clear(_theme.Sidebar);
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            var rect = new Rectangle(0, 0, Width - 1, Height - 1);
            using var path = Rounded(rect, 6);
            using var fill = new SolidBrush(_selected ? _theme.AccentWeak : _hovered ? _theme.Surface2 : _theme.Sidebar);
            e.Graphics.FillPath(fill, path);
            Color text = _selected ? _theme.Accent : _hovered ? _theme.Text : _theme.Subtle;
            using var font = new Font("Segoe UI Semibold", 9f, _selected ? FontStyle.Bold : FontStyle.Regular);
            var textRect = new Rectangle(10, 0, Width - 18, Height);
            TextRenderer.DrawText(e.Graphics, Text, font, textRect, text,
                TextFormatFlags.Left | TextFormatFlags.VerticalCenter |
                TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix);
            if (Focused && ShowFocusCues) ControlPaint.DrawFocusRectangle(e.Graphics, new Rectangle(4, 4, Width - 8, Height - 8), _theme.Text, _selected ? _theme.AccentWeak : _theme.Sidebar);
        }
        protected override AccessibleObject CreateAccessibilityInstance() => new SidebarAccessibleObject(this);
        private sealed class SidebarAccessibleObject : ControlAccessibleObject
        {
            private readonly SidebarButton _owner;
            public SidebarAccessibleObject(SidebarButton owner) : base(owner) => _owner = owner;
            public override AccessibleStates State => base.State | (_owner._selected ? AccessibleStates.Selected : AccessibleStates.None);
        }
    }

    /// <summary>The native TabControl remains the page host, but contributes no header or page-frame chrome.</summary>
    private sealed class FlatTabControl : TabControl
    {
        public FlatTabControl()
        {
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint |
                     ControlStyles.OptimizedDoubleBuffer, true);
        }

        public override Rectangle DisplayRectangle => ClientRectangle;

        protected override void OnPaintBackground(PaintEventArgs e) => e.Graphics.Clear(BackColor);
        protected override void OnPaint(PaintEventArgs e) => e.Graphics.Clear(BackColor);
    }

    private sealed class DrawerCloseButton : Control
    {
        private Theme _theme = Theme.Light;
        private bool _hovered;
        public DrawerCloseButton()
        {
            Text = "×";
            TabStop = true;
            Cursor = Cursors.Hand;
            AccessibleRole = AccessibleRole.PushButton;
            DoubleBuffered = true;
        }
        public void SetTheme(Theme theme) { _theme = theme; Invalidate(); }
        protected override void OnMouseEnter(EventArgs e) { _hovered = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hovered = false; Invalidate(); base.OnMouseLeave(e); }
        protected override void OnKeyDown(KeyEventArgs e)
        {
            if (e.KeyCode is Keys.Enter or Keys.Space) { OnClick(EventArgs.Empty); e.Handled = true; }
            base.OnKeyDown(e);
        }
        protected override void OnPaint(PaintEventArgs e)
        {
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            if (_hovered)
            {
                using var hover = new SolidBrush(_theme.AccentWeak);
                e.Graphics.FillEllipse(hover, 2, 0, Width - 4, Height - 2);
            }
            TextRenderer.DrawText(e.Graphics, Text, Font, ClientRectangle, _hovered ? _theme.Accent : _theme.Subtle,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPadding);
            if (Focused) ControlPaint.DrawFocusRectangle(e.Graphics, new Rectangle(3, 2, Width - 6, Height - 4), _theme.Text, Color.Transparent);
        }
    }

    /// <summary>LinkLabel's stock disabled renderer embosses text on dark surfaces; keep disabled semantics with flat ink.</summary>
    private sealed class FlatLinkLabel : LinkLabel
    {
        protected override void OnPaint(PaintEventArgs e)
        {
            if (Enabled) { base.OnPaint(e); return; }
            e.Graphics.Clear(BackColor);
            var flags = TextFormatFlags.VerticalCenter | TextFormatFlags.SingleLine |
                        TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix;
            flags |= TextAlign switch
            {
                ContentAlignment.TopCenter or ContentAlignment.MiddleCenter or ContentAlignment.BottomCenter => TextFormatFlags.HorizontalCenter,
                ContentAlignment.TopRight or ContentAlignment.MiddleRight or ContentAlignment.BottomRight => TextFormatFlags.Right,
                _ => TextFormatFlags.Left,
            };
            TextRenderer.DrawText(e.Graphics, Text, Font, ClientRectangle, DisabledLinkColor, flags);
        }
    }

    private sealed class MapLegend : Control
    {
        private Theme _theme = Theme.Light;
        public MapLegend() { DoubleBuffered = true; Font = new Font("Segoe UI", 8.25f); }
        public void SetTheme(Theme theme) { _theme = theme; BackColor = theme.Back; ForeColor = theme.Subtle; Invalidate(); }
        protected override void OnPaint(PaintEventArgs e)
        {
            int x = 0;
            x = DrawLegendItem(e.Graphics, x, "star", Strings.Ui("LegendMainM2"));
            x = DrawLegendItem(e.Graphics, x + 18, "hatch", Strings.Ui("LegendKeepOutM2"));
            DrawLegendItem(e.Graphics, x + 18, "dot", Strings.Ui("LegendCrossingM2"));
        }
        private int DrawLegendItem(Graphics g, int x, string glyph, string text)
        {
            var swatch = new Rectangle(x, 6, 16, 16);
            if (glyph == "star")
                TextRenderer.DrawText(g, "★", Font, swatch, _theme.Star, TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPadding);
            else if (glyph == "hatch")
            {
                using var back = new SolidBrush(_theme.Surface);
                using var hatch = new HatchBrush(HatchStyle.ForwardDiagonal, _theme.Danger, _theme.Surface);
                using var border = new Pen(_theme.Border);
                g.FillRectangle(back, swatch); g.FillRectangle(hatch, swatch); g.DrawRectangle(border, swatch);
            }
            else
            {
                using var ring = new Pen(_theme.Surface2, 2);
                using var dot = new SolidBrush(_theme.Subtle);
                g.FillEllipse(dot, x + 3, 9, 10, 10); g.DrawEllipse(ring, x + 3, 9, 10, 10);
            }
            int width = TextRenderer.MeasureText(g, text, Font, Size.Empty, TextFormatFlags.NoPadding).Width;
            TextRenderer.DrawText(g, text, Font, new Rectangle(x + 21, 3, width + 2, 22), _theme.Subtle,
                TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPadding | TextFormatFlags.NoPrefix);
            return x + 23 + width;
        }
    }

    private sealed class NumberChip : Control
    {
        public int Number; public bool Accent; private Theme _theme = Theme.Light;
        public NumberChip() { DoubleBuffered = true; Font = new Font("Segoe UI Semibold", 9f); }
        public void SetTheme(Theme theme) { _theme = theme; Invalidate(); }
        protected override void OnPaint(PaintEventArgs e)
        {
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            using var path = Rounded(new Rectangle(0, 0, Width - 1, Height - 1), 6);
            using var brush = new SolidBrush(Accent ? _theme.AccentWeak : _theme.Chip); e.Graphics.FillPath(brush, path);
            TextRenderer.DrawText(e.Graphics, Number.ToString(), Font, ClientRectangle, Accent ? _theme.Accent : _theme.ChipText,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPadding);
        }
    }

    private sealed class ModeTile : Control
    {
        public BoundaryBehavior Behavior; public string Title = ""; public string Help = "";
        private bool _selected; private Theme _theme = Theme.Light;
        [System.ComponentModel.Browsable(false)]
        [System.ComponentModel.DesignerSerializationVisibility(System.ComponentModel.DesignerSerializationVisibility.Hidden)]
        public bool Selected { get => _selected; set { _selected = value; Invalidate(); } }
        public ModeTile() { DoubleBuffered = true; Cursor = Cursors.Hand; TabStop = true; AccessibleRole = AccessibleRole.PushButton; }
        public void SetTheme(Theme theme) { _theme = theme; BackColor = theme.Surface; ForeColor = theme.Text; AccessibleName = Title + ". " + Help; Invalidate(); }
        protected override void OnEnabledChanged(EventArgs e)
        {
            Cursor = Enabled ? Cursors.Hand : Cursors.Default;
            Invalidate();
            base.OnEnabledChanged(e);
        }
        protected override void OnPaint(PaintEventArgs e)
        {
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            using var path = Rounded(new Rectangle(1, 1, Width - 3, Height - 3), 8);
            Color fillColor = !Enabled ? _theme.DisabledSurface : Selected ? _theme.AccentWeak : _theme.Surface;
            Color borderColor = !Enabled ? _theme.Border : Selected ? _theme.Accent : _theme.Border;
            Color titleColor = !Enabled ? _theme.Faint : Selected ? _theme.Accent : _theme.Text;
            using var fill = new SolidBrush(fillColor); e.Graphics.FillPath(fill, path);
            using var border = new Pen(borderColor, Enabled && Selected ? 1.8f : 1f); e.Graphics.DrawPath(border, path);
            DrawGlyph(e.Graphics, new Point(22, Height / 2), Enabled);
            bool shortTile = Height < 60 && Help.Length > 0;
            var titleRect = new Rectangle(44, Help.Length == 0 ? 4 : shortTile ? 2 : 10, Width - 52, Help.Length == 0 ? Height - 8 : shortTile ? 19 : 22);
            TextRenderer.DrawText(e.Graphics, Title, new Font(Font, FontStyle.Bold), titleRect, titleColor,
                TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPadding);
            if (Help.Length > 0)
            {
                var helpRect = shortTile
                    ? new Rectangle(44, 21, Width - 52, Height - 24)
                    : new Rectangle(44, 31, Width - 52, Height - 35);
                TextRenderer.DrawText(e.Graphics, Help, Font, helpRect, Enabled ? _theme.Subtle : _theme.Faint,
                    TextFormatFlags.Left | (shortTile ? TextFormatFlags.SingleLine : TextFormatFlags.WordBreak) |
                    TextFormatFlags.EndEllipsis | TextFormatFlags.NoPadding);
            }
            if (Enabled && Focused) ControlPaint.DrawFocusRectangle(e.Graphics, new Rectangle(4, 4, Width - 8, Height - 8), _theme.Text, Color.Transparent);
        }
        private void DrawGlyph(Graphics g, Point p, bool enabled)
        {
            Color color = enabled ? (Behavior == BoundaryBehavior.Blocked ? _theme.Text : _theme.Accent) : _theme.Faint;
            if (Behavior == BoundaryBehavior.Blocked) { using var pen = new Pen(color, 5); g.DrawLine(pen, p.X - 8, p.Y, p.X + 8, p.Y); }
            else if (Behavior == BoundaryBehavior.Push) { using var b = new SolidBrush(color); g.FillPolygon(b, new[] { new Point(p.X, p.Y - 8), new Point(p.X + 8, p.Y), new Point(p.X, p.Y + 8), new Point(p.X - 8, p.Y) }); }
            else { using var pen = new Pen(color, 2); g.DrawEllipse(pen, p.X - 7, p.Y - 7, 14, 14); }
        }
        protected override void OnKeyDown(KeyEventArgs e) { if (e.KeyCode is Keys.Enter or Keys.Space) { OnClick(EventArgs.Empty); e.Handled = true; } base.OnKeyDown(e); }
    }

    private static GraphicsPath Rounded(Rectangle r, int radius)
    {
        int d = Math.Max(2, Math.Min(radius * 2, Math.Min(r.Width, r.Height)));
        var p = new GraphicsPath(); p.AddArc(r.Left, r.Top, d, d, 180, 90); p.AddArc(r.Right - d, r.Top, d, d, 270, 90);
        p.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90); p.AddArc(r.Left, r.Bottom - d, d, d, 90, 90); p.CloseFigure(); return p;
    }
}
