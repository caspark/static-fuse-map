use crate::error::BuildError;
use crate::hash::{fingerprint, hash_key, mixsplit, splitmix64};
use crate::params::Params;
use crate::value::StaticFuseValue;
use crate::MAX_ITERATIONS;

pub(crate) trait FuseVec<T>:
    core::ops::Deref<Target = [T]> + core::ops::DerefMut<Target = [T]>
{
    fn try_reserve_len(&mut self, additional: usize) -> Result<(), BuildError>;
    fn try_reserve_exact_len(&mut self, additional: usize) -> Result<(), BuildError>;
    fn push_value(&mut self, value: T);
    fn resize_with_value(&mut self, len: usize, value: T)
    where
        T: Clone;
}

#[cfg(not(feature = "allocator-api2"))]
impl<T> FuseVec<T> for alloc::vec::Vec<T> {
    fn try_reserve_len(&mut self, additional: usize) -> Result<(), BuildError> {
        self.try_reserve(additional)?;
        Ok(())
    }

    fn try_reserve_exact_len(&mut self, additional: usize) -> Result<(), BuildError> {
        self.try_reserve_exact(additional)?;
        Ok(())
    }

    fn push_value(&mut self, value: T) {
        self.push(value);
    }

    fn resize_with_value(&mut self, len: usize, value: T)
    where
        T: Clone,
    {
        self.resize(len, value);
    }
}

#[cfg(feature = "allocator-api2")]
impl<T, A> FuseVec<T> for allocator_api2::vec::Vec<T, A>
where
    A: allocator_api2::alloc::Allocator,
{
    fn try_reserve_len(&mut self, additional: usize) -> Result<(), BuildError> {
        self.try_reserve(additional)?;
        Ok(())
    }

    fn try_reserve_exact_len(&mut self, additional: usize) -> Result<(), BuildError> {
        self.try_reserve_exact(additional)?;
        Ok(())
    }

    fn push_value(&mut self, value: T) {
        self.push(value);
    }

    fn resize_with_value(&mut self, len: usize, value: T)
    where
        T: Clone,
    {
        self.resize(len, value);
    }
}

pub(crate) trait VecStorage: Clone {
    type Vec<T>: FuseVec<T>;

    fn new_vec<T>(&self) -> Self::Vec<T>;
}

#[cfg(not(feature = "allocator-api2"))]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DefaultStorage;

#[cfg(not(feature = "allocator-api2"))]
impl VecStorage for DefaultStorage {
    type Vec<T> = alloc::vec::Vec<T>;

    fn new_vec<T>(&self) -> Self::Vec<T> {
        alloc::vec::Vec::new()
    }
}

#[cfg(feature = "allocator-api2")]
impl<A> VecStorage for A
where
    A: allocator_api2::alloc::Allocator + Clone,
{
    type Vec<T> = allocator_api2::vec::Vec<T, A>;

    fn new_vec<T>(&self) -> Self::Vec<T> {
        allocator_api2::vec::Vec::new_in(self.clone())
    }
}

pub(crate) struct BuildOutput<S: VecStorage> {
    pub(crate) params: Params,
    pub(crate) data: S::Vec<u64>,
    pub(crate) checks: Option<S::Vec<u64>>,
}

pub(crate) fn build_from_slices<V, K, S>(
    keys: &[K],
    values: &[V],
    verified: bool,
    storage: &S,
) -> Result<BuildOutput<S>, BuildError>
where
    V: StaticFuseValue,
    K: AsRef<str>,
    S: VecStorage,
{
    if keys.len() != values.len() {
        return Err(BuildError::MismatchedLengths {
            keys: keys.len(),
            values: values.len(),
        });
    }

    let mut hashes = storage.new_vec();
    hashes.try_reserve_exact_len(keys.len())?;
    for key in keys {
        hashes.push_value(hash_key(key.as_ref()));
    }

    let mut raw_values = storage.new_vec();
    raw_values.try_reserve_exact_len(values.len())?;
    for &value in values {
        raw_values.push_value(value.into_u64());
    }

    build_from_hashes(&hashes, &raw_values, verified, storage)
}

pub(crate) fn build_from_iter<V, I, K, S>(
    pairs: I,
    verified: bool,
    storage: &S,
) -> Result<BuildOutput<S>, BuildError>
where
    V: StaticFuseValue,
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    S: VecStorage,
{
    let iter = pairs.into_iter();
    let (lower, _) = iter.size_hint();

    let mut hashes = storage.new_vec();
    hashes.try_reserve_len(lower)?;
    let mut raw_values = storage.new_vec();
    raw_values.try_reserve_len(lower)?;

    for (key, value) in iter {
        hashes.push_value(hash_key(key.as_ref()));
        raw_values.push_value(value.into_u64());
    }

    build_from_hashes(&hashes, &raw_values, verified, storage)
}

fn zero_vec<T, S>(len: usize, value: T, storage: &S) -> Result<S::Vec<T>, BuildError>
where
    T: Clone,
    S: VecStorage,
{
    let mut v = storage.new_vec();
    v.try_reserve_exact_len(len)?;
    v.resize_with_value(len, value);
    Ok(v)
}

fn build_from_hashes<S>(
    hashed: &[u64],
    raw_values: &[u64],
    verified: bool,
    storage: &S,
) -> Result<BuildOutput<S>, BuildError>
where
    S: VecStorage,
{
    if hashed.len() != raw_values.len() {
        return Err(BuildError::MismatchedLengths {
            keys: hashed.len(),
            values: raw_values.len(),
        });
    }
    let n = hashed.len();
    if n > u32::MAX as usize {
        return Err(BuildError::TooManyKeys { len: n });
    }
    if n == 0 {
        return Ok(BuildOutput {
            params: Params::empty(),
            data: storage.new_vec(),
            checks: if verified {
                Some(storage.new_vec())
            } else {
                None
            },
        });
    }

    let size = n as u32;
    let mut params = Params::initialized(size);
    let capacity = params.array_len();
    let mut data = zero_vec(capacity, 0u64, storage)?;
    let mut checks = if verified {
        Some(zero_vec(capacity, 0u64, storage)?)
    } else {
        None
    };

    let mut rngcounter = 1u64;
    params.seed = splitmix64(&mut rngcounter);

    let mut alone = zero_vec(capacity, 0u32, storage)?;
    let mut t2count = zero_vec(capacity, 0u8, storage)?;
    let mut t2hash = zero_vec(capacity, 0u64, storage)?;
    let mut t2value = zero_vec(capacity, 0u64, storage)?;
    let mut reverse_h = zero_vec(size as usize, 0u8, storage)?;
    let mut reverse_order = zero_vec(size as usize + 1, 0u64, storage)?;
    let mut reverse_values = zero_vec(size as usize, 0u64, storage)?;
    reverse_order[size as usize] = 1;

    for iterations in 0.. {
        if iterations > MAX_ITERATIONS {
            return Err(BuildError::PeelFailed {
                attempts: MAX_ITERATIONS + 1,
            });
        }

        if size > 4 && size < 1_000_000 {
            match iterations % 4 {
                2 => {
                    params.segment_length /= 2;
                    params.segment_length_mask = params.segment_length - 1;
                    params.segment_count = params.segment_count * 2 + 2;
                    params.segment_count_length = params.segment_count * params.segment_length;
                }
                3 => {
                    params.segment_length *= 2;
                    params.segment_length_mask = params.segment_length - 1;
                    params.segment_count = params.segment_count / 2 - 1;
                    params.segment_count_length = params.segment_count * params.segment_length;
                }
                _ => {}
            }
        }

        let mut block_bits = 1u32;
        while (1u32 << block_bits) < params.segment_count {
            block_bits += 1;
        }
        let block_count = 1usize << block_bits;
        let mut start_pos = zero_vec(block_count, 0u32, storage)?;
        for (i, slot) in start_pos.iter_mut().enumerate() {
            *slot = ((i as u64 * size as u64) >> block_bits) as u32;
        }
        for (&key, &value) in hashed.iter().zip(raw_values) {
            let hash = mixsplit(key, params.seed);
            let mut segment_index = (hash >> (64 - block_bits)) as usize;
            loop {
                let pos = start_pos[segment_index] as usize;
                if reverse_order[pos] == 0 {
                    reverse_order[pos] = hash;
                    reverse_values[pos] = value;
                    start_pos[segment_index] += 1;
                    break;
                }
                segment_index = (segment_index + 1) & (block_count - 1);
            }
        }

        let mut has_error = false;
        for i in 0..size as usize {
            let hash = reverse_order[i];
            let value = reverse_values[i];
            let (index1, index2, index3) = params.get_hash_from_hash(hash);
            let index1 = index1 as usize;
            let index2 = index2 as usize;
            let index3 = index3 as usize;
            t2count[index1] = t2count[index1].wrapping_add(4);
            t2hash[index1] ^= hash;
            t2value[index1] ^= value;
            t2count[index2] = t2count[index2].wrapping_add(4) ^ 1;
            t2hash[index2] ^= hash;
            t2value[index2] ^= value;
            t2count[index3] = t2count[index3].wrapping_add(4) ^ 2;
            t2hash[index3] ^= hash;
            t2value[index3] ^= value;

            if (t2hash[index1] & t2hash[index2] & t2hash[index3]) == 0
                && ((t2hash[index1] == 0 && t2count[index1] == 8)
                    || (t2hash[index2] == 0 && t2count[index2] == 8)
                    || (t2hash[index3] == 0 && t2count[index3] == 8))
            {
                return Err(BuildError::DuplicateHash);
            }
            if t2count[index1] < 4 || t2count[index2] < 4 || t2count[index3] < 4 {
                has_error = true;
            }
        }
        if has_error {
            reset_attempt(
                size as usize,
                capacity,
                &mut reverse_order,
                &mut t2count,
                &mut t2hash,
                &mut t2value,
                &mut reverse_values,
            );
            params.seed = splitmix64(&mut rngcounter);
            continue;
        }

        let mut qsize = 0usize;
        for (i, &count) in t2count.iter().enumerate().take(capacity) {
            alone[qsize] = i as u32;
            if (count >> 2) == 1 {
                qsize += 1;
            }
        }

        let mut stacksize = 0usize;
        let seg_len = params.segment_length;
        let seg_len_to_minus_seg_len_x2 = seg_len ^ seg_len.wrapping_mul(2).wrapping_neg();
        while qsize > 0 {
            qsize -= 1;
            let index = alone[qsize];
            let index_usize = index as usize;
            if (t2count[index_usize] >> 2) == 1 {
                let hash = t2hash[index_usize];
                let value = t2value[index_usize];
                let found = t2count[index_usize] & 3;
                reverse_h[stacksize] = found;
                reverse_order[stacksize] = hash;
                reverse_values[stacksize] = value;
                stacksize += 1;

                let h01 = ((hash >> 18) as u32) & params.segment_length_mask;
                let h02 = (hash as u32) & params.segment_length_mask;

                let is0 = ((found.wrapping_sub(1) >> 7) as u32).wrapping_neg();
                let is1 = ((found & 1) as u32).wrapping_neg();
                let is2 = ((found >> 1) as u32).wrapping_neg();

                let mut other_index1 =
                    index.wrapping_add(seg_len ^ (seg_len_to_minus_seg_len_x2 & is2));
                let mut other_index2 =
                    index.wrapping_sub(seg_len ^ (seg_len_to_minus_seg_len_x2 & is0));

                other_index1 ^= (h01 & !is2) ^ (h02 & !is0);
                other_index2 ^= (h01 & !is0) ^ (h02 & !is1);

                let f1 = ((is0 & 1) | (is1 & 2)) as u8;
                let f2 = ((is0 & 2) | (is2 & 1)) as u8;

                let oi1 = other_index1 as usize;
                alone[qsize] = other_index1;
                if (t2count[oi1] >> 2) == 2 {
                    qsize += 1;
                }
                t2count[oi1] = t2count[oi1].wrapping_sub(4) ^ f1;
                t2hash[oi1] ^= hash;
                t2value[oi1] ^= value;

                let oi2 = other_index2 as usize;
                alone[qsize] = other_index2;
                if (t2count[oi2] >> 2) == 2 {
                    qsize += 1;
                }
                t2count[oi2] = t2count[oi2].wrapping_sub(4) ^ f2;
                t2hash[oi2] ^= hash;
                t2value[oi2] ^= value;
            }
        }

        if stacksize == size as usize {
            let mut h012 = [0u32; 5];
            for i in (0..size as usize).rev() {
                let hash = reverse_order[i];
                let val = reverse_values[i];
                let fp = fingerprint(hash);
                let (index1, index2, index3) = params.get_hash_from_hash(hash);
                let found = reverse_h[i] as usize;
                h012[0] = index1;
                h012[1] = index2;
                h012[2] = index3;
                h012[3] = h012[0];
                h012[4] = h012[1];
                let target = h012[found] as usize;
                let a = h012[found + 1] as usize;
                let b = h012[found + 2] as usize;
                data[target] = val ^ data[a] ^ data[b];
                if let Some(checks) = checks.as_mut() {
                    checks[target] = fp ^ checks[a] ^ checks[b];
                }
            }

            return Ok(BuildOutput {
                params,
                data,
                checks,
            });
        }

        reset_attempt(
            size as usize,
            capacity,
            &mut reverse_order,
            &mut t2count,
            &mut t2hash,
            &mut t2value,
            &mut reverse_values,
        );
        params.seed = splitmix64(&mut rngcounter);
    }
    unreachable!()
}

fn reset_attempt(
    size: usize,
    capacity: usize,
    reverse_order: &mut [u64],
    t2count: &mut [u8],
    t2hash: &mut [u64],
    t2value: &mut [u64],
    reverse_values: &mut [u64],
) {
    for slot in &mut reverse_order[..size] {
        *slot = 0;
    }
    for slot in &mut reverse_values[..size] {
        *slot = 0;
    }
    for slot in &mut t2count[..capacity] {
        *slot = 0;
    }
    for slot in &mut t2hash[..capacity] {
        *slot = 0;
    }
    for slot in &mut t2value[..capacity] {
        *slot = 0;
    }
}
