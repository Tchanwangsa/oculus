//! SHA-1, HMAC and TOTP, pinned by the RFC 4226/6238 test vectors below.
//! SHA-1 is broken for collisions; HMAC-SHA1 is not, and is what
//! authenticator apps implement.

fn sha1(msg: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let bit_len = (msg.len() as u64).wrapping_mul(8);

    let mut data = msg.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in data.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn hmac_sha1(key: &[u8], msg: &[u8]) -> [u8; 20] {
    const BLOCK: usize = 64;
    let mut k = if key.len() > BLOCK {
        sha1(key).to_vec()
    } else {
        key.to_vec()
    };
    k.resize(BLOCK, 0);

    let mut inner = Vec::with_capacity(BLOCK + msg.len());
    inner.extend(k.iter().map(|b| b ^ 0x36));
    inner.extend_from_slice(msg);
    let inner = sha1(&inner);

    let mut outer = Vec::with_capacity(BLOCK + 20);
    outer.extend(k.iter().map(|b| b ^ 0x5c));
    outer.extend_from_slice(&inner);
    sha1(&outer)
}

/// RFC 4648 base32. Tolerates the spaces and lowercase Okta shows the key in.
pub fn base32_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut bits: u32 = 0;
    let mut nbits: u32 = 0;
    let mut out = Vec::new();
    for ch in s.chars() {
        if ch == '=' || ch.is_whitespace() || ch == '-' {
            continue;
        }
        let v = match ch.to_ascii_uppercase() {
            c @ 'A'..='Z' => c as u32 - 'A' as u32,
            c @ '2'..='7' => c as u32 - '2' as u32 + 26,
            other => return Err(format!("'{other}' is not a base32 character")),
        };
        bits = (bits << 5) | v;
        nbits += 5;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
        }
    }
    if out.is_empty() {
        return Err("secret is empty".to_string());
    }
    Ok(out)
}

/// RFC 6238 TOTP: 6 digits, 30-second step, SHA-1.
pub fn totp_at(secret: &[u8], unix_seconds: u64, step: u64, digits: u32) -> String {
    let counter = unix_seconds / step;
    let mac = hmac_sha1(secret, &counter.to_be_bytes());
    // Dynamic truncation: the low nibble of the last byte picks the offset.
    let off = (mac[19] & 0x0f) as usize;
    let bin = ((mac[off] as u32 & 0x7f) << 24)
        | ((mac[off + 1] as u32) << 16)
        | ((mac[off + 2] as u32) << 8)
        | (mac[off + 3] as u32);
    let code = bin % 10u32.pow(digits);
    format!("{code:0width$}", width = digits as usize)
}

/// The code an authenticator app would be showing right now.
pub fn totp_now(secret_b32: &str) -> Result<String, String> {
    totp_code(secret_b32, crate::clock::now_secs())
}

/// The code an authenticator app shows at `unix_seconds`.
pub fn totp_code(secret_b32: &str, unix_seconds: u64) -> Result<String, String> {
    let secret = base32_decode(secret_b32)?;
    Ok(totp_at(&secret, unix_seconds, 30, 6))
}

/// Seconds until the current code rolls over.
pub fn totp_seconds_remaining() -> u64 {
    seconds_remaining(crate::clock::now_secs())
}

/// Seconds from `unix_seconds` until its code rolls over.
pub fn seconds_remaining(unix_seconds: u64) -> u64 {
    30 - (unix_seconds % 30)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4226 appendix D, the canonical HOTP vectors.
    #[test]
    fn matches_the_rfc_4226_hotp_vectors() {
        let secret = b"12345678901234567890";
        let expected = [
            "755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583",
            "399871", "520489",
        ];
        for (counter, want) in expected.iter().enumerate() {
            // TOTP with step 1 at time == counter is exactly HOTP(counter).
            assert_eq!(
                &totp_at(secret, counter as u64, 1, 6),
                want,
                "counter {counter}"
            );
        }
    }

    /// RFC 6238 appendix B, the SHA-1 rows.
    #[test]
    fn matches_the_rfc_6238_totp_vectors() {
        let secret = b"12345678901234567890";
        for (time, want) in [
            (59u64, "94287082"),
            (1_111_111_109, "07081804"),
            (1_111_111_111, "14050471"),
            (1_234_567_890, "89005924"),
            (2_000_000_000, "69279037"),
        ] {
            assert_eq!(totp_at(secret, time, 30, 8), want, "t={time}");
        }
    }

    #[test]
    fn decodes_base32_the_way_authenticator_apps_write_it() {
        assert_eq!(
            base32_decode("GEZDGNBVGY3TQOJQ").unwrap(),
            b"12345678901234567890"[..10].to_vec()
        );
        // Okta shows the setup key in spaced, lowercase groups.
        assert_eq!(
            base32_decode("gezd gnbv gy3t qojq").unwrap(),
            base32_decode("GEZDGNBVGY3TQOJQ").unwrap()
        );
        assert!(base32_decode("not-valid-1890").is_err());
        assert!(base32_decode("").is_err());
    }
}
