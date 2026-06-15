using System.Drawing;

namespace MouseFence;

internal readonly record struct Theme(Color Back, Color Surface, Color Text, Color Subtle, Color Accent, Color Border)
{
    public bool IsDark => Back.R < 128;

    public static readonly Theme Light = new(
        Back: Color.White,
        Surface: Color.White,
        Text: Color.FromArgb(32, 32, 32),
        Subtle: Color.FromArgb(110, 110, 110),
        Accent: Color.FromArgb(0, 120, 212),
        Border: Color.FromArgb(204, 204, 204));

    public static readonly Theme Dark = new(
        Back: Color.FromArgb(32, 32, 32),
        Surface: Color.FromArgb(45, 45, 45),
        Text: Color.FromArgb(240, 240, 240),
        Subtle: Color.FromArgb(165, 165, 165),
        Accent: Color.FromArgb(76, 160, 230),
        Border: Color.FromArgb(70, 70, 70));
}

/// <summary>Minimal light/dark theming for the one settings window. Tag controls to get accents.</summary>
internal static class Theming
{
    public static Theme Resolve(string pref) => pref switch
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
            return k?.GetValue("AppsUseLightTheme") is int v && v == 0;   // 0 = dark
        }
        catch { return false; }
    }

    public static void Apply(Control root, Theme t)
    {
        ApplyRec(root, t);
        if (root is Form f) DarkTitleBar(f, t.IsDark);
    }

    private static void ApplyRec(Control c, Theme t)
    {
        c.BackColor = t.Back;
        c.ForeColor = t.Text;

        switch (c)
        {
            case TextBox tb:
                tb.BackColor = t.Surface;
                tb.BorderStyle = BorderStyle.FixedSingle;
                break;
            case CheckedListBox clb:
                clb.BackColor = t.Surface;
                clb.ForeColor = t.Text;
                break;
            case ComboBox cb:
                cb.BackColor = t.Surface;
                cb.ForeColor = t.Text;
                cb.FlatStyle = t.IsDark ? FlatStyle.Flat : FlatStyle.Standard;
                break;
            case GroupBox gb:
                gb.ForeColor = t.Subtle;
                break;
            case Button b when (b.Tag as string) == "primary":
                b.BackColor = t.Accent;
                b.ForeColor = Color.White;
                b.FlatAppearance.BorderSize = 0;
                break;
            case Button b:
                b.BackColor = t.Surface;
                b.ForeColor = t.Text;
                b.FlatAppearance.BorderColor = t.Border;
                break;
            case Panel p when (p.Tag as string) == "rule":
                p.BackColor = t.Border;
                break;
            case Label lbl when (lbl.Tag as string) == "head":
                lbl.ForeColor = t.Accent;
                break;
            case Label lbl when (lbl.Tag as string) == "subtle":
                lbl.ForeColor = t.Subtle;
                break;
        }

        foreach (Control child in c.Controls)
            ApplyRec(child, t);
    }

    public static void DarkTitleBar(Form f, bool dark)
    {
        if (!f.IsHandleCreated) return;
        int v = dark ? 1 : 0;
        Native.DwmSetWindowAttribute(f.Handle, Native.DWMWA_USE_IMMERSIVE_DARK_MODE, ref v, sizeof(int));
    }
}
