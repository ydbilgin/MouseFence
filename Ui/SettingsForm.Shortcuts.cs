namespace MouseFence;

public sealed partial class SettingsForm
{
    private void BuildShortcutsTab(TabPage page)
    {
        var flow = PageFlow(page);
        flow.Controls.Add(PageHeader(Strings.Ui("NavShortcuts"), Strings.Ui("ShortcutSubtitle"), 800));
        var header = new TableLayoutPanel { Width = 800, Height = 32, ColumnCount = 3, Margin = Padding.Empty, Tag = "listHeader" };
        header.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100)); header.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 238)); header.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 72));
        header.Controls.Add(new Label { Text = Strings.Ui("ShortcutActionColumn"), Dock = DockStyle.Fill, Tag = "faint", TextAlign = ContentAlignment.MiddleLeft }, 0, 0);
        header.Controls.Add(new Label { Text = Strings.Ui("ShortcutColumn"), Dock = DockStyle.Fill, Tag = "faint", TextAlign = ContentAlignment.MiddleLeft }, 1, 0);
        header.Controls.Add(new Label { Text = Strings.Clear, Dock = DockStyle.Fill, Tag = "faint", TextAlign = ContentAlignment.MiddleCenter }, 2, 0);
        flow.Controls.Add(header);
        var specs = new[]
        {
            (Strings.HotkeyHead, Strings.HotkeyHint), (Strings.GameHotkeyHead, Strings.GameHotkeyHint),
            (Strings.PauseHotkeyHead, Strings.PauseHotkeyHint), (Strings.SideHotkeyHead, Strings.SideHotkeyHint),
        };
        for (int i = 0; i < specs.Length; i++)
        {
            int index = i;
            var row = new TableLayoutPanel { Width = 800, Height = 83, ColumnCount = 3, Margin = Padding.Empty, Tag = "listRow", Padding = new Padding(0, 8, 0, 8) };
            row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 238)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 72));
            row.Paint += PaintListSeparator;
            var copy = new Panel { Dock = DockStyle.Fill };
            var required = i == 0 ? Strings.Ui("RequiredStatus") : Strings.Ui("OptionalStatus");
            var head = new Label { Text = specs[i].Item1 + "  ·  " + required, Left = 0, Top = 4, Width = 440, Height = 22, Font = new Font("Segoe UI Semibold", 9.5f) };
            var help = new Label { Text = specs[i].Item2 + (i == 1 ? " " + Strings.Ui("GameModeSuffix") : ""), Left = 0, Top = 27, Width = 445, Height = 36, Tag = "subtle" };
            copy.Controls.Add(head); copy.Controls.Add(help);
            var box = new TextBox { ReadOnly = true, TextAlign = HorizontalAlignment.Center, Cursor = Cursors.Hand, Font = new Font("Segoe UI Semibold", 9f), BorderStyle = BorderStyle.None };
            box.Text = HotkeyText(index); box.KeyDown += (_, e) => CaptureHotkey(index, e);
            var frame = new ThemedControlFrame { Dock = DockStyle.Fill, Height = 36, Margin = new Padding(0, 14, 8, 14), Tag = "shortcutFrame" }; frame.Host(box);
            var clear = new ThemedButton { Text = Strings.Clear, Dock = DockStyle.Fill, Height = 36, Role = ThemedButtonRole.Quiet, Margin = new Padding(4, 14, 0, 14) };
            clear.Click += (_, _) => { ClearHotkey(index); box.Text = HotkeyText(index); UpdateShortcutErrors(); };
            if (i == 0) { clear.Enabled = false; clear.AccessibleName = Strings.Ui("RequiredClearReason"); _tips.SetToolTip(clear, Strings.Ui("RequiredClearReason")); }
            row.Controls.Add(copy, 0, 0); row.Controls.Add(frame, 1, 0); row.Controls.Add(clear, 2, 0);
            flow.Controls.Add(row); _hotkeyBoxes[i] = box;
        }
        flow.Controls.Add(HelpLabel(Strings.Ui("ShortcutsHealthy"), 800, 32));
    }

    private void CaptureHotkey(int index, KeyEventArgs e)
    {
        e.SuppressKeyPress = true; e.Handled = true;
        Keys key = e.KeyCode is Keys.ControlKey or Keys.Menu or Keys.ShiftKey or Keys.LWin or Keys.RWin ? Keys.None : e.KeyCode;
        bool win = e.KeyCode is Keys.LWin or Keys.RWin;
        SetHotkey(index, key, e.Control, e.Alt, e.Shift, win);
        _hotkeyBoxes[index].Text = HotkeyText(index);
        UpdateShortcutErrors();
    }

    private void SetHotkey(int i, Keys key, bool ctrl, bool alt, bool shift, bool win)
    {
        switch (i)
        {
            case 0: _draft.HotKey = key; _draft.ModCtrl = ctrl; _draft.ModAlt = alt; _draft.ModShift = shift; _draft.ModWin = win; break;
            case 1: _draft.ConfineHotKey = key; _draft.ConfineModCtrl = ctrl; _draft.ConfineModAlt = alt; _draft.ConfineModShift = shift; _draft.ConfineModWin = win; break;
            case 2: _draft.PauseHotKey = key; _draft.PauseModCtrl = ctrl; _draft.PauseModAlt = alt; _draft.PauseModShift = shift; _draft.PauseModWin = win; break;
            case 3: _draft.SideHotKey = key; _draft.SideModCtrl = ctrl; _draft.SideModAlt = alt; _draft.SideModShift = shift; _draft.SideModWin = win; break;
        }
    }

    private void ClearHotkey(int i) => SetHotkey(i, Keys.None, false, false, false, false);
    private string HotkeyText(int i)
    {
        (Keys key, bool c, bool a, bool s, bool w) = i switch
        {
            0 => (_draft.HotKey, _draft.ModCtrl, _draft.ModAlt, _draft.ModShift, _draft.ModWin),
            1 => (_draft.ConfineHotKey, _draft.ConfineModCtrl, _draft.ConfineModAlt, _draft.ConfineModShift, _draft.ConfineModWin),
            2 => (_draft.PauseHotKey, _draft.PauseModCtrl, _draft.PauseModAlt, _draft.PauseModShift, _draft.PauseModWin),
            _ => (_draft.SideHotKey, _draft.SideModCtrl, _draft.SideModAlt, _draft.SideModShift, _draft.SideModWin),
        };
        var parts = new List<string>(); if (c) parts.Add("Ctrl"); if (a) parts.Add("Alt"); if (s) parts.Add("Shift"); if (w) parts.Add("Win"); if (key != Keys.None) parts.Add(key.ToString());
        return parts.Count == 0 ? Strings.HotkeyPressKeys : string.Join(" + ", parts);
    }

    private bool ValidPrimaryHotkey() => _draft.HotKey != Keys.None && (_draft.ModCtrl || _draft.ModAlt || _draft.ModShift || _draft.ModWin);

    private bool HasDuplicateHotkeys()
    {
        var chords = new[]
        {
            (_draft.HotKey, _draft.ModCtrl, _draft.ModAlt, _draft.ModShift, _draft.ModWin),
            (_draft.ConfineHotKey, _draft.ConfineModCtrl, _draft.ConfineModAlt, _draft.ConfineModShift, _draft.ConfineModWin),
            (_draft.PauseHotKey, _draft.PauseModCtrl, _draft.PauseModAlt, _draft.PauseModShift, _draft.PauseModWin),
            (_draft.SideHotKey, _draft.SideModCtrl, _draft.SideModAlt, _draft.SideModShift, _draft.SideModWin),
        }.Where(x => x.Item1 != Keys.None).ToList();
        return chords.Count != chords.Distinct().Count();
    }

    private void UpdateShortcutErrors()
    {
        foreach (var frame in _hotkeyBoxes.Where(box => box != null).Select(box => box.Parent).OfType<ThemedControlFrame>())
        {
            frame.Error = false; frame.AccessibleDescription = ""; frame.Invalidate();
        }
        var chordRows = Enumerable.Range(0, 4).Select(i => (Index: i, Text: HotkeyText(i), Name: ShortcutName(i)))
            .Where(item => !string.Equals(item.Text, Strings.HotkeyPressKeys, StringComparison.Ordinal)).ToList();
        var duplicate = chordRows.GroupBy(item => item.Text, StringComparer.OrdinalIgnoreCase)
            .Where(group => group.Count() > 1)
            .Select(group => (First: group.MinBy(item => item.Index), Later: group.MaxBy(item => item.Index)))
            .OrderBy(pair => pair.Later.Index).FirstOrDefault();
        if (duplicate.Later.Text != null)
        {
            string message = string.Format(Strings.Ui("DuplicateShortcutsNamed"), duplicate.First.Name, duplicate.Later.Name);
            if (_hotkeyBoxes[duplicate.Later.Index]?.Parent is ThemedControlFrame frame)
            {
                frame.Error = true; frame.AccessibleDescription = message; frame.Invalidate();
            }
            SetInlineStatus(message, true);
        }
    }

    private string ShortcutName(int index) => index switch
    {
        0 => Strings.HotkeyHead,
        1 => Strings.GameHotkeyHead,
        2 => Strings.PauseHotkeyHead,
        _ => Strings.SideHotkeyHead,
    };

}
