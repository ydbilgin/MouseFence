using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

namespace MouseFence;

/// <summary>
/// Dev-only helper (behind the screenshot args): realizes the settings window beyond the active desktop,
/// captures its real DWM/WinForms pixels plus faithful custom-control overlays, and closes it immediately.
/// </summary>
internal static class Screenshot
{
    internal readonly record struct CaptureMetrics(Size ClientSize, string Mode, int BodyHeight,
                                                    int MapHeight, int BandHeight)
    {
        public double BandPercent => BodyHeight <= 0 ? 0 : BandHeight * 100d / BodyHeight;
    }

    public static CaptureMetrics CaptureSettings(string path, string themeOverride = null, int tabIndex = 0,
                                                  string lang = "en", float scale = 1f, bool useDemoLayout = false,
                                                  Size? clientSizeOverride = null)
    {
        var s = useDemoLayout ? DemoSettings(themeOverride, lang) : Settings.Load();
        s.Language = string.IsNullOrEmpty(lang) ? "en" : lang;
        if (!string.IsNullOrEmpty(themeOverride)) s.Theme = themeOverride;   // "Light" | "Dark"
        Strings.Use(s.Language);
        Theme captureTheme = Theming.Resolve(s.Theme);

        using var form = new SettingsForm(s, useDemoLayout ? DemoMonitors() : MonitorInfo.All(),
            showWithoutActivation: true);
        // The documentation crop uses a common 1064x820 resizable-window viewport. It fits the complete map,
        // three-screen list, or inline boundary editor without auto-scrolling the page heading out of frame.
        var captureClientSize = clientSizeOverride ?? (useDemoLayout ? new Size(1064, 820) : new Size(1064, 712));
        // FLASH-FREE (owner requirement): place the window BEYOND the whole virtual desktop so it is never
        // visible on any monitor, and never activate / topmost / bring-to-front it (those yank it to the
        // foreground = the flash the owner sees). DWM still composites an off-screen shown window, so
        // PrintWindow(PW_RENDERFULLCONTENT) supplies the complete base. The two regions it can omit in dark mode
        // (the footer and editor close button) are composited from their own DrawToBitmap output so their OnPaint
        // paths always run. We NEVER CopyFromScreen (that reads the visible screen, which would require the window
        // on a monitor).
        var vs = SystemInformation.VirtualScreen;
        var off = new Point(vs.Right + 400, vs.Top);
        form.StartPosition = FormStartPosition.Manual;
        form.Location = off;
        form.ClientSize = captureClientSize;
        form.ShowInTaskbar = false;
        form.Show();
        SelectPreviewState(form, tabIndex);
        form.ActiveControl = null;

        // Force every control (incl. the selected tab page's children) to realize a handle + lay out.
        Realize(form);
        for (int i = 0; i < 6; i++) { Application.DoEvents(); System.Threading.Thread.Sleep(40); }
        // Visual-regression captures use the preferred design size even on a small headless desktop.
        form.ClientSize = captureClientSize;
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
        Theming.DarkTitleBar(form, captureTheme);
        Application.DoEvents();
        form.PerformLayout();
        form.Refresh();
        for (int i = 0; i < 8; i++) { Application.DoEvents(); System.Threading.Thread.Sleep(45); }

        var dir = Path.GetDirectoryName(path);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);

        GetWindowRect(form.Handle, out var actualBounds);
        int w = actualBounds.Right - actualBounds.Left, h = actualBounds.Bottom - actualBounds.Top;
        Point clientOrigin = form.PointToScreen(Point.Empty);
        int clientX = clientOrigin.X - actualBounds.Left;
        int clientY = clientOrigin.Y - actualBounds.Top;
        using (var bmp = new Bitmap(w, h))
        {
            bool captureComplete = false;
            string problem = "PrintWindow drew no frame";
            for (int attempt = 0; attempt < 8; attempt++)
            {
                form.Invalidate(true);
                form.Refresh();
                Application.DoEvents();
                System.Threading.Thread.Sleep(80);
                bool printed = false;
                using (var graphics = Graphics.FromImage(bmp))
                {
                    graphics.Clear(Color.Black);
                    IntPtr hdc = graphics.GetHdc();
                    try { printed = Native.PrintWindow(form.Handle, hdc, Native.PW_RENDERFULLCONTENT); }
                    finally { graphics.ReleaseHdc(hdc); }
                }
                if (!printed || !HasRenderedFrame(bmp, clientY)) continue;
                CompositeFaithfulControls(form, bmp, actualBounds);
                problem = CaptureProblem(form, bmp, actualBounds, captureTheme);
                captureComplete = problem == null;
                if (captureComplete) break;
            }
            if (captureComplete && useDemoLayout)
            {
                // Public docs use a clean client-only crop. The native frame supplies the stable base;
                // CompositeFaithfulControls redraws DoubleBuffered children through DrawToBitmap so the
                // footer and close button cannot become blank boxes.
                var cropBounds = new Rectangle(clientX, clientY,
                    Math.Min(form.ClientSize.Width, bmp.Width - clientX),
                    Math.Min(form.ClientSize.Height, bmp.Height - clientY));
                using var crop = bmp.Clone(cropBounds, PixelFormat.Format32bppArgb);
                crop.Save(path, ImageFormat.Png);
            }
            else if (captureComplete)
                bmp.Save(path, ImageFormat.Png);
            else if (useDemoLayout)
                throw new InvalidOperationException("Could not render a complete documentation screenshot: " + problem + ".");
            else
            {
                // A client-only crop is still faithful and private if DWM declines an off-screen frame.
                using var client = RenderClient(form);
                client.Save(path, ImageFormat.Png);
            }
        }

        var layout = form.DisplayLayoutMetrics();
        var metrics = new CaptureMetrics(form.ClientSize, layout.Mode, layout.BodyHeight,
            layout.MapHeight, layout.BandHeight);
        form.Close();
        return metrics;
    }

    /// <summary>
    /// Deterministic public-documentation layout: Main with a centered upper screen for an upward gate,
    /// plus a right-side laptop that demonstrates the keep-pointer-off state. No live display data enters
    /// this model.
    /// </summary>
    internal static List<MonitorInfo> DemoMonitors()
    {
        var monitors = new List<MonitorInfo>
        {
            new()
            {
                Device = "DEMO_MAIN", StableId = "demo-main", Primary = true,
                Bounds = new Native.RECT { Left = 0, Top = 0, Right = 2560, Bottom = 1440 },
            },
            new()
            {
                Device = "DEMO_TOP", StableId = "demo-top",
                Bounds = new Native.RECT { Left = 320, Top = -1080, Right = 2240, Bottom = 0 },
            },
            new()
            {
                Device = "DEMO_LAPTOP", StableId = "demo-laptop",
                Bounds = new Native.RECT { Left = 2560, Top = 360, Right = 4480, Bottom = 1440 },
            },
        };
        MonitorInfo.SortAndNumber(monitors);
        return monitors;
    }

    internal static Settings DemoSettings(string themeOverride, string lang)
    {
        bool turkish = string.Equals(lang, "tr", StringComparison.OrdinalIgnoreCase);
        var settings = new Settings
        {
            Language = turkish ? "tr" : "en",
            Theme = string.IsNullOrEmpty(themeOverride) ? "Light" : themeOverride,
            BoundaryTipSeen = true,
            MonitorNames = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
            {
                ["demo-main"] = turkish ? "Ana" : "Main",
                ["demo-top"] = turkish ? "Üst" : "Top",
                ["demo-laptop"] = turkish ? "Dizüstü" : "Laptop",
            },
            ExcludedDevices = new List<string> { "demo-laptop" },
            BoundaryRules = new List<BoundaryRule>
            {
                new()
                {
                    FromStableId = "demo-main", ToStableId = "demo-top",
                    Direction = BoundaryDirection.Up, Behavior = BoundaryBehavior.Push,
                    Control = BoundaryControl.Automatic,
                },
            },
        };
        settings.Normalize();
        return settings;
    }

    /// <summary>
    /// Draws the real tray menu off-screen with demo settings: upward passage closed, side protection on.
    /// No hook, hotkey or tray icon is created.
    /// </summary>
    internal static void CaptureTrayMenu(string path, string themeOverride, string lang)
    {
        var settings = DemoSettings(themeOverride, lang);
        Strings.Use(settings.Language);
        using var menu = new TrayMenu(() => { }, () => { }, () => { }, () => { }, () => { }, () => { });
        menu.Apply(settings, new TrayMenuState(Enabled: true, GateOpen: false, SideOn: true, Confine: false));
        var virtualScreen = SystemInformation.VirtualScreen;
        menu.Menu.Show(new Point(virtualScreen.Right + 3000, virtualScreen.Bottom + 3000));
        for (int i = 0; i < 4; i++) { Application.DoEvents(); System.Threading.Thread.Sleep(40); }
        using var bmp = new Bitmap(menu.Menu.Width, menu.Menu.Height);
        menu.Menu.DrawToBitmap(bmp, new Rectangle(0, 0, bmp.Width, bmp.Height));
        var dir = Path.GetDirectoryName(path);
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
        bmp.Save(path, ImageFormat.Png);
        menu.Menu.Close();
    }

    private static void SelectPreviewState(SettingsForm form, int tabIndex)
    {
        // V2 mapping: 0 Displays, 1 Advanced, 2 Shortcuts, 3 General, -1 selected boundary, -2 Advanced with physical-pixel fold open.
        if (tabIndex == -2) form.OpenAdvancedForPreview();
        else if (tabIndex < 0) form.OpenFirstBoundaryForPreview(hideScreenList: true);
        else if (tabIndex == 1) form.OpenAdvancedOverviewForPreview();
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

    private static Bitmap RenderClient(Form form)
    {
        var bitmap = new Bitmap(Math.Max(1, form.ClientSize.Width), Math.Max(1, form.ClientSize.Height));
        using (var graphics = Graphics.FromImage(bitmap)) graphics.Clear(form.BackColor);
        foreach (Control child in form.Controls.Cast<Control>().Reverse())
            child.DrawToBitmap(bitmap, child.Bounds);
        return bitmap;
    }

    private static void CompositeFaithfulControls(SettingsForm form, Bitmap target, NativeRect windowBounds)
    {
        var controls = AllControls(form).Where(control => control.Visible).ToList();
        var footer = controls.OfType<TableLayoutPanel>().FirstOrDefault(control =>
            string.Equals(control.Tag as string, "surface", StringComparison.Ordinal));
        var overlays = new List<Control>();
        if (footer != null) overlays.Add(footer);
        // PrintWindow's omission is not limited to one class on every Windows build. Redraw owner-drawn leaves
        // (buttons, map, tiles, chips) and text leaves individually, without replacing whole container subtrees.
        overlays.AddRange(controls.Where(control =>
            (control.Controls.Count == 0 && control.GetType().Assembly == typeof(Screenshot).Assembly) ||
            control is ThemedControlFrame || control is Label));

        using var graphics = Graphics.FromImage(target);
        foreach (var control in overlays.Distinct().OrderBy(ControlDepth))
        {
            if (control.Width <= 0 || control.Height <= 0) continue;
            using var layer = new Bitmap(control.Width, control.Height);
            if (control is Label label && control is not LinkLabel)
            {
                Color background = label.BackColor.A > 0 ? label.BackColor
                    : label.Parent?.BackColor ?? form.BackColor;
                using var layerGraphics = Graphics.FromImage(layer);
                layerGraphics.Clear(background);
                RenderLabel(layerGraphics, label, background);
            }
            else
            {
                using (var layerGraphics = Graphics.FromImage(layer))
                    layerGraphics.Clear(control.Parent?.BackColor ?? form.BackColor);
                control.DrawToBitmap(layer, new Rectangle(Point.Empty, control.Size));
            }
            Point screen = control.PointToScreen(Point.Empty);
            Rectangle visible = VisibleScreenBounds(control, form);
            if (visible.Width <= 0 || visible.Height <= 0) continue;
            var state = graphics.Save();
            graphics.SetClip(new Rectangle(visible.X - windowBounds.Left, visible.Y - windowBounds.Top,
                visible.Width, visible.Height));
            if (control is Label renderedLabel && control is not LinkLabel)
            {
                Color background = renderedLabel.BackColor.A > 0 ? renderedLabel.BackColor
                    : renderedLabel.Parent?.BackColor ?? form.BackColor;
                using var erase = new SolidBrush(background);
                graphics.FillRectangle(erase, screen.X - windowBounds.Left, screen.Y - windowBounds.Top,
                    control.Width, control.Height);
            }
            graphics.DrawImageUnscaled(layer, screen.X - windowBounds.Left, screen.Y - windowBounds.Top);
            graphics.Restore(state);
        }
    }

    private static void RenderLabel(Graphics graphics, Label label, Color background)
    {
        var rect = new Rectangle(label.Padding.Left, label.Padding.Top,
            Math.Max(1, label.ClientSize.Width - label.Padding.Horizontal),
            Math.Max(1, label.ClientSize.Height - label.Padding.Vertical));
        var flags = TextFormatFlags.NoPrefix;
        flags |= label.TextAlign switch
        {
            ContentAlignment.TopCenter or ContentAlignment.MiddleCenter or ContentAlignment.BottomCenter => TextFormatFlags.HorizontalCenter,
            ContentAlignment.TopRight or ContentAlignment.MiddleRight or ContentAlignment.BottomRight => TextFormatFlags.Right,
            _ => TextFormatFlags.Left,
        };
        flags |= label.TextAlign switch
        {
            ContentAlignment.MiddleLeft or ContentAlignment.MiddleCenter or ContentAlignment.MiddleRight => TextFormatFlags.VerticalCenter,
            ContentAlignment.BottomLeft or ContentAlignment.BottomCenter or ContentAlignment.BottomRight => TextFormatFlags.Bottom,
            _ => TextFormatFlags.Top,
        };
        int oneLine = TextRenderer.MeasureText("Ag", label.Font, Size.Empty, TextFormatFlags.NoPadding).Height;
        bool multiline = label.Text.Contains('\n') || rect.Height > oneLine + 6;
        flags |= multiline ? TextFormatFlags.WordBreak : TextFormatFlags.SingleLine;
        if (label.AutoEllipsis) flags |= TextFormatFlags.EndEllipsis;
        // Supplying the opaque background is important for GDI ClearType output on an ARGB bitmap;
        // without it, light-theme glyph edges can retain a second dark fringe when composited.
        TextRenderer.DrawText(graphics, label.Text ?? "", label.Font, rect,
            label.ForeColor, background, flags);
    }

    /// <summary>Null when the capture looks complete, otherwise the first missing part in plain words.</summary>
    private static string CaptureProblem(SettingsForm form, Bitmap target, NativeRect windowBounds, Theme theme)
    {
        if (HasLargeBlankRegion(target)) return "a large blank region";

        var controls = AllControls(form).Where(control => control.Visible).ToList();
        var footer = controls.OfType<TableLayoutPanel>().FirstOrDefault(control =>
            string.Equals(control.Tag as string, "surface", StringComparison.Ordinal));
        if (footer == null) return "no footer";
        // Only buttons on screen: a hidden, empty validation action would otherwise be sampled where the footer shows.
        foreach (var button in footer.Controls.OfType<ThemedButton>().Where(button => button.Visible))
        {
            Point sample = button.PointToScreen(new Point(Math.Min(5, button.Width - 1), button.Height / 2));
            int x = sample.X - windowBounds.Left, y = sample.Y - windowBounds.Top;
            if (x < 0 || y < 0 || x >= target.Width || y >= target.Height) return "footer button outside the frame";
            Color expected = !button.Enabled ? theme.DisabledSurface
                : button.Role == ThemedButtonRole.Primary ? theme.Accent : theme.Surface2;
            if (!Near(target.GetPixel(x, y), expected, 12)) return $"footer button {button.Text} has {target.GetPixel(x, y)} not {expected}";
        }

        var brand = controls.OfType<Label>().FirstOrDefault(label => label.Text.Contains("▣", StringComparison.Ordinal)
            && label.Text.Contains("MouseFence", StringComparison.Ordinal));
        if (brand == null) return "no brand label";
        Point brandOrigin = brand.PointToScreen(Point.Empty);
        int ink = 0;
        // Ignore the leading square glyph: it can render even when PrintWindow drops the wordmark text.
        for (int y = 0; y < brand.Height; y++)
        for (int x = Math.Min(24, brand.Width); x < brand.Width; x++)
        {
            int targetX = brandOrigin.X - windowBounds.Left + x;
            int targetY = brandOrigin.Y - windowBounds.Top + y;
            if (targetX >= 0 && targetY >= 0 && targetX < target.Width && targetY < target.Height &&
                !Near(target.GetPixel(targetX, targetY), theme.Sidebar, 24)) ink++;
        }
        if (ink < 40) return $"brand wordmark has {ink} ink pixels";

        foreach (var close in controls.Where(control => control.GetType().Name == "DrawerCloseButton"))
        {
            Point closeOrigin = close.PointToScreen(Point.Empty);
            Color background = close.Parent?.BackColor ?? theme.Surface2;
            int glyphInk = 0;
            for (int y = 0; y < close.Height; y++)
            for (int x = 0; x < close.Width; x++)
            {
                int targetX = closeOrigin.X - windowBounds.Left + x;
                int targetY = closeOrigin.Y - windowBounds.Top + y;
                if (targetX >= 0 && targetY >= 0 && targetX < target.Width && targetY < target.Height &&
                    !Near(target.GetPixel(targetX, targetY), background, 24)) glyphInk++;
            }
            if (glyphInk < 3) return "drawer close glyph missing";
        }
        return null;
    }

    private static bool HasLargeBlankRegion(Bitmap bitmap)
    {
        // Failed off-screen native/DrawToBitmap passes leave opaque #000 rectangles. Real dark-theme
        // surfaces are deliberately lifted from black, so a nearly solid black 32px tile is always a
        // missing paint region rather than part of the design.
        const int tile = 32;
        for (int top = 0; top < bitmap.Height; top += tile)
        for (int left = 0; left < bitmap.Width; left += tile)
        {
            int width = Math.Min(tile, bitmap.Width - left);
            int height = Math.Min(tile, bitmap.Height - top);
            int blank = 0;
            for (int y = top; y < top + height; y += 2)
            for (int x = left; x < left + width; x += 2)
            {
                Color pixel = bitmap.GetPixel(x, y);
                if (pixel.A < 250 || (pixel.R <= 3 && pixel.G <= 3 && pixel.B <= 3)) blank++;
            }
            int samples = ((width + 1) / 2) * ((height + 1) / 2);
            if (blank * 10 >= samples * 9) return true;
        }
        return false;
    }

    private static bool Near(Color a, Color b, int tolerance) =>
        Math.Abs(a.R - b.R) <= tolerance && Math.Abs(a.G - b.G) <= tolerance &&
        Math.Abs(a.B - b.B) <= tolerance;

    private static int ControlDepth(Control control)
    {
        int depth = 0;
        for (Control current = control.Parent; current != null; current = current.Parent) depth++;
        return depth;
    }

    private static Rectangle VisibleScreenBounds(Control control, Form form)
    {
        Point origin = control.PointToScreen(Point.Empty);
        var visible = new Rectangle(origin, control.Size);
        for (Control ancestor = control.Parent; ancestor != null && ancestor != form; ancestor = ancestor.Parent)
        {
            Point ancestorOrigin = ancestor.PointToScreen(Point.Empty);
            visible = Rectangle.Intersect(visible, new Rectangle(ancestorOrigin, ancestor.ClientSize));
            if (visible.IsEmpty) break;
        }
        return visible;
    }

    private static IEnumerable<Control> AllControls(Control root)
    {
        foreach (Control child in root.Controls)
        {
            yield return child;
            foreach (var descendant in AllControls(child)) yield return descendant;
        }
    }

    private static bool HasRenderedFrame(Bitmap bitmap, int clientTop)
    {
        int painted = 0, sampled = 0;
        int bottom = Math.Clamp(clientTop, 2, bitmap.Height);
        for (int y = 2; y < bottom; y += 3)
        {
            for (int x = 4; x < bitmap.Width - 4; x += 8)
            {
                Color pixel = bitmap.GetPixel(x, y);
                if (pixel.R > 3 || pixel.G > 3 || pixel.B > 3) painted++;
                sampled++;
            }
        }
        return sampled > 0 && painted > sampled / 2;
    }
}
