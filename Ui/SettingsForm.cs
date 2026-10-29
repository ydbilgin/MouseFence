using System.Drawing;
using System.Drawing.Drawing2D;
using Microsoft.Win32;

namespace MouseFence;

public readonly record struct SettingsRuntimeState(bool UpwardPassageOpen, bool SideProtectionOn, bool Paused);

/// <summary>The transactional, native WinForms settings experience.</summary>
public sealed partial class SettingsForm : Form
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
        nameof(Settings.InstantCrossSpeedPxPerSecond), nameof(Settings.LearnFromMyArrivals),
    };

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
    private static void DisposeChildren(Control parent)
    {
        foreach (Control child in parent.Controls.Cast<Control>().ToArray()) child.Dispose();
    }
    internal Settings DraftForTests => _draft;
    internal string InlineStatusForTests => _inlineStatus?.Text ?? "";
    internal IReadOnlyList<string> DrawerStatusCandidatesForTests => DrawerStatusCandidates();
    internal (string Mode, int BodyHeight, int MapHeight, int BandHeight) DisplayLayoutMetrics()
    {
        // Display controls inherit Visible=false while another tab is selected. Report that page's
        // viewport instead of querying the hidden list/drawer, which has no visible last control.
        if (_tabs.SelectedTab != _tabs.TabPages[0])
        {
            string mode = _tabs.SelectedTab == _advancedPage ? "advanced"
                : _tabs.SelectedTab == _shortcutsPage ? "shortcuts" : "general";
            return (mode, _tabs.SelectedTab.ClientSize.Height, 0, 0);
        }

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
        var brandIcon = new PictureBox { Left = 12, Top = 23, Width = 20, Height = 20, SizeMode = PictureBoxSizeMode.Zoom,
            Image = AppIconBitmap(32), Tag = "brandIcon" };
        var product = new Label { Text = "MouseFence", Left = 38, Top = 21, Width = 118, Height = 25,
            Font = new Font("Segoe UI Semibold", 10.5f), Tag = "head" };
        _tabStrip.Controls.Add(brandIcon);
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
        _automaticControl = new ThemedRadioButton { Text = Strings.Ui("AutomaticControl"), Left = 0, Top = 0, Width = 760, Height = 24 };
        var autoHelp = new Label { Text = Strings.Ui("AutomaticControlHelp"), Left = 24, Top = 24, Width = 302, Height = 24, Tag = "subtle" };
        _alwaysControl = new ThemedRadioButton { Text = Strings.Ui("AlwaysControl"), Left = 0, Top = 48, Width = 760, Height = 24 };
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

    /// <summary>The shield app icon at the given pixel size, from the embedded app.ico.</summary>
    internal static Bitmap AppIconBitmap(int size)
    {
        using var stream = typeof(SettingsForm).Assembly.GetManifestResourceStream("MouseFence.app.ico");
        if (stream == null) return null;
        using var icon = new Icon(stream, size, size);
        return icon.ToBitmap();
    }

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

    private FlowLayoutPanel PageFlow(TabPage page)
    {
        var flow = new FlowLayoutPanel { Dock = DockStyle.Top, AutoSize = true, FlowDirection = FlowDirection.TopDown, WrapContents = false, Padding = Padding.Empty };
        page.Controls.Add(flow); return flow;
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
            using var titleFont = new Font(Font, FontStyle.Bold);
            TextRenderer.DrawText(e.Graphics, Title, titleFont, titleRect, titleColor,
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
