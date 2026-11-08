//! WinForms `Keys` names and the hotkey text of C# `Settings.HotKeyText()` and its siblings.
//!
//! `Keys` is a `[Flags]` enum, so `Keys.ToString()` is .NET's flags formatter: an exact value gives the name
//! of the LAST entry with that value in .NET's own order (13 is "Enter", not "Return"); other values are
//! split greedily from the largest entry down ("A, Shift"); a value that does not split into names is
//! printed as its signed number. [`KEYS`] is that order, dumped from .NET (`keys_enum` reference verb) and
//! checked against the reference by crates/fence-trace/tests/config_reference_diff.rs.

use fence_types::Hotkey;

/// The first invalid shortcut, in settings row order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutIssue {
    Incomplete { index: usize },
    Duplicate { first: usize, later: usize },
}

/// A registerable key must be a virtual key, rather than a modifier or WinForms flags value.
pub fn complete_hotkey(h: &Hotkey) -> bool {
    (1..=254).contains(&h.vk) && !matches!(h.vk, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5) && h.modifiers() != 0
}

/// Required gate, then optional Game, Pause and Side chords. Entirely cleared optional rows are valid.
pub fn shortcut_issue(settings: &crate::Settings) -> Option<ShortcutIssue> {
    shortcut_issue_with_fixes(settings, fence_engine::fixes::Fixes::ALL)
}

pub fn shortcut_issue_with_fixes(
    settings: &crate::Settings,
    fixes: fence_engine::fixes::Fixes,
) -> Option<ShortcutIssue> {
    let chords = [
        settings.hotkey(),
        settings.confine_hotkey(),
        settings.pause_hotkey(),
        settings.side_hotkey(),
    ];
    if fixes.j16_shortcut_validation {
        let mut all = chords.to_vec();
        all.push(settings.mark_accident_hotkey());
        shortcut_issue_for(&all)
    } else {
        if chords[0].vk == 0 || chords[0].modifiers() == 0 {
            return Some(ShortcutIssue::Incomplete { index: 0 });
        }
        for later in 1..4 {
            for first in 0..later {
                if chords[later].vk != 0 && chords[later] == chords[first] {
                    return Some(ShortcutIssue::Duplicate { first, later });
                }
            }
        }
        None
    }
}

fn shortcut_issue_for(chords: &[Hotkey]) -> Option<ShortcutIssue> {
    for (index, h) in chords.iter().enumerate() {
        if !(index > 0 && h.vk == 0 && h.modifiers() == 0) && !complete_hotkey(h) {
            return Some(ShortcutIssue::Incomplete { index });
        }
    }
    for later in 1..chords.len() {
        for first in 0..later {
            if chords[later].vk != 0 && chords[later] == chords[first] {
                return Some(ShortcutIssue::Duplicate { first, later });
            }
        }
    }
    None
}

/// WinForms `Keys` values used by the defaults.
pub const NONE: u32 = 0;
pub const UP: u32 = 38;
pub const RIGHT: u32 = 39;
pub const G: u32 = 71;
pub const P: u32 = 80;

/// `Enum.GetNames(typeof(Keys))` with the values as unsigned 32-bit, in .NET's order (by unsigned value;
/// equal values in .NET's own order).
#[rustfmt::skip]
pub const KEYS: &[(&str, u32)] = &[
    ("None", 0x0), ("LButton", 0x1), ("RButton", 0x2), ("Cancel", 0x3), ("MButton", 0x4), ("XButton1", 0x5),
    ("XButton2", 0x6), ("Back", 0x8), ("Tab", 0x9), ("LineFeed", 0xA), ("Clear", 0xC), ("Return", 0xD),
    ("Enter", 0xD), ("ShiftKey", 0x10), ("ControlKey", 0x11), ("Menu", 0x12), ("Pause", 0x13), ("CapsLock", 0x14),
    ("Capital", 0x14), ("HangulMode", 0x15), ("HanguelMode", 0x15), ("KanaMode", 0x15), ("JunjaMode", 0x17),
    ("FinalMode", 0x18), ("HanjaMode", 0x19), ("KanjiMode", 0x19), ("Escape", 0x1B), ("IMEConvert", 0x1C),
    ("IMENonconvert", 0x1D), ("IMEAccept", 0x1E), ("IMEAceept", 0x1E), ("IMEModeChange", 0x1F), ("Space", 0x20),
    ("Prior", 0x21), ("PageUp", 0x21), ("PageDown", 0x22), ("Next", 0x22), ("End", 0x23), ("Home", 0x24),
    ("Left", 0x25), ("Up", 0x26), ("Right", 0x27), ("Down", 0x28), ("Select", 0x29), ("Print", 0x2A),
    ("Execute", 0x2B), ("Snapshot", 0x2C), ("PrintScreen", 0x2C), ("Insert", 0x2D), ("Delete", 0x2E),
    ("Help", 0x2F), ("D0", 0x30), ("D1", 0x31), ("D2", 0x32), ("D3", 0x33), ("D4", 0x34), ("D5", 0x35),
    ("D6", 0x36), ("D7", 0x37), ("D8", 0x38), ("D9", 0x39), ("A", 0x41), ("B", 0x42), ("C", 0x43), ("D", 0x44),
    ("E", 0x45), ("F", 0x46), ("G", 0x47), ("H", 0x48), ("I", 0x49), ("J", 0x4A), ("K", 0x4B), ("L", 0x4C),
    ("M", 0x4D), ("N", 0x4E), ("O", 0x4F), ("P", 0x50), ("Q", 0x51), ("R", 0x52), ("S", 0x53), ("T", 0x54),
    ("U", 0x55), ("V", 0x56), ("W", 0x57), ("X", 0x58), ("Y", 0x59), ("Z", 0x5A), ("LWin", 0x5B), ("RWin", 0x5C),
    ("Apps", 0x5D), ("Sleep", 0x5F), ("NumPad0", 0x60), ("NumPad1", 0x61), ("NumPad2", 0x62), ("NumPad3", 0x63),
    ("NumPad4", 0x64), ("NumPad5", 0x65), ("NumPad6", 0x66), ("NumPad7", 0x67), ("NumPad8", 0x68),
    ("NumPad9", 0x69), ("Multiply", 0x6A), ("Add", 0x6B), ("Separator", 0x6C), ("Subtract", 0x6D),
    ("Decimal", 0x6E), ("Divide", 0x6F), ("F1", 0x70), ("F2", 0x71), ("F3", 0x72), ("F4", 0x73), ("F5", 0x74),
    ("F6", 0x75), ("F7", 0x76), ("F8", 0x77), ("F9", 0x78), ("F10", 0x79), ("F11", 0x7A), ("F12", 0x7B),
    ("F13", 0x7C), ("F14", 0x7D), ("F15", 0x7E), ("F16", 0x7F), ("F17", 0x80), ("F18", 0x81), ("F19", 0x82),
    ("F20", 0x83), ("F21", 0x84), ("F22", 0x85), ("F23", 0x86), ("F24", 0x87), ("NumLock", 0x90), ("Scroll", 0x91),
    ("LShiftKey", 0xA0), ("RShiftKey", 0xA1), ("LControlKey", 0xA2), ("RControlKey", 0xA3), ("LMenu", 0xA4),
    ("RMenu", 0xA5), ("BrowserBack", 0xA6), ("BrowserForward", 0xA7), ("BrowserRefresh", 0xA8),
    ("BrowserStop", 0xA9), ("BrowserSearch", 0xAA), ("BrowserFavorites", 0xAB), ("BrowserHome", 0xAC),
    ("VolumeMute", 0xAD), ("VolumeDown", 0xAE), ("VolumeUp", 0xAF), ("MediaNextTrack", 0xB0),
    ("MediaPreviousTrack", 0xB1), ("MediaStop", 0xB2), ("MediaPlayPause", 0xB3), ("LaunchMail", 0xB4),
    ("SelectMedia", 0xB5), ("LaunchApplication1", 0xB6), ("LaunchApplication2", 0xB7), ("Oem1", 0xBA),
    ("OemSemicolon", 0xBA), ("Oemplus", 0xBB), ("Oemcomma", 0xBC), ("OemMinus", 0xBD), ("OemPeriod", 0xBE),
    ("OemQuestion", 0xBF), ("Oem2", 0xBF), ("Oem3", 0xC0), ("Oemtilde", 0xC0), ("OemOpenBrackets", 0xDB),
    ("Oem4", 0xDB), ("Oem5", 0xDC), ("OemPipe", 0xDC), ("OemCloseBrackets", 0xDD), ("Oem6", 0xDD), ("Oem7", 0xDE),
    ("OemQuotes", 0xDE), ("Oem8", 0xDF), ("OemBackslash", 0xE2), ("Oem102", 0xE2), ("ProcessKey", 0xE5),
    ("Packet", 0xE7), ("Attn", 0xF6), ("Crsel", 0xF7), ("Exsel", 0xF8), ("EraseEof", 0xF9), ("Play", 0xFA),
    ("Zoom", 0xFB), ("NoName", 0xFC), ("Pa1", 0xFD), ("OemClear", 0xFE), ("KeyCode", 0xFFFF), ("Shift", 0x10000),
    ("Control", 0x20000), ("Alt", 0x40000), ("Modifiers", 0xFFFF0000),
];

/// C# `((Keys)value).ToString()` (.NET flags formatting, see the module text). `value` is the WinForms
/// `Keys` integer as stored in the settings file.
pub fn keys_to_string(value: i32) -> String {
    let v = value as u32;
    if v == 0 {
        return KEYS[0].0.to_string();
    }
    // GetSingleFlagsEnumNameForValue: walk from the largest entry to the first one <= value.
    let mut index = KEYS.len();
    for i in (0..KEYS.len()).rev() {
        if KEYS[i].1 <= v {
            if KEYS[i].1 == v {
                return KEYS[i].0.to_string();
            }
            index = i;
            break;
        }
    }
    if index == KEYS.len() {
        return value.to_string();
    }
    // TryFindFlagsNames: greedy from there down; leftover bits mean "print the number".
    let mut rest = v;
    let mut found: Vec<usize> = Vec::new();
    let mut i = index as isize;
    while i >= 0 {
        let cur = KEYS[i as usize].1;
        if i == 0 && cur == 0 {
            break;
        }
        if rest & cur == cur {
            rest &= !cur;
            found.push(i as usize);
            if rest == 0 {
                break;
            }
        }
        i -= 1;
    }
    if rest != 0 {
        return value.to_string();
    }
    found.iter().rev().map(|&i| KEYS[i].0).collect::<Vec<_>>().join(", ")
}

/// C# `HotKeyText()` and siblings: "Ctrl+Alt+Up", or "(none)" when nothing is set. Key names are the
/// WinForms `Keys.ToString()` names ([`keys_to_string`]); `vk` holds the `Keys` integer bit for bit.
pub fn hotkey_text(hotkey: &Hotkey) -> String {
    let mut parts: Vec<String> = Vec::new();
    if hotkey.ctrl {
        parts.push("Ctrl".to_string());
    }
    if hotkey.alt {
        parts.push("Alt".to_string());
    }
    if hotkey.shift {
        parts.push("Shift".to_string());
    }
    if hotkey.win {
        parts.push("Win".to_string());
    }
    if hotkey.vk != 0 {
        parts.push(keys_to_string(hotkey.vk as i32));
    }
    if parts.is_empty() {
        "(none)".to_string()
    } else {
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f05_shortcuts_require_complete_distinct_chords_and_allow_cleared_optional_rows() {
        let mut s = crate::Settings::default();
        assert_eq!(shortcut_issue(&s), None);
        for vk in [
            0, 0x10, 0x11, 0x12, 0x5B, 0x5C, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 255, 0x10087,
        ] {
            assert!(
                !complete_hotkey(&Hotkey {
                    vk,
                    ctrl: true,
                    ..Hotkey::default()
                }),
                "invalid virtual key {vk:#x} with a real modifier"
            );
        }
        assert!(complete_hotkey(&Hotkey {
            vk: 0x87,
            ctrl: true,
            ..Hotkey::default()
        }));
        for index in 0..4 {
            for vk in [0, 0x87, 0x10, 0xA3, 0x5B, 255, 0x10087] {
                let mut chords = [s.hotkey(), s.confine_hotkey(), s.pause_hotkey(), s.side_hotkey()];
                chords[index] = Hotkey {
                    vk,
                    ..Hotkey::default()
                };
                let issue = shortcut_issue_for(&chords);
                assert_eq!(
                    issue,
                    if index > 0 && vk == 0 {
                        None
                    } else {
                        Some(ShortcutIssue::Incomplete { index })
                    }
                );
            }
        }
        s.hot_key = 0x87;
        s.confine_hot_key = 0x87;
        s.confine_mod_ctrl = s.mod_ctrl;
        s.confine_mod_alt = s.mod_alt;
        assert_eq!(
            shortcut_issue(&s),
            Some(ShortcutIssue::Duplicate { first: 0, later: 1 })
        );
        s.side_hot_key = 0x87;
        s.side_mod_ctrl = false;
        s.side_mod_alt = false;
        s.side_mod_shift = false;
        s.side_mod_win = false;
        assert_eq!(
            shortcut_issue(&s),
            Some(ShortcutIssue::Incomplete { index: 3 }),
            "incomplete rows are checked before duplicate pairs"
        );
    }

    #[test]
    fn s_config_keys_names_follow_dotnet_flags_formatting() {
        // Expected values are the reference's answers (keys_text verb, 2026-10-13 probe).
        let cases: &[(i32, &str)] = &[
            (0, "None"),
            (13, "Enter"),
            (13 | 0x10000, "Enter, Shift"),
            (38 | 0x10000 | 0x20000, "Up, Shift, Control"),
            (0x70000, "Shift, Control, Alt"),
            (0xFFFF, "KeyCode"),
            (-65536, "Modifiers"),
            (-5, "-5"),
            (-1, "KeyCode, Modifiers"),
            (256, "256"),
            (0x1FFFF, "KeyCode, Shift"),
            (65601, "A, Shift"),
            (i32::MAX, "2147483647"),
            (i32::MIN, "-2147483648"),
            (0x30, "D0"),
            (21 | 0x10000, "KanaMode, Shift"),
            (34 | 0x10000, "Next, Shift"),
            (33 | 0x10000, "PageUp, Shift"),
            (30 | 0x10000, "IMEAceept, Shift"),
        ];
        for &(v, want) in cases {
            assert_eq!(keys_to_string(v), want, "value {v}");
        }
    }

    #[test]
    fn s_config_hotkey_text_matches_csharp() {
        let h = Hotkey {
            vk: 0x26,
            ctrl: true,
            alt: true,
            shift: false,
            win: false,
        };
        assert_eq!(hotkey_text(&h), "Ctrl+Alt+Up");
        assert_eq!(hotkey_text(&Hotkey::default()), "(none)");
        assert_eq!(
            hotkey_text(&Hotkey {
                win: true,
                ..Default::default()
            }),
            "Win"
        );
        assert_eq!(
            hotkey_text(&Hotkey {
                vk: 65601,
                shift: true,
                ..Default::default()
            }),
            "Shift+A, Shift"
        );
    }
}
