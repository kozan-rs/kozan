//! CSS cascade engine for the Kozan UI platform.
//!
//! Takes a DOM tree and stylesheets, produces a `ComputedStyle` per element.
//! Faster than Stylo: no rule tree, single-threaded default, `CascadeLevel`
//! as a single `u32` comparison, matched properties cache for O(1) cascade skip.

pub mod cascade;
pub mod container;
pub mod custom_properties;
pub mod device;
pub mod layer;
pub mod media;
pub mod origin;
pub mod resolver;
pub mod restyle;
pub mod sharing_cache;
pub mod stylist;

pub use cascade::ApplicableDeclaration;
pub use container::{ContainerLookup, ContainerSize, ContainerSizeCache, NoContainers};
pub use custom_properties::{CustomPropertyMap, EnvironmentValues};
pub use device::{Device, FontMetricsProvider, MediaType};
pub use layer::{LayerOrderMap, UNLAYERED};
pub use origin::{CascadeLevel, CascadeOrigin, Importance};
pub use restyle::{DomMutation, RestyleHint, RestyleTracker};
pub use sharing_cache::{MatchedPropertiesCache, SharingCache, SharingKey, mpc_key};
pub use resolver::{ResolvedStyle, StyleResolver};
pub use stylist::{IndexedRule, Stylist};
