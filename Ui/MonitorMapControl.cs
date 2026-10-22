using System.ComponentModel;
using System.Drawing;
using System.Drawing.Drawing2D;

namespace MouseFence;

internal sealed class BoundaryView
{
    public MonitorInfo First { get; init; }
    public MonitorInfo Second { get; init; }
    public BoundaryDirection FirstToSecondDirection { get; init; }
    public BoundaryDirection SecondToFirstDirection => FirstToSecondDirection switch
    {
        BoundaryDirection.Up => BoundaryDirection.Down,
        BoundaryDirection.Down => BoundaryDirection.Up,
        BoundaryDirection.Left => BoundaryDirection.Right,
        _ => BoundaryDirection.Left,
    };
    public int CorridorStart { get; init; }
    public int CorridorEnd { get; init; }
    public int GapPx { get; init; }
    public RuntimeTransition FirstToSecond { get; set; }
    public RuntimeTransition SecondToFirst { get; set; }
    public Rectangle HitTarget { get; set; }
    public Point Handle { get; set; }
    public Point LineStart { get; set; }
    public Point LineEnd { get; set; }
    public bool VerticalConnection => FirstToSecondDirection is BoundaryDirection.Up or BoundaryDirection.Down;
    public bool Customized => FirstToSecond?.ExplicitRule == true || SecondToFirst?.ExplicitRule == true;
    public bool SafetyDemoted => FirstToSecond?.SafetyDemoted == true || SecondToFirst?.SafetyDemoted == true;
    public bool HasExcludedScreen { get; init; }
    public int SortOrder => Math.Min(FirstToSecond?.SortOrder ?? int.MaxValue, SecondToFirst?.SortOrder ?? int.MaxValue);
}

/// <summary>A keyboard-reachable, read-only picture of the live display layout and its editable MouseFence boundaries.</summary>
internal sealed class MonitorMapControl : Control
{
    private readonly ToolTip _toolTip = new() { ShowAlways = true };
    private readonly List<MonitorInfo> _monitors = new();
    private readonly List<BoundaryView> _boundaries = new();
    private readonly Dictionary<string, string> _names = new(StringComparer.OrdinalIgnoreCase);
    private Settings _settings = new();
    private Theme _theme = Theme.Light;
    private string _selectedStableId;
    private string _hoveredStableId;
    private BoundaryView _selectedBoundary;
    private BoundaryView _hoveredBoundary;
    private Point _lastClick;
    private long _lastClickTick;
    private int _clusterIndex;

    public event Action<MonitorInfo> MonitorClicked;
    public event Action<MonitorInfo> MonitorHovered;
    public event Action<BoundaryView> BoundaryClicked;
    public event Action BoundaryEscape;

    public MonitorMapControl()
    {
        DoubleBuffered = true;
        ResizeRedraw = true;
        TabStop = true;
        AccessibleRole = AccessibleRole.Graphic;
        AccessibleName = Strings.Ui("TabScreens");
        Cursor = Cursors.Default;
        SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer |
                 ControlStyles.ResizeRedraw | ControlStyles.UserPaint | ControlStyles.Selectable, true);
    }

    [Browsable(false), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public string SelectedStableId
    {
        get => _selectedStableId;
        set { _selectedStableId = value; _selectedBoundary = null; Invalidate(); }
    }

    [Browsable(false), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public string ExternalHoveredStableId
    {
        set { _hoveredStableId = value; Invalidate(); }
    }

    [Browsable(false), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public BoundaryView SelectedBoundary
    {
        get => _selectedBoundary;
        set { _selectedBoundary = value; _selectedStableId = null; Invalidate(); AccessibilityNotifyClients(AccessibleEvents.Focus, -1); }
    }

    internal IReadOnlyList<BoundaryView> Boundaries => _boundaries;

    public void SetTheme(Theme theme)
    {
        _theme = theme;
        BackColor = theme.Surface2;
        ForeColor = theme.Text;
        _toolTip.BackColor = theme.Surface;
        _toolTip.ForeColor = theme.Text;
        Invalidate();
    }

    public void SetModel(IEnumerable<MonitorInfo> monitors, Settings settings,
                         IReadOnlyDictionary<string, string> names = null)
    {
        _monitors.Clear();
        _monitors.AddRange(monitors ?? Enumerable.Empty<MonitorInfo>());
        _settings = settings ?? new Settings();
        _names.Clear();
        foreach (var pair in names ?? _settings.MonitorNames)
            if (!string.IsNullOrWhiteSpace(pair.Key) && !string.IsNullOrWhiteSpace(pair.Value))
                _names[pair.Key] = pair.Value;
        BuildBoundaries();
        if (_selectedBoundary != null)
            _selectedBoundary = _boundaries.FirstOrDefault(b => SamePair(b, _selectedBoundary));
        Invalidate();
        AccessibilityNotifyClients(AccessibleEvents.Reorder, -1);
    }

    public BoundaryView FindBoundary(string from, string to, BoundaryDirection direction) =>
        _boundaries.FirstOrDefault(b =>
            (Same(b.First.StableId, from) && Same(b.Second.StableId, to) && b.FirstToSecondDirection == direction) ||
            (Same(b.Second.StableId, from) && Same(b.First.StableId, to) && b.SecondToFirstDirection == direction));

    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        var g = e.Graphics;
        g.SmoothingMode = SmoothingMode.AntiAlias;
        var frame = new Rectangle(0, 0, Math.Max(1, Width - 1), Math.Max(1, Height - 1));
        using var framePath = Rounded(frame, 8);
        using (var fill = new SolidBrush(_theme.Sidebar)) g.FillPath(fill, framePath);
        var state = g.Save();
        g.SetClip(framePath);
        var rects = LayoutRects();
        EnsureMinimumFootprints(rects);
        PositionBoundaries(rects);
        var pills = new[] { _selectedBoundary, _hoveredBoundary }
            .Where(boundary => boundary != null)
            .Distinct()
            .Select(boundary => (Boundary: boundary, Rect: BoundaryPillRect(g, boundary)))
            .ToList();

        var cloneCounts = _monitors.GroupBy(m => (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom))
            .ToDictionary(x => x.Key, x => x.Count());
        for (int i = 0; i < _monitors.Count && i < rects.Count; i++)
            DrawMonitor(g, _monitors[i], rects[i], cloneCounts[(_monitors[i].Bounds.Left, _monitors[i].Bounds.Top,
                _monitors[i].Bounds.Right, _monitors[i].Bounds.Bottom)], pills);

        var standing = StandingBoundaries()
            .OrderBy(boundary => AlwaysPaintHandle(boundary) ? 0 : 1)
            .ThenByDescending(boundary => boundary.CorridorEnd - boundary.CorridorStart)
            .ThenBy(boundary => boundary.SortOrder);
        var paintedHandleCenters = new List<Point>();
        foreach (var boundary in standing)
        {
            bool paintHandle = AlwaysPaintHandle(boundary) || !paintedHandleCenters.Any(center =>
                (center.X - boundary.Handle.X) * (center.X - boundary.Handle.X) +
                (center.Y - boundary.Handle.Y) * (center.Y - boundary.Handle.Y) < 18 * 18);
            DrawBoundary(g, boundary, paintHandle);
            if (paintHandle) paintedHandleCenters.Add(boundary.Handle);
        }

        DrawWindowsLockChip(g);

        if (Focused)
        {
            var focus = ClientRectangle; focus.Inflate(-2, -2);
            using var pen = new Pen(_theme.Accent, 1) { DashStyle = DashStyle.Dot };
            g.DrawRectangle(pen, focus);
        }
        g.Restore(state);
        using var border = new Pen(_theme.Border);
        g.DrawPath(border, framePath);
    }

    private void DrawWindowsLockChip(Graphics g)
    {
        string text = Strings.Ui("PositionsFromWindows");
        int width = Math.Min(230, TextRenderer.MeasureText(text, Font).Width + 31);
        var rect = new Rectangle(Math.Max(8, Width - width - 10), 10, width, 27);
        using var path = Rounded(rect, 5);
        using (var fill = new SolidBrush(_theme.Surface)) g.FillPath(fill, path);
        using (var border = new Pen(_theme.Border)) g.DrawPath(border, path);
        using (var lockPen = new Pen(_theme.Subtle, 1.3f))
        {
            g.DrawRectangle(lockPen, rect.Left + 9, rect.Top + 12, 8, 8);
            g.DrawArc(lockPen, rect.Left + 10, rect.Top + 6, 6, 9, 180, -180);
        }
        TextRenderer.DrawText(g, text, Font, new Rectangle(rect.Left + 23, rect.Top, rect.Width - 27, rect.Height),
            _theme.Subtle, TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPrefix);
    }

    private void DrawMonitor(Graphics g, MonitorInfo monitor, Rectangle rect, int clones,
        IReadOnlyList<(BoundaryView Boundary, Rectangle Rect)> pills)
    {
        bool excluded = _settings.ExcludedDevices.Contains(monitor.StableId, StringComparer.OrdinalIgnoreCase);
        bool selected = Same(_selectedStableId, monitor.StableId);
        bool hovered = Same(_hoveredStableId, monitor.StableId);
        var shadowRect = rect; shadowRect.Offset(0, 2);
        using (var shadowPath = Rounded(shadowRect, 7))
        using (var shadow = new SolidBrush(Color.FromArgb(_theme.IsDark ? 42 : 25, Color.Black))) g.FillPath(shadow, shadowPath);
        using (var path = Rounded(rect, 7))
        using (var fill = new SolidBrush(_theme.Surface2)) g.FillPath(fill, path);
        if (excluded)
        {
            using var hatch = new HatchBrush(HatchStyle.ForwardDiagonal,
                Color.FromArgb(_theme.IsDark ? 72 : 42, _theme.Danger), Color.Transparent);
            using var path = Rounded(rect, 7);
            g.FillPath(hatch, path);
        }
        using (var path = Rounded(rect, 7))
        using (var pen = new Pen(selected || hovered ? _theme.Accent : _theme.BorderStrong, selected ? 2.5f : 1.5f))
            g.DrawPath(pen, path);

        int chipSize = Math.Max(18, Math.Min(24, rect.Height / 3));
        var chip = new Rectangle(rect.Left + 5, rect.Top + 5, Math.Max(chipSize, 18), chipSize);
        using (var chipPath = Rounded(chip, 5))
        using (var brush = new SolidBrush(monitor.Primary ? _theme.AccentWeak : _theme.Chip)) g.FillPath(brush, chipPath);
        using var chipFont = new Font(Font, FontStyle.Bold);
        TextRenderer.DrawText(g, monitor.Index.ToString(), chipFont, chip,
            monitor.Primary ? _theme.Accent : _theme.ChipText,
            TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPadding);

        if (monitor.Primary)
        {
            string main = "★ " + Strings.Ui("MainWord");
            int badgeWidth = Math.Min(Math.Max(48, TextRenderer.MeasureText(main, Font).Width + 10), Math.Max(48, rect.Width - 10));
            var badge = new Rectangle(rect.Right - badgeWidth - 5, rect.Bottom - 23, badgeWidth, 18);
            using var badgePath = Rounded(badge, 4); using var badgeFill = new SolidBrush(_theme.AccentWeak); g.FillPath(badgeFill, badgePath);
            TextRenderer.DrawText(g, main, Font, badge, _theme.Accent,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPadding);
        }
        if (excluded && rect.Width >= 64 && rect.Height >= 34)
        {
            using var xPen = new Pen(_theme.Danger, 2);
            int cx = rect.Right - 13, cy = rect.Bottom - 13;
            g.DrawLine(xPen, cx - 4, cy - 4, cx + 4, cy + 4);
            g.DrawLine(xPen, cx + 4, cy - 4, cx - 4, cy + 4);
        }
        if (clones > 1)
        {
            var badge = new Rectangle(rect.Right - 35, rect.Bottom - 21, 30, 16);
            TextRenderer.DrawText(g, "×" + clones, Font, badge, _theme.Subtle,
                TextFormatFlags.Right | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPadding);
        }

        string name = NameFor(monitor);
        if (rect.Width >= 64 && rect.Height >= 34)
        {
            var label = new Rectangle(rect.Left + 6, rect.Top + 20, rect.Width - 12, Math.Max(15, rect.Height - (monitor.Primary ? 42 : 34)));
            bool coveredByEndpointPill = pills.Any(pill => pill.Rect.IntersectsWith(label) &&
                (Same(pill.Boundary.First.StableId, monitor.StableId) || Same(pill.Boundary.Second.StableId, monitor.StableId)));
            if (!coveredByEndpointPill)
            {
                using var nameFont = new Font(Font.FontFamily, Math.Max(9f, Font.Size), FontStyle.Bold);
                TextRenderer.DrawText(g, name, nameFont, label,
                    excluded ? _theme.Subtle : _theme.Text, TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter |
                    TextFormatFlags.EndEllipsis | TextFormatFlags.SingleLine | TextFormatFlags.NoPadding);
            }
            if (!coveredByEndpointPill && rect.Width >= 104 && rect.Height >= 88)
            {
                var resolution = new Rectangle(rect.Left + 8, Math.Min(rect.Bottom - 20, label.Bottom), rect.Width - 16, 17);
                using var resolutionFont = new Font(Font.FontFamily, Math.Max(7.5f, Font.Size - 1f));
                TextRenderer.DrawText(g, $"{monitor.Bounds.Width} × {monitor.Bounds.Height}",
                    resolutionFont, resolution, _theme.Subtle,
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.Top | TextFormatFlags.NoPadding);
            }
        }
    }

    private bool AlwaysPaintHandle(BoundaryView boundary) =>
        boundary.Customized || boundary.SafetyDemoted || boundary == _selectedBoundary || boundary == _hoveredBoundary;

    private IEnumerable<BoundaryView> StandingBoundaries()
    {
        var required = _boundaries.Where(b => b.Customized || b.SafetyDemoted || b == _selectedBoundary || b == _hoveredBoundary);
        var neutral = _boundaries.Where(b => !b.Customized && !b.SafetyDemoted && b != _selectedBoundary && b != _hoveredBoundary)
            .OrderByDescending(b => b.CorridorEnd - b.CorridorStart).ThenBy(b => b.GapPx).ThenBy(b => b.SortOrder)
            .Take(Math.Min(12, Math.Max(2, _monitors.Count * 2)));
        return required.Concat(neutral).Distinct();
    }

    private void DrawBoundary(Graphics g, BoundaryView b, bool paintHandle = true)
    {
        bool selected = b == _selectedBoundary;
        bool hovered = b == _hoveredBoundary;
        Color line = selected || hovered ? _theme.Accent : _theme.BorderStrong;
        using var pen = new Pen(line, selected ? 2.4f : 2f);
        if (b.GapPx > _settings.SeamTolerancePx) pen.DashStyle = DashStyle.Dot;
        DrawInterruptedSeam(g, b, pen);
        if (b.GapPx > _settings.SeamTolerancePx)
        {
            DrawTick(g, b.LineStart, b.VerticalConnection, line);
            DrawTick(g, b.LineEnd, b.VerticalConnection, line);
        }

        if (hovered)
        {
            using var halo = new SolidBrush(_theme.AccentWeak);
            g.FillEllipse(halo, b.Handle.X - 10, b.Handle.Y - 10, 20, 20);
        }

        if (b.Customized && paintHandle)
        {
            var first = b.FirstToSecond?.Behavior ?? BoundaryBehavior.Free;
            var second = b.SecondToFirst?.Behavior ?? BoundaryBehavior.Free;
            if (first == second) DrawModeGlyph(g, b.Handle, first);
            else
            {
                DrawModeGlyph(g, new Point(b.Handle.X - (b.VerticalConnection ? 9 : 0), b.Handle.Y - (b.VerticalConnection ? 0 : 9)), first, 0.75f);
                DrawModeGlyph(g, new Point(b.Handle.X + (b.VerticalConnection ? 9 : 0), b.Handle.Y + (b.VerticalConnection ? 0 : 9)), second, 0.75f);
            }
        }
        if (b.SafetyDemoted)
        {
            using var shield = new Pen(_theme.Accent, 1.5f);
            var pts = new[] { new Point(b.Handle.X - 7, b.Handle.Y + 7), new Point(b.Handle.X, b.Handle.Y + 11), new Point(b.Handle.X + 7, b.Handle.Y + 7) };
            g.DrawLines(shield, pts);
        }
        if (selected || hovered)
        {
            DrawDirectionArrow(g, b);
            string text = BoundaryPairText(b);
            var label = BoundaryPillRect(g, b);
            using (var pill = Rounded(label, 8))
            using (var fill = new SolidBrush(_theme.Surface))
            using (var outline = new Pen(_theme.Border))
            {
                g.FillPath(fill, pill);
                g.DrawPath(outline, pill);
            }
            var textRect = Rectangle.Inflate(label, -4, -1);
            TextRenderer.DrawText(g, text, Font, textRect, _theme.Text,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis | TextFormatFlags.NoPadding);
        }
    }

    private Rectangle BoundaryPillRect(Graphics g, BoundaryView boundary)
    {
        string text = BoundaryPairText(boundary);
        int measured = Math.Min(190, TextRenderer.MeasureText(g, text, Font, Size.Empty, TextFormatFlags.NoPadding).Width + 12);
        int labelX = Math.Clamp(boundary.Handle.X - measured / 2, 3, Math.Max(3, Width - measured - 3));
        int labelY = Math.Clamp(boundary.Handle.Y - 36, 3, Math.Max(3, Height - 25));
        return new Rectangle(labelX, labelY, measured, 22);
    }

    private void DrawModeGlyph(Graphics g, Point p, BoundaryBehavior behavior, float scale = 1f)
    {
        int n = Math.Max(5, (int)(6 * scale));
        if (behavior == BoundaryBehavior.Blocked)
        {
            using var bar = new Pen(_theme.Text, Math.Max(3, 5 * scale));
            g.DrawLine(bar, p.X - n, p.Y, p.X + n, p.Y);
        }
        else if (behavior is BoundaryBehavior.Push or BoundaryBehavior.OverallFeel)
        {
            using var brush = new SolidBrush(_theme.Accent);
            g.FillPolygon(brush, new[] { new Point(p.X, p.Y - n), new Point(p.X + n, p.Y), new Point(p.X, p.Y + n), new Point(p.X - n, p.Y) });
        }
        else
        {
            using var ring = new Pen(_theme.Accent, Math.Max(2, 2 * scale));
            g.DrawEllipse(ring, p.X - n, p.Y - n, n * 2, n * 2);
        }
    }

    private static void DrawTick(Graphics g, Point p, bool horizontalLine, Color color)
    {
        using var pen = new Pen(color, 1.5f);
        if (horizontalLine) g.DrawLine(pen, p.X, p.Y - 4, p.X, p.Y + 4);
        else g.DrawLine(pen, p.X - 4, p.Y, p.X + 4, p.Y);
    }

    private void DrawInterruptedSeam(Graphics g, BoundaryView boundary, Pen pen)
    {
        const int halfOpening = 14, post = 6;
        using var eraser = new Pen(_theme.Sidebar, Math.Max(4f, pen.Width + 2f))
            { StartCap = LineCap.Flat, EndCap = LineCap.Flat };
        if (boundary.VerticalConnection)
        {
            g.DrawLine(eraser, boundary.Handle.X - halfOpening, boundary.Handle.Y,
                boundary.Handle.X + halfOpening, boundary.Handle.Y);
            g.DrawLine(pen, boundary.LineStart, new Point(Math.Max(boundary.LineStart.X, boundary.Handle.X - halfOpening), boundary.Handle.Y));
            g.DrawLine(pen, new Point(Math.Min(boundary.LineEnd.X, boundary.Handle.X + halfOpening), boundary.Handle.Y), boundary.LineEnd);
            g.DrawLine(pen, boundary.Handle.X - halfOpening, boundary.Handle.Y - post, boundary.Handle.X - halfOpening, boundary.Handle.Y + post);
            g.DrawLine(pen, boundary.Handle.X + halfOpening, boundary.Handle.Y - post, boundary.Handle.X + halfOpening, boundary.Handle.Y + post);
        }
        else
        {
            g.DrawLine(eraser, boundary.Handle.X, boundary.Handle.Y - halfOpening,
                boundary.Handle.X, boundary.Handle.Y + halfOpening);
            g.DrawLine(pen, boundary.LineStart, new Point(boundary.Handle.X, Math.Max(boundary.LineStart.Y, boundary.Handle.Y - halfOpening)));
            g.DrawLine(pen, new Point(boundary.Handle.X, Math.Min(boundary.LineEnd.Y, boundary.Handle.Y + halfOpening)), boundary.LineEnd);
            g.DrawLine(pen, boundary.Handle.X - post, boundary.Handle.Y - halfOpening, boundary.Handle.X + post, boundary.Handle.Y - halfOpening);
            g.DrawLine(pen, boundary.Handle.X - post, boundary.Handle.Y + halfOpening, boundary.Handle.X + post, boundary.Handle.Y + halfOpening);
        }
    }

    private void DrawDirectionArrow(Graphics g, BoundaryView b)
    {
        using var brush = new SolidBrush(_theme.Accent);
        int x = b.Handle.X, y = b.Handle.Y;
        Point[] a, c;
        if (b.VerticalConnection)
        {
            a = new[] { new Point(x - 4, y - 13), new Point(x + 4, y - 13), new Point(x, y - 18) };
            c = new[] { new Point(x - 4, y + 13), new Point(x + 4, y + 13), new Point(x, y + 18) };
        }
        else
        {
            a = new[] { new Point(x - 13, y - 4), new Point(x - 13, y + 4), new Point(x - 18, y) };
            c = new[] { new Point(x + 13, y - 4), new Point(x + 13, y + 4), new Point(x + 18, y) };
        }
        g.FillPolygon(brush, a); g.FillPolygon(brush, c);
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        var boundary = HitBoundaries(e.Location).FirstOrDefault();
        var monitor = boundary == null ? HitMonitor(e.Location, false) : null;
        bool changed = boundary != _hoveredBoundary || !Same(monitor?.StableId, _hoveredStableId);
        _hoveredBoundary = boundary;
        _hoveredStableId = monitor?.StableId;
        Cursor = boundary != null || monitor != null ? Cursors.Hand : Cursors.Default;
        _toolTip.SetToolTip(this, boundary != null ? BoundaryDescription(boundary) : monitor != null ? ScreenDescription(monitor) : null);
        if (changed) { MonitorHovered?.Invoke(monitor); Invalidate(); }
    }

    protected override void OnMouseLeave(EventArgs e)
    {
        base.OnMouseLeave(e);
        _hoveredBoundary = null; _hoveredStableId = null; Cursor = Cursors.Default;
        _toolTip.SetToolTip(this, null); MonitorHovered?.Invoke(null); Invalidate();
    }

    protected override void OnMouseDown(MouseEventArgs e)
    {
        base.OnMouseDown(e);
        Focus();
        if (e.Button != MouseButtons.Left) return;
        var hits = HitBoundaries(e.Location).ToList();
        if (hits.Count > 0)
        {
            long now = Environment.TickCount64;
            bool cycle = now - _lastClickTick <= 900 && DistanceSquared(e.Location, _lastClick) <= 16;
            _clusterIndex = cycle ? (_clusterIndex + 1) % hits.Count : 0;
            _lastClick = e.Location; _lastClickTick = now;
            _selectedBoundary = hits[_clusterIndex]; _selectedStableId = null;
            BoundaryClicked?.Invoke(_selectedBoundary); Invalidate(); return;
        }
        var monitor = HitMonitor(e.Location, true);
        if (monitor != null) SelectMonitor(monitor, true);
    }

    protected override bool IsInputKey(Keys keyData) =>
        (keyData & Keys.KeyCode) is Keys.Left or Keys.Right or Keys.Up or Keys.Down || base.IsInputKey(keyData);

    protected override void OnKeyDown(KeyEventArgs e)
    {
        base.OnKeyDown(e);
        if (e.KeyCode is Keys.Left or Keys.Right)
        {
            int current = Math.Max(0, _monitors.FindIndex(m => Same(m.StableId, _selectedStableId)));
            int delta = e.KeyCode == Keys.Left ? -1 : 1;
            int next = (current + delta + _monitors.Count) % Math.Max(1, _monitors.Count);
            if (_monitors.Count > 0) SelectMonitor(_monitors.OrderBy(m => m.Index).ElementAt(next), false);
            e.Handled = true;
        }
        else if (e.KeyCode is Keys.Up or Keys.Down)
        {
            var anchor = _selectedBoundary?.First ?? _monitors.FirstOrDefault(m => Same(m.StableId, _selectedStableId)) ?? _monitors.FirstOrDefault();
            var connected = _boundaries.Where(b => Same(b.First.StableId, anchor?.StableId) || Same(b.Second.StableId, anchor?.StableId))
                .OrderBy(b => BoundaryAngle(b, anchor)).ToList();
            if (connected.Count > 0)
            {
                int i = Math.Max(0, connected.IndexOf(_selectedBoundary));
                i = (i + (e.KeyCode == Keys.Up ? -1 : 1) + connected.Count) % connected.Count;
                _selectedBoundary = connected[i]; _selectedStableId = null; Invalidate();
            }
            e.Handled = true;
        }
        else if (e.KeyCode is Keys.Enter or Keys.Space)
        {
            if (_selectedBoundary != null) BoundaryClicked?.Invoke(_selectedBoundary);
            else
            {
                var monitor = _monitors.FirstOrDefault(m => Same(m.StableId, _selectedStableId)) ?? _monitors.FirstOrDefault();
                if (monitor != null) MonitorClicked?.Invoke(monitor);
            }
            e.Handled = true;
        }
        else if (e.KeyCode == Keys.Escape && _selectedBoundary != null)
        {
            BoundaryEscape?.Invoke(); e.Handled = true;
        }
    }

    private void SelectMonitor(MonitorInfo monitor, bool activate)
    {
        _selectedStableId = monitor.StableId; _selectedBoundary = null; Invalidate();
        if (activate) MonitorClicked?.Invoke(monitor);
    }

    private void BuildBoundaries()
    {
        _boundaries.Clear();
        var excluded = new HashSet<string>(_settings.ExcludedDevices ?? new(), StringComparer.OrdinalIgnoreCase);
        var compiled = BoundaryGraph.Compile(_settings, BoundaryGraph.FromMonitors(_monitors, excluded));
        for (int i = 0; i < _monitors.Count; i++)
        for (int j = i + 1; j < _monitors.Count; j++)
        {
            var a = _monitors[i]; var b = _monitors[j];
            if (!TryBoundary(a, b, out var direction, out int start, out int end, out int gap)) continue;
            var pairTransitions = compiled.Transitions.Where(t =>
                (Same(t.FromStableId, a.StableId) && Same(t.ToStableId, b.StableId)) ||
                (Same(t.FromStableId, b.StableId) && Same(t.ToStableId, a.StableId))).ToList();
            var segments = pairTransitions.Select(t => (t.CorridorStart, t.CorridorEnd, t.GapPx)).Distinct().ToList();
            bool hasExcluded = excluded.Contains(a.StableId) || excluded.Contains(b.StableId);
            if (segments.Count == 0 && hasExcluded) segments.Add((start, end, gap));
            foreach (var segment in segments)
            {
                var view = new BoundaryView
                {
                    First = a, Second = b, FirstToSecondDirection = direction,
                    CorridorStart = segment.CorridorStart, CorridorEnd = segment.CorridorEnd, GapPx = segment.GapPx,
                    HasExcludedScreen = hasExcluded,
                };
                view.FirstToSecond = pairTransitions.FirstOrDefault(t => Same(t.FromStableId, a.StableId) &&
                    Same(t.ToStableId, b.StableId) && t.Direction == direction
                    && t.CorridorStart == segment.CorridorStart && t.CorridorEnd == segment.CorridorEnd);
                view.SecondToFirst = pairTransitions.FirstOrDefault(t => Same(t.FromStableId, b.StableId) &&
                    Same(t.ToStableId, a.StableId) && t.Direction == view.SecondToFirstDirection
                    && t.CorridorStart == segment.CorridorStart && t.CorridorEnd == segment.CorridorEnd);
                _boundaries.Add(view);
            }
        }
        _boundaries.Sort((a, b) =>
        {
            int c = (a.VerticalConnection ? 0 : 1).CompareTo(b.VerticalConnection ? 0 : 1);
            if (c != 0) return c;
            c = a.CorridorStart.CompareTo(b.CorridorStart); if (c != 0) return c;
            c = a.CorridorEnd.CompareTo(b.CorridorEnd); if (c != 0) return c;
            c = string.Compare(a.First.StableId, b.First.StableId, StringComparison.Ordinal); if (c != 0) return c;
            return string.Compare(a.Second.StableId, b.Second.StableId, StringComparison.Ordinal);
        });
    }

    private static bool TryBoundary(MonitorInfo a, MonitorInfo b, out BoundaryDirection direction,
                                    out int start, out int end, out int gap)
    {
        direction = BoundaryDirection.Unknown; start = end = gap = 0;
        if (b.Bounds.Bottom <= a.Bounds.Top && (end = Math.Min(a.Bounds.Right, b.Bounds.Right)) > (start = Math.Max(a.Bounds.Left, b.Bounds.Left)))
        { direction = BoundaryDirection.Up; gap = a.Bounds.Top - b.Bounds.Bottom; return true; }
        if (b.Bounds.Top >= a.Bounds.Bottom && (end = Math.Min(a.Bounds.Right, b.Bounds.Right)) > (start = Math.Max(a.Bounds.Left, b.Bounds.Left)))
        { direction = BoundaryDirection.Down; gap = b.Bounds.Top - a.Bounds.Bottom; return true; }
        if (b.Bounds.Right <= a.Bounds.Left && (end = Math.Min(a.Bounds.Bottom, b.Bounds.Bottom)) > (start = Math.Max(a.Bounds.Top, b.Bounds.Top)))
        { direction = BoundaryDirection.Left; gap = a.Bounds.Left - b.Bounds.Right; return true; }
        if (b.Bounds.Left >= a.Bounds.Right && (end = Math.Min(a.Bounds.Bottom, b.Bounds.Bottom)) > (start = Math.Max(a.Bounds.Top, b.Bounds.Top)))
        { direction = BoundaryDirection.Right; gap = b.Bounds.Left - a.Bounds.Right; return true; }
        return false;
    }

    private List<Rectangle> LayoutRects() => GuardCore.MapLayout(
        _monitors.Select(m => (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom)).ToList(),
        ClientSize.Width, ClientSize.Height, 14).Select(r => Rectangle.FromLTRB(r.L, r.T, r.R, r.B)).ToList();

    private void EnsureMinimumFootprints(List<Rectangle> rects)
    {
        for (int i = 0; i < rects.Count; i++)
        {
            var r = rects[i]; int w = Math.Max(12, r.Width), h = Math.Max(12, r.Height);
            int x = Math.Clamp(r.X - (w - r.Width) / 2, 2, Math.Max(2, Width - w - 2));
            int y = Math.Clamp(r.Y - (h - r.Height) / 2, 2, Math.Max(2, Height - h - 2));
            rects[i] = new Rectangle(x, y, w, h);
        }
    }

    private void PositionBoundaries(IReadOnlyList<Rectangle> rects)
    {
        foreach (var b in _boundaries)
        {
            int ai = _monitors.IndexOf(b.First), bi = _monitors.IndexOf(b.Second);
            if (ai < 0 || bi < 0 || ai >= rects.Count || bi >= rects.Count) continue;
            var ar = rects[ai]; var br = rects[bi];
            if (b.VerticalConnection)
            {
                float scale = b.First.Bounds.Width == 0 ? 1 : ar.Width / (float)b.First.Bounds.Width;
                int x1 = ar.Left + (int)Math.Round((b.CorridorStart - b.First.Bounds.Left) * scale);
                int x2 = ar.Left + (int)Math.Round((b.CorridorEnd - b.First.Bounds.Left) * scale);
                int yA = b.FirstToSecondDirection == BoundaryDirection.Up ? ar.Top : ar.Bottom;
                int yB = b.FirstToSecondDirection == BoundaryDirection.Up ? br.Bottom : br.Top;
                int y = (yA + yB) / 2;
                b.LineStart = new Point(x1, y); b.LineEnd = new Point(Math.Max(x1 + 1, x2), y);
            }
            else
            {
                float scale = b.First.Bounds.Height == 0 ? 1 : ar.Height / (float)b.First.Bounds.Height;
                int y1 = ar.Top + (int)Math.Round((b.CorridorStart - b.First.Bounds.Top) * scale);
                int y2 = ar.Top + (int)Math.Round((b.CorridorEnd - b.First.Bounds.Top) * scale);
                int xA = b.FirstToSecondDirection == BoundaryDirection.Left ? ar.Left : ar.Right;
                int xB = b.FirstToSecondDirection == BoundaryDirection.Left ? br.Right : br.Left;
                int x = (xA + xB) / 2;
                b.LineStart = new Point(x, y1); b.LineEnd = new Point(x, Math.Max(y1 + 1, y2));
            }
            b.Handle = new Point((b.LineStart.X + b.LineEnd.X) / 2, (b.LineStart.Y + b.LineEnd.Y) / 2);
            b.HitTarget = new Rectangle(b.Handle.X - 12, b.Handle.Y - 12, 24, 24);
        }
    }

    private IEnumerable<BoundaryView> HitBoundaries(Point p) => _boundaries.Where(b =>
            b.HitTarget.Contains(p) || DistanceToSegmentSquared(p, b.LineStart, b.LineEnd) <= 36)
        .OrderBy(b => !b.HitTarget.Contains(p)).ThenBy(b => DistanceSquared(p, b.Handle))
        .ThenBy(b => b.CorridorEnd - b.CorridorStart).ThenBy(b => b.SortOrder);

    private MonitorInfo HitMonitor(Point p, bool inflate)
    {
        var rects = LayoutRects(); EnsureMinimumFootprints(rects);
        for (int i = rects.Count - 1; i >= 0; i--)
        {
            var r = rects[i];
            if (inflate && (r.Width < 24 || r.Height < 24)) r.Inflate(Math.Max(0, (24 - r.Width) / 2), Math.Max(0, (24 - r.Height) / 2));
            if (r.Contains(p)) return _monitors[i];
        }
        return null;
    }

    private string NameFor(MonitorInfo monitor)
    {
        string name = Settings.MonitorNameFor(_names, monitor?.StableId);
        return name.Length > 0 ? name : (monitor?.Primary == true ? Strings.Ui("MainWord") : string.Format(Strings.Ui("DefaultScreenName"), monitor?.Index));
    }

    private string BoundaryPairText(BoundaryView boundary) =>
        NameFor(boundary.First) + (boundary.VerticalConnection ? "  ↕  " : "  ↔  ") + NameFor(boundary.Second);

    private string ScreenDescription(MonitorInfo m) => string.Format(Strings.Ui("ScreenAccessible"), m.Index, NameFor(m),
        m.Bounds.Width, m.Bounds.Height, m.Primary ? Strings.Ui("MainScreenAvailable") :
        (_settings.ExcludedDevices.Contains(m.StableId, StringComparer.OrdinalIgnoreCase) ? Strings.Ui("BlockedByKeepOut") : ""));

    private string BoundaryDescription(BoundaryView b)
    {
        string geometry = b.GapPx > _settings.SeamTolerancePx ? string.Format(Strings.Ui("GapFormat"), b.GapPx) : Strings.Ui("TouchingBoundary");
        return string.Format(Strings.Ui("BoundaryAccessible"), BoundaryPairText(b), Strings.Ui("DrawerTitle"), geometry,
            Strings.Ui("FirstDirection"), BehaviorText(b.FirstToSecond?.Behavior), Strings.Ui("ReturnDirection"),
            BehaviorText(b.SecondToFirst?.Behavior), b.HasExcludedScreen ? Strings.Ui("BlockedByKeepOut") : "");
    }

    private static string BehaviorText(BoundaryBehavior? b) => b switch
    {
        BoundaryBehavior.Blocked => Strings.Ui("NeverCrosses"),
        BoundaryBehavior.Push or BoundaryBehavior.OverallFeel => Strings.Ui("NeedsFirmPush"),
        _ => Strings.Ui("CrossesFreely"),
    };

    private static float BoundaryAngle(BoundaryView b, MonitorInfo anchor)
    {
        var other = Same(b.First.StableId, anchor?.StableId) ? b.Second : b.First;
        double ax = (anchor.Bounds.Left + anchor.Bounds.Right) / 2.0, ay = (anchor.Bounds.Top + anchor.Bounds.Bottom) / 2.0;
        double ox = (other.Bounds.Left + other.Bounds.Right) / 2.0, oy = (other.Bounds.Top + other.Bounds.Bottom) / 2.0;
        return (float)Math.Atan2(ox - ax, ay - oy);
    }

    protected override AccessibleObject CreateAccessibilityInstance() => new MapAccessibleObject(this);

    private sealed class MapAccessibleObject : ControlAccessibleObject
    {
        private readonly MonitorMapControl _owner;
        public MapAccessibleObject(MonitorMapControl owner) : base(owner) => _owner = owner;
        public override int GetChildCount() => _owner._monitors.Count + _owner._boundaries.Count;
        public override AccessibleObject GetChild(int index)
        {
            if (index < 0 || index >= GetChildCount()) return null;
            return new MapChildAccessibleObject(_owner, index);
        }
    }

    private sealed class MapChildAccessibleObject : AccessibleObject
    {
        private readonly MonitorMapControl _owner; private readonly int _index;
        public MapChildAccessibleObject(MonitorMapControl owner, int index) { _owner = owner; _index = index; }
        private bool IsMonitor => _index < _owner._monitors.Count;
        public override string Name => IsMonitor ? _owner.ScreenDescription(_owner._monitors[_index]) :
            _owner.BoundaryDescription(_owner._boundaries[_index - _owner._monitors.Count]);
        public override AccessibleRole Role => AccessibleRole.PushButton;
        public override string DefaultAction => IsMonitor ? Strings.Ui("IdentifyLabel") : Strings.Ui("OpenBoundary");
        public override AccessibleStates State => AccessibleStates.Focusable | AccessibleStates.Selectable;
        public override void DoDefaultAction()
        {
            if (IsMonitor) _owner.SelectMonitor(_owner._monitors[_index], true);
            else
            {
                var b = _owner._boundaries[_index - _owner._monitors.Count];
                _owner.SelectedBoundary = b; _owner.BoundaryClicked?.Invoke(b);
            }
        }
    }

    protected override void Dispose(bool disposing) { if (disposing) _toolTip.Dispose(); base.Dispose(disposing); }
    private static GraphicsPath Rounded(Rectangle r, int radius)
    {
        int d = Math.Min(radius * 2, Math.Min(r.Width, r.Height));
        var p = new GraphicsPath();
        if (d <= 1) { p.AddRectangle(r); return p; }
        p.AddArc(r.Left, r.Top, d, d, 180, 90); p.AddArc(r.Right - d, r.Top, d, d, 270, 90);
        p.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90); p.AddArc(r.Left, r.Bottom - d, d, d, 90, 90); p.CloseFigure(); return p;
    }
    private static bool Same(string a, string b) => string.Equals(a, b, StringComparison.OrdinalIgnoreCase);
    private static bool SamePair(BoundaryView a, BoundaryView b) =>
        (Same(a.First.StableId, b.First.StableId) && Same(a.Second.StableId, b.Second.StableId)) ||
        (Same(a.First.StableId, b.Second.StableId) && Same(a.Second.StableId, b.First.StableId));
    private static int DistanceSquared(Point a, Point b) { int x = a.X - b.X, y = a.Y - b.Y; return x * x + y * y; }
    private static double DistanceToSegmentSquared(Point p, Point a, Point b)
    {
        double dx = b.X - a.X, dy = b.Y - a.Y;
        if (dx == 0 && dy == 0) return DistanceSquared(p, a);
        double t = Math.Clamp(((p.X - a.X) * dx + (p.Y - a.Y) * dy) / (dx * dx + dy * dy), 0, 1);
        double x = a.X + t * dx - p.X, y = a.Y + t * dy - p.Y; return x * x + y * y;
    }
}
