namespace MouseFence;

public sealed partial class SettingsForm
{
    private void RebuildScreenRows()
    {
        _rebuilding = true;
        _screenRows.SuspendLayout(); DisposeChildren(_screenRows); _screenRows.RowStyles.Clear(); _screenRows.RowCount = 0;
        _screenRows.ColumnCount = 1;
        int headerRow = _screenRows.RowCount++;
        _screenRows.RowStyles.Add(new RowStyle(SizeType.Absolute, 31));
        var header = ScreenListGrid(31);
        header.Tag = "listHeader";
        header.Controls.Add(ListHeader(Strings.Ui("ScreenColumn")), 0, 0);
        header.Controls.Add(ListHeader(Strings.Ui("NameColumn")), 1, 0);
        header.Controls.Add(ListHeader(Strings.Ui("ResolutionColumn")), 2, 0);
        header.Controls.Add(ListHeader(Strings.Ui("PointerAccessColumn")), 3, 0);
        _screenRows.Controls.Add(header, 0, headerRow);
        foreach (var monitor in _monitors.OrderBy(m => m.Index))
        {
            int row = _screenRows.RowCount++;
            _screenRows.RowStyles.Add(new RowStyle(SizeType.Absolute, 53));
            var panel = ScreenListGrid(53); panel.Tag = "screenRow"; panel.AccessibleName = ScreenName(monitor); panel.Paint += PaintListSeparator;
            var screenCell = new Panel { Dock = DockStyle.Fill };
            var chip = new NumberChip { Number = monitor.Index, Accent = monitor.Primary, Left = 0, Top = 10, Size = new Size(26, 26) };
            screenCell.Controls.Add(chip);
            var name = new TextBox { Text = Settings.MonitorNameFor(_draft.MonitorNames, monitor.StableId), BorderStyle = BorderStyle.None };
            name.HandleCreated += (_, _) => Native.SendMessage(name.Handle, Native.EM_SETCUEBANNER, (IntPtr)1, Strings.Ui("OptionalName"));
            name.TextChanged += (_, _) =>
            {
                if (_rebuilding) return;
                string clean = Settings.NormalizeMonitorName(name.Text);
                if (clean.Length == 0) _draft.MonitorNames.Remove(monitor.StableId); else _draft.MonitorNames[monitor.StableId] = clean;
                RefreshMap();
            };
            var nameFrame = new ThemedControlFrame { Dock = DockStyle.Fill, Height = 36, Margin = new Padding(0, 8, 12, 8) }; nameFrame.Host(name);
            var resolution = new Label { Dock = DockStyle.Fill, Text = $"{monitor.Bounds.Width} × {monitor.Bounds.Height}", Tag = "subtle", TextAlign = ContentAlignment.MiddleLeft };
            Control keep;
            if (monitor.Primary)
                keep = new Label { Text = Strings.Ui("MainAlwaysV2"), Dock = DockStyle.Fill, Tag = "accent", TextAlign = ContentAlignment.MiddleLeft };
            else
            {
                var check = new ThemedCheckBox { Text = Strings.Ui("KeepPointerOff"), Dock = DockStyle.Fill, Height = 32,
                    Checked = _draft.ExcludedDevices.Contains(monitor.StableId, StringComparer.OrdinalIgnoreCase) };
                check.CheckedChanged += (_, _) =>
                {
                    if (_rebuilding) return;
                    if (check.Checked) _draft.ExcludedDevices.Add(monitor.StableId);
                    else _draft.ExcludedDevices.RemoveAll(id => Same(id, monitor.StableId));
                    SanitizeExcluded(); RefreshMap(); RebuildScreenRows(); UpdateDrawer();
                };
                keep = check;
            }
            panel.Controls.Add(screenCell, 0, 0); panel.Controls.Add(nameFrame, 1, 0); panel.Controls.Add(resolution, 2, 0); panel.Controls.Add(keep, 3, 0);
            panel.Click += (_, _) => MonitorChosen(monitor);
            panel.MouseEnter += (_, _) => _map.ExternalHoveredStableId = monitor.StableId;
            panel.MouseLeave += (_, _) => _map.ExternalHoveredStableId = null;
            _screenRows.Controls.Add(panel, 0, row);
        }
        _screenRows.ResumeLayout(); _rebuilding = false;
        _screenRows.Height = Math.Max(1, 31 + Math.Max(0, _screenRows.RowCount - 1) * 53);
        ApplyTheme();
    }

    private void UpdateScreenRowPitch(bool narrow)
    {
        if (_screenRows.IsDisposed || _rebuilding) return;
        int pitch = DpiPx(narrow ? 72 : 53);
        if (_screenRows.RowStyles.Count < 2 || Math.Abs(_screenRows.RowStyles[1].Height - pitch) < 0.1f) return;
        for (int i = 1; i < _screenRows.RowStyles.Count; i++) _screenRows.RowStyles[i].Height = pitch;
        _screenRows.Height = Math.Max(1, DpiPx(31) + Math.Max(0, _screenRows.RowCount - 1) * pitch);
    }

    private void DisplaySettingsChanged(object sender, EventArgs e)
    {
        if (!IsHandleCreated || IsDisposed) return;
        BeginInvoke((Action)(() => { _displayRefreshTimer.Stop(); _displayRefreshTimer.Start(); }));
    }

    private void DisplayRefreshTimer_Tick(object sender, EventArgs e)
    {
        _displayRefreshTimer.Stop(); _monitors.Clear(); _monitors.AddRange(MonitorInfo.All()); SanitizeExcluded(); RebuildScreenRows(); RefreshMap(); CloseDrawer();
        SetInlineStatus(Strings.LayoutChangedBanner);
    }

    private bool _inResizeLayout;
    private void ResizeV2Layout()
    {
        if (_bodyFlow == null || _bodyFlow.IsDisposed) return;
        // Re-entrancy guard: this method sets child .Width/.Height, which re-fires the _bodyFlow/page
        // SizeChanged handlers that call back into here. Without this, one window-resize tick triggers a
        // storm of full relayouts (each doing whole-tree LINQ walks + grid rebuilds) → heavy drag lag.
        if (_inResizeLayout) return;
        _inResizeLayout = true;
        SuspendLayout();
        try
        {
        bool narrow = ClientSize.Width < DpiPx(900);
        int horizontal = DpiPx(narrow ? 18 : 28);
        int bodyViewport = Math.Max(DpiPx(420), _bodyFlow.ClientSize.Width - SystemInformation.VerticalScrollBarWidth - DpiPx(4));
        int width = Math.Max(DpiPx(420), Math.Min(DpiPx(WideContentMeasure), bodyViewport - horizontal * 2));
        int bodySpare = Math.Max(0, bodyViewport - width);
        int bodyLeft = Math.Max(horizontal, bodySpare / 2);
        int bodyRight = Math.Max(horizontal, bodySpare - bodyLeft);
        _bodyFlow.Padding = new Padding(bodyLeft, DpiPx(23), bodyRight, DpiPx(32));
        foreach (Control child in _bodyFlow.Controls) child.Width = width;
        LayoutScreenSectionText();
        _screenRows.Width = width;
        UpdateScreenRowPitch(narrow);
        ReflowScreenRows(narrow);
        foreach (var page in new[] { _advancedPage, _shortcutsPage, _generalPage })
        {
            int pageViewport = Math.Max(DpiPx(420), page.ClientSize.Width - SystemInformation.VerticalScrollBarWidth - DpiPx(4));
            int pageWidth = Math.Max(DpiPx(420), Math.Min(DpiPx(WideContentMeasure), pageViewport - horizontal * 2));
            int pageSpare = Math.Max(0, pageViewport - pageWidth);
            int pageLeft = Math.Max(horizontal, pageSpare / 2);
            int pageRight = Math.Max(horizontal, pageSpare - pageLeft);
            page.Padding = new Padding(pageLeft, DpiPx(23), pageRight, DpiPx(32));
            foreach (Control child in page.Controls) child.Width = pageWidth;
            if (page != _advancedPage)
            {
                var flow = page.Controls.OfType<FlowLayoutPanel>().FirstOrDefault();
                if (flow != null) foreach (Control child in flow.Controls) child.Width = pageWidth;
            }
        }
        int advancedContentWidth = Math.Max(DpiPx(420), _advancedPage.ClientSize.Width - _advancedPage.Padding.Horizontal - SystemInformation.VerticalScrollBarWidth - DpiPx(4));
        ResizeAdvanced(advancedContentWidth);
        var numericGrid = _advancedPanel.Controls.Cast<Control>().OfType<TableLayoutPanel>().FirstOrDefault(c => (c.Tag as string) == "numericGrid");
        if (numericGrid != null) ReflowGrid(numericGrid, narrow, Math.Max(DpiPx(420), _advancedWidth));
        var appearanceGrid = _generalPage.Controls.Cast<Control>().SelectMany(AllControls).OfType<TableLayoutPanel>().FirstOrDefault(c => (c.Tag as string) == "appearanceGrid");
        if (appearanceGrid != null) ReflowGrid(appearanceGrid, narrow,
            Math.Max(DpiPx(420), _generalPage.ClientSize.Width - _generalPage.Padding.Horizontal - SystemInformation.VerticalScrollBarWidth - DpiPx(4)));
        if (narrow)
        {
            _modeTiles.FlowDirection = FlowDirection.TopDown; _modeTiles.Height = DpiPx(250);
            foreach (ModeTile tile in _modeTiles.Controls) { tile.Width = Math.Max(DpiPx(220), width - DpiPx(28)); tile.Height = DpiPx(78); }
        }
        else
        {
            _modeTiles.FlowDirection = FlowDirection.LeftToRight; _modeTiles.Height = DpiPx(84);
        }
        int drawerHeight = RestingBoundaryDrawerHeight(width);
        int listHeight = _screenSection.Height + _screenSection.Margin.Vertical
            + _screenRows.Height + _screenRows.Margin.Vertical;
        var mapHeader = _bodyFlow.Controls.Cast<Control>().First(control => (control.Tag as string) == "mapHeader");
        int nonMapHeight = _bodyFlow.Padding.Vertical
            + mapHeader.Height + mapHeader.Margin.Vertical
            + _mapHint.Height + _mapHint.Margin.Vertical
            + Math.Max(drawerHeight, listHeight);
        int bodyHeight = _bodyFlow.ClientSize.Height;
        int leftoverHeight = bodyHeight - nonMapHeight;
        int preferredFloor = Math.Max(DpiPx(220), (int)Math.Round(bodyHeight * PreferredMapHeightRatio));
        int proportionalCeiling = Math.Max(preferredFloor, (int)Math.Round(bodyHeight * MaximumMapHeightRatio));
        _map.Height = Math.Clamp(leftoverHeight, preferredFloor, proportionalCeiling);
        if (_screenSplit.Visible) LayoutBoundaryDrawer();
        }
        finally { ResumeLayout(true); _inResizeLayout = false; }
    }

    private void LayoutScreenSectionText()
    {
        if (_screenSection == null || _screenSectionHelp == null) return;
        int gap = DpiPx(12), right = DpiPx(10);
        int titleWidth = MeasureSingleLineWidth(_screenSectionTitle);
        _screenSectionTitle.Width = Math.Min(titleWidth, Math.Max(DpiPx(80), _screenSection.ClientSize.Width / 2));
        int minimumLeft = _screenSectionTitle.Right + gap;
        FitRightAlignedText(_screenSectionHelp, minimumLeft, right);
    }

    private void LayoutDrawerQuestionText()
    {
        if (_overallLink?.Parent == null) return;
        int gap = DpiPx(12), right = DpiPx(4);
        FitRightAlignedText(_overallLink, Math.Min(DpiPx(260), _overallLink.Parent.ClientSize.Width / 2), right);
        _drawerDirection.Width = Math.Max(DpiPx(120), _overallLink.Left - gap - _drawerDirection.Left);
    }

    private void FitRightAlignedText(Label label, int minimumLeft, int rightPadding)
    {
        int available = Math.Max(DpiPx(80), label.Parent.ClientSize.Width - minimumLeft - rightPadding);
        label.Width = Math.Min(MeasureSingleLineWidth(label), available);
        label.Left = Math.Max(minimumLeft, label.Parent.ClientSize.Width - rightPadding - label.Width);
        int measuredHeight = TextRenderer.MeasureText(label.Text, label.Font, new Size(label.Width, 0),
            TextFormatFlags.WordBreak | TextFormatFlags.NoPrefix | TextFormatFlags.NoPadding).Height;
        label.Height = Math.Max(DpiPx(20), measuredHeight + DpiPx(2));
    }

    private int MeasureSingleLineWidth(Label label)
    {
        int rendered = TextRenderer.MeasureText(label.Text, label.Font, Size.Empty,
            TextFormatFlags.SingleLine | TextFormatFlags.NoPrefix | TextFormatFlags.NoPadding).Width + DpiPx(4);
        return Math.Max(rendered, label.GetPreferredSize(Size.Empty).Width);
    }

    private void ReflowScreenRows(bool narrow)
    {
        var rows = _screenRows.Controls.OfType<TableLayoutPanel>().ToList();
        if (rows.Count == 0) return;
        rows[0].Visible = !narrow;
        foreach (var row in rows.Skip(1))
        {
            var screen = row.GetControlFromPosition(0, 0);
            var name = row.GetControlFromPosition(1, 0);
            var resolution = row.GetControlFromPosition(2, 0) ?? row.GetControlFromPosition(1, 1);
            var access = row.GetControlFromPosition(3, 0);
            if (screen == null || name == null || resolution == null || access == null) continue;
            row.SuspendLayout(); row.RowStyles.Clear(); row.ColumnStyles.Clear();
            if (narrow)
            {
                row.RowCount = 2; row.RowStyles.Add(new RowStyle(SizeType.Percent, 58)); row.RowStyles.Add(new RowStyle(SizeType.Percent, 42));
                row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 62)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
                row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 0)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 176));
                row.SetCellPosition(screen, new TableLayoutPanelCellPosition(0, 0)); row.SetRowSpan(screen, 2);
                row.SetCellPosition(name, new TableLayoutPanelCellPosition(1, 0));
                row.SetCellPosition(resolution, new TableLayoutPanelCellPosition(1, 1));
                row.SetCellPosition(access, new TableLayoutPanelCellPosition(3, 0)); row.SetRowSpan(access, 2);
            }
            else
            {
                row.RowCount = 1; row.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
                row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 70)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
                row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 128)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 190));
                row.SetRowSpan(screen, 1); row.SetRowSpan(access, 1);
                row.SetCellPosition(screen, new TableLayoutPanelCellPosition(0, 0)); row.SetCellPosition(name, new TableLayoutPanelCellPosition(1, 0));
                row.SetCellPosition(resolution, new TableLayoutPanelCellPosition(2, 0)); row.SetCellPosition(access, new TableLayoutPanelCellPosition(3, 0));
            }
            row.ResumeLayout(true);
        }
    }

    private static void ReflowGrid(TableLayoutPanel grid, bool narrow, int width)
    {
        var controls = grid.Controls.Cast<Control>().ToList();
        grid.SuspendLayout(); grid.ColumnStyles.Clear(); grid.RowStyles.Clear(); grid.Width = width;
        grid.ColumnCount = narrow ? 1 : 2;
        for (int i = 0; i < grid.ColumnCount; i++) grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100f / grid.ColumnCount));
        grid.RowCount = narrow ? controls.Count : (controls.Count + 1) / 2;
        if ((grid.Tag as string) == "appearanceGrid") grid.Height = narrow ? 156 : 78;
        for (int i = 0; i < controls.Count; i++) grid.SetCellPosition(controls[i], new TableLayoutPanelCellPosition(narrow ? 0 : i % 2, narrow ? i : i / 2));
        grid.ResumeLayout(true);
    }

    private void ReserveDrawerStatusHeight()
    {
        if (_drawerStatus == null) return;
        int width = Math.Max(DpiPx(220), _drawerStatus.Width);
        _drawerStatus.Height = DrawerStatusCandidates().Max(text => TextRenderer.MeasureText(text, _drawerStatus.Font,
            new Size(width, 0), TextFormatFlags.WordBreak | TextFormatFlags.NoPrefix).Height) + DpiPx(4);
    }

    private string[] DrawerStatusCandidates()
    {
        string longestName = _monitors.Select(ScreenName).OrderByDescending(n => n?.Length ?? 0).FirstOrDefault() ?? "";
        return new[]
        {
            Strings.Ui("StatusUpClosed"), Strings.Ui("StatusSideOff"),
            string.Format(Strings.Ui("StatusSafety"), longestName), Strings.Ui("BlockedByKeepOut"),
            string.Format(Strings.Ui("ExcludedTargetExplanation"), longestName),
            Strings.Ui("SavedInactive"), " "
        };
    }

}
