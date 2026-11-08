//! Live monitor enumeration (port of C# `MonitorInfo.All`, Core/MonitorInfo.cs:209-306). Read-only:
//! EnumDisplayMonitors, GetMonitorInfo and EnumDisplayDevices never change the display configuration.
//!
//! Numbering and key disambiguation are the pure Track B helpers in `fence_engine::layout`; this module only
//! feeds them, in the C# order: build, `sort_and_number`, then `disambiguate_keys` with the raw identity key
//! `stable + "\n" + device`.

use fence_engine::layout::{self, KeyedMonitor};
use fence_types::{MonitorInfo, Rect};
use windows_sys::Win32::Foundation::{LPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    MONITORINFOEXW,
};
use windows_sys::core::BOOL;

use crate::wide::{from_wide, to_wide};

/// `MONITORINFOF_PRIMARY`.
const MONITORINFOF_PRIMARY: u32 = 0x1;
/// `EDD_GET_DEVICE_INTERFACE_NAME`: ask for the per-port device interface path in DeviceID.
const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 0x1;

/// One monitor as Windows reports it, before numbering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawMonitor {
    pub device: String,
    /// The raw stable id (interface path, else instance id, else the GDI name).
    pub stable_raw: String,
    pub instance_raw: Option<String>,
    pub bounds: Rect,
    pub primary: bool,
}

/// C# `MonitorInfo.All()`.
pub fn enumerate() -> Vec<MonitorInfo> {
    assemble(enumerate_raw())
}

/// The pure half of `All()`: number with `sort_and_number`, then give shared raw keys distinct suffixes.
pub fn assemble(raw: Vec<RawMonitor>) -> Vec<MonitorInfo> {
    use fence_engine::fixes::Fixes;
    use std::sync::OnceLock;
    static SESSION: OnceLock<String> = OnceLock::new();
    let mut raw = raw;
    raw.sort_by(|a, b| {
        a.bounds
            .l
            .cmp(&b.bounds.l)
            .then(a.bounds.t.cmp(&b.bounds.t))
            .then_with(|| layout::ordinal_ignore_case_cmp(&a.stable_raw, &b.stable_raw))
            .then_with(|| layout::ordinal_cmp(&a.stable_raw, &b.stable_raw))
            .then_with(|| layout::ordinal_ignore_case_cmp(&a.device, &b.device))
            .then_with(|| layout::ordinal_cmp(&a.device, &b.device))
            .then(a.bounds.r.cmp(&b.bounds.r))
            .then(a.bounds.b.cmp(&b.bounds.b))
    });
    let session = SESSION.get_or_init(|| format!("{}-{:?}", std::process::id(), std::time::SystemTime::now()));
    let keyed: Vec<KeyedMonitor> = raw
        .iter()
        .map(|r| {
            let instance = r
                .instance_raw
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .filter(|s| layout::canonical_native_identity(s) != layout::canonical_native_identity(&r.stable_raw))
                .map(layout::canonical_native_identity)
                .unwrap_or_else(|| r.stable_raw.clone());
            KeyedMonitor {
                key: r.stable_raw.clone(),
                left: r.bounds.l,
                top: r.bounds.t,
                raw_identity_key: instance,
            }
        })
        .collect();
    let keys = layout::disambiguate_keys_with(&keyed, Fixes::ALL, session);
    let mut list: Vec<MonitorInfo> = raw
        .into_iter()
        .zip(keys)
        .map(|(r, key)| MonitorInfo {
            device: r.device,
            stable_id: key,
            bounds: r.bounds,
            primary: r.primary,
            index: 0,
        })
        .collect();
    layout::sort_and_number(&mut list);
    list
}

/// The 4-tuples C# hands to `GuardCore.DisambiguateKeys` (MonitorInfo.cs:275-277).
pub fn keyed(list: &[MonitorInfo]) -> Vec<KeyedMonitor> {
    list.iter()
        .map(|m| KeyedMonitor {
            key: m.stable_id.clone(),
            left: m.bounds.l,
            top: m.bounds.t,
            raw_identity_key: format!("{}\n{}", m.stable_id, m.device),
        })
        .collect()
}

/// C# `StableIdFor` choice: the interface path, else the instance id, else the GDI name (never empty-keyed).
pub fn choose_stable_id(interface_path: Option<String>, instance_id: Option<String>, device: &str) -> String {
    interface_path
        .filter(|s| !s.is_empty())
        .or_else(|| instance_id.filter(|s| !s.is_empty()))
        .unwrap_or_else(|| device.to_string())
}

unsafe extern "system" fn monitor_proc(h: HMONITOR, _hdc: HDC, _rc: *mut RECT, data: LPARAM) -> BOOL {
    // SAFETY: `data` is the &mut Vec passed by `enumerate_raw`, alive for the whole enumeration.
    let list = unsafe { &mut *(data as *mut Vec<RawMonitor>) };
    // SAFETY: MONITORINFOEXW is plain data; zero is valid.
    let mut mi: MONITORINFOEXW = unsafe { std::mem::zeroed() };
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    // SAFETY: MONITORINFOEXW starts with MONITORINFO and cbSize names the extended size.
    if unsafe { GetMonitorInfoW(h, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO) } != 0 {
        let device = from_wide(&mi.szDevice);
        let r = mi.monitorInfo.rcMonitor;
        list.push(RawMonitor {
            stable_raw: stable_id_for(&device),
            instance_raw: device_id(&device, 0),
            device,
            bounds: Rect::new(r.left, r.top, r.right, r.bottom),
            primary: mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        });
    }
    1
}

fn enumerate_raw() -> Vec<RawMonitor> {
    let mut list: Vec<RawMonitor> = Vec::new();
    // SAFETY: the callback only writes into `list`, which outlives the call.
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(monitor_proc),
            &mut list as *mut Vec<RawMonitor> as LPARAM,
        );
    }
    list
}

/// DeviceID of the monitor child of a GDI display (`iDevNum` 0), read-only.
fn device_id(device: &str, flags: u32) -> Option<String> {
    let name = to_wide(device);
    // SAFETY: DISPLAY_DEVICEW is plain data; zero is valid.
    let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
    dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
    // SAFETY: name is NUL-terminated; dd is a valid out-pointer with cb set.
    if unsafe { EnumDisplayDevicesW(name.as_ptr(), 0, &mut dd, flags) } != 0 {
        Some(from_wide(&dd.DeviceID))
    } else {
        None
    }
}

fn stable_id_for(device: &str) -> String {
    let iface = device_id(device, EDD_GET_DEVICE_INTERFACE_NAME);
    if iface.as_deref().is_some_and(|s| !s.is_empty()) {
        return choose_stable_id(iface, None, device);
    }
    choose_stable_id(None, device_id(device, 0), device)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j11_native_identity_survives_unplug_renumber_and_ordinary_interface_keys() {
        let raw = |device: &str, key: &str, instance: &str, left| RawMonitor {
            device: device.into(),
            stable_raw: key.into(),
            instance_raw: Some(instance.into()),
            bounds: Rect::new(left, 0, left + 100, 100),
            primary: left == 0,
        };
        let pair = assemble(vec![
            raw("DISPLAY1", "SYN-SHARED", r"MONITOR\SYN\A", 0),
            raw("DISPLAY2", "SYN-SHARED", r"MONITOR\SYN\B", 100),
        ]);
        let alone = assemble(vec![raw("DISPLAY9", "SYN-SHARED", r"MONITOR\SYN\B", -100)]);
        assert_eq!(pair[1].stable_id, alone[0].stable_id);
        let iface = r"\\?\DISPLAY#SYN#INSTANCE#{class-guid}";
        let ordinary = assemble(vec![raw("DISPLAY1", iface, r"MONITOR\SYN\INSTANCE", 0)]);
        assert_eq!(ordinary[0].stable_id, iface);
        let input = vec![
            raw("DISPLAY1", "SYN-SHARED", "SYN-SHARED", 0),
            raw("DISPLAY2", "SYN-SHARED", "SYN-SHARED", 100),
        ];
        let first = assemble(input.clone());
        let mut reverse = input;
        reverse.reverse();
        let refresh = assemble(reverse);
        assert_eq!(first, refresh);
        assert!(first.iter().all(|m| m.stable_id.contains("#ambiguous:")));
    }

    #[test]
    fn s_win_stable_id_prefers_interface_then_instance_then_gdi_name() {
        let gdi = r"\\.\DISPLAY1";
        assert_eq!(
            choose_stable_id(Some("IFACE#1".into()), Some("MONITOR\\SYN1000".into()), gdi),
            "IFACE#1"
        );
        assert_eq!(
            choose_stable_id(Some(String::new()), Some("MONITOR\\SYN1000".into()), gdi),
            "MONITOR\\SYN1000"
        );
        assert_eq!(choose_stable_id(None, Some(String::new()), gdi), gdi);
        assert_eq!(choose_stable_id(None, None, gdi), gdi);
    }

    #[test]
    fn s_win_monitors_feed_disambiguation_with_the_csharp_raw_identity_key() {
        let raw = vec![
            RawMonitor {
                device: r"\\.\DISPLAY2".into(),
                stable_raw: "SYN1000".into(),
                instance_raw: Some("MONITOR\\SYN\\PORT-B".into()),
                bounds: Rect::new(1920, 0, 3840, 1080),
                primary: false,
            },
            RawMonitor {
                device: r"\\.\DISPLAY1".into(),
                stable_raw: "SYN1000".into(),
                instance_raw: Some("MONITOR\\SYN\\PORT-A".into()),
                bounds: Rect::new(0, 0, 1920, 1080),
                primary: true,
            },
        ];
        let list = assemble(raw);
        assert_eq!(list.len(), 2);
        let k = keyed(&list);
        for (m, km) in list.iter().zip(&k) {
            assert_eq!(km.left, m.bounds.l);
            assert_eq!(km.top, m.bounds.t);
            assert!(km.raw_identity_key.ends_with(&format!("\n{}", m.device)));
        }
        assert_eq!(list.iter().filter(|m| m.primary).count(), 1);
    }

    #[test]
    fn s_win_live_enumeration_is_read_only_and_sane() {
        // Read-only query of the live displays. Prints nothing: his desk layout stays private (L16).
        let list = enumerate();
        assert!(!list.is_empty(), "at least one monitor");
        assert!(list.iter().all(|m| m.bounds.is_valid()), "every monitor has an area");
        assert!(list.iter().all(|m| !m.stable_id.is_empty() && !m.device.is_empty()));
        assert!(list.iter().filter(|m| m.primary).count() <= 1, "at most one primary");
    }
}
