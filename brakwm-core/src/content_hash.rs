/// Combine two 64-bit hashes into one.
pub fn combine_hash(a: u64, b: u64) -> u64 {
    a.rotate_left(5) ^ b.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Content-defined hashing used to detect IR changes between pipeline stages
/// (incremental compilation and optimizer convergence checks).
pub trait ContentHash {
    fn content_hash(&self) -> u64;
}

impl ContentHash for &str {
    fn content_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in self.bytes() {
            h = combine_hash(h, b as u64);
        }
        h
    }
}

impl ContentHash for str {
    fn content_hash(&self) -> u64 {
        ContentHash::content_hash(&self)
    }
}

impl ContentHash for String {
    fn content_hash(&self) -> u64 {
        self.as_str().content_hash()
    }
}

impl ContentHash for u64 {
    fn content_hash(&self) -> u64 {
        *self
    }
}

impl ContentHash for usize {
    fn content_hash(&self) -> u64 {
        *self as u64
    }
}

impl<T: ContentHash> ContentHash for Vec<T> {
    fn content_hash(&self) -> u64 {
        let mut h = self.len() as u64;
        for item in self {
            h = combine_hash(h, item.content_hash());
        }
        h
    }
}

impl<T: ContentHash> ContentHash for Option<T> {
    fn content_hash(&self) -> u64 {
        match self {
            Some(v) => combine_hash(1, v.content_hash()),
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_hash_is_stable_and_sensitive() {
        let a = "hello".content_hash();
        assert_eq!(a, "hello".content_hash());
        assert_ne!(a, "hellp".content_hash());
    }

    #[test]
    fn combine_deterministic() {
        assert_eq!(combine_hash(1, 2), combine_hash(1, 2));
    }
}