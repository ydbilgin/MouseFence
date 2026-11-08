//! Stable-id string rules of the C# 1.1.1 code, in one place (Track A).
//!
//! The C# compiler and configure code compare monitor ids in three different ways, and parity depends on
//! using the right one at each call site:
//! - `same`: `BoundaryGraph.Same`, i.e. `string.Equals(a?.Trim(), b?.Trim(), OrdinalIgnoreCase)`;
//! - `eq_ignore_case`: a `StringComparer.OrdinalIgnoreCase` set or `Contains` (no trimming);
//! - plain `==`: a default `HashSet<string>` / `Dictionary<string, _>` / `List.Contains` (exact, case-sensitive).
//!
//! One implementation of the .NET string rules (join 2026-10-13): the ordinal and OrdinalIgnoreCase rules are
//! the ones in [`crate::layout`] (Track B), which the whole-plane reference sweep proved against .NET 9
//! (crates/fence-trace/tests/config_reference_diff.rs: case rules and text rules, 0 mismatches). They differ from a
//! plain `char::to_uppercase` fold where .NET does: a non-ASCII character never folds to an ASCII one (U+0131,
//! U+017F), Greek letters with ypogegrammeni take their simple mapping, and characters newer than .NET 9's tables
//! stay themselves. `Trim` and `IsNullOrWhiteSpace` use .NET's `char.IsWhiteSpace`, which is the same set as
//! Rust's `char::is_whitespace` (Unicode White_Space).

pub use crate::layout::{ordinal_cmp, ordinal_ignore_case_cmp};

/// `string.Equals(a, b, StringComparison.OrdinalIgnoreCase)` ([`crate::layout::ordinal_ignore_case_eq`]).
pub fn eq_ignore_case(a: &str, b: &str) -> bool {
    crate::layout::ordinal_ignore_case_eq(a, b)
}

/// C# `BoundaryGraph.Same`: trimmed, case-insensitive equality.
pub fn same(a: &str, b: &str) -> bool {
    eq_ignore_case(a.trim(), b.trim())
}

/// C# `string.IsNullOrWhiteSpace`.
pub fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn s_engine_ids_follow_dotnet_ordinal_rules() {
        assert!(eq_ignore_case("MONITOR\\Syn1000", "monitor\\SYN1000"));
        assert!(!eq_ignore_case("a", "ab"));
        assert!(same("  dock-A ", "DOCK-a"));
        assert!(!same("a", "b"));
        assert!(is_blank(" \t\r\n"));
        assert!(!is_blank(" x "));
        assert_eq!(ordinal_cmp("B", "a"), Ordering::Less, "ordinal: uppercase sorts first");
        assert_eq!(ordinal_ignore_case_cmp("B", "a"), Ordering::Greater);
        assert!(eq_ignore_case("straße", "STRAßE"), "ß keeps its simple mapping");
        // A supplementary character sorts above U+E000..U+FFFF in UTF-16 order, below it in UTF-8 order.
        assert_eq!(ordinal_cmp("\u{10000}", "\u{FFFD}"), Ordering::Less);
        assert_eq!("\u{10000}".cmp("\u{FFFD}"), Ordering::Greater);
        // The .NET folds the old private copy got wrong (it used char::to_uppercase as is).
        assert!(
            !eq_ignore_case("\u{131}", "i"),
            "dotless i never folds to ASCII I under .NET"
        );
        assert!(
            !eq_ignore_case("\u{17F}", "s"),
            "long s never folds to ASCII S under .NET"
        );
        assert!(!same(" \u{131}d ", "ID"));
    }
}
