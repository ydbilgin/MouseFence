using System.Text.Json;

namespace MouseFence;

public sealed partial class SettingsForm
{
    private (ComboBox Language, ComboBox Theme, CheckBox StartGate, CheckBox StartSide, CheckBox AutoStart, CheckBox StartConfine, CheckBox StartPaused) BuildGeneralTab(TabPage page)
    {
        var flow = PageFlow(page); flow.Controls.Add(PageHeader(Strings.Ui("NavGeneral"), Strings.Ui("GeneralSubtitle"), 800));
        AddSectionHeading(flow, Strings.StartupHead);
        var gate = new ThemedCheckBox { Text = Strings.StartGateClosed, Width = 800, Height = 32, Checked = _draft.StartGateClosed };
        var side = new ThemedCheckBox { Text = Strings.StartSideContainOn, Width = 800, Height = 32, Checked = _draft.StartSideContainOn };
        var auto = new ThemedCheckBox { Text = Strings.StartWithWindows, Width = 800, Height = 32, Checked = _draft.AutoStart };
        flow.Controls.Add(gate); flow.Controls.Add(side); flow.Controls.Add(auto);
        _tips.SetToolTip(gate, Strings.StartGateClosedTip); _tips.SetToolTip(side, Strings.StartSideContainTip); _tips.SetToolTip(auto, Strings.StartWithWindowsTip);
        var more = new ThemedButton { Text = "›  " + Strings.Ui("MoreStartupChoices"), Width = 800, Height = 36, Role = ThemedButtonRole.Quiet, TextAlign = ContentAlignment.MiddleLeft };
        var morePanel = new Panel { Width = 800, Height = 206, Visible = false, Tag = "surface" };
        var confine = new ThemedCheckBox { Text = Strings.Ui("StartConfineOn"), Left = 0, Top = 0, Width = 790, Height = 32, Checked = _draft.StartConfineOn };
        var confineHelp = new Label { Text = Strings.Ui("StartConfineHelp"), Left = 28, Top = 31, Width = 760, Height = 30, Tag = "subtle" };
        var paused = new ThemedCheckBox { Text = Strings.Ui("StartPaused"), Left = 0, Top = 64, Width = 790, Height = 32, Checked = _draft.StartPaused };
        var pausedHelp = new Label { Text = Strings.Ui("StartPausedHelp"), Left = 28, Top = 95, Width = 760, Height = 38, Tag = "subtle" };
        var warning = new Label { Left = 0, Top = 138, Width = 790, Height = 64, Tag = "warning", Text = Strings.Ui("StartupConfineWarning") };
        void UpdateWarning() { bool active = confine.Checked && auto.Checked; warning.Enabled = active; warning.Text = active ? Strings.Ui("StartupConfineWarning") : ""; }
        confine.CheckedChanged += (_, _) => UpdateWarning(); auto.CheckedChanged += (_, _) => UpdateWarning(); UpdateWarning();
        morePanel.Controls.Add(confine); morePanel.Controls.Add(confineHelp); morePanel.Controls.Add(paused); morePanel.Controls.Add(pausedHelp); morePanel.Controls.Add(warning);
        more.Click += (_, _) => { morePanel.Visible = !morePanel.Visible; more.Text = (morePanel.Visible ? "⌄  " : "›  ") + Strings.Ui("MoreStartupChoices"); };
        flow.Controls.Add(more); flow.Controls.Add(morePanel);
        AddSectionHeading(flow, Strings.AppearanceHead);
        var row = new TableLayoutPanel { Width = 800, Height = 78, ColumnCount = 2, Tag = "appearanceGrid" };
        row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50)); row.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        var langLabel = new Label { Text = Strings.LanguageLabel, Left = 0, Top = 0, Width = 310, Height = 24, Tag = "subtle" };
        var themeLabel = new Label { Text = Strings.ThemeLabel, Left = 0, Top = 0, Width = 310, Height = 24, Tag = "subtle" };
        var lang = new ThemedComboBox { Left = 0, Top = 28, Width = 370, Height = 36 };
        lang.Items.AddRange(new object[] { Strings.LangAuto, Strings.LangEnglish, Strings.LangTurkish }); lang.SelectedIndex = _draft.Language switch { "en" => 1, "tr" => 2, _ => 0 };
        var theme = new ThemedComboBox { Left = 0, Top = 28, Width = 370, Height = 36 };
        theme.Items.AddRange(new object[] { Strings.ThemeSystem, Strings.ThemeLight, Strings.ThemeDark }); theme.SelectedIndex = _draft.Theme switch { "Light" => 1, "Dark" => 2, _ => 0 };
        var left = new Panel { Dock = DockStyle.Fill }; left.Controls.Add(lang); left.Controls.Add(langLabel);
        var right = new Panel { Dock = DockStyle.Fill }; right.Controls.Add(theme); right.Controls.Add(themeLabel);
        row.Controls.Add(left, 0, 0); row.Controls.Add(right, 1, 0); flow.Controls.Add(row);
        theme.SelectedIndexChanged += (_, _) => { _draft.Theme = theme.SelectedIndex switch { 1 => "Light", 2 => "Dark", _ => "System" }; _theme = Theming.Resolve(_draft.Theme); ApplyTheme(); };
        AddSectionHeading(flow, Strings.Ui("BackupHead"));
        flow.Controls.Add(HelpLabel(Strings.Ui("BackupHelpV2"), 800, 44));
        var backupRow = new FlowLayoutPanel { Width = 800, Height = 48, FlowDirection = FlowDirection.LeftToRight };
        var export = new ThemedButton { Text = Strings.ExportLabel, Width = 170, Height = 36, Role = ThemedButtonRole.Secondary }; export.Click += Export_Click;
        var import = new ThemedButton { Text = Strings.ImportLabel, Width = 170, Height = 36, Role = ThemedButtonRole.Secondary }; import.Click += Import_Click;
        backupRow.Controls.Add(export); backupRow.Controls.Add(import); flow.Controls.Add(backupRow);
        return (lang, theme, gate, side, auto, confine, paused);
    }

    private void SyncSimpleControls()
    {
        _draft.StartGateClosed = _startGate.Checked; _draft.StartSideContainOn = _startSide.Checked; _draft.AutoStart = _autoStart.Checked;
        _draft.StartConfineOn = _startConfine.Checked; _draft.StartPaused = _startPaused.Checked;
        _draft.Language = _langCombo.SelectedIndex switch { 1 => "en", 2 => "tr", _ => "auto" };
        _draft.Theme = _themeCombo.SelectedIndex switch { 1 => "Light", 2 => "Dark", _ => "System" };
    }

    private void Export_Click(object sender, EventArgs e)
    {
        SyncSimpleControls();
        if (!ValidateDraftForBackup()) return;
        string path = PickBackupPath();
        if (!string.IsNullOrEmpty(path)) WriteBackupFile(path);
    }

    private string PickBackupPath()
    {
        using var dialog = new SaveFileDialog { Title = Strings.ExportTitle, Filter = Strings.SettingsFileFilter,
            FileName = "MouseFence-settings.json", DefaultExt = "json", AddExtension = true };
        return dialog.ShowDialog(this) == DialogResult.OK ? dialog.FileName : null;
    }

    private bool ValidateDraftForBackup()
    {
        var issues = _draft.ValidateNoSeal(_monitors);
        if (issues.Length == 0) return true;
        _pendingIssue = issues[0];
        ShowInline(string.Format(Strings.Ui("InlineSealError"), ScreenName(issues[0].StableId)), true);
        return false;
    }

    private bool WriteBackupFile(string path)
    {
        try
        {
            File.WriteAllText(path, JsonSerializer.Serialize(_draft, new JsonSerializerOptions { WriteIndented = true }));
            SetInlineStatus(Strings.Ui("BackupDone"));
            return true;
        }
        catch { ShowInline(Strings.Ui("ExportFailed")); return false; }
    }

    private void Import_Click(object sender, EventArgs e)
    {
        string path = PickRestorePath();
        if (string.IsNullOrEmpty(path)) return;
        try
        {
            string json = File.ReadAllText(path);
            var imported = JsonSerializer.Deserialize<Settings>(json);
            if (imported == null) { ShowInline(Strings.ImportInvalid); return; }
            int version = Settings.ReadVersion(json); imported.Normalize();
            if (version < Settings.CurrentSettingsVersion) imported.MigrateLayoutKeysToStable(_monitors);
            var issues = imported.ValidateNoSeal(_monitors);
            if (imported.HasRepairableImportErrors || issues.Length > 0)
            {
                CopySettings(imported, _draft);
                LoadDraftIntoControls();
                if (issues.Length > 0) { _pendingIssue = issues[0]; ShowInline(string.Format(Strings.Ui("InlineSealError"), ScreenName(issues[0].StableId)), true); }
                else ShowInline(Strings.ImportInvalid);
                return;
            }
            bool confirmed = RestoreConfirmForTests?.Invoke() ?? MessageBox.Show(this, Strings.ImportConfirm, Strings.SettingsTitle,
                MessageBoxButtons.YesNo, MessageBoxIcon.Question) == DialogResult.Yes;
            if (!confirmed) return;
            CopySettings(imported, _draft);
            Result = _draft.DeepClone(); DialogResult = DialogResult.OK;
        }
        catch { ShowInline(Strings.ImportInvalid); }
    }

    private string PickRestorePath()
    {
        if (RestorePathPickerForTests != null) return RestorePathPickerForTests();
        using var dialog = new OpenFileDialog { Title = Strings.ImportTitle, Filter = Strings.SettingsFileFilter, CheckFileExists = true };
        return dialog.ShowDialog(this) == DialogResult.OK ? dialog.FileName : null;
    }

    private static void CopySettings(Settings source, Settings target)
    {
        var json = JsonSerializer.Serialize(source);
        var copy = JsonSerializer.Deserialize<Settings>(json);
        foreach (var property in typeof(Settings).GetProperties().Where(p => p.CanRead && p.CanWrite && p.SetMethod?.IsPublic == true))
            property.SetValue(target, property.GetValue(copy));
        target.Normalize();
    }

    private void LoadDraftIntoControls()
    {
        _rebuilding = true;
        _startGate.Checked = _draft.StartGateClosed; _startSide.Checked = _draft.StartSideContainOn; _autoStart.Checked = _draft.AutoStart;
        _startConfine.Checked = _draft.StartConfineOn; _startPaused.Checked = _draft.StartPaused;
        _langCombo.SelectedIndex = _draft.Language switch { "en" => 1, "tr" => 2, _ => 0 };
        _themeCombo.SelectedIndex = _draft.Theme switch { "Light" => 1, "Dark" => 2, _ => 0 };
        for (int i = 0; i < _hotkeyBoxes.Length; i++) _hotkeyBoxes[i].Text = HotkeyText(i);
        _rebuilding = false;
        RebuildScreenRows(); RebuildAdvancedContents(); RefreshMap(); CloseDrawer();
    }

}
