use alloc::vec::Vec;
#[cfg(feature = "serde")]
use core::fmt;
use core::marker::PhantomData;

use crate::build::{build_from_iter, build_from_slices, BuildOutput, DefaultStorage};
use crate::hash::{fingerprint, hash_key, mixsplit};
use crate::params::Params;
use crate::{BuildError, StaticFuseValue};

/// A compact immutable map from strings to `u64`-encoded values.
///
/// Lookups are unchecked: if a key was not present during construction, the
/// returned value is unspecified.
#[derive(Clone, Debug)]
pub struct StaticFuseMap<V = u64> {
    params: Params,
    data: Vec<u64>,
    marker: PhantomData<V>,
}

/// A compact immutable map from strings to `u64`-encoded values with probabilistic missing-key checks.
///
/// Missing-key lookups return `None` except for a false positive with
/// probability about 2^-64 (roughly 1 in 18.4 quintillion), in which case an
/// unspecified `Some` value is returned instead.
#[derive(Clone, Debug)]
pub struct VerifiedStaticFuseMap<V = u64> {
    params: Params,
    data: Vec<u64>,
    checks: Vec<u64>,
    marker: PhantomData<V>,
}

impl<V: StaticFuseValue> StaticFuseMap<V> {
    /// Builds a map from parallel key and value slices.
    pub fn new<K: AsRef<str>>(keys: &[K], values: &[V]) -> Result<Self, BuildError> {
        Self::from_built(build_from_slices(keys, values, false, &DefaultStorage)?)
    }

    /// Builds a map from an iterator of key/value pairs.
    pub fn try_from_iter<I, K>(pairs: I) -> Result<Self, BuildError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
    {
        Self::from_built(build_from_iter(pairs, false, &DefaultStorage)?)
    }

    fn from_built(built: BuildOutput<DefaultStorage>) -> Result<Self, BuildError> {
        Ok(Self {
            params: built.params,
            data: built.data,
            marker: PhantomData,
        })
    }

    /// Returns the number of keys used to build the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.params.len
    }

    /// Returns true if the map was built from no keys.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.params.len == 0
    }

    /// Returns the value for `key`.
    ///
    /// The key must have been present during construction. If not, the
    /// returned value is unspecified.
    #[inline]
    pub fn get(&self, key: &str) -> V {
        let hash = mixsplit(hash_key(key), self.params.seed);
        let (h0, h1, h2) = self.params.get_hash_from_hash(hash);
        V::from_u64(self.data[h0 as usize] ^ self.data[h1 as usize] ^ self.data[h2 as usize])
    }

    /// Returns the retained table size in bytes, excluding the struct itself.
    #[inline]
    pub fn table_bytes(&self) -> usize {
        self.data.capacity() * core::mem::size_of::<u64>()
    }
}

impl<V: StaticFuseValue> VerifiedStaticFuseMap<V> {
    /// Builds a verified map from parallel key and value slices.
    pub fn new<K: AsRef<str>>(keys: &[K], values: &[V]) -> Result<Self, BuildError> {
        Self::from_built(build_from_slices(keys, values, true, &DefaultStorage)?)
    }

    /// Builds a verified map from an iterator of key/value pairs.
    pub fn try_from_iter<I, K>(pairs: I) -> Result<Self, BuildError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
    {
        Self::from_built(build_from_iter(pairs, true, &DefaultStorage)?)
    }

    fn from_built(built: BuildOutput<DefaultStorage>) -> Result<Self, BuildError> {
        Ok(Self {
            params: built.params,
            data: built.data,
            checks: built.checks.expect("verified build returns checks"),
            marker: PhantomData,
        })
    }

    /// Returns the number of keys used to build the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.params.len
    }

    /// Returns true if the map was built from no keys.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.params.len == 0
    }

    /// Returns the value for `key`, or `None` if the key is probably absent.
    ///
    /// Missing-key detection is probabilistic: a missing key has about a 2^-64
    /// chance (roughly 1 in 18.4 quintillion) of passing the fingerprint check
    /// and returning an unspecified `Some` value instead of `None`.
    #[inline]
    pub fn get(&self, key: &str) -> Option<V> {
        if self.data.is_empty() {
            return None;
        }
        let hash = mixsplit(hash_key(key), self.params.seed);
        let (h0, h1, h2) = self.params.get_hash_from_hash(hash);
        let h0 = h0 as usize;
        let h1 = h1 as usize;
        let h2 = h2 as usize;
        let fp = self.checks[h0] ^ self.checks[h1] ^ self.checks[h2];
        if fp != fingerprint(hash) {
            return None;
        }
        Some(V::from_u64(self.data[h0] ^ self.data[h1] ^ self.data[h2]))
    }

    /// Returns the retained table size in bytes, excluding the struct itself.
    #[inline]
    pub fn table_bytes(&self) -> usize {
        (self.data.capacity() + self.checks.capacity()) * core::mem::size_of::<u64>()
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::*;
    use serde::de::{Error as DeError, MapAccess, Visitor};
    use serde::ser::SerializeStruct;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    const FIELDS: &[&str] = &[
        "seed",
        "segment_length",
        "segment_count",
        "len",
        "data",
        "checks",
    ];

    impl<V> Serialize for StaticFuseMap<V> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut st = serializer.serialize_struct("StaticFuseMap", 5)?;
            st.serialize_field("seed", &self.params.seed)?;
            st.serialize_field("segment_length", &self.params.segment_length)?;
            st.serialize_field("segment_count", &self.params.segment_count)?;
            st.serialize_field("len", &self.params.len)?;
            st.serialize_field("data", &self.data)?;
            st.end()
        }
    }

    impl<'de, V> Deserialize<'de> for StaticFuseMap<V> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let raw: RawMap = deserializer.deserialize_struct(
                "StaticFuseMap",
                FIELDS,
                MapVisitor::<V> {
                    verified: false,
                    marker: PhantomData,
                },
            )?;
            raw.params
                .validate(raw.data.len(), None)
                .map_err(D::Error::custom)?;
            Ok(Self {
                params: raw.params,
                data: raw.data,
                marker: PhantomData,
            })
        }
    }

    impl<V> Serialize for VerifiedStaticFuseMap<V> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut st = serializer.serialize_struct("VerifiedStaticFuseMap", 6)?;
            st.serialize_field("seed", &self.params.seed)?;
            st.serialize_field("segment_length", &self.params.segment_length)?;
            st.serialize_field("segment_count", &self.params.segment_count)?;
            st.serialize_field("len", &self.params.len)?;
            st.serialize_field("data", &self.data)?;
            st.serialize_field("checks", &self.checks)?;
            st.end()
        }
    }

    impl<'de, V> Deserialize<'de> for VerifiedStaticFuseMap<V> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let raw: RawMap = deserializer.deserialize_struct(
                "VerifiedStaticFuseMap",
                FIELDS,
                MapVisitor::<V> {
                    verified: true,
                    marker: PhantomData,
                },
            )?;
            let params = raw.params;
            let checks = raw
                .checks
                .ok_or_else(|| D::Error::missing_field("checks"))?;
            params
                .validate(raw.data.len(), Some(checks.len()))
                .map_err(D::Error::custom)?;
            Ok(Self {
                params,
                data: raw.data,
                checks,
                marker: PhantomData,
            })
        }
    }

    struct RawMap {
        params: Params,
        data: Vec<u64>,
        checks: Option<Vec<u64>>,
    }

    struct MapVisitor<V> {
        verified: bool,
        marker: PhantomData<V>,
    }

    impl<'de, V> Visitor<'de> for MapVisitor<V> {
        type Value = RawMap;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a static fuse map")
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut seed = None;
            let mut segment_length = None;
            let mut segment_count = None;
            let mut len = None;
            let mut data = None;
            let mut checks = None;

            while let Some(key) = map.next_key::<&str>()? {
                match key {
                    "seed" => seed = Some(map.next_value()?),
                    "segment_length" => segment_length = Some(map.next_value()?),
                    "segment_count" => segment_count = Some(map.next_value()?),
                    "len" => len = Some(map.next_value()?),
                    "data" => data = Some(map.next_value()?),
                    "checks" => checks = Some(map.next_value()?),
                    _ => return Err(M::Error::unknown_field(key, FIELDS)),
                }
            }

            let segment_length =
                segment_length.ok_or_else(|| M::Error::missing_field("segment_length"))?;
            let segment_count =
                segment_count.ok_or_else(|| M::Error::missing_field("segment_count"))?;
            let params = Params {
                seed: seed.ok_or_else(|| M::Error::missing_field("seed"))?,
                segment_length,
                segment_length_mask: segment_length.wrapping_sub(1),
                segment_count,
                segment_count_length: segment_count.saturating_mul(segment_length),
                len: len.ok_or_else(|| M::Error::missing_field("len"))?,
            };
            let data = data.ok_or_else(|| M::Error::missing_field("data"))?;
            let raw = RawMap {
                params,
                data,
                checks,
            };
            if !self.verified {
                raw.params
                    .validate(raw.data.len(), None)
                    .map_err(M::Error::custom)?;
            }
            Ok(raw)
        }
    }
}
