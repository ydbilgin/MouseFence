using System.Drawing;
using System.Drawing.Imaging;

namespace MouseFence;

/// <summary>
/// Dev-only helper (behind the --screenshot arg): renders the settings window to a PNG without ever
/// showing it on a visible monitor — the form is placed far off the virtual desktop and captured via
/// PrintWindow (which renders the whole window, title bar included, even when off-screen).
/// </summary>
internal static class Screenshot
{
    public static void CaptureSettings(string path, string themeOverride = null, int tabIndex = 0, string lang = "en")
    {
        var s = Settings.Load();
        s.Language = string.IsNullOrEmpty(lang) ? "en" : lang;
        if (!string.IsNullOrEmpty(themeOverride)) s.Theme = themeOverride;   // "Light" | "Dark"
        Strings.Use(s.Language);

        using var form = new SettingsForm(s, MonitorInfo.All());
        form.StartPosition = FormStartPosition.Manual;
        form.Location = new Point(6000, 2000);   // beyond all monitors -> never visible
        form.ShowInTaskbar = false;
        form.Show();
        form.Activate();
        form.SelectTab(tabIndex);

        // Force every control (incl. the selected tab page's children) to realize a handle + lay out,
        // then settle, so DrawToBitmap captures the full UI.
        Realize(form);
        for (int i = 0; i < 6; i++) { Application.DoEvents(); System.Threading.Thread.Sleep(40); }
        form.PerformLayout();
        form.Refresh();
        Application.DoEvents();

        int w = form.Width, h = form.Height;
        using (var bmp = new Bitmap(w, h))
        {
            form.DrawToBitmap(bmp, new Rectangle(0, 0, w, h));
            var dir = Path.GetDirectoryName(path);
            if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
            bmp.Save(path, ImageFormat.Png);
        }

        form.Close();
    }

    private static void Realize(Control c)
    {
        _ = c.Handle;            // force handle creation
        foreach (Control child in c.Controls) Realize(child);
    }
}
