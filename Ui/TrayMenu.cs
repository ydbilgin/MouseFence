namespace MouseFence;

/// <summary>What the tray menu shows: the guard's state as ticks, named like the Settings pages.</summary>
internal readonly record struct TrayMenuState(bool Enabled, bool GateOpen, bool SideOn, bool Confine);

/// <summary>
/// The tray menu. Each switch is named as in Settings (Upward passage, Side protection, Game mode, Pause),
/// its state is a tick, and its global shortcut is shown on the right. The menu follows the app theme.
/// Built apart from the tray so the documentation harness and the tests can draw it without hooks.
/// </summary>
internal sealed class TrayMenu : IDisposable
{
    public readonly ContextMenuStrip Menu = new();
    public readonly ToolStripMenuItem Gate, Side, Game, Pause, SettingsItem, Exit;

    public TrayMenu(Action gate, Action side, Action game, Action pause, Action settings, Action exit)
    {
        Gate = new ToolStripMenuItem("", null, (_, _) => gate());
        Side = new ToolStripMenuItem("", null, (_, _) => side());
        Game = new ToolStripMenuItem("", null, (_, _) => game());
        Pause = new ToolStripMenuItem("", null, (_, _) => pause());
        SettingsItem = new ToolStripMenuItem("", null, (_, _) => settings());
        Exit = new ToolStripMenuItem("", null, (_, _) => exit());
        Menu.Items.Add(Gate);
        Menu.Items.Add(Side);
        Menu.Items.Add(Game);
        Menu.Items.Add(Pause);
        Menu.Items.Add(new ToolStripSeparator());
        Menu.Items.Add(SettingsItem);
        Menu.Items.Add(Exit);
        Menu.ShowImageMargin = false;
        Menu.ShowCheckMargin = true;
        Menu.Padding = new Padding(2, 4, 2, 4);
    }

    /// <summary>Sets theme, texts, shortcuts, ticks and enabled states. Same states as before the refresh.</summary>
    public void Apply(Settings settings, TrayMenuState state)
    {
        var theme = Theming.Resolve(settings.Theme);
        if (Menu.Renderer is not ThemedMenuRenderer current || current.Theme != theme)
            Menu.Renderer = new ThemedMenuRenderer(theme);

        SettingsItem.Text = Strings.MenuSettings;
        Exit.Text = Strings.MenuExit;
        Game.Text = Strings.MenuGame;
        Pause.Text = Strings.MenuPause;

        Gate.ShortcutKeyDisplayString = Shortcut(settings.HotKey, settings.HotKeyText());
        Side.ShortcutKeyDisplayString = Shortcut(settings.SideHotKey, settings.SideHotKeyText());
        Game.ShortcutKeyDisplayString = Shortcut(settings.ConfineHotKey, settings.ConfineHotKeyText());
        Pause.ShortcutKeyDisplayString = Shortcut(settings.PauseHotKey, settings.PauseHotKeyText());

        if (!state.Enabled)
        {
            Gate.Text = Strings.GatePausedMenu;
            Gate.Enabled = false;
            Side.Text = Strings.SideGatePausedMenu;
            Side.Enabled = false;
            Game.Enabled = false;
            Pause.Checked = true;
            return;
        }

        Gate.Enabled = true;
        Side.Enabled = true;
        Pause.Checked = false;
        Gate.Text = Strings.HotkeyHead;
        Gate.Checked = state.GateOpen;
        Side.Text = Strings.SideHotkeyHead;
        Side.Checked = state.SideOn;
        Game.Checked = state.Confine;
        Game.Enabled = true;
    }

    /// <summary>"Ctrl+Alt+Up" for a set shortcut, nothing for a cleared one.</summary>
    internal static string Shortcut(Keys key, string text) => key == Keys.None ? "" : text;

    public void Dispose() => Menu.Dispose();
}
