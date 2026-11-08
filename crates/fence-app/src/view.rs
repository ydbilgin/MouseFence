//! The tray's view model helpers: the fixed menu order (B002) and the ids that tie native events (menu
//! commands, WM_HOTKEY ids, WM_TIMER ids) back to controller events.

use crate::{HotkeySlot, MenuItem, TrayIcon, TrayView};

/// The six commands of the tray menu (C# `_gateItem` .. `_exitItem`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuCommand {
    Gate,
    Side,
    Game,
    Pause,
    Settings,
    Exit,
}

impl MenuCommand {
    pub const ALL: [MenuCommand; 6] = [
        MenuCommand::Gate,
        MenuCommand::Side,
        MenuCommand::Game,
        MenuCommand::Pause,
        MenuCommand::Settings,
        MenuCommand::Exit,
    ];

    /// The native menu item id (never 0: TrackPopupMenu returns 0 for "nothing chosen").
    pub const fn id(self) -> u32 {
        match self {
            MenuCommand::Gate => 1,
            MenuCommand::Side => 2,
            MenuCommand::Game => 3,
            MenuCommand::Pause => 4,
            MenuCommand::Settings => 5,
            MenuCommand::Exit => 6,
        }
    }

    pub fn from_id(id: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.id() == id)
    }
}

/// One entry of the context menu, in display order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuEntry<'a> {
    Item(MenuCommand, &'a MenuItem),
    Separator,
}

impl TrayView {
    /// The view before the first `UpdateUi`: closed icon, tip "MouseFence", empty enabled items (a fresh
    /// ToolStripMenuItem is enabled and unchecked).
    pub fn initial() -> Self {
        let item = || MenuItem {
            text: String::new(),
            shortcut: String::new(),
            checked: false,
            enabled: true,
        };
        Self {
            icon: TrayIcon::Closed,
            tooltip: "MouseFence".to_string(),
            gate: item(),
            side: item(),
            game: item(),
            pause: item(),
            settings: item(),
            exit: item(),
        }
    }

    /// C# menu order (Ui/TrayApplicationContext.cs:63-70): gate, side, game, pause, separator, settings, exit.
    pub fn menu(&self) -> Vec<MenuEntry<'_>> {
        vec![
            MenuEntry::Item(MenuCommand::Gate, &self.gate),
            MenuEntry::Item(MenuCommand::Side, &self.side),
            MenuEntry::Item(MenuCommand::Game, &self.game),
            MenuEntry::Item(MenuCommand::Pause, &self.pause),
            MenuEntry::Separator,
            MenuEntry::Item(MenuCommand::Settings, &self.settings),
            MenuEntry::Item(MenuCommand::Exit, &self.exit),
        ]
    }
}

impl HotkeySlot {
    pub const ALL: [HotkeySlot; 5] = [
        HotkeySlot::Gate,
        HotkeySlot::Confine,
        HotkeySlot::Pause,
        HotkeySlot::Side,
        HotkeySlot::MarkAccident,
    ];

    /// The WM_HOTKEY id: the 1.x id 0xB10C plus the slot number.
    pub const fn id(self) -> i32 {
        0xB10C
            + match self {
                HotkeySlot::Gate => 0,
                HotkeySlot::Confine => 1,
                HotkeySlot::Pause => 2,
                HotkeySlot::Side => 3,
                HotkeySlot::MarkAccident => 4,
            }
    }

    pub fn from_id(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// The two UI-thread timers (C# `_reconfigTimer`, `_hookProbeTimer`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimerId {
    /// 750 ms display-change debounce.
    Reconfigure,
    /// 3000 ms hook liveness probe.
    HookProbe,
    TraceDrain,
}

impl TimerId {
    pub const fn id(self) -> usize {
        match self {
            TimerId::Reconfigure => 1,
            TimerId::HookProbe => 2,
            TimerId::TraceDrain => 3,
        }
    }

    pub fn from_id(id: usize) -> Option<Self> {
        [TimerId::Reconfigure, TimerId::HookProbe, TimerId::TraceDrain]
            .into_iter()
            .find(|t| t.id() == id)
    }
}
