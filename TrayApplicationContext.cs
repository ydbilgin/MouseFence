using System.Drawing;
using System.Runtime.InteropServices;
using Microsoft.Win32;

namespace MouseFence;

/// <summary>Owns the tray icon, the mouse guard, the global hotkey, and the settings.</summary>
public sealed class TrayApplicationContext : ApplicationContext
{
    [DllImport("user32.dll")]
    private static extern bool DestroyIcon(IntPtr hIcon);

    private const int GateInset = 24;   // shrink each crossing gate by this many px on each side

    private readonly NotifyIcon _tray;
    private readonly MouseGuard _guard = new();
    private readonly HotKeyWindow _hotkey = new();
    private readonly HotKeyWindow _confineHotkey = new();
    private readonly HotKeyWindow _pauseHotkey = new();   // universal keyboard escape from any trap
    private readonly HotKeyWindow _sideHotkey = new();    // toggle main->side (left/right) containment

    // Re-apply the barrier when the monitor layout changes (arrangement / resolution / dock / undock),
    // so the gates and barrier line always track the live displays without a restart. The OS event can
    // arrive on a non-UI thread and in bursts, so it is marshalled to the UI thread via _uiSync and
    // debounced through _reconfigTimer.
    private readonly Control _uiSync = new();
    private readonly System.Windows.Forms.Timer _reconfigTimer;

    private readonly ToolStripMenuItem _gateItem;
    private readonly ToolStripMenuItem _sideItem;
    private readonly ToolStripMenuItem _pauseItem;
    private readonly ToolStripMenuItem _gameItem;
    private readonly ToolStripMenuItem _settingsItem;
    private readonly ToolStripMenuItem _exitItem;

    private Settings _settings;
    private Icon _iconClosed;
    private Icon _iconOpen;
    private Icon _iconPaused;

    private List<string> _isolatedDevices = new();   // stable-keyed latch: warn once per bad layout, re-arm when it heals
    private bool _pauseRegistered;                    // did the pause hotkey actually register? (for the escape-key hint)

    public TrayApplicationContext()
    {
        _settings = Settings.Load();
        Strings.Use(_settings.Language);

        _iconClosed = MakeIcon(Color.FromArgb(220, 60, 60));
        _iconOpen = MakeIcon(Color.FromArgb(60, 180, 75));
        _iconPaused = MakeIcon(Color.FromArgb(150, 150, 150));

        _gateItem = new ToolStripMenuItem("", null, (s, e) => ToggleGate());
        _sideItem = new ToolStripMenuItem("", null, (s, e) => ToggleSide());
        _pauseItem = new ToolStripMenuItem("", null, (s, e) => TogglePause());
        _gameItem = new ToolStripMenuItem("", null, (s, e) => ToggleGame());
        _settingsItem = new ToolStripMenuItem("", null, (s, e) => OpenSettings());
        _exitItem = new ToolStripMenuItem("", null, (s, e) => ExitApp());

        var menu = new ContextMenuStrip();
        menu.Items.Add(_gateItem);
        menu.Items.Add(_sideItem);
        menu.Items.Add(_gameItem);
        menu.Items.Add(_pauseItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(_settingsItem);
        menu.Items.Add(_exitItem);

        _tray = new NotifyIcon { Visible = true, ContextMenuStrip = menu, Icon = _iconClosed, Text = "MouseFence" };
        _tray.DoubleClick += (s, e) => ToggleGate();

        _hotkey.HotKeyPressed += ToggleGate;
        _confineHotkey.HotKeyPressed += ToggleGame;
        _pauseHotkey.HotKeyPressed += TogglePause;
        _sideHotkey.HotKeyPressed += ToggleSide;

        RegisterHotkey();   // before Configure() so _pauseRegistered is known for the first isolation warning
        Configure();
        _guard.GateOpen = !_settings.StartGateClosed;
        _guard.SideContain = _settings.StartSideContainOn;
        _guard.Start();

        _ = _uiSync.Handle;   // force the marshaling window handle onto this (UI) thread
        _reconfigTimer = new System.Windows.Forms.Timer { Interval = 750 };
        _reconfigTimer.Tick += (s, e) => { _reconfigTimer.Stop(); Configure(); UpdateUi(); };
        SystemEvents.DisplaySettingsChanged += OnDisplaySettingsChanged;

        RebuildMenuText();
        UpdateUi();
        WarnIntelRotation();
    }

    // OS fires this when the display arrangement/resolution changes; marshal to the UI thread and debounce
    // (one change often raises several events), then rebuild the gates/barrier from the live monitors.
    private void OnDisplaySettingsChanged(object sender, EventArgs e)
    {
        if (!_uiSync.IsHandleCreated) return;
        _uiSync.BeginInvoke((Action)(() => { _reconfigTimer.Stop(); _reconfigTimer.Start(); }));
    }

    private void Configure()
    {
        var allMonitors = MonitorInfo.All();

        // Wall-off: split the live monitors into the EXCLUDED set (whose bounds become forbidden rects) and the
        // topology set (everything else). Keep BOTH — derive forbidden rects from one and tops/gates/AntiTrap/routes
        // from the other; never mutate one before deriving the other (binding fix #4).
        //
        // SAFETY (binding fix #2): never wall the PRIMARY display and never leave zero usable monitors. Sanitize the
        // exclude list via the SHARED pure helper (GuardCore.SanitizeExcluded) so an imported/edited settings.json
        // can't lock the user out. The exclude identity is the STABLE per-monitor key (MonitorInfo.StableId, FIX 4),
        // so it survives \\.\DISPLAYn renumbering. If pruning happens, warn.
        var primaryStable = (allMonitors.FirstOrDefault(m => m.Primary) ?? allMonitors.FirstOrDefault())?.StableId;
        var requested = new HashSet<string>(_settings.ExcludedDevices);
        var presentStable = allMonitors.Select(m => m.StableId).ToList();
        var excludedStable = new HashSet<string>(
            GuardCore.SanitizeExcluded(presentStable, primaryStable, requested, out bool pruned));

        // The monitors that may safely be walled off (their bounds become forbidden rects), matched by StableId.
        var excludedMonitors = allMonitors.Where(m => excludedStable.Contains(m.StableId)).ToList();

        // topology set = all monitors minus the excluded ones (so a dummy can never be a gate target, a safety-gate
        // owner, a descent landing, or a confine target — WALL-OFF subsumes IGNORE-ONLY). Match by StableId.
        var monitors = allMonitors.Where(m => !excludedStable.Contains(m.StableId)).ToList();

        var tops = (_settings.Mode == "Manual" && _settings.ManualMonitors.Count > 0
            ? monitors.Where(m => _settings.ManualMonitors.Contains(m.StableId))
            : monitors.Where(MonitorInfo.IsAbovePrimary)).ToList();

        var topSet = new HashSet<string>(tops.Select(t => t.StableId));
        var primary = monitors.FirstOrDefault(m => m.Primary) ?? monitors.FirstOrDefault();

        // Allowed crossings: configured UpLinks, else default = primary -> every top monitor.
        var links = _settings.UpLinks.Count > 0
            ? _settings.UpLinks
            : (primary != null
                ? tops.Select(t => new UpLink { FromDevice = primary.StableId, ToDevice = t.StableId }).ToList()
                : new List<UpLink>());

        var byStable = monitors.ToDictionary(m => m.StableId);
        var gates = new List<(int Min, int Max)>();
        foreach (var lk in links)
        {
            if (!topSet.Contains(lk.ToDevice)) continue;                 // target must be a top monitor
            if (!byStable.TryGetValue(lk.FromDevice, out var from)) continue;
            if (!byStable.TryGetValue(lk.ToDevice, out var to)) continue;

            int lo = Math.Max(from.Bounds.Left, to.Bounds.Left);
            int hi = Math.Min(from.Bounds.Right, to.Bounds.Right);
            if (hi <= lo) continue;                                       // no horizontal overlap
            int inset = Math.Min(GateInset, Math.Max(0, (hi - lo) / 4));
            gates.Add((lo + inset, hi - inset));
        }

        // Anti-trap: from the live rects, find isolated screens (warn) and owner-bound safety gates (so the
        // barrier never blocks a screen's only exit). One pure pass feeds both the warning and the gates.
        var rects = monitors.Select(m => (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom)).ToList();
        var topIdx = new HashSet<int>();
        for (int i = 0; i < monitors.Count; i++)
            if (topSet.Contains(monitors[i].StableId)) topIdx.Add(i);
        var (warn, safety) = GuardCore.AntiTrap(rects, topIdx);

        // Descent routes (opt-in): derive (Top, Landing) pairs from the same UpLinks so a descent off a top's overhang
        // clamps back onto the intended bottom. The guards (ambiguity, SF-1 vertical, X-overlap) live in the pure,
        // unit-tested GuardCore.DeriveRoutes — this keeps Native.RECT out of GuardCore and lets the rules be tested.
        var deviceRects = byStable.ToDictionary(
            kv => kv.Key,
            kv => (kv.Value.Bounds.Left, kv.Value.Bounds.Top, kv.Value.Bounds.Right, kv.Value.Bounds.Bottom));
        var routes = GuardCore.DeriveRoutes(
            links.Select(lk => (lk.FromDevice, lk.ToDevice)), deviceRects, topSet);

        // Side containment: the cursor is contained within the MAIN (primary) screen horizontally. Only engage when
        // a real side screen flanks the main left/right (else the feature is inert — Windows already blocks the void),
        // so single-monitor / top-only layouts don't install the hook needlessly. The barrier lines are main's edges.
        var main = primary;
        bool hasSides = false;
        if (main != null)
        {
            var mainRect = (main.Bounds.Left, main.Bounds.Top, main.Bounds.Right, main.Bounds.Bottom);
            hasSides = monitors.Any(m => m.StableId != main.StableId &&
                (GuardCore.Adjacent(mainRect, (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom), Dir.Left) ||
                 GuardCore.Adjacent(mainRect, (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom), Dir.Right)));
        }

        _guard.Configure(tops.Select(t => t.Bounds), gates, monitors.Select(m => m.Bounds), safety, routes,
                         excludedMonitors.Select(m => m.Bounds),
                         hasSides, main?.Bounds ?? default);
        _guard.DeliberateCross = _settings.DeliberateCross;
        _guard.DescentRouting = _settings.DescentRouting;

        if (pruned)
            _tray.ShowBalloonTip(5000, "MouseFence", Strings.ExcludePruned, ToolTipIcon.Warning);

        WarnIsolationIfChanged(warn.Select(i => monitors[i].StableId).ToList(),
                               warn.Select(i => monitors[i].Index).ToList());
    }

    // Warn (once per distinct bad layout) when a screen is isolated. Latch on the StableId so DISPLAYn reordering
    // doesn't re-nag; show the LIVE pause-hotkey text so the user always sees the working escape key.
    private void WarnIsolationIfChanged(List<string> devices, List<int> displayIndices)
    {
        bool changed = !devices.OrderBy(d => d).SequenceEqual(_isolatedDevices.OrderBy(d => d));
        _isolatedDevices = devices;
        if (devices.Count > 0 && changed)
            _tray.ShowBalloonTip(5000, "MouseFence",
                Strings.TipIsolated(string.Join(", ", displayIndices), PauseKeyLabel()), ToolTipIcon.Warning);
    }

    private string PauseKeyLabel()
    {
        bool pauseSet = _settings.PauseHotKey != Keys.None && _settings.PauseModifiers() != 0;
        if (!pauseSet) return _settings.PauseHotKeyText();          // "(none)" — intentionally cleared
        return _pauseRegistered ? _settings.PauseHotKeyText() : Strings.NoneRegistered;
    }

    private void RegisterHotkey()
    {
        if (!_hotkey.Register(_settings.Modifiers(), (uint)_settings.HotKey))
            _tray.ShowBalloonTip(3000, "MouseFence", Strings.TipHotkeyFail(_settings.HotKeyText()), ToolTipIcon.Warning);
        if (!_confineHotkey.Register(_settings.ConfineModifiers(), (uint)_settings.ConfineHotKey))
            _tray.ShowBalloonTip(3000, "MouseFence", Strings.TipHotkeyFail(_settings.ConfineHotKeyText()), ToolTipIcon.Warning);
        bool sideSet = _settings.SideHotKey != Keys.None && _settings.SideModifiers() != 0;
        if (sideSet && !_sideHotkey.Register(_settings.SideModifiers(), (uint)_settings.SideHotKey))
            _tray.ShowBalloonTip(3000, "MouseFence", Strings.TipHotkeyFail(_settings.SideHotKeyText()), ToolTipIcon.Warning);
        _pauseRegistered = _pauseHotkey.Register(_settings.PauseModifiers(), (uint)_settings.PauseHotKey);
        bool pauseSet = _settings.PauseHotKey != Keys.None && _settings.PauseModifiers() != 0;
        if (pauseSet && !_pauseRegistered)   // only nag if a key was set but couldn't register (not when cleared)
            _tray.ShowBalloonTip(3000, "MouseFence", Strings.TipHotkeyFail(_settings.PauseHotKeyText()), ToolTipIcon.Warning);
    }

    private void RebuildMenuText()
    {
        _settingsItem.Text = Strings.MenuSettings;
        _exitItem.Text = Strings.MenuExit;
        _gameItem.Text = Strings.MenuGame;
    }

    private void ToggleGame()
    {
        _guard.Confine = !_guard.Confine;
        UpdateUi();
        if (_guard.Enabled)
            _tray.ShowBalloonTip(1200, "MouseFence", _guard.Confine ? Strings.TipGameOn : Strings.TipGameOff, ToolTipIcon.Info);
    }

    private void ToggleGate()
    {
        _guard.GateOpen = !_guard.GateOpen;
        UpdateUi();
        if (_guard.Enabled)
            _tray.ShowBalloonTip(1000, "MouseFence", _guard.GateOpen ? Strings.TipOpen : Strings.TipClosed, ToolTipIcon.Info);
    }

    private void ToggleSide()
    {
        _guard.SideContain = !_guard.SideContain;
        UpdateUi();
        if (_guard.Enabled)
            _tray.ShowBalloonTip(1000, "MouseFence", _guard.SideContain ? Strings.TipSideOn : Strings.TipSideOff, ToolTipIcon.Info);
    }

    private void TogglePause()
    {
        if (_guard.Enabled) _guard.Stop();
        else _guard.Start();
        UpdateUi();
        // A keyboard user who pauses to escape a trap can't see the grey tray icon -> confirm with a balloon.
        _tray.ShowBalloonTip(1200, "MouseFence",
            _guard.Enabled ? Strings.TipResumed : Strings.TipPaused, ToolTipIcon.Info);
    }

    private void UpdateUi()
    {
        string hk = _settings.HotKeyText();

        if (!_guard.Enabled)
        {
            _tray.Icon = _iconPaused;
            _tray.Text = Strings.TrayPaused(hk);
            _gateItem.Text = Strings.GatePausedMenu;
            _gateItem.Enabled = false;
            _sideItem.Text = Strings.SideGatePausedMenu;
            _sideItem.Enabled = false;
            _gameItem.Enabled = false;
            _pauseItem.Text = Strings.MenuResume;
            _pauseItem.Checked = true;
            return;
        }

        _gateItem.Enabled = true;
        _sideItem.Enabled = true;
        _pauseItem.Text = Strings.MenuPause;
        _pauseItem.Checked = false;

        bool open = _guard.GateOpen;
        _tray.Icon = open ? _iconOpen : _iconClosed;
        _tray.Text = open ? Strings.TrayOpen(hk) : Strings.TrayClosed(hk);
        _gateItem.Text = open ? Strings.GateOpenMenu : Strings.GateClosedMenu;
        _gateItem.Checked = open;

        bool sideOn = _guard.SideContain;
        _sideItem.Text = sideOn ? Strings.SideContainOnMenu : Strings.SideContainOffMenu;
        _sideItem.Checked = sideOn;

        _gameItem.Checked = _guard.Confine;
        _gameItem.Enabled = true;
    }

    private void OpenSettings()
    {
        using var f = new SettingsForm(_settings, MonitorInfo.All());
        if (f.ShowDialog() != DialogResult.OK) return;

        _hotkey.Unregister();
        _confineHotkey.Unregister();
        _pauseHotkey.Unregister();
        _sideHotkey.Unregister();
        _settings = f.Result;
        _settings.Save();
        Strings.Use(_settings.Language);
        AutoStart.Apply(_settings.AutoStart);
        RegisterHotkey();   // before Configure() so _pauseRegistered is fresh for any isolation warning
        Configure();
        RebuildMenuText();
        UpdateUi();
        WarnIntelRotation();
    }

    // Intel GPU drivers bind Ctrl+Alt+Arrow to screen rotation, clashing with MouseFence's arrow hotkeys. On a fresh
    // Intel install we already added Shift to the defaults (Settings.AddShiftToArrowHotkeys) -> show an info balloon
    // explaining it; otherwise (e.g. a user/imported config that still uses Ctrl+Alt+Arrow) warn and name the clashing
    // hotkeys so the user can fix them. No-op when no Intel adapter is present.
    private void WarnIntelRotation()
    {
        bool intel;
        try { intel = Native.HasIntelGpu(); }
        catch { intel = false; }
        if (!intel) return;

        if (_settings.IntelDefaultsApplied)
        {
            _tray.ShowBalloonTip(8000, "MouseFence",
                Strings.TipIntelDefaults(_settings.HotKeyText(), _settings.SideHotKeyText()), ToolTipIcon.Info);
            return;
        }

        var conflicts = new List<string>();
        if (GuardCore.IsArrowRotationHotkey((int)_settings.HotKey, _settings.ModCtrl, _settings.ModAlt, _settings.ModShift, _settings.ModWin))
            conflicts.Add(_settings.HotKeyText());
        if (GuardCore.IsArrowRotationHotkey((int)_settings.SideHotKey, _settings.SideModCtrl, _settings.SideModAlt, _settings.SideModShift, _settings.SideModWin))
            conflicts.Add(_settings.SideHotKeyText());
        if (GuardCore.IsArrowRotationHotkey((int)_settings.ConfineHotKey, _settings.ConfineModCtrl, _settings.ConfineModAlt, _settings.ConfineModShift, _settings.ConfineModWin))
            conflicts.Add(_settings.ConfineHotKeyText());
        if (GuardCore.IsArrowRotationHotkey((int)_settings.PauseHotKey, _settings.PauseModCtrl, _settings.PauseModAlt, _settings.PauseModShift, _settings.PauseModWin))
            conflicts.Add(_settings.PauseHotKeyText());

        if (conflicts.Count > 0)
            _tray.ShowBalloonTip(8000, "MouseFence",
                Strings.TipIntelRotation(string.Join(", ", conflicts)), ToolTipIcon.Warning);
    }

    private void ExitApp()
    {
        SystemEvents.DisplaySettingsChanged -= OnDisplaySettingsChanged;
        _reconfigTimer.Stop();
        _reconfigTimer.Dispose();
        _uiSync.Dispose();
        _tray.Visible = false;
        _guard.Dispose();
        _hotkey.Dispose();
        _confineHotkey.Dispose();
        _pauseHotkey.Dispose();
        _sideHotkey.Dispose();
        DisposeIcon(ref _iconClosed);
        DisposeIcon(ref _iconOpen);
        DisposeIcon(ref _iconPaused);
        ExitThread();
    }

    private static Icon MakeIcon(Color fill)
    {
        using var bmp = new Bitmap(32, 32);
        using (var g = Graphics.FromImage(bmp))
        {
            g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
            g.Clear(Color.Transparent);
            using var brush = new SolidBrush(fill);
            using var pen = new Pen(Color.White, 3f);
            using var white = new SolidBrush(Color.White);
            g.FillEllipse(brush, 3, 3, 26, 26);
            g.DrawEllipse(pen, 3, 3, 26, 26);
            g.FillRectangle(white, 11, 12, 10, 7);
            g.FillRectangle(white, 14, 19, 4, 2);
        }
        return (Icon)Icon.FromHandle(bmp.GetHicon()).Clone();
    }

    private static void DisposeIcon(ref Icon icon)
    {
        if (icon == null) return;
        DestroyIcon(icon.Handle);
        icon.Dispose();
        icon = null;
    }
}
