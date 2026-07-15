using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

namespace MouseFence;

/// <summary>
/// Dev-only helper (behind the --screenshot arg): realizes the settings window beyond the active desktop,
/// captures its real DWM/WinForms pixels, and closes it immediately. This path is used by headless QA.
/// </summary>
internal static class Screenshot
{
    public static void CaptureSettings(string path, string themeOverride = null, int tabIndex = 0,
                                       string lang = "en", float scale = 1f)
    {
        var s = Settings.Load();
        s.Language = string.IsNullOrEmpty(lang) ? "en" : lang;
        if (!string.IsNullOrEmpty(themeOverride)) s.Theme = themeOverride;   // "Light" | "Dark"
        Strings.Use(s.Language);

        using var form = new SettingsForm(s, MonitorInfo.All());
        // FLASH-FREE (owner requirement): place the window BEYOND the whole virtual desktop so it is never
        // visible on any monitor, and never activate / topmost / bring-to-front it (those yank it to the
        // foreground = the flash the owner sees). DWM still composites an off-screen shown window, so
        // PrintWindow(PW_RENDERFULLCONTENT) captures its real pixels; DrawToBitmap is the fallback. We NEVER
        // CopyFromScreen (that reads the visible screen, which would require the window on a monitor).
        var vs = SystemInformation.VirtualScreen;
        var off = new Point(vs.Right + 400, vs.Top);
        form.StartPosition = FormStartPosition.Manual;
        form.Location = off;
        form.ClientSize = new Size(1064, 712);
        form.ShowInTaskbar = false;
        form.Show();
        SelectPreviewState(form, tabIndex);
        form.ActiveControl = null;

        // Force every control (incl. the selected tab page's children) to realize a handle + lay out.
        Realize(form);
        for (int i = 0; i < 6; i++) { Application.DoEvents(); System.Threading.Thread.Sleep(40); }
        // Visual-regression captures use the preferred design size even on a small headless desktop.
        form.ClientSize = new Size(1064, 712);
        SelectPreviewState(form, tabIndex);
        form.ActiveControl = null;
        if (scale > 1f && scale <= 3f)
        {
            ApplySyntheticDpi(form, (int)Math.Round(form.DeviceDpi * scale));
            for (int i = 0; i < 6; i++) { Application.DoEvents(); System.Threading.Thread.Sleep(40); }
            SelectPreviewState(form, tabIndex);
            form.ActiveControl = null;
        }
        form.Location = off;   // keep it off-screen after any relayout
        Theming.DarkTitleBar(form, Theming.Resolve(s.Theme));
        Application.DoEvents();
        form.PerformLayout();
        form.Refresh();
        for (int i = 0; i < 8; i++) { Application.DoEvents(); System.Threading.Thread.Sleep(45); }

        GetWindowRect(form.Handle, out var actualBounds);
        int w = actualBounds.Right - actualBounds.Left, h = actualBounds.Bottom - actualBounds.Top;
        using (var bmp = new Bitmap(w, h))
        {
            for (int attempt = 0; attempt < 12; attempt++)
            {
                form.Invalidate(true);
                form.Refresh();
                Application.DoEvents();
                System.Threading.Thread.Sleep(100);
                bool printed = false;
                using (var graphics = Graphics.FromImage(bmp))
                {
                    graphics.Clear(Color.Black);
                    IntPtr hdc = graphics.GetHdc();
                    try { printed = Native.PrintWindow(form.Handle, hdc, Native.PW_RENDERFULLCONTENT); }
                    finally { graphics.ReleaseHdc(hdc); }
                }
                if (!printed || HasLargeBlackVoid(bmp))
                    form.DrawToBitmap(bmp, new Rectangle(0, 0, w, h));   // flash-free fallback (never CopyFromScreen)
                if (!HasLargeBlackVoid(bmp)) break;
            }
            var dir = Path.GetDirectoryName(path);
            if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
            bmp.Save(path, ImageFormat.Png);
        }

        form.Close();
    }

    private static void SelectPreviewState(SettingsForm form, int tabIndex)
    {
        // V2 mapping: 0 Displays, 1 Advanced, 2 Shortcuts, 3 General, -1 selected boundary, -2 Advanced with physical-pixel fold open.
        if (tabIndex == -2) form.OpenAdvancedForPreview();
        else if (tabIndex < 0) form.OpenFirstBoundaryForPreview();
        else form.SelectTab(tabIndex);
    }

    private static void Realize(Control c)
    {
        _ = c.Handle;            // force handle creation
        foreach (Control child in c.Controls) Realize(child);
    }

    private static void ApplySyntheticDpi(Form form, int targetDpi)
    {
        const int WmDpiChanged = 0x02E0;
        float factor = targetDpi / (float)Math.Max(1, form.DeviceDpi);
        var suggested = new NativeRect
        {
            Left = form.Left,
            Top = form.Top,
            Right = form.Left + (int)Math.Round(form.Width * factor),
            Bottom = form.Top + (int)Math.Round(form.Height * factor),
        };
        IntPtr memory = Marshal.AllocHGlobal(Marshal.SizeOf<NativeRect>());
        try
        {
            Marshal.StructureToPtr(suggested, memory, false);
            int packedDpi = targetDpi | (targetDpi << 16);
            SendMessage(form.Handle, WmDpiChanged, (IntPtr)packedDpi, memory);
        }
        finally { Marshal.FreeHGlobal(memory); }
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeRect { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll")]
    private static extern IntPtr SendMessage(IntPtr hWnd, int message, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern bool GetWindowRect(IntPtr hWnd, out NativeRect rect);

    private static bool HasLargeBlackVoid(Bitmap bitmap)
    {
        int black = 0, sampled = 0;
        for (int y = 4; y < bitmap.Height - 4; y += 8)
        {
            for (int x = 4; x < bitmap.Width - 4; x += 8)
            {
                Color pixel = bitmap.GetPixel(x, y);
                if (pixel.R <= 3 && pixel.G <= 3 && pixel.B <= 3) black++;
                sampled++;
            }
        }
        return black > sampled / 20;
    }
}
