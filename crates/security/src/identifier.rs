use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub struct IdentifierHasher {
    key: Vec<u8>,
}

impl IdentifierHasher {
    pub fn new(key: &[u8]) -> Self {
        Self { key: key.to_vec() }
    }

    pub fn hash(&self, input: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("HMAC can take any key length");
        mac.update(input);
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_non_empty_hash() {
        let hasher = IdentifierHasher::new(b"test-key");
        let hash = hasher.hash(b"hello");
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn different_inputs_produce_different_hashes() {
        let hasher = IdentifierHasher::new(b"test-key");
        let h1 = hasher.hash(b"session-a");
        let h2 = hasher.hash(b"session-b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn different_keys_produce_different_hashes() {
        let h1 = IdentifierHasher::new(b"key-a");
        let h2 = IdentifierHasher::new(b"key-b");
        assert_ne!(h1.hash(b"data"), h2.hash(b"data"));
    }
}
