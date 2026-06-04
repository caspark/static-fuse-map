#[inline]
pub(crate) fn ln(value: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        value.ln()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::log(value)
    }
}

#[inline]
pub(crate) fn floor(value: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        value.floor()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::floor(value)
    }
}

#[inline]
pub(crate) fn round(value: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        value.round()
    }
    #[cfg(not(feature = "std"))]
    {
        libm::round(value)
    }
}
