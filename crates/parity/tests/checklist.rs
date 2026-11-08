//! Shape checks of docs/rust2/parity.tsv, plus the strict done gate.
//!
//! Strict gate (end of a track): `MF_PARITY_STRICT=1 MF_PARITY_TRACK=A cargo test -p parity`
//! fails while any row of track A lacks its plain `#[test] fn <prefix>` (not `#[ignore]`d). Without MF_PARITY_TRACK every track is checked.

use parity::{TRACKS, has_test, load, workspace_root};
use std::collections::{BTreeMap, HashSet};

#[test]
fn s_parity_checklist_parses_with_238_p_rows_in_run_order() {
    let rows = load().expect("parity.tsv parses");
    let p: Vec<_> = rows.iter().filter(|r| r.id.starts_with('P')).collect();
    assert_eq!(p.len(), 238, "the 1.1.1 suite on 19b769c has 238 passing checks");
    for (i, r) in p.iter().enumerate() {
        assert_eq!(r.id, format!("P{:03}", i + 1));
        assert_eq!(r.test_prefix, format!("p{:03}_", i + 1));
        assert!(r.source.starts_with("tests/Tests.cs:"), "{}: source {}", r.id, r.source);
    }
}

#[test]
fn s_parity_ids_and_test_prefixes_are_unique() {
    let rows = load().unwrap();
    let mut ids = HashSet::new();
    let mut prefixes = HashSet::new();
    for r in &rows {
        assert!(ids.insert(r.id.clone()), "duplicate id {}", r.id);
        if r.test_prefix != "-" {
            assert!(
                prefixes.insert(r.test_prefix.clone()),
                "duplicate prefix {}",
                r.test_prefix
            );
        }
    }
}

#[test]
fn s_parity_tracks_and_locations_are_valid() {
    let rows = load().unwrap();
    let root = workspace_root();
    for r in &rows {
        assert!(
            TRACKS.contains(&r.track.as_str()),
            "{}: unknown track {}",
            r.id,
            r.track
        );
        let kind = r.id.chars().next().unwrap();
        assert!("PBJH".contains(kind), "{}: id must start with P, B, J or H", r.id);
        if r.track == "O" {
            assert_eq!(r.test_prefix, "-", "{}: owner rows have no agent test", r.id);
            continue;
        }
        assert_ne!(r.test_prefix, "-", "{}: agent rows need a test prefix", r.id);
        assert!(
            root.join(&r.location).exists(),
            "{}: location {} does not exist",
            r.id,
            r.location
        );
    }
}

#[test]
fn s_parity_has_all_sections() {
    let rows = load().unwrap();
    for kind in ['B', 'J', 'H'] {
        assert!(rows.iter().any(|r| r.id.starts_with(kind)), "no {kind} rows");
    }
    for t in ["A", "B", "C", "D"] {
        assert!(rows.iter().any(|r| r.track == t), "track {t} owns no rows");
    }
}

#[test]
fn s_parity_skeleton_rows_are_already_proven() {
    let rows = load().unwrap();
    let missing: Vec<_> = rows
        .iter()
        .filter(|r| r.track == "S" && !has_test(&r.location, &r.test_prefix))
        .map(|r| r.id.clone())
        .collect();
    assert!(missing.is_empty(), "skeleton rows without a test: {missing:?}");
}

#[test]
fn s_parity_rows_carry_no_real_desk_names() {
    // L16: fixtures and checklist text use the synthetic names of 22665ee, never the owner's desk.
    let rows = load().unwrap();
    for r in &rows {
        for word in ["real-geo", "Samsung", "reporter "] {
            assert!(
                !r.behaviour.contains(word),
                "{}: behaviour names the real desk ({word})",
                r.id
            );
        }
    }
}

#[test]
fn s_parity_strict_every_row_has_its_test() {
    let rows = load().unwrap();
    let only = std::env::var("MF_PARITY_TRACK").ok();
    let mut done: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut missing = Vec::new();
    for r in rows.iter().filter(|r| r.test_prefix != "-") {
        if only.as_deref().is_some_and(|t| t != r.track) {
            continue;
        }
        let e = done.entry(r.track.clone()).or_default();
        e.1 += 1;
        if has_test(&r.location, &r.test_prefix) {
            e.0 += 1;
        } else {
            missing.push(r.id.clone());
        }
    }
    for (track, (have, all)) in &done {
        eprintln!("parity track {track}: {have}/{all} rows have their test");
    }
    if std::env::var("MF_PARITY_STRICT").as_deref() == Ok("1") {
        assert!(
            missing.is_empty(),
            "{} rows without a test: {:?}",
            missing.len(),
            missing
        );
    }
}
