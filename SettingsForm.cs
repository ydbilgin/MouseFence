using System.Drawing;

namespace MouseFence;

/// <summary>Settings dialog: General (hotkey, startup), Appearance (language, theme), Monitors (top monitors + per-screen crossing rules).</summary>
public sealed class SettingsForm : Form
{
    private readonly Settings _orig;
    private readonly List<MonitorInfo> _monitors;
    private readonly List<MonitorInfo> _tops;
    private readonly List<MonitorInfo> _bottoms;

    private readonly TextBox _hotkeyBox;
    private readonly CheckBox _startActive;
    private readonly CheckBox _autoStart;
    private readonly ComboBox _langCombo;
    private readonly ComboBox _themeCombo;
    private readonly RadioButton _modeAuto;
    private readonly RadioButton _modeManual;
    private readonly CheckedListBox _monitorList;
    private readonly ComboBox _fromCombo;
    private readonly CheckedListBox _topsCheck;
    private readonly Panel _map;

    private readonly TabControl _tabs;
    private readonly TextBox _gameHotkeyBox;
    private readonly CheckBox _deliberateCheck;
    private readonly Dictionary<string, HashSet<string>> _rules = new();
    private bool _mc, _ma, _ms, _mw;
    private Keys _key;
    private bool _gc, _ga, _gs, _gw;
    private Keys _gkey;

    public Settings Result { get; private set; }

    /// <summary>Dev helper for docs screenshots: select a tab by index.</summary>
    public void SelectTab(int i) { if (i >= 0 && i < _tabs.TabPages.Count) _tabs.SelectedIndex = i; }

    public SettingsForm(Settings s, List<MonitorInfo> monitors)
    {
        _orig = s;
        _monitors = monitors;
        _mc = s.ModCtrl; _ma = s.ModAlt; _ms = s.ModShift; _mw = s.ModWin; _key = s.HotKey;
        _gc = s.ConfineModCtrl; _ga = s.ConfineModAlt; _gs = s.ConfineModShift; _gw = s.ConfineModWin; _gkey = s.ConfineHotKey;

        _tops = (s.Mode == "Manual" && s.ManualMonitors.Count > 0
            ? monitors.Where(m => s.ManualMonitors.Contains(m.Device))
            : monitors.Where(MonitorInfo.IsAbovePrimary)).ToList();
        var topSet = new HashSet<string>(_tops.Select(t => t.Device));
        _bottoms = monitors.Where(m => !topSet.Contains(m.Device)).ToList();
        foreach (var lk in s.UpLinks)
        {
            if (!_rules.TryGetValue(lk.FromDevice, out var set)) { set = new HashSet<string>(); _rules[lk.FromDevice] = set; }
            set.Add(lk.ToDevice);
        }

        Text = Strings.SettingsTitle;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = false;
        MinimizeBox = false;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.CenterScreen;
        Font = new Font("Segoe UI", 9f);
        ClientSize = new Size(470, 540);

        var headFont = new Font("Segoe UI Semibold", 9f);

        _tabs = new TabControl { Left = 12, Top = 12, Width = 446, Height = 476, DrawMode = TabDrawMode.OwnerDrawFixed, SizeMode = TabSizeMode.Fixed, ItemSize = new Size(147, 28) };
        _tabs.DrawItem += DrawTab;
        var tabGeneral = new TabPage(Strings.TabGeneral) { Padding = new Padding(10) };
        var tabAppearance = new TabPage(Strings.TabAppearance) { Padding = new Padding(10) };
        var tabMonitors = new TabPage(Strings.TabMonitors) { Padding = new Padding(10) };
        _tabs.TabPages.AddRange(new[] { tabGeneral, tabAppearance, tabMonitors });

        // ---- General ----
        int y = 12;
        tabGeneral.Controls.Add(new Label { Text = Strings.HotkeyHead, AutoSize = true, Left = 12, Top = y, Font = headFont, Tag = "head" });
        y += 20;
        tabGeneral.Controls.Add(new Label { Text = Strings.HotkeyHint, AutoSize = true, Left = 12, Top = y, Tag = "subtle" });
        y += 22;
        _hotkeyBox = new TextBox
        {
            Left = 12, Top = y, Width = 300, Height = 26, ReadOnly = true, Cursor = Cursors.Hand,
            TextAlign = HorizontalAlignment.Center, Font = new Font("Segoe UI Semibold", 9.5f), BorderStyle = BorderStyle.FixedSingle
        };
        _hotkeyBox.KeyDown += HotkeyBox_KeyDown;
        var btnClear = new Button { Text = Strings.Clear, Left = 320, Top = y - 1, Width = 96, Height = 28, FlatStyle = FlatStyle.Flat };
        btnClear.Click += (a, b) => { _mc = _ma = _ms = _mw = false; _key = Keys.None; UpdateHotkeyText(); };
        tabGeneral.Controls.Add(_hotkeyBox);
        tabGeneral.Controls.Add(btnClear);
        y += 44;
        tabGeneral.Controls.Add(new Label { Text = Strings.StartupHead, AutoSize = true, Left = 12, Top = y, Font = headFont, Tag = "head" });
        y += 22;
        _startActive = new CheckBox { Text = Strings.StartGateClosed, Left = 12, Top = y, Width = 400, Height = 22, Checked = s.StartGateClosed };
        tabGeneral.Controls.Add(_startActive);
        y += 26;
        _autoStart = new CheckBox { Text = Strings.StartWithWindows, Left = 12, Top = y, Width = 400, Height = 22, Checked = s.AutoStart };
        tabGeneral.Controls.Add(_autoStart);
        y += 34;
        tabGeneral.Controls.Add(new Label { Text = Strings.GameHotkeyHead, AutoSize = true, Left = 12, Top = y, Font = headFont, Tag = "head" });
        y += 20;
        tabGeneral.Controls.Add(new Label { Text = Strings.GameHotkeyHint, Left = 12, Top = y, Width = 412, Height = 30, Tag = "subtle" });
        y += 32;
        _gameHotkeyBox = new TextBox
        {
            Left = 12, Top = y, Width = 300, Height = 26, ReadOnly = true, Cursor = Cursors.Hand,
            TextAlign = HorizontalAlignment.Center, Font = new Font("Segoe UI Semibold", 9.5f), BorderStyle = BorderStyle.FixedSingle
        };
        _gameHotkeyBox.KeyDown += GameHotkeyBox_KeyDown;
        var btnClearGame = new Button { Text = Strings.Clear, Left = 320, Top = y - 1, Width = 96, Height = 28, FlatStyle = FlatStyle.Flat };
        btnClearGame.Click += (a, b) => { _gc = _ga = _gs = _gw = false; _gkey = Keys.None; UpdateGameHotkeyText(); };
        tabGeneral.Controls.Add(_gameHotkeyBox);
        tabGeneral.Controls.Add(btnClearGame);

        // ---- Appearance ----
        tabAppearance.Controls.Add(new Label { Text = Strings.LanguageLabel, AutoSize = true, Left = 12, Top = 14, Tag = "subtle" });
        tabAppearance.Controls.Add(new Label { Text = Strings.ThemeLabel, AutoSize = true, Left = 220, Top = 14, Tag = "subtle" });
        _langCombo = new ComboBox { Left = 12, Top = 34, Width = 190, Height = 24, DropDownStyle = ComboBoxStyle.DropDownList };
        _langCombo.Items.AddRange(new object[] { Strings.LangAuto, "English", "Türkçe" });
        _langCombo.SelectedIndex = s.Language switch { "en" => 1, "tr" => 2, _ => 0 };
        _themeCombo = new ComboBox { Left = 220, Top = 34, Width = 190, Height = 24, DropDownStyle = ComboBoxStyle.DropDownList };
        _themeCombo.Items.AddRange(new object[] { Strings.ThemeSystem, Strings.ThemeLight, Strings.ThemeDark });
        _themeCombo.SelectedIndex = s.Theme switch { "Light" => 1, "Dark" => 2, _ => 0 };
        _themeCombo.SelectedIndexChanged += (a, b) => ApplyThemeLive();
        tabAppearance.Controls.Add(_langCombo);
        tabAppearance.Controls.Add(_themeCombo);

        // ---- Monitors ----
        tabMonitors.Controls.Add(new Label { Text = Strings.MonitorsHead, AutoSize = true, Left = 12, Top = 10, Font = headFont, Tag = "head" });
        _modeAuto = new RadioButton { Text = Strings.ModeAuto, Left = 12, Top = 32, Width = 410, Checked = s.Mode != "Manual" };
        _modeManual = new RadioButton { Text = Strings.ModeManual, Left = 12, Top = 56, Width = 410, Checked = s.Mode == "Manual" };
        _monitorList = new CheckedListBox { Left = 12, Top = 82, Width = 416, Height = 78, CheckOnClick = true, IntegralHeight = false, BorderStyle = BorderStyle.FixedSingle };
        foreach (var m in monitors)
        {
            int idx = _monitorList.Items.Add(MonitorLabel(m));
            bool chk = s.Mode == "Manual" ? s.ManualMonitors.Contains(m.Device) : MonitorInfo.IsAbovePrimary(m);
            _monitorList.SetItemChecked(idx, chk);
        }
        _monitorList.Enabled = _modeManual.Checked;
        _modeManual.CheckedChanged += (a, b) => _monitorList.Enabled = _modeManual.Checked;
        tabMonitors.Controls.Add(_modeAuto);
        tabMonitors.Controls.Add(_modeManual);
        tabMonitors.Controls.Add(_monitorList);

        tabMonitors.Controls.Add(new Label { Text = Strings.RulesHead, AutoSize = true, Left = 12, Top = 172, Font = headFont, Tag = "head" });
        tabMonitors.Controls.Add(new Label { Text = (_tops.Count == 0 ? Strings.NoTopsHint : Strings.RuleHint), Left = 12, Top = 192, Width = 416, Height = 32, Tag = "subtle" });

        _map = new Panel { Left = 12, Top = 230, Width = 200, Height = 118, BorderStyle = BorderStyle.FixedSingle };
        _map.Paint += DrawMap;
        tabMonitors.Controls.Add(_map);

        tabMonitors.Controls.Add(new Label { Text = Strings.FromScreenLabel, AutoSize = true, Left = 224, Top = 230, Tag = "subtle" });
        _fromCombo = new ComboBox { Left = 224, Top = 250, Width = 204, Height = 24, DropDownStyle = ComboBoxStyle.DropDownList, Enabled = _tops.Count > 0 };
        foreach (var b in _bottoms) _fromCombo.Items.Add(MonitorLabel(b));
        _fromCombo.SelectedIndexChanged += (a, b) => { LoadTopsForSelectedFrom(); _map.Invalidate(); };
        tabMonitors.Controls.Add(_fromCombo);

        tabMonitors.Controls.Add(new Label { Text = Strings.AllowedTopsLabel, AutoSize = true, Left = 224, Top = 280, Tag = "subtle" });
        _topsCheck = new CheckedListBox { Left = 224, Top = 300, Width = 204, Height = 70, CheckOnClick = true, IntegralHeight = false, BorderStyle = BorderStyle.FixedSingle, Enabled = _tops.Count > 0 };
        foreach (var t in _tops) _topsCheck.Items.Add(MonitorLabel(t));
        _topsCheck.ItemCheck += TopsCheck_ItemCheck;
        tabMonitors.Controls.Add(_topsCheck);

        _deliberateCheck = new CheckBox { Text = Strings.DeliberateCrossLabel, Left = 12, Top = 378, Width = 416, Height = 34, Checked = s.DeliberateCross };
        tabMonitors.Controls.Add(_deliberateCheck);

        if (_bottoms.Count > 0 && _tops.Count > 0) { _fromCombo.SelectedIndex = 0; }

        // ---- buttons ----
        var cancel = new Button { Text = Strings.Cancel, Left = ClientSize.Width - 12 - 96, Top = 500, Width = 96, Height = 30, FlatStyle = FlatStyle.Flat, DialogResult = DialogResult.Cancel };
        var ok = new Button { Text = Strings.Save, Left = cancel.Left - 104, Top = 500, Width = 96, Height = 30, FlatStyle = FlatStyle.Flat, Tag = "primary", DialogResult = DialogResult.OK };
        ok.Click += Ok_Click;

        Controls.Add(_tabs);
        Controls.Add(ok);
        Controls.Add(cancel);
        AcceptButton = ok;
        CancelButton = cancel;

        Result = s;
        UpdateHotkeyText();
        UpdateGameHotkeyText();
        Theming.Apply(this, SelectedTheme());
    }

    private string MonitorLabel(MonitorInfo m)
    {
        string dev = m.Device.Replace(@"\\.\", "");
        return $"#{m.Index}  {dev}  {m.Bounds.Width}x{m.Bounds.Height}{(m.Primary ? "  " + Strings.MainTag : "")}";
    }

    private string SelectedFromDevice() =>
        _fromCombo.SelectedIndex >= 0 && _fromCombo.SelectedIndex < _bottoms.Count ? _bottoms[_fromCombo.SelectedIndex].Device : null;

    private void LoadTopsForSelectedFrom()
    {
        string from = SelectedFromDevice();
        _rules.TryGetValue(from ?? "", out var allowed);
        for (int i = 0; i < _topsCheck.Items.Count; i++)
        {
            bool on = allowed != null && allowed.Contains(_tops[i].Device);
            _topsCheck.SetItemChecked(i, on);
        }
    }

    private void TopsCheck_ItemCheck(object sender, ItemCheckEventArgs e)
    {
        string from = SelectedFromDevice();
        if (from == null) return;
        if (!_rules.TryGetValue(from, out var set)) { set = new HashSet<string>(); _rules[from] = set; }
        string to = _tops[e.Index].Device;
        if (e.NewValue == CheckState.Checked) set.Add(to); else set.Remove(to);
        if (_map.IsHandleCreated) _map.Invalidate();
    }

    private void DrawMap(object sender, PaintEventArgs e)
    {
        var t = SelectedTheme();
        var g = e.Graphics;
        g.Clear(t.Surface);
        if (_monitors.Count == 0) return;

        int vl = _monitors.Min(m => m.Bounds.Left), vt = _monitors.Min(m => m.Bounds.Top);
        int vr = _monitors.Max(m => m.Bounds.Right), vb = _monitors.Max(m => m.Bounds.Bottom);
        float vw = vr - vl, vh = vb - vt;
        if (vw <= 0 || vh <= 0) return;

        var area = new RectangleF(6, 6, _map.Width - 12, _map.Height - 12);
        float scale = Math.Min(area.Width / vw, area.Height / vh);
        float ox = area.X + (area.Width - vw * scale) / 2f;
        float oy = area.Y + (area.Height - vh * scale) / 2f;

        string from = SelectedFromDevice();
        _rules.TryGetValue(from ?? "", out var allowed);
        var topSet = new HashSet<string>(_tops.Select(m => m.Device));

        using var penEdge = new Pen(t.Border);
        using var fontN = new Font("Segoe UI", 7.5f);
        foreach (var m in _monitors)
        {
            var r = new RectangleF(ox + (m.Bounds.Left - vl) * scale, oy + (m.Bounds.Top - vt) * scale, m.Bounds.Width * scale, m.Bounds.Height * scale);
            r.Inflate(-1, -1);
            Color fill = t.Surface;
            if (m.Device == from) fill = ControlPaint.Light(t.Accent, 0.2f);
            else if (allowed != null && allowed.Contains(m.Device)) fill = t.Accent;
            else if (topSet.Contains(m.Device)) fill = ControlPaint.Dark(t.Surface, 0.06f);
            using var b = new SolidBrush(fill);
            g.FillRectangle(b, r);
            g.DrawRectangle(penEdge, r.X, r.Y, r.Width, r.Height);
            bool brightFill = (allowed != null && allowed.Contains(m.Device));
            using var tb = new SolidBrush(brightFill ? Color.White : t.Text);
            g.DrawString(m.Index.ToString(), fontN, tb, r.X + 2, r.Y + 1);
        }
    }

    private Theme SelectedTheme() => Theming.Resolve(_themeCombo.SelectedIndex switch { 1 => "Light", 2 => "Dark", _ => "System" });

    private void DrawTab(object sender, DrawItemEventArgs e)
    {
        var t = SelectedTheme();
        var rect = _tabs.GetTabRect(e.Index);
        bool sel = e.Index == _tabs.SelectedIndex;
        using (var bg = new SolidBrush(sel ? t.Surface : t.Back))
            e.Graphics.FillRectangle(bg, rect);
        if (sel)
            using (var bar = new SolidBrush(t.Accent))
                e.Graphics.FillRectangle(bar, rect.X, rect.Bottom - 2, rect.Width, 2);
        TextRenderer.DrawText(e.Graphics, _tabs.TabPages[e.Index].Text, _tabs.Font, rect,
            sel ? t.Accent : t.Subtle, TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis);
    }

    private void ApplyThemeLive()
    {
        Theming.Apply(this, SelectedTheme());
        _tabs.Invalidate();
        _map.Invalidate();
        Invalidate(true);
    }

    protected override void OnShown(EventArgs e)
    {
        base.OnShown(e);
        Theming.DarkTitleBar(this, SelectedTheme().IsDark);
    }

    private void HotkeyBox_KeyDown(object sender, KeyEventArgs e)
    {
        e.SuppressKeyPress = true;
        e.Handled = true;
        _mc = e.Control; _ma = e.Alt; _ms = e.Shift;
        var kc = e.KeyCode;
        if (kc is Keys.ControlKey or Keys.Menu or Keys.ShiftKey) { _key = Keys.None; UpdateHotkeyText(); return; }
        if (kc is Keys.LWin or Keys.RWin) { _mw = true; _key = Keys.None; UpdateHotkeyText(); return; }
        _key = kc;
        UpdateHotkeyText();
    }

    private void UpdateHotkeyText()
    {
        var parts = new List<string>();
        if (_mc) parts.Add("Ctrl");
        if (_ma) parts.Add("Alt");
        if (_ms) parts.Add("Shift");
        if (_mw) parts.Add("Win");
        if (_key != Keys.None) parts.Add(_key.ToString());
        _hotkeyBox.Text = parts.Count == 0 ? Strings.HotkeyPressKeys : string.Join(" + ", parts);
    }

    private void GameHotkeyBox_KeyDown(object sender, KeyEventArgs e)
    {
        e.SuppressKeyPress = true;
        e.Handled = true;
        _gc = e.Control; _ga = e.Alt; _gs = e.Shift;
        var kc = e.KeyCode;
        if (kc is Keys.ControlKey or Keys.Menu or Keys.ShiftKey) { _gkey = Keys.None; UpdateGameHotkeyText(); return; }
        if (kc is Keys.LWin or Keys.RWin) { _gw = true; _gkey = Keys.None; UpdateGameHotkeyText(); return; }
        _gkey = kc;
        UpdateGameHotkeyText();
    }

    private void UpdateGameHotkeyText()
    {
        var parts = new List<string>();
        if (_gc) parts.Add("Ctrl");
        if (_ga) parts.Add("Alt");
        if (_gs) parts.Add("Shift");
        if (_gw) parts.Add("Win");
        if (_gkey != Keys.None) parts.Add(_gkey.ToString());
        _gameHotkeyBox.Text = parts.Count == 0 ? Strings.HotkeyPressKeys : string.Join(" + ", parts);
    }

    private void Ok_Click(object sender, EventArgs e)
    {
        bool anyMod = _mc || _ma || _ms || _mw;
        if (_key == Keys.None || !anyMod)
        {
            MessageBox.Show(Strings.ValidationPickModifier, "MouseFence", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            DialogResult = DialogResult.None;
            return;
        }

        var res = new Settings
        {
            ModCtrl = _mc, ModAlt = _ma, ModShift = _ms, ModWin = _mw, HotKey = _key,
            StartGateClosed = _startActive.Checked,
            AutoStart = _autoStart.Checked,
            Mode = _modeManual.Checked ? "Manual" : "AutoTop",
            Language = _langCombo.SelectedIndex switch { 1 => "en", 2 => "tr", _ => "auto" },
            Theme = _themeCombo.SelectedIndex switch { 1 => "Light", 2 => "Dark", _ => "System" },
            ConfineModCtrl = _gc, ConfineModAlt = _ga, ConfineModShift = _gs, ConfineModWin = _gw, ConfineHotKey = _gkey,
            DeliberateCross = _deliberateCheck.Checked,
        };

        if (res.Mode == "Manual")
            for (int i = 0; i < _monitorList.Items.Count; i++)
                if (_monitorList.GetItemChecked(i))
                    res.ManualMonitors.Add(_monitors[i].Device);

        foreach (var kv in _rules)
            foreach (var to in kv.Value)
                res.UpLinks.Add(new UpLink { FromDevice = kv.Key, ToDevice = to });

        Result = res;
    }
}
