//! The three tray icons, embedded at build time (C# `LoadTrayIcon`, Ui/TrayApplicationContext.cs:454-464):
//! closed, open and paused, with exact DPI frames and light/dark taskbar variants.

use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIconFromResourceEx, DestroyIcon, HICON, LR_DEFAULTCOLOR};

pub const TRAY_CLOSED_ICO: &[u8] = include_bytes!("../../../assets/tray-closed.ico");
pub const TRAY_OPEN_ICO: &[u8] = include_bytes!("../../../assets/tray-open.ico");
pub const TRAY_PAUSED_ICO: &[u8] = include_bytes!("../../../assets/tray-paused.ico");

/// The frame sizes every tray icon carries.
pub const FRAME_SIZES: [u32; 7] = [16, 20, 24, 32, 40, 48, 64];

/// One image of an .ico directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IcoFrame {
    pub width: u32,
    pub height: u32,
    pub bit_count: u16,
    pub offset: usize,
    pub size: usize,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Parse an .ico directory (ICONDIR + ICONDIRENTRY[]); every frame must lie inside the file.
pub fn parse_ico(bytes: &[u8]) -> Result<Vec<IcoFrame>, String> {
    let reserved = u16_at(bytes, 0).ok_or("short header")?;
    let kind = u16_at(bytes, 2).ok_or("short header")?;
    let count = u16_at(bytes, 4).ok_or("short header")? as usize;
    if reserved != 0 || kind != 1 {
        return Err(format!("not an icon (reserved {reserved}, type {kind})"));
    }
    let mut frames = Vec::with_capacity(count);
    for i in 0..count {
        let e = 6 + 16 * i;
        let w = *bytes.get(e).ok_or("short directory")?;
        let h = *bytes.get(e + 1).ok_or("short directory")?;
        let bit_count = u16_at(bytes, e + 6).ok_or("short directory")?;
        let size = u32_at(bytes, e + 8).ok_or("short directory")? as usize;
        let offset = u32_at(bytes, e + 12).ok_or("short directory")? as usize;
        if offset.checked_add(size).is_none_or(|end| end > bytes.len()) {
            return Err(format!("frame {i} lies outside the file"));
        }
        frames.push(IcoFrame {
            width: if w == 0 { 256 } else { w as u32 },
            height: if h == 0 { 256 } else { h as u32 },
            bit_count,
            offset,
            size,
        });
    }
    Ok(frames)
}

/// The frame for a `desired` px square: exact size, else the smallest larger one, else the largest.
pub fn pick_frame(frames: &[IcoFrame], desired: u32) -> Option<IcoFrame> {
    frames
        .iter()
        .filter(|f| f.width >= desired)
        .min_by_key(|f| (f.width, std::cmp::Reverse(f.bit_count)))
        .or_else(|| frames.iter().max_by_key(|f| (f.width, f.bit_count)))
        .copied()
}

/// An owned HICON, destroyed on drop.
#[derive(Debug)]
pub struct Icon(usize);

impl Icon {
    pub fn handle(&self) -> HICON {
        self.0 as HICON
    }
}

impl Drop for Icon {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateIconFromResourceEx and is destroyed once.
        unsafe { DestroyIcon(self.0 as HICON) };
    }
}

/// Make an HICON of `size` px from an embedded .ico (CreateIconFromResourceEx; PNG frames are supported).
pub fn create_icon(ico: &[u8], size: u32) -> Option<Icon> {
    let frames = parse_ico(ico).ok()?;
    let f = pick_frame(&frames, size)?;
    let data = &ico[f.offset..f.offset + f.size];
    // SAFETY: `data` is one complete icon image; the call copies it.
    let h = unsafe {
        CreateIconFromResourceEx(
            data.as_ptr(),
            data.len() as u32,
            1,
            0x0003_0000,
            size as i32,
            size as i32,
            LR_DEFAULTCOLOR,
        )
    };
    (!h.is_null()).then(|| Icon(h as usize))
}

/// Taskbar palette is independent of the application/settings theme.
pub fn taskbar_icons() -> [&'static [u8]; 3] {
    if crate::system::read_hkcu_dword(
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "SystemUsesLightTheme",
    ) == Some(1)
    {
        [
            include_bytes!("../../../assets/tray-closed-light.ico"),
            include_bytes!("../../../assets/tray-open-light.ico"),
            include_bytes!("../../../assets/tray-paused-light.ico"),
        ]
    } else {
        [TRAY_CLOSED_ICO, TRAY_OPEN_ICO, TRAY_PAUSED_ICO]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(&str, &[u8]); 3] = [
        ("closed", TRAY_CLOSED_ICO),
        ("open", TRAY_OPEN_ICO),
        ("paused", TRAY_PAUSED_ICO),
    ];

    #[test]
    fn b016_three_embedded_icons_carry_all_seven_frames() {
        for (name, ico) in ALL {
            let frames = parse_ico(ico).unwrap_or_else(|e| panic!("{name}: {e}"));
            let sizes: Vec<u32> = frames.iter().map(|f| f.width).collect();
            assert_eq!(sizes, FRAME_SIZES, "{name}: 16/20/24/32/40/48/64");
            assert!(frames.iter().all(|f| f.width == f.height), "{name}: square frames");
        }
        assert_ne!(TRAY_CLOSED_ICO, TRAY_OPEN_ICO);
        assert_ne!(TRAY_OPEN_ICO, TRAY_PAUSED_ICO);
    }

    #[test]
    fn b016_frame_choice_is_exact_then_next_larger_then_largest() {
        let frames = parse_ico(TRAY_CLOSED_ICO).unwrap();
        for s in FRAME_SIZES {
            assert_eq!(pick_frame(&frames, s).unwrap().width, s);
        }
        assert_eq!(pick_frame(&frames, 18).unwrap().width, 20);
        assert_eq!(pick_frame(&frames, 30).unwrap().width, 32);
        assert_eq!(pick_frame(&frames, 128).unwrap().width, 64);
        assert_eq!(pick_frame(&[], 16), None);
    }

    #[test]
    fn b016_each_icon_becomes_a_windows_icon_at_tray_sizes() {
        // CreateIconFromResourceEx only builds an icon object in memory; nothing is shown.
        for (name, ico) in ALL {
            for size in [16, 20, 24, 32] {
                let icon = create_icon(ico, size).unwrap_or_else(|| panic!("{name} at {size}px"));
                assert!(!icon.handle().is_null());
            }
        }
    }

    #[test]
    fn s_win_ico_parser_rejects_broken_files() {
        assert!(parse_ico(&[]).is_err());
        assert!(parse_ico(&[0, 0, 2, 0, 0, 0]).is_err(), "type 2 is a cursor");
        let mut bad = TRAY_CLOSED_ICO[..22].to_vec();
        bad[4] = 1;
        assert!(parse_ico(&bad).is_err(), "frame outside the file");
    }
}
