//! A small PNG writer (RGBA, 8 bit) for the screenshot modes, so the settings UI needs no image crate
//! (PLAN T5: only cached crates; `image` is not one of the agreed dependencies).
//!
//! Each row gets the PNG filter (None, Sub or Up) with the smallest absolute sum; the rows are compressed
//! with deflate using the fixed Huffman code and a hash-chain LZ77 matcher. UI screenshots are mostly flat
//! colour, so this is enough for a 20-50x reduction. `decode_rgba` reads back exactly what `encode_rgba`
//! writes (stored or fixed-Huffman blocks); the tests use it for round trips.

use std::io::Write as _;
use std::path::Path;

const SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

/// Encode `rgba` (row-major, 4 bytes per pixel, unpremultiplied) as a PNG file in memory.
pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    assert_eq!(
        rgba.len(),
        width as usize * height as usize * 4,
        "rgba length must be width*height*4"
    );
    let stride = width as usize * 4;
    let mut filtered = Vec::with_capacity((stride + 1) * height as usize);
    let zero_row = vec![0u8; stride];
    let mut candidate = vec![0u8; stride];
    for y in 0..height as usize {
        let row = &rgba[y * stride..(y + 1) * stride];
        let prev = if y == 0 {
            &zero_row[..]
        } else {
            &rgba[(y - 1) * stride..y * stride]
        };
        let mut best_kind = 0u8;
        let mut best_sum = row.iter().map(|&b| (b as i8).unsigned_abs() as u64).sum::<u64>();
        let mut best = row.to_vec();
        for kind in [1u8, 2u8] {
            for x in 0..stride {
                let a = if x >= 4 { row[x - 4] } else { 0 };
                let b = prev[x];
                candidate[x] = row[x].wrapping_sub(if kind == 1 { a } else { b });
            }
            let sum = candidate.iter().map(|&b| (b as i8).unsigned_abs() as u64).sum::<u64>();
            if sum < best_sum {
                best_sum = sum;
                best_kind = kind;
                best.copy_from_slice(&candidate);
            }
        }
        filtered.push(best_kind);
        filtered.extend_from_slice(&best);
    }

    let mut out = Vec::with_capacity(filtered.len() / 8 + 64);
    out.extend_from_slice(&SIGNATURE);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_compress(&filtered));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Write `rgba` as a PNG file, creating the parent folder.
pub fn write_rgba(path: &Path, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir)?;
    }
    let bytes = encode_rgba(width, height, rgba);
    let mut f = std::fs::File::create(path)?;
    f.write_all(&bytes)?;
    f.flush()
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (n, slot) in t.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        t
    });
    let mut c = 0xFFFF_FFFFu32;
    for &b in bytes {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in bytes.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, value: u32, bits: u32) {
        self.acc |= (value as u64) << self.n;
        self.n += bits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    /// Huffman codes are sent most-significant bit first.
    fn put_code(&mut self, code: u32, len: u32) {
        let mut rev = 0u32;
        for i in 0..len {
            rev |= ((code >> i) & 1) << (len - 1 - i);
        }
        self.put(rev, len);
    }
    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

fn fixed_literal(w: &mut BitWriter, sym: u32) {
    match sym {
        0..=143 => w.put_code(0x30 + sym, 8),
        144..=255 => w.put_code(0x190 + (sym - 144), 9),
        256..=279 => w.put_code(sym - 256, 7),
        _ => w.put_code(0xC0 + (sym - 280), 8),
    }
}

const LEN_BASE: [u32; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258,
];
const LEN_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u32; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13,
];

fn put_match(w: &mut BitWriter, len: u32, dist: u32) {
    let li = LEN_BASE.iter().rposition(|&b| b <= len).unwrap_or(0);
    fixed_literal(w, 257 + li as u32);
    w.put(len - LEN_BASE[li], LEN_EXTRA[li]);
    let di = DIST_BASE.iter().rposition(|&b| b <= dist).unwrap_or(0);
    w.put_code(di as u32, 5);
    w.put(dist - DIST_BASE[di], DIST_EXTRA[di]);
}

/// zlib stream with one fixed-Huffman deflate block.
fn zlib_compress(data: &[u8]) -> Vec<u8> {
    const WINDOW: usize = 32 * 1024;
    const HASH_BITS: u32 = 15;
    const MAX_CHAIN: usize = 48;
    let mut w = BitWriter {
        out: vec![0x78, 0x01],
        acc: 0,
        n: 0,
    };
    w.put(1, 1); // BFINAL
    w.put(1, 2); // BTYPE = 01 (fixed Huffman)
    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    let mut prev = vec![usize::MAX; data.len()];
    let hash = |i: usize| -> usize {
        let v = (data[i] as u32) << 16 | (data[i + 1] as u32) << 8 | data[i + 2] as u32;
        (v.wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize
    };
    let insert = |i: usize, head: &mut [usize], prev: &mut [usize]| {
        if i + 2 < data.len() {
            let h = hash(i);
            prev[i] = head[h];
            head[h] = i;
        }
    };
    let mut i = 0;
    while i < data.len() {
        let mut best_len = 0usize;
        let mut best_dist = 0usize;
        if i + 2 < data.len() {
            let mut cand = head[hash(i)];
            let mut chain = 0;
            let max_len = (data.len() - i).min(258);
            while cand != usize::MAX && i - cand <= WINDOW && chain < MAX_CHAIN {
                let mut l = 0;
                while l < max_len && data[cand + l] == data[i + l] {
                    l += 1;
                }
                if l > best_len {
                    best_len = l;
                    best_dist = i - cand;
                    if l == max_len {
                        break;
                    }
                }
                cand = prev[cand];
                chain += 1;
            }
        }
        if best_len >= 3 {
            put_match(&mut w, best_len as u32, best_dist as u32);
            for k in i..i + best_len {
                insert(k, &mut head, &mut prev);
            }
            i += best_len;
        } else {
            fixed_literal(&mut w, data[i] as u32);
            insert(i, &mut head, &mut prev);
            i += 1;
        }
    }
    fixed_literal(&mut w, 256);
    let mut out = w.finish();
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// A decoded PNG (RGBA 8 bit only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Read back an RGBA-8 PNG whose IDAT holds stored or fixed-Huffman deflate blocks (what `encode_rgba` writes).
pub fn decode_rgba(bytes: &[u8]) -> Result<Decoded, String> {
    if bytes.len() < 8 || bytes[..8] != SIGNATURE {
        return Err("not a PNG".into());
    }
    let mut pos = 8;
    let (mut width, mut height) = (0u32, 0u32);
    let mut idat = Vec::new();
    while pos + 12 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = &bytes[pos + 4..pos + 8];
        let data = bytes.get(pos + 8..pos + 8 + len).ok_or("truncated chunk")?;
        let crc = u32::from_be_bytes(bytes[pos + 8 + len..pos + 12 + len].try_into().unwrap());
        if crc32(&bytes[pos + 4..pos + 8 + len]) != crc {
            return Err("bad CRC".into());
        }
        match kind {
            b"IHDR" => {
                width = u32::from_be_bytes(data[0..4].try_into().unwrap());
                height = u32::from_be_bytes(data[4..8].try_into().unwrap());
                if data[8] != 8 || data[9] != 6 {
                    return Err("only RGBA 8 bit".into());
                }
            }
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        pos += 12 + len;
    }
    let raw = inflate_zlib(&idat)?;
    let stride = width as usize * 4;
    if raw.len() != (stride + 1) * height as usize {
        return Err(format!("{} bytes, want {}", raw.len(), (stride + 1) * height as usize));
    }
    let mut rgba = vec![0u8; stride * height as usize];
    for y in 0..height as usize {
        let kind = raw[y * (stride + 1)];
        let src = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        for x in 0..stride {
            let a = if x >= 4 { rgba[y * stride + x - 4] } else { 0 };
            let b = if y > 0 { rgba[(y - 1) * stride + x] } else { 0 };
            rgba[y * stride + x] = match kind {
                0 => src[x],
                1 => src[x].wrapping_add(a),
                2 => src[x].wrapping_add(b),
                _ => return Err(format!("filter {kind} not written by this encoder")),
            };
        }
    }
    Ok(Decoded { width, height, rgba })
}

fn inflate_zlib(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 6 {
        return Err("short zlib stream".into());
    }
    let mut out = Vec::new();
    let mut bit = 16usize; // after the 2-byte header
    let get = |bit: &mut usize, n: u32| -> Result<u32, String> {
        let mut v = 0u32;
        for i in 0..n {
            let byte = *data.get(*bit / 8).ok_or("out of data")?;
            v |= (((byte >> (*bit % 8)) & 1) as u32) << i;
            *bit += 1;
        }
        Ok(v)
    };
    loop {
        let last = get(&mut bit, 1)?;
        let kind = get(&mut bit, 2)?;
        match kind {
            0 => {
                bit = bit.div_ceil(8) * 8;
                let len = get(&mut bit, 16)? as usize;
                let _nlen = get(&mut bit, 16)?;
                let start = bit / 8;
                out.extend_from_slice(data.get(start..start + len).ok_or("stored block overrun")?);
                bit += len * 8;
            }
            1 => loop {
                // Fixed Huffman: read 7 bits MSB-first and extend as needed.
                let mut code = 0u32;
                for _ in 0..7 {
                    code = (code << 1) | get(&mut bit, 1)?;
                }
                let sym = if code <= 0x17 {
                    256 + code
                } else {
                    code = (code << 1) | get(&mut bit, 1)?;
                    if (0x30..=0xBF).contains(&code) {
                        code - 0x30
                    } else if (0xC0..=0xC7).contains(&code) {
                        280 + code - 0xC0
                    } else {
                        code = (code << 1) | get(&mut bit, 1)?;
                        144 + code - 0x190
                    }
                };
                match sym {
                    0..=255 => out.push(sym as u8),
                    256 => break,
                    _ => {
                        let li = (sym - 257) as usize;
                        let len = LEN_BASE[li] + get(&mut bit, LEN_EXTRA[li])?;
                        let mut d = 0u32;
                        for _ in 0..5 {
                            d = (d << 1) | get(&mut bit, 1)?;
                        }
                        let dist = (DIST_BASE[d as usize] + get(&mut bit, DIST_EXTRA[d as usize])?) as usize;
                        if dist > out.len() {
                            return Err("distance too far".into());
                        }
                        for _ in 0..len {
                            out.push(out[out.len() - dist]);
                        }
                    }
                }
            },
            _ => return Err("dynamic Huffman blocks are not written by this encoder".into()),
        }
        if last == 1 {
            break;
        }
    }
    let start = bit.div_ceil(8);
    let want = u32::from_be_bytes(data.get(start..start + 4).ok_or("no adler32")?.try_into().unwrap());
    if adler32(&out) != want {
        return Err("adler32 mismatch".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(w: u32, h: u32, seed: u32) -> Vec<u8> {
        let mut state = seed;
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let noisy = (state >> 16) as u8;
                let flat = if (x / 7 + y / 5) % 3 == 0 { 40 } else { 200 };
                v.extend_from_slice(&[flat, if x % 11 == 0 { noisy } else { flat }, (y * 3) as u8, 255]);
            }
        }
        v
    }

    #[test]
    fn s_png_round_trips_flat_and_noisy_images() {
        for (w, h, seed) in [(1u32, 1u32, 1u32), (17, 9, 2), (320, 200, 3), (5, 300, 4)] {
            let rgba = pattern(w, h, seed);
            let png = encode_rgba(w, h, &rgba);
            let back = decode_rgba(&png).expect("decodes");
            assert_eq!((back.width, back.height), (w, h));
            assert_eq!(back.rgba, rgba, "{w}x{h}");
        }
    }

    #[test]
    fn s_png_compresses_flat_ui_images() {
        let rgba = vec![37u8; 1064 * 820 * 4];
        let png = encode_rgba(1064, 820, &rgba);
        assert!(png.len() < 40_000, "flat 1064x820 image took {} bytes", png.len());
    }

    #[test]
    fn s_png_crc_matches_the_png_specification_sample() {
        // CRC-32 of "IEND" with no data is the constant every PNG ends with.
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }
}
