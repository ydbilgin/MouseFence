using System.Drawing;

namespace MouseFence;

/// <summary>
/// A brief, self-closing "which physical screen is this?" overlay drawn on ONE monitor (the MouseFence answer to
/// Windows Display Settings' "Identify"). It covers the monitor with a translucent tint (via <see cref="Form.Opacity"/>
/// — a proper layered window, NO TransparencyKey, so no fringe) and shows a bold frame plus the monitor's big
/// MouseFence number and optional nickname. The content uses a normal WinForms Label — NO custom GDI+ paint
/// (a large-pixel-font DrawString was crashing the process). Top-most + WS_EX_NOACTIVATE so it never steals focus.
/// A life timer (started in OnLoad, before the window is interactable) closes it even if something throws, so it can
/// never strand on screen; it is also Owner-bound to the settings dialog. Triggered from the modal settings dialog,
/// whose message loop pumps the WM_TIMER that closes the overlays.
/// </summary>
internal sealed class IdentifyOverlay : Form
{
    private const int DurationMs = 1800;

    private readonly System.Windows.Forms.Timer _life;

    private IdentifyOverlay(MonitorInfo m, string monitorName, Theme theme)
    {
        int w = m.Bounds.Width, h = m.Bounds.Height;
        int side = Math.Min(w, h);
        int frame = Math.Max(10, side / 45);

        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        TopMost = true;
        BackColor = theme.Accent;
        Opacity = 0.62;                          // uniform alpha (layered) — see-through, no color-key fringe
        Bounds = Rectangle.FromLTRB(m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom);

        string displayText = DisplayText(m, monitorName);
        float pt = Math.Clamp(Math.Min(side / 14f,
            w / Math.Max(1f, displayText.Length * 0.72f)), 24f, 150f);

        // ONE single-line Label only. On this hardware, ANY richer content on these layered, borderless, no-activate
        // overlays (a second docked Label, a multi-line Label, or custom GDI+ paint) reliably access-violates GDI
        // (0xC0000005) when the message loop pumps; a single single-line control is rock-solid. The number is the
        // identifier; the primary gets a star. A nickname only enriches this SAME one-line Text value.
        var label = new Label
        {
            Dock = DockStyle.Fill,
            Text = displayText,
            ForeColor = Color.White,
            TextAlign = ContentAlignment.MiddleCenter,
            Font = new Font("Segoe UI", pt, FontStyle.Bold),
            AutoEllipsis = true,
            UseMnemonic = false,
        };
        Controls.Add(label);

        _life = new System.Windows.Forms.Timer { Interval = DurationMs };
        _life.Tick += (s, e) => Close();
    }

    // Show without taking focus from the settings dialog; keep out of Alt-Tab.
    protected override bool ShowWithoutActivation => true;

    protected override CreateParams CreateParams
    {
        get
        {
            var cp = base.CreateParams;
            cp.ExStyle |= Native.WS_EX_NOACTIVATE | Native.WS_EX_TOOLWINDOW;
            return cp;
        }
    }

    protected override void OnLoad(EventArgs e)
    {
        base.OnLoad(e);
        _life.Start();   // started before the overlay is interactable -> it can never strand on screen
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        _life.Stop();
        _life.Dispose();
        base.OnFormClosed(e);
    }

    /// <summary>
    /// Flash a brief identify overlay on one physical monitor. The overlay keeps the same hardware-safe rendering
    /// model as the all-monitor path: exactly one, single-line <see cref="Label"/>.
    /// </summary>
    public static void Flash(MonitorInfo m, Theme theme, IWin32Window owner)
        => Flash(m, null, theme, owner);

    public static void Flash(MonitorInfo m, string monitorName, Theme theme, IWin32Window owner)
    {
        if (m == null) return;
        var f = new IdentifyOverlay(m, monitorName, theme);
        if (owner is Form ownerForm) f.Owner = ownerForm;
        f.Show();
    }

    /// <summary>
    /// Flash a brief identify overlay on EVERY physical monitor. Each overlay self-closes via its own timer and is
    /// Owner-bound to <paramref name="owner"/> (the settings dialog) so closing the dialog also tears them down.
    /// </summary>
    public static void Flash(IEnumerable<MonitorInfo> monitors, Theme theme, IWin32Window owner)
        => Flash(monitors, null, theme, owner);

    public static void Flash(IEnumerable<MonitorInfo> monitors, IReadOnlyDictionary<string, string> monitorNames,
                             Theme theme, IWin32Window owner)
    {
        foreach (var m in monitors)
        {
            string name = Settings.MonitorNameFor(monitorNames, m.StableId);
            Flash(m, name, theme, owner);
        }
    }

    internal static string DisplayText(MonitorInfo monitor, string monitorName)
    {
        string number = monitor.Primary ? $"{monitor.Index}  ★" : monitor.Index.ToString();
        string name = Settings.NormalizeMonitorName(monitorName);
        return name.Length == 0 ? number : $"{number} — {name}";
    }
}
