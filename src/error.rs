use core::fmt;

#[cfg(feature = "allocator-api2")]
type ReserveError = allocator_api2::collections::TryReserveError;
#[cfg(not(feature = "allocator-api2"))]
type ReserveError = alloc::collections::TryReserveError;

/// Error returned when a static fuse map cannot be constructed.
#[derive(Debug)]
pub enum BuildError {
    /// Slice construction was called with different key and value lengths.
    MismatchedLengths { keys: usize, values: usize },
    /// The implementation uses 32-bit indexes internally.
    TooManyKeys { len: usize },
    /// A duplicate mixed key hash was detected.
    DuplicateHash,
    /// Peeling failed after repeated seed attempts.
    PeelFailed { attempts: u32 },
    /// Allocation failed while reserving memory.
    Reserve(ReserveError),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::MismatchedLengths { keys, values } => {
                write!(f, "static-fuse-map: keys and values must have equal length (keys={keys}, values={values})")
            }
            BuildError::TooManyKeys { len } => {
                write!(
                    f,
                    "static-fuse-map: too many keys for 32-bit index space (len={len})"
                )
            }
            BuildError::DuplicateHash => {
                f.write_str("static-fuse-map: duplicate key hash detected")
            }
            BuildError::PeelFailed { attempts } => {
                write!(f, "static-fuse-map: failed to construct map after {attempts} attempts, possible duplicate keys")
            }
            BuildError::Reserve(err) => write!(f, "static-fuse-map: allocation failed: {err}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BuildError::Reserve(err) => Some(err),
            _ => None,
        }
    }
}

#[cfg(not(feature = "allocator-api2"))]
impl From<alloc::collections::TryReserveError> for BuildError {
    fn from(err: alloc::collections::TryReserveError) -> Self {
        BuildError::Reserve(err)
    }
}

#[cfg(feature = "allocator-api2")]
impl From<allocator_api2::collections::TryReserveError> for BuildError {
    fn from(err: allocator_api2::collections::TryReserveError) -> Self {
        BuildError::Reserve(err)
    }
}
