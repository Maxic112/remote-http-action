//! API key generation and hashing.

use rand::distr::{Alphanumeric, SampleString};
use sha2::{Digest, Sha256};

pub const KEY_PREFIX: &str = "rha_";
const KEY_RANDOM_LEN: usize = 40;
/// Number of leading characters stored in clear text to help recognise a key.
pub const DISPLAY_PREFIX_LEN: usize = 12;

/// Generates a new secret: `rha_` + 40 alphanumeric chars (~238 bits of entropy).
pub fn generate_key() -> String {
    let random = Alphanumeric.sample_string(&mut rand::rng(), KEY_RANDOM_LEN);
    format!("{KEY_PREFIX}{random}")
}

pub fn hash_key(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

pub fn display_prefix(secret: &str) -> String {
    secret.chars().take(DISPLAY_PREFIX_LEN).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_keys_have_expected_shape_and_are_unique() {
        let a = generate_key();
        let b = generate_key();
        assert!(a.starts_with(KEY_PREFIX));
        assert_eq!(a.len(), KEY_PREFIX.len() + KEY_RANDOM_LEN);
        assert!(
            a[KEY_PREFIX.len()..]
                .chars()
                .all(|c| c.is_ascii_alphanumeric())
        );
        assert_ne!(a, b);
    }

    #[test]
    fn hash_is_stable_sha256_hex() {
        assert_eq!(
            hash_key("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn display_prefix_takes_first_chars() {
        assert_eq!(display_prefix("rha_ABCDEFGHIJK"), "rha_ABCDEFGH");
    }
}
