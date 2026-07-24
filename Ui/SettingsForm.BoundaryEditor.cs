namespace MouseFence;

public sealed partial class SettingsForm
{
    private void OpenBoundary(BoundaryView boundary)
    {
        if (boundary == null) return;
        _boundary = boundary; _map.SelectedBoundary = boundary;
        ConfigureEndpoints();
        _draft.BoundaryTipSeen = true;
        if (_inlineStatus.Text == Strings.Ui("BoundaryTip")) SetInlineStatus(Strings.Ui("UnsavedFooter"));
        _screenSplit.Visible = true;
        LayoutBoundaryDrawer();
        UpdateDrawer();
        BeginInvoke((Action)(() =>
        {
            _bodyFlow.ScrollControlIntoView(_screenSplit);
            Rectangle hint = _bodyFlow.RectangleToClient(_mapHint.RectangleToScreen(_mapHint.ClientRectangle));
            if (hint.Top < 0) _bodyFlow.VerticalScroll.Value = Math.Max(0, _bodyFlow.VerticalScroll.Value + hint.Top);
        }));
    }

    private void ConfigureEndpoints()
    {
        if (_boundary.VerticalConnection)
        {
            bool firstUp = _boundary.FirstToSecondDirection == BoundaryDirection.Up;
            _primary = firstUp
                ? new(_boundary.First, _boundary.Second, BoundaryDirection.Up, _boundary.FirstToSecond)
                : new(_boundary.Second, _boundary.First, BoundaryDirection.Up, _boundary.SecondToFirst);
            _reverse = firstUp
                ? new(_boundary.Second, _boundary.First, BoundaryDirection.Down, _boundary.SecondToFirst)
                : new(_boundary.First, _boundary.Second, BoundaryDirection.Down, _boundary.FirstToSecond);
        }
        else
        {
            _primary = new(_boundary.First, _boundary.Second, _boundary.FirstToSecondDirection, _boundary.FirstToSecond);
            _reverse = new(_boundary.Second, _boundary.First, _boundary.SecondToFirstDirection, _boundary.SecondToFirst);
        }
    }

    private void UpdateDrawer()
    {
        if (_boundary == null) return;
        _rebuilding = true;
        _endpointFrom.Text = ChipText(_primary.From);
        _endpointTo.Text = ChipText(_primary.To);
        _pairSeparator.Text = _boundary.VerticalConnection ? "↕" : "↔";
        _drawerTitle.Text = Strings.Ui("DrawerTitle");
        _drawerMeta.Text = _boundary.GapPx > _draft.SeamTolerancePx
            ? string.Format(Strings.Ui("EditorMeetGap"), _boundary.GapPx) : Strings.Ui("EditorMeetTouching");
        bool horizontal = !_boundary.VerticalConnection;
        bool splitTruth = horizontal && DirectionsDiffer();
        if (splitTruth) _separateExpanded = true;
        _drawerDirection.Text = splitTruth ? Strings.Ui("DifferentByDirection") : horizontal ? Strings.Ui("BothDirections") : string.Format(Strings.Ui("GoingUpInto"), ScreenName(_primary.To));
        _returnLine.Text = horizontal ? "" : ResolvedBehavior(_reverse) switch
        {
            BoundaryBehavior.Blocked => Strings.Ui("ComingDownBlocked"),
            BoundaryBehavior.Push => Strings.Ui("ComingDownPush"),
            _ => Strings.Ui("ComingDownFree"),
        };
        _separateLink.Enabled = horizontal;
        if (!horizontal) _separateExpanded = false;
        _separateLink.Text = (_separateExpanded ? "⌄  " : "›  ") + (splitTruth ? Strings.Ui("DifferentByDirection") : Strings.Ui("SetDirectionsSeparately"));

        var excludedTarget = ExcludedBoundaryTarget();
        bool excluded = excludedTarget != null;
        _allowScreen.Enabled = excluded;
        _allowScreen.Role = excluded ? ThemedButtonRole.Primary : ThemedButtonRole.Quiet;
        _allowScreen.AccessibleDescription = excluded
            ? string.Format(Strings.Ui("ExcludedTargetExplanation"), ScreenName(excludedTarget)) : "";
        _allowScreen.Invalidate();
        _modeTiles.Enabled = !excluded;
        _overallLink.Enabled = !excluded;
        _separateLink.Enabled = horizontal && !excluded;
        _separatePanel.Enabled = !excluded;
        _whenLink.Enabled = !excluded;
        _automaticControl.Enabled = !excluded;
        _alwaysControl.Enabled = !excluded;
        _returnLine.Enabled = !excluded;
        if (excluded) _drawerDirection.Text = Strings.Ui("BlockedByKeepOut");

        var selected = ResolvedBehavior(_primary);
        _freeTile.Selected = !excluded && !splitTruth && selected == BoundaryBehavior.Free;
        _pushTile.Selected = !excluded && !splitTruth && selected is BoundaryBehavior.Push or BoundaryBehavior.OverallFeel;
        _blockedTile.Selected = !excluded && !splitTruth && selected == BoundaryBehavior.Blocked;
        var primaryRule = FindRule(_primary);
        _overallLink.Text = primaryRule == null ? Strings.Ui("UsingOverallBehavior") : Strings.Ui("UseOverallBehavior");
        _overallLink.Enabled = primaryRule != null && !excluded;
        var control = primaryRule?.Control ?? BoundaryControl.Automatic;
        _automaticControl.Checked = control == BoundaryControl.Automatic;
        _alwaysControl.Checked = control == BoundaryControl.Always;
        BuildSeparateRows(excluded);
        UpdateLiveStatus();
        ReserveDrawerStatusHeight();
        _rebuilding = false;
        LayoutBoundaryDrawer();
        ApplyTheme();
    }

    private void BuildSeparateRows(bool suppressSelection = false)
    {
        _separatePanel.SuspendLayout(); _separatePanel.Controls.Clear(); _separatePanel.RowStyles.Clear(); _separatePanel.RowCount = 0;
        if (!_separateExpanded) { _separatePanel.Visible = false; _separatePanel.ResumeLayout(); return; }
        _separatePanel.Visible = true;
        AddDirectionRow(_primary, suppressSelection); AddDirectionRow(_reverse, suppressSelection);
        _separatePanel.ResumeLayout();
    }

    private void AddDirectionRow(BoundaryEndpoint endpoint, bool suppressSelection)
    {
        int row = _separatePanel.RowCount++;
        _separatePanel.RowStyles.Add(new RowStyle(SizeType.Absolute, 78));
        var panel = new Panel { Dock = DockStyle.Fill, Tag = "surface2" };
        panel.Controls.Add(new Label { Text = string.Format(Strings.Ui("DirectionFromTo"), ScreenName(endpoint.From), ScreenName(endpoint.To)), Left = 0, Top = 0, Width = 190, Height = 72, Font = new Font("Segoe UI Semibold", 8.75f), TextAlign = ContentAlignment.MiddleLeft });
        var modes = new[] { BoundaryBehavior.Free, BoundaryBehavior.Push, BoundaryBehavior.Blocked };
        for (int i = 0; i < modes.Length; i++)
        {
            var behavior = modes[i];
            var tile = NewModeTile(behavior, BehaviorTitle(behavior), "", true);
            tile.Left = 198 + i * 195; tile.Top = 5; tile.Width = 188; tile.Height = 66;
            tile.Selected = !suppressSelection && ResolvedBehavior(endpoint) == behavior;
            tile.Click += (_, _) => { SetEndpointBehavior(endpoint, behavior); RefreshMap(); UpdateDrawer(); };
            panel.Controls.Add(tile);
        }
        _separatePanel.Controls.Add(panel, 0, row);
    }

    private ModeTile NewModeTile(BoundaryBehavior behavior, string title, string help, bool compact = false)
    {
        var tile = new ModeTile
        {
            Behavior = behavior, Title = title, Help = help,
            Width = compact ? 188 : 254, Height = compact ? 66 : 78,
            Margin = compact ? new Padding(0, 0, 7, 0) : new Padding(0, 0, 8, 0),
        };
        tile.SetTheme(_theme); tile.Click += (_, _) => { if (!compact) ChoosePrimaryBehavior(behavior); }; return tile;
    }

    private void ChoosePrimaryBehavior(BoundaryBehavior behavior)
    {
        if (_boundary == null) return;
        SetEndpointBehavior(_primary, behavior);
        if (!_boundary.VerticalConnection) { SetEndpointBehavior(_reverse, behavior); _separateExpanded = false; }
        else if (_boundary.VerticalConnection && FindRule(_reverse) == null) SetEndpointBehavior(_reverse, BoundaryBehavior.Free);
        RefreshMap(); UpdateDrawer();
    }

    private void SetEndpointBehavior(BoundaryEndpoint endpoint, BoundaryBehavior behavior)
    {
        var rule = FindRule(endpoint);
        if (rule == null)
        {
            rule = new BoundaryRule { FromStableId = endpoint.From.StableId, ToStableId = endpoint.To.StableId,
                Direction = endpoint.Direction, Control = BoundaryControl.Automatic };
            _draft.BoundaryRules.Add(rule);
        }
        rule.Behavior = behavior;
    }

    private void SetControl(BoundaryControl control)
    {
        var targets = !_boundary.VerticalConnection && !_separateExpanded ? new[] { _primary, _reverse } : new[] { _primary };
        foreach (var endpoint in targets)
        {
            var rule = FindRule(endpoint);
            if (rule != null) rule.Control = control;
        }
        RefreshMap(); UpdateLiveStatus();
    }

    private void UseOverall()
    {
        RemoveRule(_primary);
        if (!_boundary.VerticalConnection && !_separateExpanded) RemoveRule(_reverse);
        RefreshMap(); UpdateDrawer();
    }

    private void ToggleSeparate()
    {
        if (_boundary == null || _boundary.VerticalConnection) return;
        _separateExpanded = !_separateExpanded; BuildSeparateRows();
    }

    private void ToggleWhen()
    {
        _whenExpanded = !_whenExpanded; _whenPanel.Visible = _whenExpanded;
    }

    private void AllowBoundaryScreen()
    {
        if (_boundary == null) return;
        _draft.ExcludedDevices.RemoveAll(id => Same(id, _primary.To.StableId) || Same(id, _primary.From.StableId));
        RebuildScreenRows(); RefreshMap(); UpdateDrawer();
    }

    private void UpdateLiveStatus()
    {
        if (_boundary == null || _drawerStatus == null) return;
        string status = "";
        var excludedTarget = ExcludedBoundaryTarget();
        if (excludedTarget != null)
            status = string.Format(Strings.Ui("ExcludedTargetExplanation"), ScreenName(excludedTarget));
        else if (_primary.Runtime?.SafetyDemoted == true) status = string.Format(Strings.Ui("StatusSafety"), ScreenName(_primary.From));
        else if (_runtimeState != null)
        {
            var state = _runtimeState();
            var rule = FindRule(_primary);
            var chosen = ResolvedBehavior(_primary);
            if (rule?.Control != BoundaryControl.Always && _primary.Direction == BoundaryDirection.Up && !state.UpwardPassageOpen && chosen != BoundaryBehavior.Blocked)
                status = Strings.Ui("StatusUpClosed");
            else if (rule?.Control != BoundaryControl.Always && _primary.Direction is BoundaryDirection.Left or BoundaryDirection.Right && !state.SideProtectionOn && chosen != BoundaryBehavior.Free)
                status = Strings.Ui("StatusSideOff");
        }
        _drawerStatus.Text = status;
        _drawerStatus.AccessibleName = status;
    }

    private MonitorInfo ExcludedBoundaryTarget()
    {
        if (_boundary == null || _primary == null) return null;
        if (_draft.ExcludedDevices.Contains(_primary.To.StableId, StringComparer.OrdinalIgnoreCase))
            return _primary.To;
        if (_draft.ExcludedDevices.Contains(_primary.From.StableId, StringComparer.OrdinalIgnoreCase))
            return _primary.From;
        return null;
    }

    private void RefreshMap()
    {
        _map?.SetModel(_monitors, _draft, _draft.MonitorNames);
        if (_mapHint != null && _monitors.Count(m => !_draft.ExcludedDevices.Contains(m.StableId, StringComparer.OrdinalIgnoreCase)) > 1)
        {
            _mapHint.Text = Strings.Ui("MapHintV2");
            UpdateBoundaryTipVisibility();
        }
        if (_boundary != null)
        {
            _boundary = _map.FindBoundary(_primary.From.StableId, _primary.To.StableId, _primary.Direction);
            if (_boundary != null) { ConfigureEndpoints(); _map.SelectedBoundary = _boundary; }
        }
    }

    private void CloseDrawer()
    {
        if (!_screenSplit.Visible) return;
        _screenSplit.Visible = false; _screenSplit.Height = 0; _boundary = null; _map.SelectedBoundary = null; _map.Focus();
    }

    private void LayoutBoundaryDrawer()
    {
        if (_screenSplit == null || !_screenSplit.Visible || _screenSplit.Width <= 0) return;
        int contentWidth = Math.Max(DpiPx(420), _bodyFlow.ClientSize.Width - _bodyFlow.Padding.Horizontal - SystemInformation.VerticalScrollBarWidth - DpiPx(4));
        PrepareBoundaryDrawerWidth(contentWidth);
        int width = Math.Max(DpiPx(430), _screenSplit.ClientSize.Width - DpiPx(28));
        int preferred = _drawerFlow.GetPreferredSize(new Size(width, 0)).Height + _drawer.Padding.Vertical + DpiPx(2);
        _screenSplit.Height = Math.Max(DpiPx(245), preferred);
    }

    private void PrepareBoundaryDrawerWidth(int contentWidth)
    {
        _screenSplit.Width = contentWidth;
        int width = Math.Max(DpiPx(430), _screenSplit.ClientSize.Width - DpiPx(28));
        foreach (Control child in _drawerFlow.Controls) child.Width = width;
        LayoutDrawerQuestionText();
        _modeTiles.Width = width;
        int tileWidth = Math.Max(DpiPx(150), (width - DpiPx(16)) / 3);
        foreach (ModeTile tile in _modeTiles.Controls) tile.Width = tileWidth;
        _separatePanel.Width = width; _whenPanel.Width = width;
    }

    private int RestingBoundaryDrawerHeight(int contentWidth)
    {
        PrepareBoundaryDrawerWidth(contentWidth);
        ReserveDrawerStatusHeight();
        int height = _drawer.Padding.Vertical + DpiPx(2);
        foreach (Control child in _drawerFlow.Controls)
        {
            if (child == _separatePanel || child == _whenPanel) continue;
            height += child.Height + child.Margin.Vertical;
        }
        return Math.Max(DpiPx(245), height);
    }

    private int DpiPx(int logicalPixels) => (int)Math.Round(logicalPixels * DeviceDpi / 96f);

    private void MonitorChosen(MonitorInfo monitor)
    {
        if (monitor == null) return;
        // Clicking a screen only SELECTS/highlights it — identification (the on-screen overlay) is
        // triggered exclusively by the "Identify all screens" button (owner request 2026-07-15).
        _map.SelectedStableId = monitor.StableId;
    }

    private void HoverRow(string stableId)
    {
        foreach (Control panel in _screenRows.Controls) panel.Tag = "screenRow";
    }

    private BoundaryValidationIssue _pendingIssue;
    private void OpenValidationBoundary(object sender, EventArgs e)
    {
        if (_pendingIssue == null) return;
        var boundary = _map.FindBoundary(_pendingIssue.StableId, _pendingIssue.FocusToStableId, _pendingIssue.FocusDirection);
        if (boundary != null) { _tabs.SelectedIndex = 0; OpenBoundary(boundary); }
    }

    private void ShowInline(string text, bool action = false)
    {
        SetInlineStatus(text, true, action);
    }

    private void SetInlineStatus(string text, bool error = false, bool action = false)
    {
        if (_inlineStatus == null) return;
        _inlineStatusIsError = error;
        _inlineStatus.Text = text ?? "";
        _inlineStatus.ForeColor = error ? _theme.Danger : _theme.Faint;
        _inlineAction.Enabled = action;
        _inlineAction.Visible = action;
        _inlineAction.Text = action ? Strings.Ui("OpenBoundary") : "";
    }

    private void UpdateBoundaryTipVisibility()
    {
        if (_inlineStatus == null || _tabs == null) return;
        bool showingTip = _inlineStatus.Text == Strings.Ui("BoundaryTip");
        if (_tabs.SelectedIndex != 0)
        {
            if (showingTip) SetInlineStatus("");
            return;
        }
        if (_inlineStatus.Text.Length == 0 && !_draft.BoundaryTipSeen && !_draft.BoundaryRules.Any() &&
            _monitors.Count(m => !_draft.ExcludedDevices.Contains(m.StableId, StringComparer.OrdinalIgnoreCase)) > 1)
            SetInlineStatus(Strings.Ui("BoundaryTip"));
    }

    private void SanitizeExcluded()
    {
        var ids = _monitors.Select(m => m.StableId).ToList();
        string primary = (_monitors.FirstOrDefault(m => m.Primary) ?? _monitors.FirstOrDefault())?.StableId;
        _draft.ExcludedDevices = GuardCore.SanitizeExcluded(ids, primary,
            new HashSet<string>(_draft.ExcludedDevices, StringComparer.OrdinalIgnoreCase), out _);
    }

    private BoundaryRule FindRule(BoundaryEndpoint e) => _draft.BoundaryRules.LastOrDefault(r => Same(r.FromStableId, e.From.StableId) && Same(r.ToStableId, e.To.StableId) && r.Direction == e.Direction);
    private void RemoveRule(BoundaryEndpoint e) => _draft.BoundaryRules.RemoveAll(r => Same(r.FromStableId, e.From.StableId) && Same(r.ToStableId, e.To.StableId) && r.Direction == e.Direction);
    private BoundaryBehavior ResolvedBehavior(BoundaryEndpoint e)
    {
        var saved = FindRule(e)?.Behavior;
        if (saved == BoundaryBehavior.OverallFeel) return BoundaryBehavior.Push;
        if (saved is BoundaryBehavior.Free or BoundaryBehavior.Push or BoundaryBehavior.Blocked) return saved.Value;
        return e.Runtime?.Behavior ?? BoundaryBehavior.Free;
    }
    private bool DirectionsDiffer() => ResolvedBehavior(_primary) != ResolvedBehavior(_reverse) || FindRule(_primary)?.Control != FindRule(_reverse)?.Control;
    private string BehaviorTitle(BoundaryBehavior b) => b switch { BoundaryBehavior.Blocked => Strings.Ui("NeverCrosses"), BoundaryBehavior.Push => Strings.Ui("NeedsFirmPush"), _ => Strings.Ui("CrossesFreely") };
    private string ScreenName(string id) => ScreenName(_monitors.FirstOrDefault(m => Same(m.StableId, id)));
    private string ScreenName(MonitorInfo m)
    {
        if (m == null) return "";
        string name = Settings.MonitorNameFor(_draft.MonitorNames, m.StableId);
        return name.Length > 0 ? name : (m.Primary ? Strings.Ui("MainWord") : string.Format(Strings.Ui("DefaultScreenName"), m.Index));
    }
    private string ChipText(MonitorInfo m) => $"{m.Index} · {ScreenName(m)}";

}
