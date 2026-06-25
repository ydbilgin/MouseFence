using System.Drawing;

namespace MouseFence;

/// <summary>
/// A brief, self-closing "which physical screen is this?" overlay drawn on ONE monitor (the MouseFence answer to
/// Windows Display Settings' "Identify"). It covers the monitor with a translucent tint (via <see cref="Form.Opacity"/>
/// — a proper layered window, NO TransparencyKey, so no fringe) and shows a bold frame border + the monitor's big
/// number + its resolution (and [MAIN] for the primary). The frame is the form's own background revealed by Padding
/// around an inner accent panel; the number/resolution are Labels — all GDI-control rendering, NO custom GDI+ paint
/// (a large-pixel-font DrawString was crashing the process). Top-most + WS_EX_NOACTIVATE so it never steals focus.
/// A life timer (started in OnLoad, before the window is interactable) closes it even if something throws, so it can
/// never strand on screen; it is also Owner-bound to the settings dialog. Triggered from the modal settings dialog,
/// whose message loop pumps the WM_TIMER that closes the overlays.
/// </summary>
internal sealed class IdentifyOverlay : Form
{
    private const int DurationMs = 1800;

    private readonly System.Windows.Forms.Timer _life;

    private IdentifyOverlay(MonitorInfo m, Theme theme)
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

        float pt = Math.Clamp(side / 14f, 36f, 150f);

        // ONE single-line Label only. On this hardware, ANY richer content on these layered, borderless, no-activate
        // overlays (a second docked Label, a multi-line Label, or custom GDI+ paint) reliably access-violates GDI
        // (0xC0000005) when the message loop pumps; a single single-line control is rock-solid. The number is the
        // identifier; the primary gets a star. (Resolution/role intentionally dropped for this robustness.)
        var label = new Label
        {
            Dock = DockStyle.Fill,
            Text = m.Primary ? $"{m.Index}  ★" : m.Index.ToString(),
            ForeColor = Color.White,
            TextAlign = ContentAlignment.MiddleCenter,
            Font = new Font("Segoe UI", pt, FontStyle.Bold),
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
    /// Flash a brief identify overlay on EVERY physical monitor. Each overlay self-closes via its own timer and is
    /// Owner-bound to <paramref name="owner"/> (the settings dialog) so closing the dialog also tears them down.
    /// </summary>
    public static void Flash(IEnumerable<MonitorInfo> monitors, Theme theme, IWin32Window owner)
    {
        var ownerForm = owner as Form;
        foreach (var m in monitors)
        {
            var f = new IdentifyOverlay(m, theme);
            if (ownerForm != null) f.Owner = ownerForm;   // closing the dialog tears these down too
            f.Show();                                      // self-closes via its life timer; GC reclaims it
        }
    }
}
