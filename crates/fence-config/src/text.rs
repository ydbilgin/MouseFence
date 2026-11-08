//! Monitor nicknames: C# `Settings.NormalizeMonitorName` and `Settings.MonitorNameFor`, and the text-element
//! count they use (`StringInfo.ParseCombiningCharacters`, extended grapheme clusters).
//!
//! .NET 9 segments text with the Unicode 15 rules WITHOUT rule GB9c (Indic conjuncts), proven by the reference:
//! "क्षि" is two text elements in 1.1.1. The `unicode-segmentation` crate (Unicode 17) implements GB9c, so a
//! cluster it joined only through GB9c is split again here, and the characters whose break property differs
//! between the two Unicode versions are seen the .NET way (see [`dotnet_view`]). The whole-plane sweep in
//! crates/fence-trace/tests/config_reference_diff.rs compares this with .NET character by character.

use unicode_segmentation::UnicodeSegmentation;

/// The maximum number of text elements a nickname keeps (C# `NormalizeMonitorName`).
pub const MAX_NAME_ELEMENTS: usize = 24;

/// Marks added after .NET 9's Unicode data (Extend, SpacingMark or Prepend in Unicode 16/17): .NET does not
/// know them, so they break like any letter. Found by the whole-plane sweep against the reference (2026-10-13).
const DOTNET_UNKNOWN_MARKS: &[(u32, u32)] = &[
    (0x0897, 0x0897),
    (0x1ACF, 0x1ADD),
    (0x1AE0, 0x1AEB),
    (0x10D69, 0x10D6D),
    (0x10EFA, 0x10EFC),
    (0x113B8, 0x113C0),
    (0x113C2, 0x113C2),
    (0x113C5, 0x113C5),
    (0x113C7, 0x113CA),
    (0x113CC, 0x113D2),
    (0x113E1, 0x113E2),
    (0x11B60, 0x11B67),
    (0x11F5A, 0x11F5A),
    (0x1611E, 0x1612F),
    (0x16D63, 0x16D63),
    (0x16D67, 0x16D6A),
    (0x1E5EE, 0x1E5EF),
    (0x1E6E3, 0x1E6E3),
    (0x1E6E6, 0x1E6E6),
    (0x1E6EE, 0x1E6EF),
    (0x1E6F5, 0x1E6F5),
];

/// Characters that were Prepend in .NET 9's Unicode data and are not any more (U+11A3A, Unicode 16).
const DOTNET_PREPEND: &[u32] = &[0x11A3A];

/// Pictographs .NET 9 joins after an emoji ZWJ (rule GB11, Extended_Pictographic) and the crate does not:
/// the Extended_Pictographic characters that are not emoji (689 code points). Found by the whole-plane
/// sweep's `c ZWJ c` pattern against the reference (2026-10-13).
#[rustfmt::skip]
const DOTNET_PICTOGRAPHIC: &[(u32, u32)] = &[
    (0x2388, 0x2388), (0x2605, 0x2605), (0x2607, 0x260D), (0x260F, 0x2610), (0x2612, 0x2612), (0x2616, 0x2617),
    (0x2619, 0x261C), (0x261E, 0x261F), (0x2621, 0x2621), (0x2624, 0x2625), (0x2627, 0x2629), (0x262B, 0x262D),
    (0x2630, 0x2637), (0x263B, 0x263F), (0x2641, 0x2641), (0x2643, 0x2647), (0x2654, 0x265E), (0x2661, 0x2662),
    (0x2664, 0x2664), (0x2667, 0x2667), (0x2669, 0x267A), (0x267C, 0x267D), (0x2680, 0x2685), (0x2690, 0x2691),
    (0x2698, 0x2698), (0x269A, 0x269A), (0x269D, 0x269F), (0x26A2, 0x26A6), (0x26A8, 0x26A9), (0x26AC, 0x26AF),
    (0x26B2, 0x26BC), (0x26BF, 0x26C3), (0x26C6, 0x26C7), (0x26C9, 0x26CD), (0x26D0, 0x26D0), (0x26D2, 0x26D2),
    (0x26D5, 0x26E8), (0x26EB, 0x26EF), (0x26F6, 0x26F6), (0x26FB, 0x26FC), (0x26FE, 0x2701), (0x2703, 0x2704),
    (0x270E, 0x270E), (0x2710, 0x2711), (0x2765, 0x2767), (0x1F000, 0x1F003), (0x1F005, 0x1F02B),
    (0x1F030, 0x1F093), (0x1F0A0, 0x1F0AE), (0x1F0B1, 0x1F0BF), (0x1F0C1, 0x1F0CE), (0x1F0D1, 0x1F0F5),
    (0x1F10D, 0x1F10F), (0x1F12F, 0x1F12F), (0x1F16C, 0x1F16F), (0x1F1AD, 0x1F1AD), (0x1F260, 0x1F265),
    (0x1F322, 0x1F323), (0x1F394, 0x1F395), (0x1F398, 0x1F398), (0x1F39C, 0x1F39D), (0x1F3F1, 0x1F3F2),
    (0x1F3F6, 0x1F3F6), (0x1F4FE, 0x1F4FE), (0x1F546, 0x1F548), (0x1F54F, 0x1F54F), (0x1F568, 0x1F56E),
    (0x1F571, 0x1F572), (0x1F57B, 0x1F586), (0x1F588, 0x1F589), (0x1F58E, 0x1F58F), (0x1F591, 0x1F594),
    (0x1F597, 0x1F5A3), (0x1F5A6, 0x1F5A7), (0x1F5A9, 0x1F5B0), (0x1F5B3, 0x1F5BB), (0x1F5BD, 0x1F5C1),
    (0x1F5C5, 0x1F5D0), (0x1F5D4, 0x1F5DB), (0x1F5DF, 0x1F5E0), (0x1F5E2, 0x1F5E2), (0x1F5E4, 0x1F5E7),
    (0x1F5E9, 0x1F5EE), (0x1F5F0, 0x1F5F2), (0x1F5F4, 0x1F5F9), (0x1F6C6, 0x1F6CA), (0x1F6D3, 0x1F6D4),
    (0x1F6E6, 0x1F6E8), (0x1F6EA, 0x1F6EA), (0x1F6F1, 0x1F6F2), (0x1F774, 0x1F77F), (0x1F7D5, 0x1F7D9),
    (0x1F8B0, 0x1F8BB), (0x1F8C0, 0x1F8C1), (0x1F8D0, 0x1F8D8), (0x1FA00, 0x1FA57), (0x1FA60, 0x1FA6D),
];

fn in_ranges(ranges: &[(u32, u32)], cp: u32) -> bool {
    ranges.iter().any(|&(lo, hi)| (lo..=hi).contains(&cp))
}

/// The character .NET 9 would segment like `c`: an unknown mark becomes a plain letter, a former Prepend
/// becomes U+0600 (Prepend in both versions), a pictograph the crate does not treat as one becomes U+1F600
/// (a plain emoji in both), anything else stays.
fn dotnet_view(c: char) -> char {
    let cp = c as u32;
    if DOTNET_PREPEND.contains(&cp) {
        '\u{600}'
    } else if in_ranges(DOTNET_UNKNOWN_MARKS, cp) {
        'a'
    } else if in_ranges(DOTNET_PICTOGRAPHIC, cp) {
        '\u{1F600}'
    } else {
        c
    }
}

fn graphemes(s: &str) -> usize {
    s.graphemes(true).count()
}

/// InCB=Consonant as the crate sees it: joins after "KA + VIRAMA" (GB9c) but not after a plain letter.
fn is_conjunct_consonant(c: char) -> bool {
    graphemes(&format!("\u{915}\u{94D}{c}")) == 1 && graphemes(&format!("a{c}")) == 2
}

/// GCB=Prepend: joins the letter after it (GB9b).
fn is_prepend(c: char) -> bool {
    graphemes(&format!("{c}a")) == 1
}

/// Byte offsets where .NET 9's `StringInfo.ParseCombiningCharacters` starts a text element.
pub fn text_element_starts(s: &str) -> Vec<usize> {
    if !s.chars().any(|c| dotnet_view(c) != c) {
        return crate_starts_without_gb9c(s);
    }
    // Segment the .NET view of the text, then map its starts back to `s` character by character.
    let original: Vec<usize> = s.char_indices().map(|(i, _)| i).collect();
    let view: String = s.chars().map(dotnet_view).collect();
    let view_starts = crate_starts_without_gb9c(&view);
    let mut out = Vec::with_capacity(view_starts.len());
    let mut next = view_starts.iter().peekable();
    for (n, (i, _)) in view.char_indices().enumerate() {
        if next.peek() == Some(&&i) {
            out.push(original[n]);
            next.next();
        }
    }
    out
}

/// The crate's extended grapheme clusters with the GB9c joins split again.
fn crate_starts_without_gb9c(s: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    for (i, cluster) in s.grapheme_indices(true) {
        starts.push(i);
        let mut prev: Option<char> = None;
        for (j, c) in cluster.char_indices() {
            if let Some(p) = prev
                && is_conjunct_consonant(c)
                && !is_prepend(p)
            {
                starts.push(i + j);
            }
            prev = Some(c);
        }
    }
    starts
}

/// C# `Settings.NormalizeMonitorName`: tabs and line breaks become spaces, the ends are trimmed, and at most
/// 24 text elements are kept (no second trim after the cut, as in C#).
pub fn normalize_monitor_name(value: &str) -> String {
    let replaced = value.replace(['\t', '\r', '\n'], " ");
    let clean = replaced.trim();
    let starts = text_element_starts(clean);
    if starts.len() <= MAX_NAME_ELEMENTS {
        clean.to_string()
    } else {
        clean[..starts[MAX_NAME_ELEMENTS]].to_string()
    }
}

/// C# `Settings.MonitorNameFor(names, stableId)`: the normalized nickname for a stable id, or "" (an empty id
/// or an absent one never matches; keys compare OrdinalIgnoreCase like the normalized C# dictionary).
pub fn monitor_name_for(names: &[(String, String)], stable_id: &str) -> String {
    fence_engine::layout::monitor_name_lookup(names, stable_id)
        .map(normalize_monitor_name)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p027_screen_names_become_one_line_and_keep_24_text_elements() {
        let input = format!("  {}\tStudio\r\n  ", "e\u{301}".repeat(30));
        let normalized = normalize_monitor_name(&input);
        assert_eq!(text_element_starts(&normalized).len(), 24, "{normalized:?}");
        assert!(!normalized.contains(['\t', '\r', '\n']));
        assert_eq!(normalized, "e\u{301}".repeat(24));
    }

    #[test]
    fn s_config_text_elements_match_dotnet_samples() {
        // UTF-8 byte offsets of the starts the reference reported in UTF-16 units (2026-10-13 probe).
        assert_eq!(text_element_starts("e\u{301}e\u{301}e\u{301}"), vec![0, 3, 6]);
        assert_eq!(
            text_element_starts("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"),
            vec![0]
        );
        assert_eq!(text_element_starts("\u{1F1F9}\u{1F1F7}\u{1F1FA}"), vec![0, 8]);
        assert_eq!(
            text_element_starts("\u{915}\u{94D}\u{937}\u{93F}"),
            vec![0, 6],
            "no GB9c in .NET 9"
        );
        assert_eq!(text_element_starts("\u{1100}\u{1161}\u{11A8}"), vec![0]);
        assert_eq!(text_element_starts("a\r\nb"), vec![0, 1, 3]);
        assert_eq!(text_element_starts("\u{600}a"), vec![0], "prepend joins");
        assert_eq!(text_element_starts("\u{E01}\u{E33}"), vec![0]);
        assert_eq!(
            text_element_starts("a\u{897}a"),
            vec![0, 1, 4],
            "a Unicode 16 mark .NET 9 does not know"
        );
        assert_eq!(text_element_starts("\u{11A3A}a"), vec![0], "still Prepend for .NET 9");
        assert_eq!(
            text_element_starts("\u{2605}\u{200D}\u{2605}"),
            vec![0],
            "GB11 over a non-emoji pictograph"
        );
    }

    #[test]
    fn s_config_monitor_name_for_is_case_insensitive_and_normalized() {
        let names = vec![("Stable-A".to_string(), "  Left\tDesk ".to_string())];
        assert_eq!(monitor_name_for(&names, "stable-a"), "Left Desk");
        assert_eq!(monitor_name_for(&names, ""), "");
        assert_eq!(monitor_name_for(&names, "stable-b"), "");
    }
}
