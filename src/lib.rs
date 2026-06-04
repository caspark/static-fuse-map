//! A compact immutable string map based on binary fuse filters.
//!
//! `static-fuse-map` is intended for cases where all keys are known up front and
//! lookups dominate construction. Values are stored as `u64` internally and are
//! exposed through [`StaticFuseValue`].
//!
//! ```
//! use static_fuse_map::StaticFuseMap;
//!
//! let map = StaticFuseMap::try_from_iter([
//!     ("apple", 100u64),
//!     ("banana", 200),
//! ])?;
//!
//! assert_eq!(map.get("banana"), 200);
//! # Ok::<(), static_fuse_map::BuildError>(())
//! ```
//!
//! [`StaticFuseMap`] is the smallest and fastest map, but lookups are unchecked:
//! querying a key that was not present during construction returns an unspecified
//! value. Use [`VerifiedStaticFuseMap`] for a probabilistically checked API:
//! missing keys return `None` except for a false positive with probability about
//! 2^-64 (roughly 1 in 18.4 quintillion), in which case an unspecified `Some`
//! value is returned instead.
//!
//! ```
//! use static_fuse_map::VerifiedStaticFuseMap;
//!
//! let map = VerifiedStaticFuseMap::try_from_iter([
//!     ("apple", 100u64),
//!     ("banana", u64::MAX),
//! ])?;
//!
//! assert_eq!(map.get("banana"), Some(u64::MAX));
//! assert_eq!(map.get("missing"), None);
//! # Ok::<(), static_fuse_map::BuildError>(())
//! ```
//!
//! The crate is `no_std` capable. Disable default features and enable `libm` for
//! the construction-time floating-point math:
//!
//! ```toml
//! static-fuse-map = { version = "0.1", default-features = false, features = ["libm"] }
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(all(not(feature = "std"), not(feature = "libm")))]
compile_error!("no_std builds require enabling the `libm` feature");

pub(crate) const MAX_ITERATIONS: u32 = 100;

mod build;
mod error;
mod hash;
mod math;
mod params;
mod value;

#[cfg(feature = "allocator-api2")]
mod allocator_impl;
#[cfg(not(feature = "allocator-api2"))]
mod default_impl;

pub use error::BuildError;
pub use value::StaticFuseValue;

#[cfg(feature = "allocator-api2")]
pub use allocator_api2::alloc::Global;

#[cfg(feature = "allocator-api2")]
pub use allocator_impl::{StaticFuseMap, VerifiedStaticFuseMap};
#[cfg(all(feature = "allocator-api2", feature = "serde"))]
pub use allocator_impl::{StaticFuseMapSeed, VerifiedStaticFuseMapSeed};

#[cfg(not(feature = "allocator-api2"))]
pub use default_impl::{StaticFuseMap, VerifiedStaticFuseMap};

#[cfg(test)]
mod tests;
