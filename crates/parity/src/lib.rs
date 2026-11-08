//! Parity checklist reader. SHARED (additive only): each track adds rows to its own section of
//! docs/rust2/parity.tsv; nobody rewrites another track's rows.
//!
//! The checklist lists every behaviour of MouseFence 1.1.1 with the Rust test that proves it:
//! - `P` rows: the 238 checks of the C# suite on public main 19b769c, in run order.
//! - `B` rows: behaviours no C# test covers (tray, hotkeys, balloons, startup, settings file, dev CLI).
//! - `J` rows: judged differences from 1.1.1 (fixes that land after exact parity).
//! - `H` rows: the owner's hand checks; agents never run them.
//!
//! `cargo test -p parity` checks the file's shape. With `MF_PARITY_STRICT=1` it also checks that every
//! row with a test prefix has a plain (`#[test]`, not `#[ignore]`d) `fn <prefix>...` in the named location;
//! that is the done gate of a track.

use std::fs;
use std::path::{Path, PathBuf};

/// One checklist row (columns of parity.tsv, in order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub track: String,
    pub how: String,
    pub test_prefix: String,
    pub location: String,
    pub source: String,
    pub behaviour: String,
    pub note: String,
}

pub const HEADER: &str = "id\ttrack\thow\trust_test_prefix\trust_location\tsource\tbehaviour\tnote";
pub const TRACKS: [&str; 6] = ["S", "A", "B", "C", "D", "O"];

/// Workspace root (two levels above this crate).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

pub fn checklist_path() -> PathBuf {
    workspace_root().join("docs").join("rust2").join("parity.tsv")
}

/// Parse the checklist text. Comment lines start with `#`; the first non-comment line is the header.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    let mut header_seen = false;
    for (n, line) in text.lines().enumerate() {
        let line_no = n + 1;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if !header_seen {
            if line != HEADER {
                return Err(format!("line {line_no}: header must be `{HEADER}`"));
            }
            header_seen = true;
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 8 {
            return Err(format!("line {line_no}: {} columns, want 8", cols.len()));
        }
        if cols.iter().any(|c| c.trim().is_empty()) {
            return Err(format!("line {line_no}: empty column (use - for none)"));
        }
        rows.push(Row {
            id: cols[0].into(),
            track: cols[1].into(),
            how: cols[2].into(),
            test_prefix: cols[3].into(),
            location: cols[4].into(),
            source: cols[5].into(),
            behaviour: cols[6].into(),
            note: cols[7].into(),
        });
    }
    if !header_seen {
        return Err("no header line".into());
    }
    Ok(rows)
}

pub fn load() -> Result<Vec<Row>, String> {
    let path = checklist_path();
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text)
}

/// All `.rs` files under `dir` (recursive).
pub fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if dir.is_file() {
        if dir.extension().is_some_and(|e| e == "rs") {
            out.push(dir.to_path_buf());
        }
        return out;
    }
    let Ok(entries) = fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            out.extend(rust_files(&p));
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
    out
}

/// True when some file under `location` (relative to the workspace root) declares a test `fn <prefix>...`
/// (see [`declares_test`]).
pub fn has_test(location: &str, prefix: &str) -> bool {
    rust_files(&workspace_root().join(location))
        .iter()
        .any(|f| fs::read_to_string(f).is_ok_and(|t| declares_test(&t, prefix)))
}

/// True when `text` declares `fn <prefix>...` as a test that a plain `cargo test` runs: `#[test]` among the
/// attributes right above it, or the fn sits inside a `scenarios! { ... }` block (the fence-engine parity macro
/// adds `#[test]` to every fn), and no `#[ignore` above it. A helper fn or an ignored test no longer counts
/// (JUDGE-FINAL S2, fix round 2026-10-13). Whether the test passes is still the test run's job.
pub fn declares_test(text: &str, prefix: &str) -> bool {
    let needle = format!("fn {prefix}");
    let mut from = 0;
    while let Some(i) = text[from..].find(&needle) {
        let at = from + i;
        from = at + needle.len();
        if text[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            continue;
        }
        let attrs = attributes_above(text, at);
        if attrs.iter().any(|a| a.starts_with("#[ignore")) {
            continue;
        }
        if attrs.contains(&"#[test]") || inside_scenarios(text, at) {
            return true;
        }
    }
    false
}

/// The attribute lines directly above the line holding byte `at` (blank and `//` lines skipped).
fn attributes_above(text: &str, at: usize) -> Vec<&str> {
    let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
    let mut attrs = Vec::new();
    for line in text[..line_start].lines().rev() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("//") {
            continue;
        }
        if t.starts_with("#[") {
            attrs.push(t);
            continue;
        }
        break;
    }
    attrs
}

/// True when byte `at` lies inside the braces of the last `scenarios! {` before it.
fn inside_scenarios(text: &str, at: usize) -> bool {
    let Some(open) = text[..at].rfind("scenarios! {") else {
        return false;
    };
    let body = &text[open + "scenarios! ".len()..at];
    let depth = body.chars().fold(0i64, |d, c| match c {
        '{' => d + 1,
        '}' => d - 1,
        _ => d,
    });
    depth > 0
}

#[cfg(test)]
mod tests {
    use super::declares_test;

    #[test]
    fn s_parity_only_a_plain_test_counts_for_a_row() {
        let plain = "#[test]\nfn p001_x() {}\n";
        assert!(declares_test(plain, "p001_"));
        let with_more = "// row P001\n#[test]\n#[cfg(windows)]\n\n    pub fn p001_x() {}\n";
        assert!(declares_test(with_more, "p001_"));
        assert!(
            !declares_test("fn p001_helper() {}\n", "p001_"),
            "a helper is not a test"
        );
        assert!(
            !declares_test("#[test]\n#[ignore = \"reference\"]\nfn p001_x() {}\n", "p001_"),
            "an ignored test is not a plain test"
        );
        assert!(
            !declares_test("#[test]\nfn xp001_x() {}\n", "p001_"),
            "a longer name is not the prefix"
        );
        assert!(!declares_test("#[test]\nfn p002_x() {}\n", "p001_"));
        let both = "fn p001_helper() {}\n#[test]\nfn p001_real() {}\n";
        assert!(declares_test(both, "p001_"), "one real test among helpers is enough");
        let macro_body = "scenarios! {\n    /// doc\n    fn p001_x() {\n        let s = format!(\"{a}\");\n    }\n\n    fn p002_y() {}\n}\nfn p003_after() {}\n";
        assert!(declares_test(macro_body, "p001_"));
        assert!(
            declares_test(macro_body, "p002_"),
            "after a nested body, still inside the macro"
        );
        assert!(
            !declares_test(macro_body, "p003_"),
            "after the macro closes it is a plain fn"
        );
        let ignored_in_macro = "scenarios! {\n    #[ignore]\n    fn p001_x() {}\n}\n";
        assert!(!declares_test(ignored_in_macro, "p001_"));
    }
}
