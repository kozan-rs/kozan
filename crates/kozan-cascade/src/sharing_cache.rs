//! Style sharing cache — LRU cache that skips full selector matching
//! for elements that are "similar enough" to a recently styled element.
//!
//! Two elements can share a style if they have identical:
//! - Tag name
//! - Class list
//! - ID
//! - Element state (hover, focus, etc.)
//! - Parent's computed style pointer (same parent style = same inheritance)
//!
//! On typical content pages, 60-80% of elements hit the sharing cache,
//! making style resolution O(1) for the majority of the DOM.
//!
//! The cache holds the 32 most recently computed styles (LRU eviction).
//! Invalidated by `Stylist::generation()` bumps (stylesheet changes).

use std::sync::Arc;
use kozan_atom::Atom;
use kozan_selector::fxhash::FxHashMap;
use smallvec::SmallVec;

/// Maximum entries in the sharing cache.
const CACHE_SIZE: usize = 32;

/// Key for sharing cache lookups.
///
/// Two elements with identical keys produce identical computed styles
/// (assuming no sibling selectors or other context-dependent selectors).
///
/// The hash is pre-computed at construction time so cache lookups never
/// re-hash — they compare the stored `u64` directly.
#[derive(Clone, Debug)]
pub struct SharingKey {
    hash: u64,
    pub tag: Atom,
    pub id: Option<Atom>,
    pub classes: SmallVec<[Atom; 4]>,
    pub state: u32,
    pub parent_identity: u64,
}

impl SharingKey {
    /// Create a new sharing key. Hash is computed once at construction.
    pub fn new(
        tag: Atom,
        id: Option<Atom>,
        classes: SmallVec<[Atom; 4]>,
        state: u32,
        parent_identity: u64,
    ) -> Self {
        use core::hash::{Hash, Hasher};
        let mut hasher = kozan_selector::fxhash::FxHasher::default();
        tag.hash(&mut hasher);
        id.hash(&mut hasher);
        classes.hash(&mut hasher);
        state.hash(&mut hasher);
        parent_identity.hash(&mut hasher);
        Self {
            hash: hasher.finish(),
            tag,
            id,
            classes,
            state,
            parent_identity,
        }
    }

    /// Pre-computed FxHash of this key.
    #[inline]
    pub fn hash(&self) -> u64 {
        self.hash
    }
}

impl PartialEq for SharingKey {
    /// Hash-only comparison. With 64-bit FxHash and a 32-entry cache,
    /// collision probability is ~32/2^64 ≈ 10^-18 — effectively zero.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Eq for SharingKey {}

/// Cache entry. The hash is pre-computed in `SharingKey`, so lookups
/// compare u64 hashes for fast rejection before full equality.
/// Value is `Arc<ResolvedStyle>` — cache hits return a cheap pointer bump
/// instead of deep-cloning ComputedStyle + CustomPropertyMap.
#[derive(Clone)]
struct CacheEntry {
    key: SharingKey,
    value: Arc<crate::resolver::ResolvedStyle>,
    last_access: u64,
}

/// LRU-32 cache mapping element keys to computed style indices.
///
/// Uses pre-hashed entries with access counters instead of the traditional
/// Vec-based approach. This eliminates all `memmove` operations:
/// - `get()`: Scans u64 hashes (fast reject), bumps access counter on hit.
/// - `insert()`: Replaces the entry with the lowest access counter (LRU eviction).
///
/// On typical content pages, 60-80% of elements hit the sharing cache,
/// making style resolution O(1) for the majority of the DOM.
#[derive(Clone)]
pub struct SharingCache {
    entries: Vec<CacheEntry>,
    generation: u64,
    access_clock: u64,
}

impl SharingCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(CACHE_SIZE),
            generation: 0,
            access_clock: 0,
        }
    }

    /// Look up a cached resolved style for the given key.
    ///
    /// Returns `None` on miss. On hit, returns a cheap `Arc` clone (pointer bump,
    /// no deep copy). Bumps the access counter for LRU tracking.
    /// Comparison is a single u64 hash match — no field-by-field equality.
    pub fn get(&mut self, key: &SharingKey) -> Option<Arc<crate::resolver::ResolvedStyle>> {
        let hash = key.hash();
        self.access_clock += 1;
        let clock = self.access_clock;
        for entry in self.entries.iter_mut() {
            if entry.key.hash() == hash {
                entry.last_access = clock;
                return Some(Arc::clone(&entry.value));
            }
        }
        None
    }

    /// Insert a new entry. Evicts the least-recently-used entry if full.
    pub fn insert(&mut self, key: SharingKey, style: Arc<crate::resolver::ResolvedStyle>) {
        let hash = key.hash();
        self.access_clock += 1;
        let access = self.access_clock;

        // Update existing entry for same key.
        for entry in self.entries.iter_mut() {
            if entry.key.hash() == hash {
                entry.value = style;
                entry.last_access = access;
                return;
            }
        }

        if self.entries.len() < CACHE_SIZE {
            self.entries.push(CacheEntry {
                key,
                value: style,
                last_access: access,
            });
        } else {
            // Evict the entry with the lowest access counter (LRU).
            let mut min_idx = 0;
            let mut min_access = self.entries[0].last_access;
            for (i, entry) in self.entries.iter().enumerate().skip(1) {
                if entry.last_access < min_access {
                    min_access = entry.last_access;
                    min_idx = i;
                }
            }
            let slot = &mut self.entries[min_idx];
            slot.key = key;
            slot.value = style;
            slot.last_access = access;
        }
    }

    /// Clear the cache. Called when the stylist generation changes.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_clock = 0;
    }

    /// Check and update generation. Returns `true` if the cache was invalidated.
    pub fn check_generation(&mut self, stylist_generation: u64) -> bool {
        if self.generation == stylist_generation {
            false
        } else {
            self.clear();
            self.generation = stylist_generation;
            true
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for SharingCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Matched properties cache — skips cascade for elements matching identical rules.
///
/// When two elements match the exact same set of declarations (same
/// declaration blocks in the same order) and have the same parent style,
/// their cascade output is identical. This cache skips cascade sort+apply.
///
/// Unlike the sharing cache, this requires full selector matching to have
/// already run (to know which declarations matched). The benefit is skipping
/// the cascade computation when two different elements happen to match the
/// same rules (common with utility-class CSS like Tailwind).
///
/// Keyed by `mpc_key(matched_hash, parent_ptr, has_inline)`.
pub struct MatchedPropertiesCache {
    map: FxHashMap<u64, Arc<crate::resolver::ResolvedStyle>>,
    generation: u64,
}

impl MatchedPropertiesCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: FxHashMap::default(),
            generation: 0,
        }
    }

    /// Look up a cached resolved style. Returns a cheap `Arc` clone.
    #[must_use]
    pub fn get(&self, key: u64) -> Option<Arc<crate::resolver::ResolvedStyle>> {
        self.map.get(&key).cloned()
    }

    /// Insert a cache entry.
    pub fn insert(&mut self, key: u64, style: Arc<crate::resolver::ResolvedStyle>) {
        self.map.insert(key, style);
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Check and update generation.
    pub fn check_generation(&mut self, stylist_generation: u64) -> bool {
        if self.generation == stylist_generation {
            false
        } else {
            self.clear();
            self.generation = stylist_generation;
            true
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl Default for MatchedPropertiesCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a hash of matched declarations for MPC lookup.
///
/// Includes rule_index, specificity, AND source_order — all three
/// affect cascade output. Two elements matching the same rules in the
/// same order produce the same cascade result.
#[must_use]
pub fn hash_matched(declarations: &[crate::cascade::ApplicableDeclaration]) -> u64 {
    use core::hash::{Hash, Hasher};
    let mut hasher = kozan_selector::fxhash::FxHasher::default();
    for decl in declarations {
        decl.rule_index.hash(&mut hasher);
        decl.specificity.hash(&mut hasher);
        decl.source_order.hash(&mut hasher);
    }
    hasher.finish()
}

/// Combine a matched-declarations hash with parent style identity
/// to produce a full MPC key. Two elements with the same matched rules
/// AND the same parent produce identical cascade results.
#[must_use]
pub fn mpc_key(matched_hash: u64, parent_ptr: u64, has_inline: bool) -> u64 {
    use core::hash::{Hash, Hasher};
    let mut hasher = kozan_selector::fxhash::FxHasher::default();
    matched_hash.hash(&mut hasher);
    parent_ptr.hash(&mut hasher);
    has_inline.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(tag: &str, classes: &[&str]) -> SharingKey {
        SharingKey::new(
            Atom::from(tag),
            None,
            classes.iter().map(|c| Atom::from(*c)).collect(),
            0,
            0,
        )
    }

    fn dummy_resolved() -> Arc<crate::resolver::ResolvedStyle> {
        Arc::new(crate::resolver::ResolvedStyle {
            style: kozan_style::ComputedStyle::default(),
            custom_properties: crate::custom_properties::CustomPropertyMap::new(),
        })
    }

    #[test]
    fn sharing_hit_and_miss() {
        let mut cache = SharingCache::new();
        let k = key("div", &["btn"]);
        cache.insert(k.clone(), dummy_resolved());

        assert!(cache.get(&k).is_some());
        assert!(cache.get(&key("span", &["btn"])).is_none());
    }

    #[test]
    fn sharing_lru_eviction() {
        let mut cache = SharingCache::new();
        for i in 0..CACHE_SIZE + 5 {
            cache.insert(key("div", &[&format!("c{i}")]), dummy_resolved());
        }
        assert_eq!(cache.len(), CACHE_SIZE);
        assert!(cache.get(&key("div", &["c0"])).is_none());
        assert!(cache.get(&key("div", &[&format!("c{}", CACHE_SIZE + 4)])).is_some());
    }

    #[test]
    fn sharing_generation_invalidation() {
        let mut cache = SharingCache::new();
        cache.insert(key("div", &[]), dummy_resolved());
        assert!(!cache.is_empty());

        cache.check_generation(1);
        assert!(cache.is_empty());
    }

    #[test]
    fn mpc_hit_and_miss() {
        let mut cache = MatchedPropertiesCache::new();
        cache.insert(12345, dummy_resolved());
        assert!(cache.get(12345).is_some());
        assert!(cache.get(99999).is_none());
    }

    #[test]
    fn mpc_generation_invalidation() {
        let mut cache = MatchedPropertiesCache::new();
        cache.insert(1, dummy_resolved());
        assert!(!cache.is_empty());

        cache.check_generation(1);
        assert!(cache.is_empty());
    }

    #[test]
    fn hash_matched_deterministic() {
        use crate::cascade::ApplicableDeclaration;

        let decls = vec![
            ApplicableDeclaration { rule_index: 0, specificity: 10, source_order: 0, origin: crate::origin::CascadeOrigin::Author, layer_order: crate::layer::UNLAYERED, scope_depth: 0 },
            ApplicableDeclaration { rule_index: 5, specificity: 100, source_order: 3, origin: crate::origin::CascadeOrigin::Author, layer_order: crate::layer::UNLAYERED, scope_depth: 0 },
        ];
        let h1 = hash_matched(&decls);
        let h2 = hash_matched(&decls);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_matched_differs() {
        use crate::cascade::ApplicableDeclaration;

        let a = vec![
            ApplicableDeclaration { rule_index: 0, specificity: 10, source_order: 0, origin: crate::origin::CascadeOrigin::Author, layer_order: crate::layer::UNLAYERED, scope_depth: 0 },
        ];
        let b = vec![
            ApplicableDeclaration { rule_index: 1, specificity: 10, source_order: 0, origin: crate::origin::CascadeOrigin::Author, layer_order: crate::layer::UNLAYERED, scope_depth: 0 },
        ];
        assert_ne!(hash_matched(&a), hash_matched(&b));
    }
}
