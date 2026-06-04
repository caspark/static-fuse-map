use xxhash_rust::xxh64::xxh64;

#[inline]
fn murmur64(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
    h ^= h >> 33;
    h
}

#[inline]
pub(crate) fn mixsplit(key: u64, seed: u64) -> u64 {
    murmur64(key.wrapping_add(seed))
}

#[inline]
pub(crate) fn splitmix64(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

#[inline]
pub(crate) fn fingerprint(hash: u64) -> u64 {
    hash ^ (hash >> 32)
}

#[inline]
pub(crate) fn hash_key(key: &str) -> u64 {
    xxh64(key.as_bytes(), 0)
}
