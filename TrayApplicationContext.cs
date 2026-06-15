using System.Drawing;
using System.Runtime.InteropServices;

namespace MouseFence;

/// <summary>Owns the tray icon, the mouse guard, the global hotkey, and the settings.</summary>
public sealed class TrayApplicationContext : ApplicationContext
{
    [DllImport("user32.dll")]
    private static extern bool DestroyIcon(IntPtr hIcon);

    private readonly NotifyIcon _tray;
    private readonly MouseGuard _guard = new();
    private readonly HotKeyWindow _hotkey = new();
    private readonly ToolStripMenuItem _gateItem;
    private readonly ToolStripMenuItem _pauseItem;

    private Settings _settings;
    private Icon _iconClosed;   // red    = main -> top blocked
    private Icon _iconOpen;     // green  = main -> top allowed
    private Icon _iconPaused;   // grey   = tool paused

    public TrayApplicationContext()
    {
        _settings = Settings.Load();

        _iconClosed = MakeIcon(Color.FromArgb(220, 60, 60));
        _iconOpen = MakeIcon(Color.FromArgb(60, 180, 75));
        _iconPaused = MakeIcon(Color.FromArgb(150, 150, 150));

        _gateItem = new ToolStripMenuItem("Ana→üst geçiş", null, (s, e) => ToggleGate());
        _pauseItem = new ToolStripMenuItem("Aracı duraklat", null, (s, e) => TogglePause());

        var menu = new ContextMenuStrip();
        menu.Items.Add(_gateItem);
        menu.Items.Add(_pauseItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(new ToolStripMenuItem("Ayarlar...", null, (s, e) => OpenSettings()));
        menu.Items.Add(new ToolStripMenuItem("Çıkış", null, (s, e) => ExitApp()));

        _tray = new NotifyIcon
        {
            Visible = true,
            ContextMenuStrip = menu,
            Icon = _iconClosed,
            Text = "MouseFence",
        };
        _tray.DoubleClick += (s, e) => ToggleGate();

        _hotkey.HotKeyPressed += ToggleGate;

        Configure();
        _guard.GateOpen = !_settings.StartGateClosed;
        _guard.Start();
        RegisterHotkey();
        UpdateUi();
    }

    private void Configure()
    {
        var monitors = MonitorInfo.All();

        IEnumerable<MonitorInfo> blocked =
            _settings.Mode == "Manual" && _settings.ManualMonitors.Count > 0
                ? monitors.Where(m => _settings.ManualMonitors.Contains(m.Device))
                : monitors.Where(MonitorInfo.IsAbovePrimary);

        var primary = monitors.FirstOrDefault(m => m.Primary) ?? monitors.FirstOrDefault();
        int gateLeft = primary?.Bounds.Left ?? 0;
        int gateRight = primary?.Bounds.Right ?? 0;

        _guard.Configure(blocked.Select(b => b.Bounds), gateLeft, gateRight);
    }

    private void RegisterHotkey()
    {
        if (!_hotkey.Register(_settings.Modifiers(), (uint)_settings.HotKey))
            _tray.ShowBalloonTip(3000, "MouseFence",
                $"Kısayol kaydedilemedi: {_settings.HotKeyText()} — başka bir uygulama kullanıyor olabilir.",
                ToolTipIcon.Warning);
    }

    private void ToggleGate()
    {
        _guard.GateOpen = !_guard.GateOpen;
        UpdateUi();
        if (_guard.Enabled)
            _tray.ShowBalloonTip(1000, "MouseFence",
                _guard.GateOpen
                    ? "Ana ekrandan üst ekrana geçiş AÇIK 🔓  (yan ekranlar yine kapalı)"
                    : "Üst ekrana geçiş KAPALI 🔒",
                ToolTipIcon.Info);
    }

    private void TogglePause()
    {
        if (_guard.Enabled) _guard.Stop();
        else _guard.Start();
        UpdateUi();
    }

    private void UpdateUi()
    {
        if (!_guard.Enabled)
        {
            _tray.Icon = _iconPaused;
            _tray.Text = $"MouseFence: DURAKLATILDI ({_settings.HotKeyText()})";
            _gateItem.Text = "Ana→üst geçiş (duraklatıldı)";
            _gateItem.Enabled = false;
            _pauseItem.Text = "Aracı sürdür";
            _pauseItem.Checked = true;
            return;
        }

        _gateItem.Enabled = true;
        _pauseItem.Text = "Aracı duraklat";
        _pauseItem.Checked = false;

        bool open = _guard.GateOpen;
        _tray.Icon = open ? _iconOpen : _iconClosed;
        _tray.Text = (open ? "MouseFence: ana→üst AÇIK" : "MouseFence: ana→üst KAPALI")
                     + $" ({_settings.HotKeyText()})";
        _gateItem.Text = open ? "Ana→üst geçiş: AÇIK — kapat" : "Ana→üst geçiş: KAPALI — aç";
        _gateItem.Checked = open;
    }

    private void OpenSettings()
    {
        using var f = new SettingsForm(_settings, MonitorInfo.All());
        if (f.ShowDialog() != DialogResult.OK) return;

        _hotkey.Unregister();
        _settings = f.Result;
        _settings.Save();
        AutoStart.Apply(_settings.AutoStart);
        Configure();
        RegisterHotkey();
        UpdateUi();
    }

    private void ExitApp()
    {
        _tray.Visible = false;
        _guard.Dispose();
        _hotkey.Dispose();
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
            g.FillRectangle(white, 11, 12, 10, 7);   // little monitor glyph
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
