using System.Drawing;
using System.Runtime.InteropServices;

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

    private readonly ToolStripMenuItem _gateItem;
    private readonly ToolStripMenuItem _pauseItem;
    private readonly ToolStripMenuItem _gameItem;
    private readonly ToolStripMenuItem _settingsItem;
    private readonly ToolStripMenuItem _exitItem;

    private Settings _settings;
    private Icon _iconClosed;
    private Icon _iconOpen;
    private Icon _iconPaused;

    public TrayApplicationContext()
    {
        _settings = Settings.Load();
        Strings.Use(_settings.Language);

        _iconClosed = MakeIcon(Color.FromArgb(220, 60, 60));
        _iconOpen = MakeIcon(Color.FromArgb(60, 180, 75));
        _iconPaused = MakeIcon(Color.FromArgb(150, 150, 150));

        _gateItem = new ToolStripMenuItem("", null, (s, e) => ToggleGate());
        _pauseItem = new ToolStripMenuItem("", null, (s, e) => TogglePause());
        _gameItem = new ToolStripMenuItem("", null, (s, e) => ToggleGame());
        _settingsItem = new ToolStripMenuItem("", null, (s, e) => OpenSettings());
        _exitItem = new ToolStripMenuItem("", null, (s, e) => ExitApp());

        var menu = new ContextMenuStrip();
        menu.Items.Add(_gateItem);
        menu.Items.Add(_gameItem);
        menu.Items.Add(_pauseItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(_settingsItem);
        menu.Items.Add(_exitItem);

        _tray = new NotifyIcon { Visible = true, ContextMenuStrip = menu, Icon = _iconClosed, Text = "MouseFence" };
        _tray.DoubleClick += (s, e) => ToggleGate();

        _hotkey.HotKeyPressed += ToggleGate;
        _confineHotkey.HotKeyPressed += ToggleGame;

        Configure();
        _guard.GateOpen = !_settings.StartGateClosed;
        _guard.Start();
        RegisterHotkey();
        RebuildMenuText();
        UpdateUi();
    }

    private void Configure()
    {
        var monitors = MonitorInfo.All();

        var tops = (_settings.Mode == "Manual" && _settings.ManualMonitors.Count > 0
            ? monitors.Where(m => _settings.ManualMonitors.Contains(m.Device))
            : monitors.Where(MonitorInfo.IsAbovePrimary)).ToList();

        var topSet = new HashSet<string>(tops.Select(t => t.Device));
        var primary = monitors.FirstOrDefault(m => m.Primary) ?? monitors.FirstOrDefault();

        // Allowed crossings: configured UpLinks, else default = primary -> every top monitor.
        var links = _settings.UpLinks.Count > 0
            ? _settings.UpLinks
            : (primary != null
                ? tops.Select(t => new UpLink { FromDevice = primary.Device, ToDevice = t.Device }).ToList()
                : new List<UpLink>());

        var byDevice = monitors.ToDictionary(m => m.Device);
        var gates = new List<(int Min, int Max)>();
        foreach (var lk in links)
        {
            if (!topSet.Contains(lk.ToDevice)) continue;                 // target must be a top monitor
            if (!byDevice.TryGetValue(lk.FromDevice, out var from)) continue;
            if (!byDevice.TryGetValue(lk.ToDevice, out var to)) continue;

            int lo = Math.Max(from.Bounds.Left, to.Bounds.Left);
            int hi = Math.Min(from.Bounds.Right, to.Bounds.Right);
            if (hi <= lo) continue;                                       // no horizontal overlap
            int inset = Math.Min(GateInset, Math.Max(0, (hi - lo) / 4));
            gates.Add((lo + inset, hi - inset));
        }

        _guard.Configure(tops.Select(t => t.Bounds), gates, monitors.Select(m => m.Bounds));
    }

    private void RegisterHotkey()
    {
        if (!_hotkey.Register(_settings.Modifiers(), (uint)_settings.HotKey))
            _tray.ShowBalloonTip(3000, "MouseFence", Strings.TipHotkeyFail(_settings.HotKeyText()), ToolTipIcon.Warning);
        if (!_confineHotkey.Register(_settings.ConfineModifiers(), (uint)_settings.ConfineHotKey))
            _tray.ShowBalloonTip(3000, "MouseFence", Strings.TipHotkeyFail(_settings.ConfineHotKeyText()), ToolTipIcon.Warning);
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

    private void TogglePause()
    {
        if (_guard.Enabled) _guard.Stop();
        else _guard.Start();
        UpdateUi();
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
            _gameItem.Enabled = false;
            _pauseItem.Text = Strings.MenuResume;
            _pauseItem.Checked = true;
            return;
        }

        _gateItem.Enabled = true;
        _pauseItem.Text = Strings.MenuPause;
        _pauseItem.Checked = false;

        bool open = _guard.GateOpen;
        _tray.Icon = open ? _iconOpen : _iconClosed;
        _tray.Text = open ? Strings.TrayOpen(hk) : Strings.TrayClosed(hk);
        _gateItem.Text = open ? Strings.GateOpenMenu : Strings.GateClosedMenu;
        _gateItem.Checked = open;
        _gameItem.Checked = _guard.Confine;
        _gameItem.Enabled = true;
    }

    private void OpenSettings()
    {
        using var f = new SettingsForm(_settings, MonitorInfo.All());
        if (f.ShowDialog() != DialogResult.OK) return;

        _hotkey.Unregister();
        _confineHotkey.Unregister();
        _settings = f.Result;
        _settings.Save();
        Strings.Use(_settings.Language);
        AutoStart.Apply(_settings.AutoStart);
        Configure();
        RegisterHotkey();
        RebuildMenuText();
        UpdateUi();
    }

    private void ExitApp()
    {
        _tray.Visible = false;
        _guard.Dispose();
        _hotkey.Dispose();
        _confineHotkey.Dispose();
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
