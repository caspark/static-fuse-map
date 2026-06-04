use super::*;
use alloc::format;
#[cfg(feature = "serde")]
use alloc::string::ToString;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Id(u64);

impl StaticFuseValue for Id {
    fn into_u64(self) -> u64 {
        self.0
    }
    fn from_u64(raw: u64) -> Self {
        Id(raw)
    }
}

#[test]
fn basic() {
    let keys = ["apple", "banana", "cherry", "date", "elderberry"];
    let values = [100u64, 200, 300, 400, 500];
    let map = StaticFuseMap::new(&keys, &values).unwrap();
    for (key, value) in keys.iter().zip(values) {
        assert_eq!(map.get(key), value);
    }
}

#[test]
fn iterator_construction() {
    let map = StaticFuseMap::try_from_iter([("apple", 100u64), ("banana", 200)]).unwrap();
    assert_eq!(map.get("apple"), 100);
    assert_eq!(map.get("banana"), 200);
}

#[test]
fn empty() {
    let map = StaticFuseMap::<u64>::new::<&str>(&[], &[]).unwrap();
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
}

#[test]
fn large() {
    let n = 100_000usize;
    let mut keys = Vec::new();
    let mut values = Vec::new();
    for i in 0..n {
        keys.push(format!("key-{i}"));
        values.push((i * 7) as u64);
    }
    let map = StaticFuseMap::new(&keys, &values).unwrap();
    for (key, value) in keys.iter().zip(values) {
        assert_eq!(map.get(key), value);
    }
}

#[test]
fn mismatched_lengths() {
    let err = StaticFuseMap::new(&["a"], &[1u64, 2]).unwrap_err();
    assert!(matches!(err, BuildError::MismatchedLengths { .. }));
}

#[test]
fn duplicate_key_hash() {
    let err = StaticFuseMap::try_from_iter([("a", 1u64), ("a", 2)]).unwrap_err();
    assert!(matches!(
        err,
        BuildError::DuplicateHash | BuildError::PeelFailed { .. }
    ));
}

#[test]
fn randomish_values() {
    let n = 50_000usize;
    let mut x = 42u64;
    let mut keys = Vec::new();
    let mut values = Vec::new();
    for i in 0..n {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        keys.push(format!("random-key-{i}-{x}"));
        values.push(x);
    }
    let map = StaticFuseMap::new(&keys, &values).unwrap();
    for (key, value) in keys.iter().zip(values) {
        assert_eq!(map.get(key), value);
    }
}

#[test]
fn verified_basic() {
    let keys = ["apple", "banana", "cherry", "date", "elderberry"];
    let values = [100u64, 200, 300, 400, 500];
    let map = VerifiedStaticFuseMap::new(&keys, &values).unwrap();
    for (key, value) in keys.iter().zip(values) {
        assert_eq!(map.get(key), Some(value));
    }
}

#[test]
fn verified_missing() {
    let map =
        VerifiedStaticFuseMap::try_from_iter([("apple", 100u64), ("banana", 200), ("cherry", 300)])
            .unwrap();
    for key in ["grape", "kiwi", "mango", "pear", "plum"] {
        assert_eq!(map.get(key), None);
    }
}

#[test]
fn verified_large() {
    let n = 100_000usize;
    let mut keys = Vec::new();
    let mut values = Vec::new();
    for i in 0..n {
        keys.push(format!("key-{i}"));
        values.push((i * 7) as u64);
    }
    let map = VerifiedStaticFuseMap::new(&keys, &values).unwrap();
    for (key, value) in keys.iter().zip(values) {
        assert_eq!(map.get(key), Some(value));
    }
    for i in 0..10_000 {
        assert_eq!(map.get(&format!("missing-{i}")), None);
    }
}

#[test]
fn verified_empty() {
    let map = VerifiedStaticFuseMap::<u64>::new::<&str>(&[], &[]).unwrap();
    assert_eq!(map.get("anything"), None);
}

#[test]
fn verified_u64_max_is_value() {
    let map = VerifiedStaticFuseMap::try_from_iter([("sentinel", u64::MAX)]).unwrap();
    assert_eq!(map.get("sentinel"), Some(u64::MAX));
    assert_eq!(map.get("missing"), None);
}

#[test]
fn newtype_value() {
    let keys = ["a", "b"];
    let values = [Id(10), Id(20)];
    let map = StaticFuseMap::new(&keys, &values).unwrap();
    assert_eq!(map.get("a"), Id(10));
    assert_eq!(map.get("b"), Id(20));
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trip_static() {
    let map = StaticFuseMap::try_from_iter([("apple", 100u64), ("banana", 200)]).unwrap();
    let json = serde_json::to_string(&map).unwrap();
    let decoded: StaticFuseMap = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.get("apple"), 100);
    assert_eq!(decoded.get("banana"), 200);
}

#[cfg(feature = "serde")]
#[test]
fn serde_round_trip_verified() {
    let map =
        VerifiedStaticFuseMap::try_from_iter([("apple", 100u64), ("banana", u64::MAX)]).unwrap();
    let json = serde_json::to_string(&map).unwrap();
    let decoded: VerifiedStaticFuseMap = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.get("apple"), Some(100));
    assert_eq!(decoded.get("banana"), Some(u64::MAX));
    assert_eq!(decoded.get("missing"), None);
}

#[cfg(feature = "serde")]
#[test]
fn serde_rejects_malformed_static_map() {
    let json = r#"{"seed":1,"segment_length":4,"segment_count":1,"len":1,"data":[]}"#;
    let err = serde_json::from_str::<StaticFuseMap>(json).unwrap_err();
    assert!(err.to_string().contains("data length"));
}

#[cfg(feature = "serde")]
#[test]
fn serde_rejects_malformed_verified_map() {
    let json = r#"{"seed":1,"segment_length":4,"segment_count":1,"len":1,"data":[],"checks":[]}"#;
    let err = serde_json::from_str::<VerifiedStaticFuseMap>(json).unwrap_err();
    assert!(err.to_string().contains("data length"));
}

#[cfg(feature = "serde")]
#[test]
fn serde_rejects_mismatched_verified_checks() {
    let json = r#"{"seed":1,"segment_length":4,"segment_count":1,"len":1,"data":[0,0,0,0,0,0,0,0,0,0,0,0],"checks":[]}"#;
    let err = serde_json::from_str::<VerifiedStaticFuseMap>(json).unwrap_err();
    assert!(err.to_string().contains("checks length"));
}

#[cfg(all(feature = "serde", feature = "allocator-api2", feature = "std"))]
#[test]
fn serde_allocator_seed_round_trip() {
    use allocator_api2::alloc::System;
    use serde::de::DeserializeSeed;

    let map =
        StaticFuseMap::try_from_iter_in([("apple", 100u64), ("banana", 200)], System).unwrap();
    let json = serde_json::to_string(&map).unwrap();
    let mut de = serde_json::Deserializer::from_str(&json);
    let decoded: StaticFuseMap<u64, System> =
        StaticFuseMapSeed::new(System).deserialize(&mut de).unwrap();
    assert_eq!(decoded.get("apple"), 100);
    assert_eq!(decoded.get("banana"), 200);

    let verified =
        VerifiedStaticFuseMap::try_from_iter_in([("apple", 100u64), ("banana", u64::MAX)], System)
            .unwrap();
    let json = serde_json::to_string(&verified).unwrap();
    let mut de = serde_json::Deserializer::from_str(&json);
    let decoded: VerifiedStaticFuseMap<u64, System> = VerifiedStaticFuseMapSeed::new(System)
        .deserialize(&mut de)
        .unwrap();
    assert_eq!(decoded.get("apple"), Some(100));
    assert_eq!(decoded.get("banana"), Some(u64::MAX));
    assert_eq!(decoded.get("missing"), None);
}
