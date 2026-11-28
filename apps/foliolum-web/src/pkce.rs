//! PKCE (RFC 7636) and the base64url it is written in, for signing in to Claviger
//! ([`crate::account`]) and to TDPdf Sign ([`crate::sign`]).

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// base64url without padding (RFC 4648 §5).
pub fn base64url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 2);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// Decodes base64url, padded or not; None if it holds anything else.
pub fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim_end_matches('=');
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for b in s.bytes() {
        let v = ALPHABET.iter().position(|&a| a == b)? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The S256 challenge for a `verifier`.
pub fn challenge(verifier: &str) -> String {
    use sha2::Digest as _;
    base64url(&sha2::Sha256::digest(verifier.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_matches_the_rfc_example_and_round_trips() {
        // RFC 7636 appendix B.
        assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        for n in 0..40u8 {
            let bytes: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37).wrapping_add(n)).collect();
            assert_eq!(base64url_decode(&base64url(&bytes)).as_deref(), Some(bytes.as_slice()));
        }
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url_decode("Zm8="), Some(b"fo".to_vec()));
        assert_eq!(base64url_decode("a+b"), None);
    }
}
