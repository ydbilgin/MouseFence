//! UTF-16 helpers for the W-suffixed Win32 calls. Pure.

/// `s` as UTF-16 with a terminating NUL (for PCWSTR arguments).
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The text up to the first NUL of a fixed Win32 buffer (lossy for unpaired surrogates).
pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Copy `s` into a fixed buffer, truncated so the last slot always holds the NUL. A surrogate pair is never
/// split: when only its first half would fit, both halves are dropped. Returns the UTF-16 units written.
pub fn fill_wide(buf: &mut [u16], s: &str) -> usize {
    if buf.is_empty() {
        return 0;
    }
    let cap = buf.len() - 1;
    let mut n = 0;
    for ch in s.chars() {
        let mut units = [0u16; 2];
        let enc = ch.encode_utf16(&mut units);
        if n + enc.len() > cap {
            break;
        }
        buf[n..n + enc.len()].copy_from_slice(enc);
        n += enc.len();
    }
    for slot in &mut buf[n..] {
        *slot = 0;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_win_wide_round_trips_and_terminates() {
        let w = to_wide("Ekran — ğüşiöç");
        assert_eq!(*w.last().unwrap(), 0);
        assert_eq!(from_wide(&w), "Ekran — ğüşiöç");
        assert_eq!(from_wide(&[0x41, 0x42]), "AB", "no NUL: whole buffer");
    }

    #[test]
    fn s_win_fill_wide_truncates_and_keeps_the_nul() {
        let mut buf = [0xFFFFu16; 5];
        assert_eq!(fill_wide(&mut buf, "abcdefgh"), 4);
        assert_eq!(buf, [0x61, 0x62, 0x63, 0x64, 0]);
        let mut small = [7u16; 3];
        // U+1F512 (padlock) is a surrogate pair: "a" + pair needs 3 units, only 2 fit before the NUL.
        assert_eq!(fill_wide(&mut small, "a\u{1F512}"), 1);
        assert_eq!(small, [0x61, 0, 0]);
        let mut empty: [u16; 0] = [];
        assert_eq!(fill_wide(&mut empty, "x"), 0);
    }
}
