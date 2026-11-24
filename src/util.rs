//! Tiny std-only helpers: POSIX shell quoting, base64, hashing.

/// Quote `s` as one POSIX shell word.
pub fn sh_quote(s: &str) -> String {
    if !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-./=:@%+,".contains(&b))
    {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Standard base64 (RFC 4648, padded).
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = match chunk.len() {
            3 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8 | chunk[2] as u32,
            2 => (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8,
            _ => (chunk[0] as u32) << 16,
        };
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Decode standard base64, ignoring ASCII whitespace. `None` on any other invalid byte.
pub fn base64_decode(text: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    let mut padded = false;
    for &c in text {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                padded = true;
                continue;
            }
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        if padded {
            return None; // data after padding
        }
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// gzip, so large generated scripts fit in a remote command line (the remote side runs `gunzip`).
pub fn gzip(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    // writing to a Vec cannot fail
    let _ = enc.write_all(data);
    enc.finish().unwrap_or_default()
}

/// FNV-1a 64 over several byte slices (stable across runs, unlike `DefaultHasher`).
pub fn fnv1a(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for part in parts {
        for b in *part {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h ^= 0xff; // part separator
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_plain_and_hostile() {
        assert_eq!(sh_quote("scene.py"), "scene.py");
        assert_eq!(sh_quote("a b"), "'a b'");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
        assert_eq!(sh_quote(""), "''");
        assert_eq!(sh_quote("$(rm -rf /)"), "'$(rm -rf /)'");
    }

    #[test]
    fn base64_rfc_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_roundtrips_binary_and_ignores_line_breaks() {
        let data: Vec<u8> = (0..=255u8).cycle().take(1001).collect();
        let enc = base64(&data);
        let wrapped: String = enc
            .as_bytes()
            .chunks(76)
            .map(|c| format!("{}\n", std::str::from_utf8(c).unwrap()))
            .collect();
        assert_eq!(base64_decode(wrapped.as_bytes()).unwrap(), data);
        assert!(base64_decode(b"ab!d").is_none());
        assert!(base64_decode(b"Zg==Zg==").is_none());
    }

    #[test]
    fn gzip_shrinks_repetitive_scripts_and_is_a_valid_gzip_stream() {
        let script = "var SPEC = {\"cues\":[{\"s\":1.0,\"e\":2.0}]};\n".repeat(500);
        let z = gzip(script.as_bytes());
        assert!(
            z.len() * 8 < script.len(),
            "{} -> {}",
            script.len(),
            z.len()
        );
        assert_eq!(&z[..2], &[0x1f, 0x8b], "gzip magic");
    }

    #[test]
    fn fnv_separates_parts() {
        assert_ne!(fnv1a(&[b"ab", b"c"]), fnv1a(&[b"a", b"bc"]));
    }
}
