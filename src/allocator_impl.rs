use allocator_api2::alloc::Allocator;
use allocator_api2::vec::Vec;
#[cfg(feature = "serde")]
use core::fmt;
use core::marker::PhantomData;

use crate::build::{build_from_iter, build_from_slices, BuildOutput};
use crate::hash::{fingerprint, hash_key, mixsplit};
use crate::params::Params;
use crate::{BuildError, Global, StaticFuseValue};

/// A compact immutable map from strings to `u64`-encoded values.
///
/// Lookups are unchecked: if a key was not present during construction, the
/// returned value is unspecified.
#[derive(Clone, Debug)]
pub struct StaticFuseMap<V = u64, A: Allocator = Global> {
    params: Params,
    data: Vec<u64, A>,
    marker: PhantomData<V>,
}

/// A compact immutable map from strings to `u64`-encoded values with probabilistic missing-key checks.
///
/// Missing-key lookups return `None` except for a false positive with
/// probability about 2^-64 (roughly 1 in 18.4 quintillion), in which case an
/// unspecified `Some` value is returned instead.
#[derive(Clone, Debug)]
pub struct VerifiedStaticFuseMap<V = u64, A: Allocator = Global> {
    params: Params,
    data: Vec<u64, A>,
    checks: Vec<u64, A>,
    marker: PhantomData<V>,
}

impl<V: StaticFuseValue> StaticFuseMap<V, Global> {
    /// Builds a map from parallel key and value slices.
    pub fn new<K: AsRef<str>>(keys: &[K], values: &[V]) -> Result<Self, BuildError> {
        Self::new_in(keys, values, Global)
    }

    /// Builds a map from an iterator of key/value pairs.
    pub fn try_from_iter<I, K>(pairs: I) -> Result<Self, BuildError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
    {
        Self::try_from_iter_in(pairs, Global)
    }
}

impl<V: StaticFuseValue, A: Allocator + Clone> StaticFuseMap<V, A> {
    /// Builds a map from parallel key and value slices in `alloc`.
    pub fn new_in<K: AsRef<str>>(keys: &[K], values: &[V], alloc: A) -> Result<Self, BuildError> {
        Self::from_built(build_from_slices(keys, values, false, &alloc)?)
    }

    /// Builds a map from an iterator of key/value pairs in `alloc`.
    pub fn try_from_iter_in<I, K>(pairs: I, alloc: A) -> Result<Self, BuildError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
    {
        Self::from_built(build_from_iter(pairs, false, &alloc)?)
    }

    fn from_built(built: BuildOutput<A>) -> Result<Self, BuildError> {
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

impl<V: StaticFuseValue> VerifiedStaticFuseMap<V, Global> {
    /// Builds a verified map from parallel key and value slices.
    pub fn new<K: AsRef<str>>(keys: &[K], values: &[V]) -> Result<Self, BuildError> {
        Self::new_in(keys, values, Global)
    }

    /// Builds a verified map from an iterator of key/value pairs.
    pub fn try_from_iter<I, K>(pairs: I) -> Result<Self, BuildError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
    {
        Self::try_from_iter_in(pairs, Global)
    }
}

impl<V: StaticFuseValue, A: Allocator + Clone> VerifiedStaticFuseMap<V, A> {
    /// Builds a verified map from parallel key and value slices in `alloc`.
    pub fn new_in<K: AsRef<str>>(keys: &[K], values: &[V], alloc: A) -> Result<Self, BuildError> {
        Self::from_built(build_from_slices(keys, values, true, &alloc)?)
    }

    /// Builds a verified map from an iterator of key/value pairs in `alloc`.
    pub fn try_from_iter_in<I, K>(pairs: I, alloc: A) -> Result<Self, BuildError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
    {
        Self::from_built(build_from_iter(pairs, true, &alloc)?)
    }

    fn from_built(built: BuildOutput<A>) -> Result<Self, BuildError> {
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
    use serde::de::{DeserializeSeed, Error as DeError, MapAccess, Visitor};
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

    /// Serde seed for deserializing a [`StaticFuseMap`] into a chosen allocator.
    pub struct StaticFuseMapSeed<V, A> {
        alloc: A,
        marker: PhantomData<V>,
    }

    impl<V, A> StaticFuseMapSeed<V, A> {
        pub fn new(alloc: A) -> Self {
            Self {
                alloc,
                marker: PhantomData,
            }
        }
    }

    /// Serde seed for deserializing a [`VerifiedStaticFuseMap`] into a chosen allocator.
    pub struct VerifiedStaticFuseMapSeed<V, A> {
        alloc: A,
        marker: PhantomData<V>,
    }

    impl<V, A> VerifiedStaticFuseMapSeed<V, A> {
        pub fn new(alloc: A) -> Self {
            Self {
                alloc,
                marker: PhantomData,
            }
        }
    }

    impl<V, A: Allocator> Serialize for StaticFuseMap<V, A> {
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

    impl<'de, V> Deserialize<'de> for StaticFuseMap<V, Global> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            StaticFuseMapSeed::<V, Global>::new(Global).deserialize(deserializer)
        }
    }

    impl<V, A: Allocator> Serialize for VerifiedStaticFuseMap<V, A> {
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

    impl<'de, V> Deserialize<'de> for VerifiedStaticFuseMap<V, Global> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            VerifiedStaticFuseMapSeed::<V, Global>::new(Global).deserialize(deserializer)
        }
    }

    impl<'de, V, A: Allocator + Clone> DeserializeSeed<'de> for StaticFuseMapSeed<V, A> {
        type Value = StaticFuseMap<V, A>;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            let raw = deserializer.deserialize_struct(
                "StaticFuseMap",
                FIELDS,
                MapVisitor::<V, A> {
                    verified: false,
                    alloc: self.alloc,
                    marker: PhantomData,
                },
            )?;
            raw.params
                .validate(raw.data.len(), None)
                .map_err(D::Error::custom)?;
            Ok(StaticFuseMap {
                params: raw.params,
                data: raw.data,
                marker: PhantomData,
            })
        }
    }

    impl<'de, V, A: Allocator + Clone> DeserializeSeed<'de> for VerifiedStaticFuseMapSeed<V, A> {
        type Value = VerifiedStaticFuseMap<V, A>;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            let raw = deserializer.deserialize_struct(
                "VerifiedStaticFuseMap",
                FIELDS,
                MapVisitor::<V, A> {
                    verified: true,
                    alloc: self.alloc,
                    marker: PhantomData,
                },
            )?;
            let checks = raw
                .checks
                .ok_or_else(|| D::Error::missing_field("checks"))?;
            raw.params
                .validate(raw.data.len(), Some(checks.len()))
                .map_err(D::Error::custom)?;
            Ok(VerifiedStaticFuseMap {
                params: raw.params,
                data: raw.data,
                checks,
                marker: PhantomData,
            })
        }
    }

    struct RawMap<A: Allocator> {
        params: Params,
        data: Vec<u64, A>,
        checks: Option<Vec<u64, A>>,
    }

    struct MapVisitor<V, A> {
        verified: bool,
        alloc: A,
        marker: PhantomData<V>,
    }

    impl<'de, V, A: Allocator + Clone> Visitor<'de> for MapVisitor<V, A> {
        type Value = RawMap<A>;

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
                    "data" => data = Some(map.next_value_seed(VecSeed::new(self.alloc.clone()))?),
                    "checks" => {
                        checks = Some(map.next_value_seed(VecSeed::new(self.alloc.clone()))?)
                    }
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

    struct VecSeed<A> {
        alloc: A,
    }

    impl<A> VecSeed<A> {
        fn new(alloc: A) -> Self {
            Self { alloc }
        }
    }

    impl<'de, A: Allocator> DeserializeSeed<'de> for VecSeed<A> {
        type Value = Vec<u64, A>;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct VecVisitor<A> {
                alloc: A,
            }
            impl<'de, A: Allocator> Visitor<'de> for VecVisitor<A> {
                type Value = Vec<u64, A>;
                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a sequence of u64")
                }
                fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
                where
                    S: serde::de::SeqAccess<'de>,
                {
                    let mut values = Vec::new_in(self.alloc);
                    if let Some(hint) = seq.size_hint() {
                        values.try_reserve(hint).map_err(S::Error::custom)?;
                    }
                    while let Some(value) = seq.next_element()? {
                        values.push(value);
                    }
                    Ok(values)
                }
            }
            deserializer.deserialize_seq(VecVisitor { alloc: self.alloc })
        }
    }
}

#[cfg(feature = "serde")]
pub use serde_impl::{StaticFuseMapSeed, VerifiedStaticFuseMapSeed};
