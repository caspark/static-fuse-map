/// A value that can be stored in a [`StaticFuseMap`].
///
/// The data structure stores values as `u64` internally. Implement this trait
/// for small ID/newtype values that round-trip through `u64`.
pub trait StaticFuseValue: Copy {
    /// Encodes this value for storage.
    fn into_u64(self) -> u64;

    /// Decodes a stored value.
    fn from_u64(raw: u64) -> Self;
}

macro_rules! impl_static_fuse_value_cast {
    ($($ty:ty),* $(,)?) => {
        $(
            impl StaticFuseValue for $ty {
                #[inline]
                fn into_u64(self) -> u64 { self as u64 }

                #[inline]
                fn from_u64(raw: u64) -> Self { raw as $ty }
            }
        )*
    };
}

impl_static_fuse_value_cast!(u8, u16, u32, u64);

#[cfg(any(
    target_pointer_width = "16",
    target_pointer_width = "32",
    target_pointer_width = "64"
))]
impl StaticFuseValue for usize {
    #[inline]
    fn into_u64(self) -> u64 {
        self as u64
    }

    #[inline]
    fn from_u64(raw: u64) -> Self {
        raw as usize
    }
}
