using System.Drawing;
using System.Drawing.Drawing2D;
using System.ComponentModel;

namespace MouseFence;

/// <summary>
/// Double-buffered, read-only view of the live virtual desktop. It uses GuardCore.MapLayout for both rendering and
/// hit-testing; all changes to MouseFence settings remain in the lists/combos owned by SettingsForm.
/// </summary>
internal sealed class MonitorMapControl : Control
{
    private readonly ToolTip _toolTip = new() { ShowAlways = true };
    private readonly List<MonitorInfo> _monitors = new();
    private HashSet<string> _excluded = new(StringComparer.OrdinalIgnoreCase);
    private HashSet<string> _tops = new(StringComparer.OrdinalIgnoreCase);
    private HashSet<string> _allowed = new(StringComparer.OrdinalIgnoreCase);
    private Dictionary<string, string> _names = new(StringComparer.OrdinalIgnoreCase);
    private Theme _theme = Theme.Light;
    private string _selectedStableId;
    private string _hoveredStableId;
    private string _fromStableId;
    private bool _customPaths;

    public event Action<MonitorInfo> MonitorClicked;
    public event Action<MonitorInfo> MonitorHovered;

    public MonitorMapControl()
    {
        DoubleBuffered = true;
        ResizeRedraw = true;
        TabStop = false;
        SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer |
                 ControlStyles.ResizeRedraw | ControlStyles.UserPaint, true);
    }

    [Browsable(false), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public string SelectedStableId
    {
        get => _selectedStableId;
        set
        {
            if (string.Equals(_selectedStableId, value, StringComparison.OrdinalIgnoreCase)) return;
            _selectedStableId = value;
            Invalidate();
        }
    }

    [Browsable(false), DesignerSerializationVisibility(DesignerSerializationVisibility.Hidden)]
    public string ExternalHoveredStableId
    {
        set
        {
            if (string.Equals(_hoveredStableId, value, StringComparison.OrdinalIgnoreCase)) return;
            _hoveredStableId = value;
            Invalidate();
        }
    }

    public void SetTheme(Theme theme)
    {
        _theme = theme;
        BackColor = theme.Surface;
        ForeColor = theme.Text;
        Invalidate();
    }

    public void SetMonitors(IEnumerable<MonitorInfo> monitors)
    {
        _monitors.Clear();
        if (monitors != null) _monitors.AddRange(monitors);
        if (_selectedStableId != null && !_monitors.Any(m => Same(m.StableId, _selectedStableId)))
            _selectedStableId = null;
        if (_hoveredStableId != null && !_monitors.Any(m => Same(m.StableId, _hoveredStableId)))
            _hoveredStableId = null;
        Invalidate();
    }

    public void SetNames(IReadOnlyDictionary<string, string> names)
    {
        _names = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        if (names != null)
            foreach (var pair in names)
                if (!string.IsNullOrEmpty(pair.Key) && !string.IsNullOrWhiteSpace(pair.Value))
                    _names[pair.Key] = pair.Value.Trim();
        Invalidate();
    }

    public void SetState(IEnumerable<string> excluded, IEnumerable<string> tops, string fromStableId,
        IEnumerable<string> allowed, bool customPaths)
    {
        _excluded = new HashSet<string>(excluded ?? Enumerable.Empty<string>(), StringComparer.OrdinalIgnoreCase);
        _tops = new HashSet<string>(tops ?? Enumerable.Empty<string>(), StringComparer.OrdinalIgnoreCase);
        _allowed = new HashSet<string>(allowed ?? Enumerable.Empty<string>(), StringComparer.OrdinalIgnoreCase);
        _fromStableId = fromStableId;
        _customPaths = customPaths;
        Invalidate();
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        e.Graphics.Clear(_theme.Surface);
        var layout = LayoutRects();

        for (int i = 0; i < _monitors.Count && i < layout.Count; i++)
        {
            var m = _monitors[i];
            var lr = layout[i];
            var rect = Rectangle.FromLTRB(lr.L, lr.T, lr.R, lr.B);
            if (rect.Width < 2 || rect.Height < 2) continue;

            bool isFrom = _customPaths && Same(m.StableId, _fromStableId);
            bool isAllowed = _customPaths && _allowed.Contains(m.StableId);
            bool isExcluded = _excluded.Contains(m.StableId);
            bool isSelected = Same(m.StableId, _selectedStableId);
            bool isHovered = Same(m.StableId, _hoveredStableId);

            Color fill = _theme.Surface;
            if (_tops.Contains(m.StableId)) fill = Blend(_theme.Surface, _theme.Border, 0.38f);
            if (isFrom) fill = Blend(_theme.Surface, _theme.Accent, 0.34f);
            if (isAllowed) fill = _theme.Accent;
            using (var brush = new SolidBrush(fill)) e.Graphics.FillRectangle(brush, rect);

            if (isExcluded)
            {
                using var hatch = new HatchBrush(HatchStyle.ForwardDiagonal,
                    Color.FromArgb(190, _theme.Danger), Color.FromArgb(55, _theme.Danger));
                e.Graphics.FillRectangle(hatch, rect);
            }

            Color edge = isHovered ? ControlPaint.Light(_theme.Accent) : _theme.Border;
            float edgeWidth = isHovered ? 2f : 1f;
            if (isSelected) { edge = _theme.Accent; edgeWidth = 4f; }
            using (var pen = new Pen(edge, edgeWidth) { Alignment = PenAlignment.Inset })
                e.Graphics.DrawRectangle(pen, rect.X, rect.Y, rect.Width - 1, rect.Height - 1);

            string number = m.Primary ? $"{m.Index} ★" : m.Index.ToString();
            Color text = isAllowed ? _theme.AccentText : _theme.Text;
            string name = NameFor(m);
            bool showName = name.Length > 0 && rect.Width >= 72 && rect.Height >= 38;
            if (showName)
            {
                int numberHeight = Math.Max(20, (int)(rect.Height * 0.58f));
                var numberRect = new Rectangle(rect.X + 2, rect.Y + 1, Math.Max(0, rect.Width - 4), numberHeight);
                var nameRect = new Rectangle(rect.X + 4, numberRect.Bottom - 2,
                    Math.Max(0, rect.Width - 8), Math.Max(0, rect.Bottom - numberRect.Bottom));
                float numberSize = Math.Clamp(Math.Min(numberRect.Height * 0.46f,
                    numberRect.Width / Math.Max(1f, number.Length * 0.72f)), 8f, 34f);
                float nameSize = Math.Clamp(Math.Min(nameRect.Height * 0.48f,
                    nameRect.Width / Math.Max(1f, name.Length * 0.58f)), 7f, 13f);
                using var numberFont = new Font("Segoe UI", numberSize, FontStyle.Bold, GraphicsUnit.Point);
                using var nameFont = new Font("Segoe UI", nameSize, FontStyle.Regular, GraphicsUnit.Point);
                TextRenderer.DrawText(e.Graphics, number, numberFont, numberRect, text,
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter |
                    TextFormatFlags.SingleLine | TextFormatFlags.NoPadding | TextFormatFlags.NoPrefix);
                TextRenderer.DrawText(e.Graphics, name, nameFont, nameRect, text,
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter |
                    TextFormatFlags.SingleLine | TextFormatFlags.EndEllipsis |
                    TextFormatFlags.NoPadding | TextFormatFlags.NoPrefix);
            }
            else
            {
                float fontSize = Math.Clamp(Math.Min(rect.Height * 0.42f,
                    rect.Width / Math.Max(1f, number.Length * 0.72f)), 8f, 44f);
                using var font = new Font("Segoe UI", fontSize, FontStyle.Bold, GraphicsUnit.Point);
                TextRenderer.DrawText(e.Graphics, number, font, rect, text,
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter |
                    TextFormatFlags.SingleLine | TextFormatFlags.NoPadding | TextFormatFlags.NoPrefix);
            }
        }

        ControlPaint.DrawBorder(e.Graphics, ClientRectangle, _theme.Border, ButtonBorderStyle.Solid);
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        var monitor = HitTest(e.Location);
        string next = monitor?.StableId;
        if (Same(next, _hoveredStableId)) return;

        _hoveredStableId = next;
        Cursor = monitor == null ? Cursors.Default : Cursors.Hand;
        _toolTip.SetToolTip(this, monitor == null ? null : Detail(monitor));
        MonitorHovered?.Invoke(monitor);
        Invalidate();
    }

    protected override void OnMouseLeave(EventArgs e)
    {
        base.OnMouseLeave(e);
        _hoveredStableId = null;
        Cursor = Cursors.Default;
        _toolTip.SetToolTip(this, null);
        MonitorHovered?.Invoke(null);
        Invalidate();
    }

    protected override void OnMouseClick(MouseEventArgs e)
    {
        base.OnMouseClick(e);
        if (e.Button != MouseButtons.Left) return;
        var monitor = HitTest(e.Location);
        if (monitor != null) MonitorClicked?.Invoke(monitor);
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing) _toolTip.Dispose();
        base.Dispose(disposing);
    }

    private List<(int L, int T, int R, int B)> LayoutRects() => GuardCore.MapLayout(
        _monitors.Select(m => (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom)).ToList(),
        ClientSize.Width, ClientSize.Height, 8);

    private MonitorInfo HitTest(Point point)
    {
        var layout = LayoutRects();
        for (int i = layout.Count - 1; i >= 0; i--)
        {
            var r = layout[i];
            if (point.X >= r.L && point.X < r.R && point.Y >= r.T && point.Y < r.B)
                return _monitors[i];
        }
        return null;
    }

    private string Detail(MonitorInfo m)
    {
        string main = m.Primary ? $" • {Strings.MainTag.Trim('[', ']')}" : "";
        string title = NameFor(m) is { Length: > 0 } name ? $"{m.Index} — {name}" : m.Index.ToString();
        return $"{title} • {m.Bounds.Width}x{m.Bounds.Height} • @ {m.Bounds.Left},{m.Bounds.Top} • {m.Device}{main}";
    }

    private string NameFor(MonitorInfo m) => Settings.MonitorNameFor(_names, m?.StableId);

    private static bool Same(string a, string b) =>
        string.Equals(a, b, StringComparison.OrdinalIgnoreCase);

    private static Color Blend(Color a, Color b, float amount)
    {
        amount = Math.Clamp(amount, 0f, 1f);
        return Color.FromArgb(
            (int)(a.R + (b.R - a.R) * amount),
            (int)(a.G + (b.G - a.G) * amount),
            (int)(a.B + (b.B - a.B) * amount));
    }
}
