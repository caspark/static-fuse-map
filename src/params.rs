use crate::math;

fn calculate_segment_length(size: u32) -> u32 {
    if size == 0 {
        return 4;
    }
    1u32 << (math::floor(math::ln(f64::from(size)) / math::ln(3.33) + 2.25) as u32)
}

fn calculate_size_factor(size: u32) -> f64 {
    1.125f64.max(0.875 + 0.25 * math::ln(1_000_000.0) / math::ln(f64::from(size)))
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Params {
    pub(crate) seed: u64,
    pub(crate) segment_length: u32,
    pub(crate) segment_length_mask: u32,
    pub(crate) segment_count: u32,
    pub(crate) segment_count_length: u32,
    pub(crate) len: usize,
}

impl Params {
    pub(crate) fn empty() -> Self {
        Self {
            seed: 0,
            segment_length: 0,
            segment_length_mask: 0,
            segment_count: 0,
            segment_count_length: 0,
            len: 0,
        }
    }

    pub(crate) fn initialized(size: u32) -> Self {
        let mut segment_length = calculate_segment_length(size);
        if segment_length > 262_144 {
            segment_length = 262_144;
        }
        let segment_length_mask = segment_length - 1;
        let mut capacity = 0u32;
        if size > 1 {
            let size_factor = calculate_size_factor(size);
            capacity = math::round(f64::from(size) * size_factor) as u32;
        }
        let mut total_segment_count = capacity.div_ceil(segment_length);
        if total_segment_count < 3 {
            total_segment_count = 3;
        }
        let segment_count = total_segment_count - 2;
        let segment_count_length = segment_count * segment_length;
        Self {
            seed: 0,
            segment_length,
            segment_length_mask,
            segment_count,
            segment_count_length,
            len: size as usize,
        }
    }

    #[inline]
    pub(crate) fn array_len(&self) -> usize {
        (self.segment_count as usize + 2) * self.segment_length as usize
    }

    #[inline]
    pub(crate) fn get_hash_from_hash(&self, hash: u64) -> (u32, u32, u32) {
        let h0 = ((hash as u128 * self.segment_count_length as u128) >> 64) as u32;
        let mut h1 = h0 + self.segment_length;
        let mut h2 = h1 + self.segment_length;
        h1 ^= ((hash >> 18) as u32) & self.segment_length_mask;
        h2 ^= (hash as u32) & self.segment_length_mask;
        (h0, h1, h2)
    }

    #[allow(dead_code)]
    pub(crate) fn validate(
        &self,
        data_len: usize,
        checks_len: Option<usize>,
    ) -> Result<(), &'static str> {
        if self.len == 0 {
            if self.segment_length != 0
                || self.segment_length_mask != 0
                || self.segment_count != 0
                || self.segment_count_length != 0
                || data_len != 0
                || checks_len.unwrap_or(0) != 0
            {
                return Err("invalid empty map parameters");
            }
            return Ok(());
        }
        if self.segment_length == 0 || !self.segment_length.is_power_of_two() {
            return Err("segment length must be a nonzero power of two");
        }
        if self.segment_length_mask != self.segment_length - 1 {
            return Err("invalid segment length mask");
        }
        if self.segment_count == 0 {
            return Err("segment count must be nonzero");
        }
        if self.segment_count_length != self.segment_count.saturating_mul(self.segment_length) {
            return Err("invalid segment count length");
        }
        if data_len != self.array_len() {
            return Err("data length does not match parameters");
        }
        if let Some(checks_len) = checks_len {
            if checks_len != data_len {
                return Err("checks length does not match data length");
            }
        }
        Ok(())
    }
}
