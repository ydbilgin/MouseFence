using System.Drawing;

namespace MouseFence;

/// <summary>Simple settings dialog: pick the toggle hotkey, start behaviour, and which monitors to block.</summary>
public sealed class SettingsForm : Form
{
    private readonly TextBox _hotkeyBox;
    private readonly CheckBox _startActive;
    private readonly CheckBox _autoStart;
    private readonly RadioButton _modeAuto;
    private readonly RadioButton _modeManual;
    private readonly CheckedListBox _monitorList;
    private readonly List<MonitorInfo> _monitors;

    private bool _mc, _ma, _ms, _mw;
    private Keys _key;

    public Settings Result { get; private set; }

    public SettingsForm(Settings s, List<MonitorInfo> monitors)
    {
        _monitors = monitors;
        _mc = s.ModCtrl; _ma = s.ModAlt; _ms = s.ModShift; _mw = s.ModWin; _key = s.HotKey;

        Text = "MouseFence — Ayarlar";
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = false;
        MinimizeBox = false;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.CenterScreen;
        ClientSize = new Size(430, 478);
        Font = new Font("Segoe UI", 9f);

        int y = 14;

        var lblHk = new Label
        {
            Text = "Aç/Kapa kısayolu — kutuya tıkla, sonra tuş kombinasyonuna bas:",
            Left = 14, Top = y, Width = 402, AutoSize = false, Height = 18
        };
        y += 22;

        _hotkeyBox = new TextBox
        {
            Left = 14, Top = y, Width = 300, ReadOnly = true,
            BackColor = Color.White, Cursor = Cursors.Hand, TextAlign = HorizontalAlignment.Center
        };
        _hotkeyBox.KeyDown += HotkeyBox_KeyDown;
        var btnClear = new Button { Text = "Temizle", Left = 322, Top = y - 1, Width = 94 };
        btnClear.Click += (a, b) => { _mc = _ma = _ms = _mw = false; _key = Keys.None; UpdateHotkeyText(); };
        y += 40;

        _startActive = new CheckBox
        {
            Text = "Açılışta ana→üst geçiş KAPALI başlasın",
            Left = 14, Top = y, Width = 402, Checked = s.StartGateClosed
        };
        y += 26;

        _autoStart = new CheckBox
        {
            Text = "Windows ile birlikte başlat",
            Left = 14, Top = y, Width = 402, Checked = s.AutoStart
        };
        y += 34;

        var grp = new GroupBox { Text = "Engellenecek ekran(lar)", Left = 14, Top = y, Width = 402, Height = 214 };
        _modeAuto = new RadioButton
        {
            Text = "Otomatik — yukarıdaki (üstteki) ekranları engelle",
            Left = 12, Top = 22, Width = 380, Checked = s.Mode != "Manual"
        };
        _modeManual = new RadioButton
        {
            Text = "Elle seç:",
            Left = 12, Top = 46, Width = 380, Checked = s.Mode == "Manual"
        };
        _monitorList = new CheckedListBox
        {
            Left = 30, Top = 72, Width = 356, Height = 128, CheckOnClick = true,
            IntegralHeight = false
        };
        foreach (var m in monitors)
        {
            string dev = m.Device.Replace(@"\\.\", "");
            string label = $"#{m.Index}  {dev}  {m.Bounds.Width}x{m.Bounds.Height} @({m.Bounds.Left},{m.Bounds.Top}){(m.Primary ? "  [ANA]" : "")}";
            int idx = _monitorList.Items.Add(label);
            bool chk = s.Mode == "Manual" ? s.ManualMonitors.Contains(m.Device) : MonitorInfo.IsAbovePrimary(m);
            _monitorList.SetItemChecked(idx, chk);
        }
        _monitorList.Enabled = _modeManual.Checked;
        _modeManual.CheckedChanged += (a, b) => _monitorList.Enabled = _modeManual.Checked;

        grp.Controls.Add(_modeAuto);
        grp.Controls.Add(_modeManual);
        grp.Controls.Add(_monitorList);
        y += 224;

        var note = new Label
        {
            Text = "Not: kısayol yalnızca ANA ekrandan üst ekrana geçişi açıp kapatır.\nYan ekranlardan üst ekrana geçiş her zaman engellidir.",
            Left = 14, Top = y, Width = 402, Height = 32, ForeColor = Color.DimGray
        };
        y += 36;

        var ok = new Button { Text = "Kaydet", Left = 240, Top = y, Width = 84, DialogResult = DialogResult.OK };
        var cancel = new Button { Text = "İptal", Left = 332, Top = y, Width = 84, DialogResult = DialogResult.Cancel };
        ok.Click += Ok_Click;

        Controls.Add(lblHk);
        Controls.Add(_hotkeyBox);
        Controls.Add(btnClear);
        Controls.Add(_startActive);
        Controls.Add(_autoStart);
        Controls.Add(grp);
        Controls.Add(note);
        Controls.Add(ok);
        Controls.Add(cancel);

        AcceptButton = ok;
        CancelButton = cancel;

        Result = s;
        UpdateHotkeyText();
    }

    private void HotkeyBox_KeyDown(object sender, KeyEventArgs e)
    {
        e.SuppressKeyPress = true;
        e.Handled = true;

        _mc = e.Control;
        _ma = e.Alt;
        _ms = e.Shift;

        var kc = e.KeyCode;
        if (kc is Keys.ControlKey or Keys.Menu or Keys.ShiftKey)
        {
            _key = Keys.None;          // only a modifier down so far
            UpdateHotkeyText();
            return;
        }
        if (kc is Keys.LWin or Keys.RWin)
        {
            _mw = true;
            _key = Keys.None;
            UpdateHotkeyText();
            return;
        }

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
        _hotkeyBox.Text = parts.Count == 0 ? "(tuşa bas)" : string.Join(" + ", parts);
    }

    private void Ok_Click(object sender, EventArgs e)
    {
        bool anyMod = _mc || _ma || _ms || _mw;
        if (_key == Keys.None || !anyMod)
        {
            MessageBox.Show("Lütfen en az bir değiştirici (Ctrl / Alt / Shift / Win) ve bir normal tuş seç.",
                "MouseFence", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            DialogResult = DialogResult.None;
            return;
        }

        var res = new Settings
        {
            ModCtrl = _mc, ModAlt = _ma, ModShift = _ms, ModWin = _mw, HotKey = _key,
            StartGateClosed = _startActive.Checked,
            AutoStart = _autoStart.Checked,
            Mode = _modeManual.Checked ? "Manual" : "AutoTop",
        };

        if (res.Mode == "Manual")
        {
            for (int i = 0; i < _monitorList.Items.Count; i++)
                if (_monitorList.GetItemChecked(i))
                    res.ManualMonitors.Add(_monitors[i].Device);
        }

        Result = res;
    }
}
