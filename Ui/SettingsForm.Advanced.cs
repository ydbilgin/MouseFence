namespace MouseFence;

public sealed partial class SettingsForm
{
    private FlowLayoutPanel BuildAdvancedPanel()
    {
        var flow = new FlowLayoutPanel
        {
            Dock = DockStyle.Top, AutoSize = true, AutoSizeMode = AutoSizeMode.GrowAndShrink, Width = 800,
            FlowDirection = FlowDirection.TopDown, WrapContents = false,
            Padding = Padding.Empty, Margin = Padding.Empty,
        };
        flow.Controls.Add(PageHeader(Strings.Ui("NavAdvanced"), Strings.Ui("AdvancedSubtitle"), 800));
        AddSectionHeading(flow, Strings.Ui("DefaultBoundaryHead"));
        flow.Controls.Add(DefaultRow(Strings.Ui("UpwardDefault"), _draft.UpwardBoundaryDefault, v => _draft.UpwardBoundaryDefault = v, true));
        flow.Controls.Add(DefaultRow(Strings.Ui("SideDefault"), _draft.SideBoundaryDefault, v => _draft.SideBoundaryDefault = v, true));
        flow.Controls.Add(DefaultRow(Strings.Ui("DownDefault"), _draft.DownwardBoundaryDefault, v => _draft.DownwardBoundaryDefault = v, false));

        AddSectionHeading(flow, Strings.Ui("CrossingFeelHead"));
        var deliberate = new ThemedCheckBox { Text = Strings.Ui("FirmPushRecommended"), Width = 790, Height = 32, Checked = _draft.DeliberateCross };
        deliberate.CheckedChanged += (_, _) => _draft.DeliberateCross = deliberate.Checked; flow.Controls.Add(deliberate);
        flow.Controls.Add(HelpLabel(Strings.Ui("CrossingFeelSectionHelp"), 790, 34));
        flow.Controls.Add(_advancedToggle);
        var numericGrid = new TableLayoutPanel { Width = 790, AutoSize = true, ColumnCount = 2, Tag = "numericGrid", Margin = Padding.Empty };
        numericGrid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50)); numericGrid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        var numericRows = new[]
        {
            NumericRow(Strings.Ui("UpStrength"), Strings.Ui("UpStrengthHelp"), 1, 200, _draft.UpCrossMin, Settings.DefaultUpCrossMin, v => _draft.UpCrossMin = v),
            NumericRow(Strings.Ui("UpSlack"), Strings.Ui("UpSlackHelp"), 0, 200, _draft.UpCrossSlack, Settings.DefaultUpCrossSlack, v => _draft.UpCrossSlack = v),
            NumericRow(Strings.Ui("SideStrength"), Strings.Ui("SideStrengthHelp"), 1, 200, _draft.SideCrossMin, Settings.DefaultSideCrossMin, v => _draft.SideCrossMin = v),
            NumericRow(Strings.Ui("SideSlackM2"), Strings.Ui("SideSlackHelpM2"), 0, 200, _draft.SideCrossSlack, Settings.DefaultSideCrossSlack, v => _draft.SideCrossSlack = v),
            NumericRow(Strings.Ui("PassageInset"), Strings.Ui("PassageInsetHelp"), 0, 500, _draft.PassageInsetPx, Settings.DefaultPassageInsetPx, v => _draft.PassageInsetPx = v),
            NumericRow(Strings.Ui("EdgeHoldback"), Strings.Ui("EdgeHoldbackHelpV2"), 0, 200, _draft.EdgeHoldbackPx, Settings.DefaultEdgeHoldbackPx, v => _draft.EdgeHoldbackPx = v),
            NumericRow(Strings.Ui("SeamTolerance"), Strings.Ui("SeamToleranceHelp"), 0, 200, _draft.SeamTolerancePx, Settings.DefaultSeamTolerancePx, v => _draft.SeamTolerancePx = v),
        };
        for (int i = 0; i < numericRows.Length; i++) { numericGrid.RowStyles.Add(new RowStyle(SizeType.AutoSize)); numericGrid.Controls.Add(numericRows[i], i % 2, i / 2); }
        flow.Controls.Add(numericGrid);

        AddSectionHeading(flow, Strings.Ui("SafetyHead"));
        var safety = new ThemedCheckBox { Text = Strings.Ui("SafetyEscapes"), Width = 790, Height = 32, Checked = _draft.SafetyEscapes };
        safety.CheckedChanged += (_, _) => { _draft.SafetyEscapes = safety.Checked; RefreshMap(); };
        flow.Controls.Add(safety); flow.Controls.Add(HelpLabel(Strings.Ui("SafetyEscapesHelp"), 790));
        var descent = new ThemedCheckBox { Text = Strings.Ui("GuideDownward"), Width = 790, Height = 32, Checked = _draft.DescentRouting };
        descent.CheckedChanged += (_, _) => _draft.DescentRouting = descent.Checked;
        flow.Controls.Add(descent); flow.Controls.Add(HelpLabel(Strings.Ui("GuideDownwardHelp"), 790));

        AddSectionHeading(flow, Strings.Ui("UpperGroupHead"));
        var auto = new RadioButton { Text = Strings.Ui("UpperAutomatic"), Width = 790, Height = 28, Checked = _draft.Mode != "Manual" };
        int chooseCount = _monitors.Count(m => !m.Primary && !_draft.ExcludedDevices.Contains(m.StableId, StringComparer.OrdinalIgnoreCase));
        var manual = new RadioButton { Text = string.Format(Strings.Ui("UpperManualV2"), chooseCount), Width = 790, Height = 28, Checked = _draft.Mode == "Manual" };
        flow.Controls.Add(auto); flow.Controls.Add(manual);
        var manualList = new FlowLayoutPanel { Width = 790, AutoSize = true, FlowDirection = FlowDirection.TopDown, WrapContents = false, Padding = new Padding(24, 0, 0, 8) };
        foreach (var monitor in _monitors.Where(m => !m.Primary))
        {
            bool unusual = !MonitorInfo.IsAbovePrimary(monitor);
            var check = new ThemedCheckBox { Width = 750, Height = 32, Checked = _draft.ManualMonitors.Contains(monitor.StableId, StringComparer.OrdinalIgnoreCase),
                Enabled = !_draft.ExcludedDevices.Contains(monitor.StableId, StringComparer.OrdinalIgnoreCase),
                Text = ScreenName(monitor) + (unusual ? " · " + Strings.Ui("UnusualUpper") : "") };
            check.CheckedChanged += (_, _) => { if (check.Checked) AddUnique(_draft.ManualMonitors, monitor.StableId); else _draft.ManualMonitors.RemoveAll(x => Same(x, monitor.StableId)); RefreshMap(); };
            manualList.Controls.Add(check);
        }
        auto.CheckedChanged += (_, _) => { if (auto.Checked) { _draft.Mode = "AutoTop"; manualList.Enabled = false; RefreshMap(); } };
        manual.CheckedChanged += (_, _) => { if (manual.Checked) { _draft.Mode = "Manual"; manualList.Enabled = true; RefreshMap(); } };
        manualList.Enabled = manual.Checked; flow.Controls.Add(manualList);

        AddSectionHeading(flow, Strings.Ui("ResetBehavior"));
        flow.Controls.Add(HelpLabel(Strings.Ui("ResetBehaviorHelp"), 790, 52));
        var reset = new ThemedButton { Text = Strings.Ui("ResetBehavior"), Width = 286, Height = 36, Role = ThemedButtonRole.DangerText }; reset.Click += (_, _) =>
        {
            _draft.ResetLayoutConfig(); SetInlineStatus(Strings.Ui("ResetDone")); RefreshMap(); RebuildAdvancedContents();
        };
        flow.Controls.Add(reset);
        return flow;
    }

    private void ResizeAdvanced(int width)
    {
        if (_advancedPanel == null) return;
        _advancedWidth = Math.Max(DpiPx(420), width);
        _advancedPanel.Width = _advancedWidth;
        foreach (Control child in _advancedPanel.Controls)
        {
            child.Margin = new Padding(0, child.Margin.Top, 0, child.Margin.Bottom);
            child.Width = _advancedWidth;
            if (child is FlowLayoutPanel manualList)
            {
                foreach (Control option in manualList.Controls)
                    option.Width = Math.Max(DpiPx(200), _advancedWidth - manualList.Padding.Horizontal);
            }
        }
        _advancedPanel.PerformLayout();
    }

    private Control DefaultRow(string label, BoundaryDefault value, Action<BoundaryDefault> set, bool automatic)
    {
        var row = new TableLayoutPanel { Width = 790, Height = 44, ColumnCount = 2, Margin = Padding.Empty };
        row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 250));
        row.Controls.Add(new Label { Text = label, Dock = DockStyle.Fill, TextAlign = ContentAlignment.MiddleLeft }, 0, 0);
        var combo = new ThemedComboBox { Dock = DockStyle.Fill, Height = 36, Margin = new Padding(0, 4, 0, 4) };
        if (automatic) combo.Items.Add(Strings.Ui("DefaultAutomatic"));
        combo.Items.AddRange(new object[] { Strings.Ui("DefaultFree"), Strings.Ui("DefaultPush"), Strings.Ui("DefaultBlocked") });
        var values = automatic ? new[] { BoundaryDefault.Automatic, BoundaryDefault.Free, BoundaryDefault.Push, BoundaryDefault.Blocked }
                               : new[] { BoundaryDefault.Free, BoundaryDefault.Push, BoundaryDefault.Blocked };
        combo.SelectedIndex = Math.Max(0, Array.IndexOf(values, value));
        combo.SelectedIndexChanged += (_, _) => { if (combo.SelectedIndex >= 0) { set(values[combo.SelectedIndex]); RefreshMap(); } };
        row.Controls.Add(combo, 1, 0); return row;
    }

    private Control NumericRow(string label, string help, int min, int max, int value, int defaultValue, Action<int> set)
    {
        int helpHeight = Math.Max(36, TextRenderer.MeasureText(help, Font, new Size(335, 0), TextFormatFlags.WordBreak).Height + 4);
        const int labelWidth = 190;
        int labelHeight = Math.Max(28, TextRenderer.MeasureText(label, Font, new Size(labelWidth, 0),
            TextFormatFlags.WordBreak | TextFormatFlags.NoPrefix | TextFormatFlags.NoPadding).Height + 4);
        int helpTop = Math.Max(42, 5 + labelHeight + 4);
        var panel = new Panel { Width = 385, Height = helpTop + helpHeight, Tag = "advancedNumeric", Margin = new Padding(0, 0, 10, 8) };
        panel.Controls.Add(new Label { Text = label, Left = 0, Top = 5, Width = labelWidth, Height = labelHeight });
        var number = new ThemedNumericUpDown { Dock = DockStyle.Fill, Minimum = min, Maximum = max, Value = Math.Clamp(value, min, max) };
        number.ValueChanged += (_, _) => { set((int)number.Value); RefreshMap(); };
        var numberHost = new Panel { Left = 194, Top = 2, Width = 82, Height = 36, Tag = "surface2" };
        numberHost.Controls.Add(number);
        var spinnerRail = new ThemedSpinnerRail { Owner = number, Dock = DockStyle.Right, Width = 20 };
        numberHost.Controls.Add(spinnerRail); spinnerRail.BringToFront();
        var reset = new ThemedButton { Text = Strings.Ui("UseDefault"), Width = 98, Height = 32, Left = 282, Top = 2, Role = ThemedButtonRole.Quiet, Tag = "numericReset" }; reset.Click += (_, _) => number.Value = defaultValue;
        panel.Controls.Add(numberHost); panel.Controls.Add(reset);
        panel.Controls.Add(new Label { Text = help, Left = 0, Top = helpTop, Width = 375, Height = helpHeight, Tag = "subtle" });
        return panel;
    }

    private void ToggleAdvanced(bool? force = null)
    {
        _advancedExpanded = force ?? !_advancedExpanded;
        var numericGrid = _advancedPanel?.Controls.Cast<Control>().FirstOrDefault(c => (c.Tag as string) == "numericGrid");
        if (numericGrid != null) numericGrid.Visible = _advancedExpanded;
        _advancedToggle.Text = (_advancedExpanded ? "⌄  " : "›  ") + Strings.Ui("FineTunePixels");
        if (_advancedExpanded)
        {
            _advancedPanel.PerformLayout();
            _advancedPage.ScrollControlIntoView(_advancedToggle);
        }
    }

    private void RebuildAdvancedContents()
    {
        bool visible = _advancedPanel.Visible;
        using var fresh = BuildAdvancedPanel();
        _advancedPanel.SuspendLayout(); _advancedPanel.Controls.Clear();
        while (fresh.Controls.Count > 0)
        {
            var child = fresh.Controls[0]; fresh.Controls.Remove(child); _advancedPanel.Controls.Add(child);
        }
        _advancedPanel.Visible = visible; _advancedPanel.ResumeLayout(true); ResizeAdvanced(_advancedWidth); ApplyTheme();
    }

}
