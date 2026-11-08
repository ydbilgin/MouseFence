//! Inspect the shipped PE without launching it or touching application state.
#![cfg(windows)]

use std::{ffi::c_void, fs, os::windows::ffi::OsStrExt, path::Path};

#[link(name = "version")]
unsafe extern "system" {
    fn GetFileVersionInfoSizeW(filename: *const u16, handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(filename: *const u16, handle: u32, len: u32, data: *mut c_void) -> i32;
    fn VerQueryValueW(data: *const c_void, key: *const u16, value: *mut *mut c_void, len: *mut u32) -> i32;
}

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}

fn dword(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

struct Pe<'a> {
    bytes: &'a [u8],
    optional: usize,
    sections: usize,
    count: usize,
}

impl<'a> Pe<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        assert_eq!(&bytes[..2], b"MZ");
        let header = dword(bytes, 0x3c) as usize;
        assert_eq!(&bytes[header..header + 4], b"PE\0\0");
        assert_eq!(word(bytes, header + 4), 0x8664, "x64 PE");
        let optional = header + 24;
        assert_eq!(word(bytes, optional), 0x20b, "PE32+");
        Self {
            bytes,
            optional,
            sections: optional + word(bytes, header + 20) as usize,
            count: word(bytes, header + 6) as usize,
        }
    }

    fn directory(&self, index: usize) -> u32 {
        dword(self.bytes, self.optional + 112 + index * 8)
    }

    fn offset(&self, rva: u32) -> usize {
        for index in 0..self.count {
            let section = self.sections + 40 * index;
            let start = dword(self.bytes, section + 12);
            let size = dword(self.bytes, section + 8).max(dword(self.bytes, section + 16));
            if rva >= start && rva - start < size {
                return (dword(self.bytes, section + 20) + rva - start) as usize;
            }
        }
        panic!("unmapped RVA {rva:#x}");
    }

    fn resource_entry(&self, id: u32) -> Result<usize, String> {
        if self.directory(2) == 0 {
            return Err("resource directory missing".into());
        }
        let base = self.offset(self.directory(2));
        let count = word(self.bytes, base + 12) + word(self.bytes, base + 14);
        (0..count as usize)
            .map(|i| base + 16 + i * 8)
            .find(|&entry| dword(self.bytes, entry) == id)
            .ok_or_else(|| format!("resource type {id} missing"))
    }

    fn resource(&self, id: u32) -> Result<&[u8], String> {
        self.resource_named(id, None)
    }

    fn resource_named(&self, id: u32, name: Option<u32>) -> Result<&[u8], String> {
        let base = self.offset(self.directory(2));
        let mut entry = self.resource_entry(id)?;
        let mut select_name = name;
        loop {
            let target = dword(self.bytes, entry + 4);
            if target & 0x8000_0000 == 0 {
                let data = base + target as usize;
                let start = self.offset(dword(self.bytes, data));
                let len = dword(self.bytes, data + 4) as usize;
                return Ok(&self.bytes[start..start + len]);
            }
            let directory = base + (target & 0x7fff_ffff) as usize;
            entry = directory + 16;
            if let Some(name) = select_name.take() {
                let count = word(self.bytes, directory + 12) + word(self.bytes, directory + 14);
                entry = (0..count as usize)
                    .map(|i| directory + 16 + i * 8)
                    .find(|&candidate| dword(self.bytes, candidate) == name)
                    .ok_or_else(|| format!("resource {id}/{name} missing"))?;
            }
        }
    }
}

fn version_string(data: &[u8], name: &str) -> Result<String, String> {
    let key: Vec<u16> = format!("\\StringFileInfo\\040904b0\\{name}\0").encode_utf16().collect();
    let mut value = std::ptr::null_mut();
    let mut len = 0;
    // SAFETY: the version buffer and terminated query stay alive; Windows returns a view into data.
    if unsafe { VerQueryValueW(data.as_ptr().cast(), key.as_ptr(), &mut value, &mut len) } == 0 || len == 0 {
        return Err(format!("version string {name} missing"));
    }
    // SAFETY: VerQueryValueW reports a UTF-16 string of len code units within data.
    let chars = unsafe { std::slice::from_raw_parts(value.cast::<u16>(), len as usize) };
    Ok(String::from_utf16_lossy(chars).trim_end_matches('\0').to_owned())
}

fn validate(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let pe = Pe::new(&bytes);
    for id in [3, 14, 16, 24] {
        pe.resource_entry(id)?;
    }
    let ico = include_bytes!("../../../assets/app.ico");
    let group = pe.resource(14)?;
    if word(group, 4) != word(ico, 4) {
        return Err("application icon frame count changed".into());
    }
    for index in 0..word(ico, 4) as usize {
        let source = 6 + 16 * index;
        let embedded = 6 + 14 * index;
        // rc.exe normalizes the PNG icon's planes from zero to one; its image bytes stay identical.
        if group[embedded..embedded + 4] != ico[source..source + 4]
            || group[embedded + 6..embedded + 12] != ico[source + 6..source + 12]
            || word(group, embedded + 4) != 1
        {
            return Err(format!("application icon frame {index} metadata changed"));
        }
        let image = word(group, embedded + 12) as u32;
        let start = dword(ico, source + 12) as usize;
        let len = dword(ico, source + 8) as usize;
        if pe.resource_named(3, Some(image))? != &ico[start..start + len] {
            return Err(format!("application icon frame {index} bytes changed"));
        }
    }
    let manifest = String::from_utf8_lossy(pe.resource(24)?);
    for required in ["asInvoker", "PerMonitorV2", "8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a"] {
        if !manifest.contains(required) {
            return Err(format!("manifest missing {required}"));
        }
    }
    if pe.directory(14) != 0 {
        return Err("CLR runtime header present".into());
    }
    let mut import = pe.offset(pe.directory(1));
    while dword(&bytes, import + 12) != 0 {
        let name = pe.offset(dword(&bytes, import + 12));
        let end = bytes[name..].iter().position(|&b| b == 0).unwrap() + name;
        let dll = String::from_utf8_lossy(&bytes[name..end]).to_ascii_lowercase();
        if ["vcruntime", "msvcp", "msvcr", "ucrtbase", "api-ms-win-crt", "mscoree"]
            .iter()
            .any(|prefix| dll.starts_with(prefix))
        {
            return Err(format!("shared runtime import: {dll}"));
        }
        import += 20;
    }
    let filename: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut handle = 0;
    // SAFETY: filename is terminated and handle is writable; this reads a file without loading code.
    let size = unsafe { GetFileVersionInfoSizeW(filename.as_ptr(), &mut handle) };
    if size == 0 {
        return Err("GetFileVersionInfoSizeW found no version".into());
    }
    let mut data = vec![0_u8; size as usize];
    // SAFETY: the output buffer has the exact size requested by Windows.
    if unsafe { GetFileVersionInfoW(filename.as_ptr(), 0, size, data.as_mut_ptr().cast()) } == 0 {
        return Err("GetFileVersionInfoW failed".into());
    }
    for (key, expected) in [
        ("FileVersion", "2.0.1.0"),
        ("ProductVersion", "2.0.1.0"),
        ("ProductName", "MouseFence"),
        (
            "FileDescription",
            "MouseFence - keeps the cursor on the screen you meant",
        ),
        ("LegalCopyright", "MIT"),
        ("OriginalFilename", "MouseFence.exe"),
    ] {
        let actual = version_string(&data, key)?;
        if actual != expected {
            return Err(format!("{key}: expected {expected}, found {actual}"));
        }
    }
    let root: Vec<u16> = "\\\0".encode_utf16().collect();
    let mut fixed = std::ptr::null_mut();
    let mut length = 0;
    // SAFETY: the version buffer and root query stay alive for the call.
    if unsafe { VerQueryValueW(data.as_ptr().cast(), root.as_ptr(), &mut fixed, &mut length) } == 0 || length < 24 {
        return Err("fixed version missing".into());
    }
    // SAFETY: the fixed version is at least 24 bytes, as checked above.
    let fixed = unsafe { std::slice::from_raw_parts(fixed.cast::<u8>(), length as usize) };
    if [dword(fixed, 8), dword(fixed, 12), dword(fixed, 16), dword(fixed, 20)]
        != [0x0002_0000, 0x0001_0000, 0x0002_0000, 0x0001_0000]
    {
        return Err("fixed file/product version must be 2.0.1.0".into());
    }
    Ok(())
}

fn built_exe() -> std::path::PathBuf {
    std::env::var_os("MF_BUILT_EXE").map_or_else(|| env!("CARGO_BIN_EXE_MouseFence").into(), Into::into)
}

#[test]
fn f15_built_exe_has_version_icon_manifest_and_no_shared_runtime() {
    validate(&built_exe()).unwrap();
}

#[test]
fn f15_self_check_rejects_removed_resources_and_wrong_version() {
    let original = fs::read(built_exe()).unwrap();
    let temp = std::env::temp_dir().join(format!("mousefence-resource-check-{}.exe", std::process::id()));
    for id in [3, 14, 16, 24] {
        let mut bytes = original.clone();
        let entry = Pe::new(&bytes).resource_entry(id).unwrap();
        bytes[entry..entry + 4].copy_from_slice(&999_u32.to_le_bytes());
        fs::write(&temp, bytes).unwrap();
        assert!(validate(&temp).is_err(), "removed resource {id} must fail");
    }
    let mut bytes = original;
    let expected: Vec<u8> = "2.0.1.0\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let offset = bytes
        .windows(expected.len())
        .position(|window| window == expected)
        .unwrap();
    bytes[offset] = b'9';
    fs::write(&temp, bytes).unwrap();
    assert!(validate(&temp).is_err(), "wrong version must fail");
    fs::remove_file(temp).unwrap();
}
