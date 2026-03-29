//! FxHash — fast, non-cryptographic hasher for internal data structures.
//!
//! A single-multiply-per-word hash function that is 2-5x faster than
//! Rust's default SipHash for small keys (pointers, integers, short strings).
//!
//! # Where It's Used
//!
//! - **`RuleMap`**: HashMap<Atom, BucketRange> for O(1) rule lookup by
//!   tag/class/ID. Since Atoms are pointer-hashed, FxHash processes them
//!   in 1 multiply (vs SipHash's ~12 rounds).
//! - **`NthIndexCache`**: HashMap<OpaqueElement, i32> for cached nth indices.
//! - **Bloom filter**: `FxHasher` generates the initial hash value that is
//!   then split into 3 slot indices via multiplicative mixing.
//!
//! # Security
//!
//! NOT DoS-resistant. FxHash has no randomized seed, so an attacker who
//! controls keys can craft hash collisions. This is safe because:
//! - RuleMap keys are interned Atoms (controlled by the stylesheet parser)
//! - NthIndexCache keys are OpaqueElements (controlled by the DOM)
//! - No user input is ever used as a direct hash key
//!
//! # Algorithm
//!
//! For each word-sized chunk: `hash = hash.wrapping_mul(SEED).wrapping_add(word)`
//! where `SEED = 0x517CC1B727220A95` (a prime with good bit-mixing properties).
//! No finalization step — the accumulated multiply-add chain IS the hash.
//!
//! Originally from Firefox/Stylo (hence "Fx"). Our implementation matches
//! the `rustc-hash` crate but avoids the external dependency.

use std::hash::{BuildHasher, Hasher};

const SEED: u64 = 0x517CC1B727220A95;

/// FxHash-style hasher — single multiply per word, no finalization.
#[derive(Default)]
pub struct FxHasher {
    hash: u64,
}

impl Hasher for FxHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut remaining = bytes;
        while remaining.len() >= 8 {
            let val = u64::from_ne_bytes(remaining[..8].try_into().unwrap());
            self.hash = self.hash.wrapping_mul(SEED).wrapping_add(val);
            remaining = &remaining[8..];
        }
        if remaining.len() >= 4 {
            let val = u32::from_ne_bytes(remaining[..4].try_into().unwrap()) as u64;
            self.hash = self.hash.wrapping_mul(SEED).wrapping_add(val);
            remaining = &remaining[4..];
        }
        for &byte in remaining {
            self.hash = self.hash.wrapping_mul(SEED).wrapping_add(byte as u64);
        }
    }

    #[inline]
    fn write_usize(&mut self, val: usize) {
        self.hash = self.hash.wrapping_mul(SEED).wrapping_add(val as u64);
    }

    #[inline]
    fn write_u64(&mut self, val: u64) {
        self.hash = self.hash.wrapping_mul(SEED).wrapping_add(val);
    }
}

/// `BuildHasher` for `HashMap<K, V, FxBuildHasher>`.
#[derive(Default, Clone)]
pub struct FxBuildHasher;

impl BuildHasher for FxBuildHasher {
    type Hasher = FxHasher;

    #[inline]
    fn build_hasher(&self) -> FxHasher {
        FxHasher::default()
    }
}

/// Type alias for a HashMap using FxHash.
pub type FxHashMap<K, V> = std::collections::HashMap<K, V, FxBuildHasher>;
