using System.Drawing;
using System.Text.Json;

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
    private readonly TextBox _pauseHotkeyBox;
    private readonly TextBox _sideHotkeyBox;
    private readonly CheckBox _startSideActive;
    private readonly NumericUpDown _sideSensNum;
    private readonly CheckBox _deliberateCheck;
    private readonly CheckBox _descentCheck;
    private readonly CheckedListBox _excludeList;
    private readonly ToolTip _tips = new();
    private readonly HashSet<string> _excluded = new();
    private readonly Dictionary<string, HashSet<string>> _rules = new();
    private bool _mc, _ma, _ms, _mw;
    private Keys _key;
    private bool _gc, _ga, _gs, _gw;
    private Keys _gkey;
    private bool _pc, _pa, _ps, _pw;
    private Keys _pkey;
    private bool _sc, _sa, _ss, _sw;
    private Keys _skey;

    public Settings Result { get; private set; }

    /// <summary>Dev helper for docs screenshots: select a tab by index.</summary>
    public void SelectTab(int i) { if (i >= 0 && i < _tabs.TabPages.Count) _tabs.SelectedIndex = i; }

    public SettingsForm(Settings s, List<MonitorInfo> monitors)
    {
        _orig = s;
        _monitors = monitors;
        _mc = s.ModCtrl; _ma = s.ModAlt; _ms = s.ModShift; _mw = s.ModWin; _key = s.HotKey;
        _gc = s.ConfineModCtrl; _ga = s.ConfineModAlt; _gs = s.ConfineModShift; _gw = s.ConfineModWin; _gkey = s.ConfineHotKey;
        _pc = s.PauseModCtrl; _pa = s.PauseModAlt; _ps = s.PauseModShift; _pw = s.PauseModWin; _pkey = s.PauseHotKey;
        _sc = s.SideModCtrl; _sa = s.SideModAlt; _ss = s.SideModShift; _sw = s.SideModWin; _skey = s.SideHotKey;

        _tops = (s.Mode == "Manual" && s.ManualMonitors.Count > 0
            ? monitors.Where(m => s.ManualMonitors.Contains(m.StableId))
            : monitors.Where(MonitorInfo.IsAbovePrimary)).ToList();
        var topSet = new HashSet<string>(_tops.Select(t => t.StableId));
        _bottoms = monitors.Where(m => !topSet.Contains(m.StableId)).ToList();
        foreach (var lk in s.UpLinks)
        {
            if (!_rules.TryGetValue(lk.FromDevice, out var set)) { set = new HashSet<string>(); _rules[lk.FromDevice] = set; }
            set.Add(lk.ToDevice);
        }
        // The exclude list keys on the STABLE per-monitor id (FIX 4), so an exclusion survives \\.\DISPLAYn
        // renumbering. Never seed the primary as excluded (it can't be walled).
        var primaryStable = (monitors.FirstOrDefault(m => m.Primary) ?? monitors.FirstOrDefault())?.StableId;
        foreach (var id in s.ExcludedDevices)
            if (id != primaryStable) _excluded.Add(id);

        Text = Strings.SettingsTitle;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = false;
        MinimizeBox = false;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.CenterScreen;
        Font = new Font("Segoe UI", 9f);
        ClientSize = new Size(470, 706);

        var headFont = new Font("Segoe UI Semibold", 9f);

        _tabs = new TabControl { Left = 12, Top = 12, Width = 446, Height = 640, DrawMode = TabDrawMode.OwnerDrawFixed, SizeMode = TabSizeMode.Fixed, ItemSize = new Size(147, 28) };
        _tabs.DrawItem += DrawTab;
        var tabGeneral = new TabPage(Strings.TabGeneral) { Padding = new Padding(10) };
        var tabAppearance = new TabPage(Strings.TabAppearance) { Padding = new Padding(10) };
        var tabMonitors = new TabPage(Strings.TabMonitors) { Padding = new Padding(10) };
        tabMonitors.AutoScroll = true;
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
        y += 44;
        tabGeneral.Controls.Add(new Label { Text = Strings.PauseHotkeyHead, AutoSize = true, Left = 12, Top = y, Font = headFont, Tag = "head" });
        y += 20;
        tabGeneral.Controls.Add(new Label { Text = Strings.PauseHotkeyHint, Left = 12, Top = y, Width = 412, Height = 30, Tag = "subtle" });
        y += 32;
        _pauseHotkeyBox = new TextBox
        {
            Left = 12, Top = y, Width = 300, Height = 26, ReadOnly = true, Cursor = Cursors.Hand,
            TextAlign = HorizontalAlignment.Center, Font = new Font("Segoe UI Semibold", 9.5f), BorderStyle = BorderStyle.FixedSingle
        };
        _pauseHotkeyBox.KeyDown += PauseHotkeyBox_KeyDown;
        var btnClearPause = new Button { Text = Strings.Clear, Left = 320, Top = y - 1, Width = 96, Height = 28, FlatStyle = FlatStyle.Flat };
        btnClearPause.Click += (a, b) => { _pc = _pa = _ps = _pw = false; _pkey = Keys.None; UpdatePauseHotkeyText(); };
        tabGeneral.Controls.Add(_pauseHotkeyBox);
        tabGeneral.Controls.Add(btnClearPause);
        y += 44;
        tabGeneral.Controls.Add(new Label { Text = Strings.SideHotkeyHead, AutoSize = true, Left = 12, Top = y, Font = headFont, Tag = "head" });
        y += 20;
        tabGeneral.Controls.Add(new Label { Text = Strings.SideHotkeyHint, Left = 12, Top = y, Width = 412, Height = 30, Tag = "subtle" });
        y += 32;
        _sideHotkeyBox = new TextBox
        {
            Left = 12, Top = y, Width = 300, Height = 26, ReadOnly = true, Cursor = Cursors.Hand,
            TextAlign = HorizontalAlignment.Center, Font = new Font("Segoe UI Semibold", 9.5f), BorderStyle = BorderStyle.FixedSingle
        };
        _sideHotkeyBox.KeyDown += SideHotkeyBox_KeyDown;
        var btnClearSide = new Button { Text = Strings.Clear, Left = 320, Top = y - 1, Width = 96, Height = 28, FlatStyle = FlatStyle.Flat };
        btnClearSide.Click += (a, b) => { _sc = _sa = _ss = _sw = false; _skey = Keys.None; UpdateSideHotkeyText(); };
        tabGeneral.Controls.Add(_sideHotkeyBox);
        tabGeneral.Controls.Add(btnClearSide);
        y += 36;
        _startSideActive = new CheckBox { Text = Strings.StartSideContainOn, Left = 12, Top = y, Width = 412, Height = 22, Checked = s.StartSideContainOn };
        tabGeneral.Controls.Add(_startSideActive);
        y += 28;
        tabGeneral.Controls.Add(new Label { Text = Strings.SideSensitivityLabel, AutoSize = true, Left = 12, Top = y + 4, Tag = "subtle" });
        // The min horizontal px/move that counts as a deliberate side push — higher = a firmer barrier. Clamp the
        // seeded value into the control's range so an out-of-range imported setting can't throw on construction.
        _sideSensNum = new NumericUpDown { Left = 150, Top = y, Width = 70, Height = 26, Minimum = 1, Maximum = 200, Value = Math.Clamp(s.SideCrossMin, 1, 200) };
        tabGeneral.Controls.Add(_sideSensNum);
        // One-click reset back to the shipped default (3), so a user who over-tightens the barrier can always recover it.
        var btnDefaultSide = new Button { Text = Strings.DefaultLabel, Left = 228, Top = y - 1, Width = 110, Height = 28, FlatStyle = FlatStyle.Flat };
        btnDefaultSide.Click += (a, b) => _sideSensNum.Value = Settings.DefaultSideCrossMin;
        tabGeneral.Controls.Add(btnDefaultSide);
        y += 30;
        tabGeneral.Controls.Add(new Label { Text = Strings.SideSensitivityHint, Left = 12, Top = y, Width = 412, Height = 44, Tag = "subtle" });

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
            bool chk = s.Mode == "Manual" ? s.ManualMonitors.Contains(m.StableId) : MonitorInfo.IsAbovePrimary(m);
            _monitorList.SetItemChecked(idx, chk);
        }
        _monitorList.Enabled = _modeManual.Checked;
        _modeManual.CheckedChanged += (a, b) => _monitorList.Enabled = _modeManual.Checked;
        tabMonitors.Controls.Add(_modeAuto);
        tabMonitors.Controls.Add(_modeManual);
        tabMonitors.Controls.Add(_monitorList);

        tabMonitors.Controls.Add(new Label { Text = Strings.RulesHead, AutoSize = true, Left = 12, Top = 172, Font = headFont, Tag = "head" });
        // anti-trap: if the live layout isolates a screen, surface it right here (in the same hint slot) so the
        // user configuring monitors knows which one to realign — reuses GuardCore's pure detection.
        string ruleHint = _tops.Count == 0 ? Strings.NoTopsHint : Strings.RuleHint;
        var atRects = _monitors.Select(m => (m.Bounds.Left, m.Bounds.Top, m.Bounds.Right, m.Bounds.Bottom)).ToList();
        var atTopIdx = new HashSet<int>();
        for (int i = 0; i < _monitors.Count; i++) if (topSet.Contains(_monitors[i].StableId)) atTopIdx.Add(i);
        var (atWarn, _) = GuardCore.AntiTrap(atRects, atTopIdx);
        if (atWarn.Count > 0)
            ruleHint = Strings.IsolatedNote(string.Join(", ", atWarn.Select(i => _monitors[i].Index)));
        tabMonitors.Controls.Add(new Label { Text = ruleHint, Left = 12, Top = 192, Width = 416, Height = 32, Tag = "subtle" });

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

        _descentCheck = new CheckBox { Text = Strings.DescentRoutingLabel, Left = 12, Top = 414, Width = 416, Height = 34, Checked = s.DescentRouting };
        tabMonitors.Controls.Add(_descentCheck);

        var identify = new Button { Text = Strings.IdentifyLabel, Left = 168, Top = 8, Width = 124, Height = 26, FlatStyle = FlatStyle.Flat };
        identify.Click += (a, b) => IdentifyOverlay.Flash(_monitors, SelectedTheme(), this);
        _tips.SetToolTip(identify, Strings.IdentifyHint);
        tabMonitors.Controls.Add(identify);

        var resetLayout = new Button { Text = Strings.ResetLayoutLabel, Left = 300, Top = 8, Width = 128, Height = 26, FlatStyle = FlatStyle.Flat };
        resetLayout.Click += ResetLayout_Click;
        tabMonitors.Controls.Add(resetLayout);

        // ---- Exclude displays (wall the cursor out — for a dummy / headless screen it would get lost in) ----
        tabMonitors.Controls.Add(new Label { Text = Strings.ExcludeHead, AutoSize = true, Left = 12, Top = 452, Font = headFont, Tag = "head" });
        tabMonitors.Controls.Add(new Label { Text = Strings.ExcludeHint, Left = 12, Top = 478, Width = 416, Height = 30, Tag = "subtle" });
        _excludeList = new CheckedListBox { Left = 12, Top = 510, Width = 416, Height = 70, CheckOnClick = true, IntegralHeight = false, BorderStyle = BorderStyle.FixedSingle, DrawMode = DrawMode.OwnerDrawFixed };
        var primaryStableEx = (_monitors.FirstOrDefault(m => m.Primary) ?? _monitors.FirstOrDefault())?.StableId;
        foreach (var m in _monitors)
        {
            int idx = _excludeList.Items.Add(MonitorLabel(m));
            _excludeList.SetItemChecked(idx, _excluded.Contains(m.StableId));
        }
        // The primary display can never be excluded (it would lock the user out). FIX 2: render its row DISABLED/greyed
        // (owner-drawn, no checkable glyph) so the user can SEE it isn't selectable; keep the ItemCheck refusal below
        // as a backstop (a click that still reaches it is reverted to Unchecked). Keys are STABLE ids (FIX 4).
        _excludeList.DrawItem += (a, e) => DrawExcludeItem(e, primaryStableEx);
        _excludeList.ItemCheck += (a, e) =>
        {
            var id = _monitors[e.Index].StableId;
            if (id == primaryStableEx) { e.NewValue = CheckState.Unchecked; return; }   // backstop
            if (e.NewValue == CheckState.Checked) _excluded.Add(id); else _excluded.Remove(id);
        };
        tabMonitors.Controls.Add(_excludeList);

        if (_bottoms.Count > 0 && _tops.Count > 0) { _fromCombo.SelectedIndex = 0; }

        // ---- buttons ----
        var cancel = new Button { Text = Strings.Cancel, Left = ClientSize.Width - 12 - 96, Top = 664, Width = 96, Height = 30, FlatStyle = FlatStyle.Flat, DialogResult = DialogResult.Cancel };
        var ok = new Button { Text = Strings.Save, Left = cancel.Left - 104, Top = 664, Width = 96, Height = 30, FlatStyle = FlatStyle.Flat, Tag = "primary", DialogResult = DialogResult.OK };
        ok.Click += Ok_Click;

        // back up / restore the whole configuration as a single settings.json-format file (e.g. before a format/reinstall)
        var export = new Button { Text = Strings.ExportLabel, Left = 12, Top = 664, Width = 110, Height = 30, FlatStyle = FlatStyle.Flat };
        export.Click += Export_Click;
        var import = new Button { Text = Strings.ImportLabel, Left = export.Left + 116, Top = 664, Width = 110, Height = 30, FlatStyle = FlatStyle.Flat };
        import.Click += Import_Click;

        Controls.Add(_tabs);
        Controls.Add(export);
        Controls.Add(import);
        Controls.Add(ok);
        Controls.Add(cancel);
        AcceptButton = ok;
        CancelButton = cancel;

        Result = s;
        UpdateHotkeyText();
        UpdateGameHotkeyText();
        UpdatePauseHotkeyText();
        UpdateSideHotkeyText();
        Theming.Apply(this, SelectedTheme());
    }

    private string MonitorLabel(MonitorInfo m)
    {
        return GuardCore.MonitorLabel(m.Index, m.Bounds.Width, m.Bounds.Height, m.Bounds.Left, m.Bounds.Top, m.Primary, Strings.MainTag);
    }

    private string SelectedFromKey() =>
        _fromCombo.SelectedIndex >= 0 && _fromCombo.SelectedIndex < _bottoms.Count ? _bottoms[_fromCombo.SelectedIndex].StableId : null;

    private void LoadTopsForSelectedFrom()
    {
        string from = SelectedFromKey();
        _rules.TryGetValue(from ?? "", out var allowed);
        for (int i = 0; i < _topsCheck.Items.Count; i++)
        {
            bool on = allowed != null && allowed.Contains(_tops[i].StableId);
            _topsCheck.SetItemChecked(i, on);
        }
    }

    private void TopsCheck_ItemCheck(object sender, ItemCheckEventArgs e)
    {
        string from = SelectedFromKey();
        if (from == null) return;
        if (!_rules.TryGetValue(from, out var set)) { set = new HashSet<string>(); _rules[from] = set; }
        string to = _tops[e.Index].StableId;
        if (e.NewValue == CheckState.Checked) set.Add(to); else set.Remove(to);
        if (_map.IsHandleCreated) _map.Invalidate();
    }

    private void ResetLayout_Click(object sender, EventArgs e)
    {
        _modeAuto.Checked = true;
        _modeManual.Checked = false;
        _monitorList.Enabled = false;
        for (int i = 0; i < _monitorList.Items.Count; i++)
            _monitorList.SetItemChecked(i, MonitorInfo.IsAbovePrimary(_monitors[i]));
        _rules.Clear();
        for (int i = 0; i < _topsCheck.Items.Count; i++)
            _topsCheck.SetItemChecked(i, false);
        _map.Invalidate();
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

        string from = SelectedFromKey();
        _rules.TryGetValue(from ?? "", out var allowed);
        var topSet = new HashSet<string>(_tops.Select(m => m.StableId));

        using var penEdge = new Pen(t.Border);
        using var fontN = new Font("Segoe UI", 7.5f);
        foreach (var m in _monitors)
        {
            var r = new RectangleF(ox + (m.Bounds.Left - vl) * scale, oy + (m.Bounds.Top - vt) * scale, m.Bounds.Width * scale, m.Bounds.Height * scale);
            r.Inflate(-1, -1);
            Color fill = t.Surface;
            if (m.StableId == from) fill = ControlPaint.Light(t.Accent, 0.2f);
            else if (allowed != null && allowed.Contains(m.StableId)) fill = t.Accent;
            else if (topSet.Contains(m.StableId)) fill = ControlPaint.Dark(t.Surface, 0.06f);
            using var b = new SolidBrush(fill);
            g.FillRectangle(b, r);
            g.DrawRectangle(penEdge, r.X, r.Y, r.Width, r.Height);
            bool brightFill = (allowed != null && allowed.Contains(m.StableId));
            using var tb = new SolidBrush(brightFill ? Color.White : t.Text);
            g.DrawString(m.Index.ToString(), fontN, tb, r.X + 2, r.Y + 1);
        }
    }

    // Owner-draw the exclude list (FIX 2): the primary row is greyed and its checkbox drawn DISABLED so the user can
    // see it is not selectable; every other row draws a normal enabled checkbox reflecting its checked state.
    private void DrawExcludeItem(DrawItemEventArgs e, string primaryStable)
    {
        if (e.Index < 0) return;
        var t = SelectedTheme();
        var m = _monitors[e.Index];
        bool isPrimary = m.StableId == primaryStable;
        bool selected = (e.State & DrawItemState.Selected) != 0 && !isPrimary;
        bool isChecked = _excludeList.GetItemChecked(e.Index);

        using (var bg = new SolidBrush(selected ? t.Accent : t.Surface))
            e.Graphics.FillRectangle(bg, e.Bounds);

        // Checkbox glyph, vertically centred at the left of the row.
        int box = 14;
        int by = e.Bounds.Top + (e.Bounds.Height - box) / 2;
        var boxRect = new Rectangle(e.Bounds.Left + 3, by, box, box);
        var state = isPrimary
            ? System.Windows.Forms.VisualStyles.CheckBoxState.UncheckedDisabled
            : (isChecked ? System.Windows.Forms.VisualStyles.CheckBoxState.CheckedNormal
                         : System.Windows.Forms.VisualStyles.CheckBoxState.UncheckedNormal);
        CheckBoxRenderer.DrawCheckBox(e.Graphics, boxRect.Location, state);

        // Label: greyed (subtle) for the disabled primary row, normal/selected colour otherwise.
        Color fg = isPrimary ? t.Subtle : (selected ? Color.White : t.Text);
        var textRect = new Rectangle(boxRect.Right + 4, e.Bounds.Top, e.Bounds.Width - boxRect.Right - 4, e.Bounds.Height);
        TextRenderer.DrawText(e.Graphics, _excludeList.Items[e.Index].ToString(), _excludeList.Font, textRect, fg,
            TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis);
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

    private void PauseHotkeyBox_KeyDown(object sender, KeyEventArgs e)
    {
        e.SuppressKeyPress = true;
        e.Handled = true;
        _pc = e.Control; _pa = e.Alt; _ps = e.Shift;
        var kc = e.KeyCode;
        if (kc is Keys.ControlKey or Keys.Menu or Keys.ShiftKey) { _pkey = Keys.None; UpdatePauseHotkeyText(); return; }
        if (kc is Keys.LWin or Keys.RWin) { _pw = true; _pkey = Keys.None; UpdatePauseHotkeyText(); return; }
        _pkey = kc;
        UpdatePauseHotkeyText();
    }

    private void UpdatePauseHotkeyText()
    {
        var parts = new List<string>();
        if (_pc) parts.Add("Ctrl");
        if (_pa) parts.Add("Alt");
        if (_ps) parts.Add("Shift");
        if (_pw) parts.Add("Win");
        if (_pkey != Keys.None) parts.Add(_pkey.ToString());
        _pauseHotkeyBox.Text = parts.Count == 0 ? Strings.HotkeyPressKeys : string.Join(" + ", parts);
    }

    private void SideHotkeyBox_KeyDown(object sender, KeyEventArgs e)
    {
        e.SuppressKeyPress = true;
        e.Handled = true;
        _sc = e.Control; _sa = e.Alt; _ss = e.Shift;
        var kc = e.KeyCode;
        if (kc is Keys.ControlKey or Keys.Menu or Keys.ShiftKey) { _skey = Keys.None; UpdateSideHotkeyText(); return; }
        if (kc is Keys.LWin or Keys.RWin) { _sw = true; _skey = Keys.None; UpdateSideHotkeyText(); return; }
        _skey = kc;
        UpdateSideHotkeyText();
    }

    private void UpdateSideHotkeyText()
    {
        var parts = new List<string>();
        if (_sc) parts.Add("Ctrl");
        if (_sa) parts.Add("Alt");
        if (_ss) parts.Add("Shift");
        if (_sw) parts.Add("Win");
        if (_skey != Keys.None) parts.Add(_skey.ToString());
        _sideHotkeyBox.Text = parts.Count == 0 ? Strings.HotkeyPressKeys : string.Join(" + ", parts);
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

        Result = BuildResult();
    }

    /// <summary>Snapshot the current dialog selections into a <see cref="Settings"/> (shared by Save and Export).</summary>
    private Settings BuildResult()
    {
        var res = new Settings
        {
            ModCtrl = _mc, ModAlt = _ma, ModShift = _ms, ModWin = _mw, HotKey = _key,
            StartGateClosed = _startActive.Checked,
            AutoStart = _autoStart.Checked,
            Mode = _modeManual.Checked ? "Manual" : "AutoTop",
            Language = _langCombo.SelectedIndex switch { 1 => "en", 2 => "tr", _ => "auto" },
            Theme = _themeCombo.SelectedIndex switch { 1 => "Light", 2 => "Dark", _ => "System" },
            ConfineModCtrl = _gc, ConfineModAlt = _ga, ConfineModShift = _gs, ConfineModWin = _gw, ConfineHotKey = _gkey,
            PauseModCtrl = _pc, PauseModAlt = _pa, PauseModShift = _ps, PauseModWin = _pw, PauseHotKey = _pkey,
            SideModCtrl = _sc, SideModAlt = _sa, SideModShift = _ss, SideModWin = _sw, SideHotKey = _skey,
            StartSideContainOn = _startSideActive.Checked,
            SideCrossMin = (int)_sideSensNum.Value,
            SideCrossSlack = _orig.SideCrossSlack,   // not surfaced in the UI — preserve whatever was loaded/imported
            DeliberateCross = _deliberateCheck.Checked,
            DescentRouting = _descentCheck.Checked,
        };

        if (res.Mode == "Manual")
            for (int i = 0; i < _monitorList.Items.Count; i++)
                if (_monitorList.GetItemChecked(i))
                    res.ManualMonitors.Add(_monitors[i].StableId);

        foreach (var kv in _rules)
            foreach (var to in kv.Value)
                res.UpLinks.Add(new UpLink { FromDevice = kv.Key, ToDevice = to });

        // Carry the walled-off displays so a Save/Export doesn't silently drop them (binding fix #5).
        res.ExcludedDevices.AddRange(_excluded);

        return res;
    }

    // Export the current dialog state to a settings.json-format file the user picks.
    private void Export_Click(object sender, EventArgs e)
    {
        using var dlg = new SaveFileDialog
        {
            Title = Strings.ExportTitle,
            Filter = Strings.SettingsFileFilter,
            FileName = "MouseFence-settings.json",
            DefaultExt = "json",
            AddExtension = true,
        };
        if (dlg.ShowDialog(this) != DialogResult.OK) return;
        try
        {
            var json = JsonSerializer.Serialize(BuildResult(), new JsonSerializerOptions { WriteIndented = true });
            File.WriteAllText(dlg.FileName, json);
        }
        catch (Exception ex)
        {
            MessageBox.Show(this, ex.Message, "MouseFence", MessageBoxButtons.OK, MessageBoxIcon.Warning);
        }
    }

    // Import a settings.json-format file: validate, confirm, then hand it back through the normal Save path
    // (tray persists it and reconfigures the barrier). Same format as Export -> a round-trip just works.
    private void Import_Click(object sender, EventArgs e)
    {
        using var dlg = new OpenFileDialog
        {
            Title = Strings.ImportTitle,
            Filter = Strings.SettingsFileFilter,
            CheckFileExists = true,
        };
        if (dlg.ShowDialog(this) != DialogResult.OK) return;

        Settings imported;
        string json;
        try
        {
            json = File.ReadAllText(dlg.FileName);
            imported = JsonSerializer.Deserialize<Settings>(json);
        }
        catch
        {
            json = "";
            imported = null;
        }
        if (imported == null)
        {
            MessageBox.Show(this, Strings.ImportInvalid, "MouseFence", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            return;
        }
        if (MessageBox.Show(this, Strings.ImportConfirm, "MouseFence", MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes)
            return;

        if (Settings.ReadVersion(json) < Settings.CurrentSettingsVersion)
        {
            imported.MigrateLayoutKeysToStable(_monitors);
            imported.SettingsVersion = Settings.CurrentSettingsVersion;
        }
        Result = imported;
        DialogResult = DialogResult.OK;   // closes the dialog; the tray saves + reconfigures via the existing path
    }
}
