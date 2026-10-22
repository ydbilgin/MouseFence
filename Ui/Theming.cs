using System.Drawing;
using System.Drawing.Drawing2D;

namespace MouseFence;

internal readonly record struct Theme(
    Color Back, Color Surface, Color Surface2, Color Text, Color Subtle, Color Faint,
    Color Accent, Color AccentWeak, Color AccentLine, Color Border, Color BorderStrong,
    Color Chip, Color ChipText, Color Danger, Color Warning,
    Color FocusRing, Color DisabledSurface)
{
    public bool IsDark => Back.R < 128;
    public Color AccentText => IsDark ? Color.FromArgb(23, 27, 33) : Color.FromArgb(253, 253, 254);
    public Color Sidebar => IsDark ? Color.FromArgb(25, 30, 37) : Color.FromArgb(238, 241, 245);

    public static readonly Theme Light = new(
        Back: Color.FromArgb(246, 247, 249), Surface: Color.FromArgb(253, 253, 254),
        Surface2: Color.FromArgb(254, 254, 255), Text: Color.FromArgb(32, 37, 45),
        Subtle: Color.FromArgb(95, 105, 119), Faint: Color.FromArgb(132, 142, 156),
        Accent: Color.FromArgb(53, 111, 214), AccentWeak: Color.FromArgb(231, 238, 251),
        AccentLine: Color.FromArgb(169, 196, 242), Border: Color.FromArgb(210, 216, 224),
        BorderStrong: Color.FromArgb(183, 192, 203), Chip: Color.FromArgb(238, 241, 245),
        ChipText: Color.FromArgb(65, 73, 85),
        Danger: Color.FromArgb(180, 35, 24), Warning: Color.FromArgb(139, 93, 0),
        FocusRing: Color.FromArgb(169, 196, 242),
        DisabledSurface: Color.FromArgb(238, 241, 245));

    public static readonly Theme Dark = new(
        Back: Color.FromArgb(23, 27, 33), Surface: Color.FromArgb(29, 34, 42),
        Surface2: Color.FromArgb(35, 42, 52), Text: Color.FromArgb(232, 235, 240),
        Subtle: Color.FromArgb(173, 181, 194), Faint: Color.FromArgb(125, 136, 151),
        Accent: Color.FromArgb(116, 160, 242), AccentWeak: Color.FromArgb(37, 51, 74),
        AccentLine: Color.FromArgb(63, 93, 137), Border: Color.FromArgb(54, 63, 75),
        BorderStrong: Color.FromArgb(81, 93, 108), Chip: Color.FromArgb(42, 50, 61),
        ChipText: Color.FromArgb(202, 209, 219),
        Danger: Color.FromArgb(255, 139, 126), Warning: Color.FromArgb(240, 196, 109),
        FocusRing: Color.FromArgb(63, 93, 137),
        DisabledSurface: Color.FromArgb(25, 30, 37));

    public static readonly Theme HighContrast = new(
        Back: SystemColors.Window, Surface: SystemColors.Window, Surface2: SystemColors.Window,
        Text: SystemColors.WindowText, Subtle: SystemColors.WindowText, Faint: SystemColors.GrayText,
        Accent: SystemColors.Highlight, AccentWeak: SystemColors.Window, AccentLine: SystemColors.Highlight,
        Border: SystemColors.WindowText, BorderStrong: SystemColors.WindowText,
        Chip: SystemColors.Control, ChipText: SystemColors.ControlText,
        Danger: SystemColors.WindowText, Warning: SystemColors.WindowText,
        FocusRing: SystemColors.Highlight, DisabledSurface: SystemColors.Control);
}

internal enum ThemedButtonRole { Primary, Secondary, Quiet, DangerText }

internal static class Theming
{
    public static Theme Resolve(string pref) => SystemInformation.HighContrast ? Theme.HighContrast : pref switch
    {
        "Dark" => Theme.Dark,
        "Light" => Theme.Light,
        _ => IsSystemDark() ? Theme.Dark : Theme.Light,
    };

    public static bool IsSystemDark()
    {
        try
        {
            using var k = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(
                @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
            return k?.GetValue("AppsUseLightTheme") is int v && v == 0;
        }
        catch { return false; }
    }

    public static void Apply(Control root, Theme theme)
    {
        ApplyRec(root, theme);
        if (root is Form form) DarkTitleBar(form, theme);
    }

    private static void ApplyRec(Control control, Theme theme)
    {
        control.BackColor = theme.Back;
        control.ForeColor = theme.Text;
        switch (control)
        {
            case ThemedButton button: button.SetTheme(theme); break;
            case ThemedCheckBox check: check.SetTheme(theme); break;
            case ThemedControlFrame frame: frame.SetTheme(theme); break;
            case ThemedNumericUpDown numeric: numeric.SetTheme(theme); break;
            case ThemedSpinnerRail rail: rail.SetTheme(theme); break;
            case ThemedComboBox combo: combo.SetTheme(theme); break;
            case TextBox text:
                text.BackColor = text.Enabled ? theme.Surface2 : theme.DisabledSurface;
                text.ForeColor = text.Enabled ? theme.Text : theme.Faint;
                if (text.Parent is not ThemedControlFrame) text.BorderStyle = BorderStyle.FixedSingle;
                break;
            case NumericUpDown numeric:
                numeric.BackColor = numeric.Enabled ? theme.Surface2 : theme.DisabledSurface;
                numeric.ForeColor = numeric.Enabled ? theme.Text : theme.Faint;
                break;
            case CheckedListBox list: list.BackColor = theme.Surface; list.ForeColor = theme.Text; break;
            case ComboBox combo:
                combo.BackColor = theme.Surface2; combo.ForeColor = theme.Text; combo.FlatStyle = FlatStyle.Flat; break;
            case Button button:
                button.BackColor = theme.Surface2; button.ForeColor = theme.Text;
                button.FlatAppearance.BorderColor = theme.BorderStrong; break;
            case Panel panel when (panel.Tag as string) == "rule": panel.BackColor = theme.Border; break;
            case Panel panel when (panel.Tag as string) == "surface": panel.BackColor = theme.Surface; break;
            case Panel panel when (panel.Tag as string) == "surface2": panel.BackColor = theme.Surface2; break;
            case Panel panel when (panel.Tag as string) == "sidebar": panel.BackColor = theme.Sidebar; break;
            case Panel panel when (panel.Tag as string) == "accentWeak": panel.BackColor = theme.AccentWeak; break;
            case LinkLabel link: link.BackColor = link.Parent?.BackColor ?? theme.Back; break;
            case Label label:
                label.BackColor = label.Parent?.BackColor ?? theme.Back;
                label.ForeColor = (label.Tag as string) switch
                {
                    "head" => theme.Text,
                    "subtle" => theme.Subtle,
                    "faint" => theme.Faint,
                    "accent" => theme.Accent,
                    "warning" => theme.Warning,
                    "danger" => theme.Danger,
                    _ => theme.Text,
                };
                break;
            case CheckBox or RadioButton: control.BackColor = control.Parent?.BackColor ?? theme.Back; break;
        }
        foreach (Control child in control.Controls) ApplyRec(child, theme);
    }

    public static void DarkTitleBar(Form form, Theme theme)
    {
        if (!form.IsHandleCreated) return;
        int dark = theme.IsDark ? 1 : 0;
        if (Native.DwmSetWindowAttribute(form.Handle, Native.DWMWA_USE_IMMERSIVE_DARK_MODE, ref dark, sizeof(int)) != 0)
            Native.DwmSetWindowAttribute(form.Handle, Native.DWMWA_USE_IMMERSIVE_DARK_MODE_OLD, ref dark, sizeof(int));
        int caption = ToColorRef(theme.Surface), captionText = ToColorRef(theme.Text);
        Native.DwmSetWindowAttribute(form.Handle, Native.DWMWA_CAPTION_COLOR, ref caption, sizeof(int));
        Native.DwmSetWindowAttribute(form.Handle, Native.DWMWA_TEXT_COLOR, ref captionText, sizeof(int));
        Native.SetWindowPos(form.Handle, IntPtr.Zero, 0, 0, 0, 0,
            Native.SWP_NOMOVE | Native.SWP_NOSIZE | Native.SWP_NOZORDER | Native.SWP_NOACTIVATE | Native.SWP_FRAMECHANGED);
    }

    private static int ToColorRef(Color color) => color.R | (color.G << 8) | (color.B << 16);
}

internal sealed class ThemedButton : Button
{
    private Theme _theme = Theme.Light;
    private bool _hovered;
    private bool _pressed;
    public ThemedButtonRole Role = ThemedButtonRole.Secondary;

    public ThemedButton()
    {
        FlatStyle = FlatStyle.Flat;
        FlatAppearance.BorderSize = 0;
        Height = 36;
        Cursor = Cursors.Hand;
        SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer, true);
    }

    public void SetTheme(Theme theme) { _theme = theme; Invalidate(); }
    protected override void OnMouseEnter(EventArgs e) { _hovered = true; Invalidate(); base.OnMouseEnter(e); }
    protected override void OnMouseLeave(EventArgs e) { _hovered = false; _pressed = false; Invalidate(); base.OnMouseLeave(e); }
    protected override void OnMouseDown(MouseEventArgs e) { _pressed = true; Invalidate(); base.OnMouseDown(e); }
    protected override void OnMouseUp(MouseEventArgs e) { _pressed = false; Invalidate(); base.OnMouseUp(e); }
    protected override void OnEnabledChanged(EventArgs e) { Invalidate(); base.OnEnabledChanged(e); }
    protected override void OnPaint(PaintEventArgs e)
    {
        e.Graphics.Clear(Parent?.BackColor ?? _theme.Back);
        e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
        Color fill = Role == ThemedButtonRole.Primary ? _theme.Accent : _theme.Surface2;
        Color text = Role == ThemedButtonRole.Primary ? _theme.AccentText : Role == ThemedButtonRole.DangerText ? _theme.Danger : _theme.Text;
        Color border = Role is ThemedButtonRole.Quiet or ThemedButtonRole.DangerText ? Color.Transparent : _theme.BorderStrong;
        if (!Enabled) { fill = _theme.DisabledSurface; text = _theme.Faint; border = Role == ThemedButtonRole.Quiet ? Color.Transparent : _theme.Border; }
        else if (_pressed) fill = Role == ThemedButtonRole.Primary ? ControlPaint.Dark(_theme.Accent, .08f) : _theme.AccentWeak;
        else if (_hovered) fill = Role == ThemedButtonRole.Primary ? ControlPaint.Light(_theme.Accent, .08f) : _theme.AccentWeak;
        using var path = Rounded(ClientRectangle, 5);
        using (var brush = new SolidBrush(fill)) e.Graphics.FillPath(brush, path);
        if (border.A > 0) using (var pen = new Pen(border)) e.Graphics.DrawPath(pen, path);
        TextRenderer.DrawText(e.Graphics, Text, Font, ClientRectangle, text,
            TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix);
        if (Focused && ShowFocusCues)
        {
            var focus = ClientRectangle; focus.Inflate(-2, -2);
            using var pen = new Pen(_theme.FocusRing, 2);
            using var focusPath = Rounded(focus, 4);
            e.Graphics.DrawPath(pen, focusPath);
        }
    }

    private static GraphicsPath Rounded(Rectangle rect, int radius)
    {
        rect.Width = Math.Max(1, rect.Width - 1); rect.Height = Math.Max(1, rect.Height - 1);
        int d = Math.Min(radius * 2, Math.Min(rect.Width, rect.Height));
        var path = new GraphicsPath(); path.AddArc(rect.Left, rect.Top, d, d, 180, 90); path.AddArc(rect.Right - d, rect.Top, d, d, 270, 90);
        path.AddArc(rect.Right - d, rect.Bottom - d, d, d, 0, 90); path.AddArc(rect.Left, rect.Bottom - d, d, d, 90, 90); path.CloseFigure(); return path;
    }
}

internal sealed class ThemedCheckBox : CheckBox
{
    private Theme _theme = Theme.Light;
    private bool _hovered;
    public ThemedCheckBox()
    {
        AutoSize = false;
        Height = 32;
        SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer, true);
    }
    public void SetTheme(Theme theme) { _theme = theme; Invalidate(); }
    protected override void OnMouseEnter(EventArgs e) { _hovered = true; Invalidate(); base.OnMouseEnter(e); }
    protected override void OnMouseLeave(EventArgs e) { _hovered = false; Invalidate(); base.OnMouseLeave(e); }
    protected override void OnCheckedChanged(EventArgs e) { Invalidate(); base.OnCheckedChanged(e); }
    protected override void OnEnabledChanged(EventArgs e) { Invalidate(); base.OnEnabledChanged(e); }
    protected override void OnPaint(PaintEventArgs e)
    {
        e.Graphics.Clear(Parent?.BackColor ?? _theme.Back);
        e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
        int box = Math.Max(18, (int)Math.Round(18 * DeviceDpi / 96f));
        int x = 2, y = (Height - box) / 2;
        var rect = new Rectangle(x, y, box, box);
        Color fill = !Enabled ? _theme.DisabledSurface : Checked ? _theme.Accent : _theme.Surface2;
        Color border = Focused ? _theme.Accent : _hovered ? _theme.Subtle : _theme.BorderStrong;
        using (var brush = new SolidBrush(fill)) e.Graphics.FillRectangle(brush, rect);
        using (var pen = new Pen(border, Focused ? 2 : 1)) e.Graphics.DrawRectangle(pen, rect);
        if (Checked)
        {
            using var pen = new Pen(Enabled ? _theme.AccentText : _theme.Faint, Math.Max(2, box / 8f)) { StartCap = LineCap.Round, EndCap = LineCap.Round };
            e.Graphics.DrawLines(pen, new[] { new Point(x + box / 4, y + box / 2), new Point(x + box * 7 / 16, y + box * 11 / 16), new Point(x + box * 3 / 4, y + box / 3) });
        }
        if (Focused && ShowFocusCues)
        {
            var ring = rect; ring.Inflate(3, 3); using var pen = new Pen(_theme.FocusRing, 2); e.Graphics.DrawRectangle(pen, ring);
        }
        var textRect = new Rectangle(rect.Right + 9, 0, Math.Max(1, Width - rect.Right - 9), Height);
        TextRenderer.DrawText(e.Graphics, Text, Font, textRect, Enabled ? _theme.Text : _theme.Faint,
            TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.WordEllipsis | TextFormatFlags.NoPrefix);
    }
}

internal sealed class ThemedControlFrame : Panel
{
    private Theme _theme = Theme.Light;
    private bool _hovered;
    public bool Error;
    public Control FramedControl;
    public ThemedControlFrame()
    {
        Height = 36;
        Padding = new Padding(7, 5, 7, 5);
        SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer, true);
    }
    public void Host(Control control)
    {
        FramedControl = control; control.Dock = DockStyle.Fill;
        if (control is TextBox text) text.BorderStyle = BorderStyle.None;
        Controls.Add(control);
        control.GotFocus += (_, _) => Invalidate(); control.LostFocus += (_, _) => Invalidate();
        control.MouseEnter += (_, _) => { _hovered = true; Invalidate(); };
        control.MouseLeave += (_, _) => { _hovered = false; Invalidate(); };
    }
    public void SetTheme(Theme theme) { _theme = theme; BackColor = Enabled ? theme.Surface2 : theme.DisabledSurface; Invalidate(); }
    protected override void OnMouseEnter(EventArgs e) { _hovered = true; Invalidate(); base.OnMouseEnter(e); }
    protected override void OnMouseLeave(EventArgs e) { _hovered = false; Invalidate(); base.OnMouseLeave(e); }
    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e); e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
        Color border = Error ? _theme.Danger : FramedControl?.Focused == true ? _theme.Accent : _hovered ? _theme.Subtle : _theme.BorderStrong;
        using var path = Rounded(new Rectangle(0, 0, Math.Max(1, Width - 1), Math.Max(1, Height - 1)), 5);
        using var pen = new Pen(border); e.Graphics.DrawPath(pen, path);
        if (Error || FramedControl?.Focused == true)
        {
            var outer = new Rectangle(2, 2, Math.Max(1, Width - 5), Math.Max(1, Height - 5));
            using var ring = new Pen(Error ? Color.FromArgb(110, _theme.Danger) : _theme.FocusRing, 2);
            using var outerPath = Rounded(outer, 4);
            e.Graphics.DrawPath(ring, outerPath);
        }
    }
    private static GraphicsPath Rounded(Rectangle rect, int radius)
    {
        int d = Math.Max(2, Math.Min(radius * 2, Math.Min(rect.Width, rect.Height))); var path = new GraphicsPath();
        path.AddArc(rect.Left, rect.Top, d, d, 180, 90); path.AddArc(rect.Right - d, rect.Top, d, d, 270, 90);
        path.AddArc(rect.Right - d, rect.Bottom - d, d, d, 0, 90); path.AddArc(rect.Left, rect.Bottom - d, d, d, 90, 90); path.CloseFigure(); return path;
    }
}

internal sealed class ThemedNumericUpDown : NumericUpDown
{
    private Theme _theme = Theme.Light;
    private SpinnerOverlay _overlay;
    public void SetTheme(Theme theme)
    {
        _theme = theme; BackColor = Enabled ? theme.Surface2 : theme.DisabledSurface; ForeColor = Enabled ? theme.Text : theme.Faint;
        _overlay?.SetTheme(theme); Invalidate();
    }
    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e); _overlay ??= new SpinnerOverlay(); Controls.Add(_overlay); LayoutOverlay(); _overlay.SetTheme(_theme); _overlay.BringToFront();
        foreach (Control child in Controls) if (child.IsHandleCreated) _ = Native.SetWindowTheme(child.Handle, _theme.IsDark ? "DarkMode_Explorer" : "Explorer", null);
    }
    protected override void OnResize(EventArgs e) { base.OnResize(e); LayoutOverlay(); }
    private void LayoutOverlay()
    {
        if (_overlay == null) return;
        int rail = Math.Max(20, (int)Math.Round(20 * DeviceDpi / 96f));
        _overlay.Bounds = new Rectangle(Math.Max(0, Width - rail - 1), 1, rail, Math.Max(1, Height - 2));
        _overlay.BringToFront();
    }
    private sealed class SpinnerOverlay : Control
    {
        private const int WmNcHitTest = 0x0084, HtTransparent = -1;
        private Theme _theme = Theme.Light;
        public SpinnerOverlay()
        {
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer, true);
        }
        public void SetTheme(Theme theme) { _theme = theme; Invalidate(); }
        protected override void WndProc(ref Message m) { if (m.Msg == WmNcHitTest) { m.Result = (IntPtr)HtTransparent; return; } base.WndProc(ref m); }
        protected override void OnPaint(PaintEventArgs e)
        {
            using var fill = new SolidBrush(_theme.Sidebar); e.Graphics.FillRectangle(fill, ClientRectangle);
            using var separator = new Pen(_theme.Border); e.Graphics.DrawLine(separator, 0, 0, 0, Height);
            e.Graphics.DrawLine(separator, 0, Height / 2, Width, Height / 2);
            using var glyph = new Pen(_theme.Subtle, 1.4f) { StartCap = LineCap.Round, EndCap = LineCap.Round };
            int cx = Width / 2; e.Graphics.DrawLines(glyph, new[] { new Point(cx - 3, Height / 4 + 1), new Point(cx, Height / 4 - 2), new Point(cx + 3, Height / 4 + 1) });
            e.Graphics.DrawLines(glyph, new[] { new Point(cx - 3, Height * 3 / 4 - 1), new Point(cx, Height * 3 / 4 + 2), new Point(cx + 3, Height * 3 / 4 - 1) });
        }
    }
}

internal sealed class ThemedSpinnerRail : Control
{
    private Theme _theme = Theme.Light;
    private bool _hovered;
    public ThemedNumericUpDown Owner;
    public ThemedSpinnerRail() { Cursor = Cursors.Hand; SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer, true); }
    public void SetTheme(Theme theme) { _theme = theme; Invalidate(); }
    protected override void OnMouseMove(MouseEventArgs e) { _hovered = true; Invalidate(); base.OnMouseMove(e); }
    protected override void OnMouseLeave(EventArgs e) { _hovered = false; Invalidate(); base.OnMouseLeave(e); }
    protected override void OnMouseDown(MouseEventArgs e)
    {
        if (Owner?.Enabled == true && e.Button == MouseButtons.Left)
        {
            decimal delta = e.Y < Height / 2 ? Owner.Increment : -Owner.Increment;
            Owner.Value = Math.Clamp(Owner.Value + delta, Owner.Minimum, Owner.Maximum); Owner.Focus();
        }
        base.OnMouseDown(e);
    }
    protected override void OnPaint(PaintEventArgs e)
    {
        using var fill = new SolidBrush(_hovered ? _theme.AccentWeak : _theme.Sidebar); e.Graphics.FillRectangle(fill, ClientRectangle);
        using var separator = new Pen(_theme.Border); e.Graphics.DrawLine(separator, 0, 0, 0, Height); e.Graphics.DrawLine(separator, 0, Height / 2, Width, Height / 2);
        int cx = Width / 2; using var glyph = new Pen(_theme.Subtle, 1.4f) { StartCap = LineCap.Round, EndCap = LineCap.Round };
        e.Graphics.DrawLines(glyph, new[] { new Point(cx - 3, Height / 4 + 1), new Point(cx, Height / 4 - 2), new Point(cx + 3, Height / 4 + 1) });
        e.Graphics.DrawLines(glyph, new[] { new Point(cx - 3, Height * 3 / 4 - 1), new Point(cx, Height * 3 / 4 + 2), new Point(cx + 3, Height * 3 / 4 - 1) });
    }
}

internal sealed class ThemedComboBox : ComboBox
{
    private Theme _theme = Theme.Light;
    private bool _hovered;
    private ComboOverlay _overlay;
    public ThemedComboBox()
    {
        DropDownStyle = ComboBoxStyle.DropDownList; FlatStyle = FlatStyle.Flat; DrawMode = DrawMode.OwnerDrawFixed;
        ItemHeight = 24; SetStyle(ControlStyles.OptimizedDoubleBuffer, true);
    }
    public void SetTheme(Theme theme)
    {
        _theme = theme; BackColor = Enabled ? theme.Surface2 : theme.DisabledSurface; ForeColor = Enabled ? theme.Text : theme.Faint;
        _overlay?.SetTheme(theme, Focused || _hovered); Invalidate();
    }
    protected override void OnDrawItem(DrawItemEventArgs e)
    {
        if (e.Index < 0) return; bool selected = (e.State & DrawItemState.Selected) != 0;
        using var fill = new SolidBrush(selected ? _theme.AccentWeak : _theme.Surface2); e.Graphics.FillRectangle(fill, e.Bounds);
        TextRenderer.DrawText(e.Graphics, GetItemText(Items[e.Index]), Font, Rectangle.Inflate(e.Bounds, -7, 0), _theme.Text,
            TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix);
    }
    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e); _overlay ??= new ComboOverlay(this); Controls.Add(_overlay); LayoutOverlay(); _overlay.SetTheme(_theme, Focused || _hovered); _overlay.BringToFront();
    }
    protected override void OnResize(EventArgs e) { base.OnResize(e); LayoutOverlay(); }
    protected override void OnSelectedIndexChanged(EventArgs e) { base.OnSelectedIndexChanged(e); _overlay?.Invalidate(); }
    protected override void OnMouseEnter(EventArgs e) { _hovered = true; UpdateOverlay(); base.OnMouseEnter(e); }
    protected override void OnMouseLeave(EventArgs e) { _hovered = false; UpdateOverlay(); base.OnMouseLeave(e); }
    protected override void OnGotFocus(EventArgs e) { UpdateOverlay(); base.OnGotFocus(e); }
    protected override void OnLostFocus(EventArgs e) { UpdateOverlay(); base.OnLostFocus(e); }
    private void UpdateOverlay() { Invalidate(); _overlay?.SetTheme(_theme, Focused || _hovered); }
    private void LayoutOverlay() { if (_overlay != null && Width > 2 && Height > 2) { _overlay.Bounds = ClientRectangle; _overlay.BringToFront(); } }
    private sealed class ComboOverlay : Control
    {
        private const int WmNcHitTest = 0x0084, HtTransparent = -1;
        private Theme _theme = Theme.Light; private bool _strong; private readonly ThemedComboBox _owner;
        public ComboOverlay(ThemedComboBox owner) { _owner = owner; SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.SupportsTransparentBackColor, true); BackColor = Color.Transparent; }
        public void SetTheme(Theme theme, bool strong) { _theme = theme; _strong = strong; Invalidate(); }
        protected override void WndProc(ref Message m) { if (m.Msg == WmNcHitTest) { m.Result = (IntPtr)HtTransparent; return; } base.WndProc(ref m); }
        protected override void OnPaint(PaintEventArgs e)
        {
            int arrowWidth = Math.Min(30, Math.Max(24, Height));
            using (var fill = new SolidBrush(_owner.Enabled ? _theme.Surface2 : _theme.DisabledSurface)) e.Graphics.FillRectangle(fill, ClientRectangle);
            using (var border = new Pen(_strong ? _theme.Accent : _theme.BorderStrong)) e.Graphics.DrawRectangle(border, 0, 0, Width - 1, Height - 1);
            string selected = _owner.SelectedIndex >= 0 ? _owner.GetItemText(_owner.SelectedItem) : "";
            TextRenderer.DrawText(e.Graphics, selected, _owner.Font, new Rectangle(8, 1, Math.Max(0, Width - arrowWidth - 10), Height - 2),
                _owner.Enabled ? _theme.Text : _theme.Faint, TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix);
            int cx = Width - arrowWidth / 2, cy = Height / 2; using var glyph = new Pen(_theme.Subtle, 1.6f) { StartCap = LineCap.Round, EndCap = LineCap.Round };
            e.Graphics.DrawLines(glyph, new[] { new Point(cx - 4, cy - 2), new Point(cx, cy + 2), new Point(cx + 4, cy - 2) });
        }
    }
}

/// <summary>Tray and context menus in the app theme: themed surface, accent tick, rounded hover, shortcut on the right.</summary>
internal sealed class ThemedMenuRenderer : ToolStripProfessionalRenderer
{
    public Theme Theme { get; }
    public ThemedMenuRenderer(Theme theme) : base(new ThemedMenuColors(theme)) { Theme = theme; RoundedEdges = false; }

    protected override void OnRenderToolStripBackground(ToolStripRenderEventArgs e)
    {
        using var fill = new SolidBrush(Theme.Surface); e.Graphics.FillRectangle(fill, e.AffectedBounds);
    }
    protected override void OnRenderToolStripBorder(ToolStripRenderEventArgs e)
    {
        using var pen = new Pen(Theme.BorderStrong);
        e.Graphics.DrawRectangle(pen, 0, 0, e.ToolStrip.Width - 1, e.ToolStrip.Height - 1);
    }
    protected override void OnRenderImageMargin(ToolStripRenderEventArgs e)
    {
        using var fill = new SolidBrush(Theme.Surface); e.Graphics.FillRectangle(fill, e.AffectedBounds);
    }
    protected override void OnRenderMenuItemBackground(ToolStripItemRenderEventArgs e)
    {
        if (!e.Item.Selected || !e.Item.Enabled) return;
        e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
        var rect = new Rectangle(4, 1, e.Item.Width - 8, e.Item.Height - 2);
        using var path = new GraphicsPath();
        int d = 8;
        path.AddArc(rect.Left, rect.Top, d, d, 180, 90); path.AddArc(rect.Right - d, rect.Top, d, d, 270, 90);
        path.AddArc(rect.Right - d, rect.Bottom - d, d, d, 0, 90); path.AddArc(rect.Left, rect.Bottom - d, d, d, 90, 90); path.CloseFigure();
        using var fill = new SolidBrush(Theme.AccentWeak); e.Graphics.FillPath(fill, path);
    }
    protected override void OnRenderItemText(ToolStripItemTextRenderEventArgs e)
    {
        bool shortcut = e.Item is ToolStripMenuItem item && !string.IsNullOrEmpty(item.ShortcutKeyDisplayString)
            && e.Text == item.ShortcutKeyDisplayString;
        e.TextColor = !e.Item.Enabled ? Theme.Faint : shortcut ? Theme.Subtle : Theme.Text;
        base.OnRenderItemText(e);
    }
    protected override void OnRenderItemCheck(ToolStripItemImageRenderEventArgs e)
    {
        e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
        var r = e.ImageRectangle; int s = Math.Min(r.Width, r.Height);
        var box = new Rectangle(r.Left + (r.Width - s) / 2, r.Top + (r.Height - s) / 2, s, s);
        using var pen = new Pen(e.Item.Enabled ? Theme.Accent : Theme.Faint, Math.Max(2f, s / 8f)) { StartCap = LineCap.Round, EndCap = LineCap.Round };
        e.Graphics.DrawLines(pen, new[] { new Point(box.Left + s / 5, box.Top + s / 2), new Point(box.Left + s * 2 / 5, box.Top + s * 7 / 10), new Point(box.Right - s / 5, box.Top + s * 3 / 10) });
    }
    protected override void OnRenderSeparator(ToolStripSeparatorRenderEventArgs e)
    {
        using var pen = new Pen(Theme.Border); int y = e.Item.Height / 2;
        e.Graphics.DrawLine(pen, 10, y, e.Item.Width - 10, y);
    }
    protected override void OnRenderArrow(ToolStripArrowRenderEventArgs e) { e.ArrowColor = Theme.Subtle; base.OnRenderArrow(e); }

    private sealed class ThemedMenuColors : ProfessionalColorTable
    {
        private readonly Theme _t;
        public ThemedMenuColors(Theme theme) { _t = theme; UseSystemColors = false; }
        public override Color ToolStripDropDownBackground => _t.Surface;
        public override Color ImageMarginGradientBegin => _t.Surface;
        public override Color ImageMarginGradientMiddle => _t.Surface;
        public override Color ImageMarginGradientEnd => _t.Surface;
        public override Color MenuBorder => _t.BorderStrong;
        public override Color MenuItemBorder => Color.Transparent;
        public override Color MenuItemSelected => _t.AccentWeak;
        public override Color SeparatorDark => _t.Border;
        public override Color SeparatorLight => _t.Border;
        public override Color CheckBackground => Color.Transparent;
        public override Color CheckSelectedBackground => Color.Transparent;
        public override Color CheckPressedBackground => Color.Transparent;
    }
}
