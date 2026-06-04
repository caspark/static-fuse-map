# static-fuse-map

> **Clanker port:** this crate is an AI-assisted Rust port of Daniel Lemire's Go [`constmap`](https://github.com/lemire/constmap) implementation.

A fast, compact, immutable map from strings to `u64`-encoded values using the binary fuse filter construction.

It is intended for cases where all keys are known at construction time and fast, compact lookups are required afterward.

## Usage

```rust
use static_fuse_map::StaticFuseMap;

let map = StaticFuseMap::try_from_iter([
    ("apple", 100u64),
    ("banana", 200),
])?;

assert_eq!(map.get("banana"), 200);
# Ok::<(), static_fuse_map::BuildError>(())
```

`StaticFuseMap::get` is unchecked: querying a key that was not present during construction returns an unspecified value.

Use `VerifiedStaticFuseMap` when missing keys should be detected probabilistically. A missing key returns `None` except for a false positive with probability about 2^-64 (roughly 1 in 18.4 quintillion), in which case an unspecified `Some` value is returned instead:

```rust
use static_fuse_map::VerifiedStaticFuseMap;

let map = VerifiedStaticFuseMap::try_from_iter([
    ("apple", 100u64),
    ("banana", u64::MAX),
])?;

assert_eq!(map.get("banana"), Some(u64::MAX));
assert_eq!(map.get("missing"), None);
# Ok::<(), static_fuse_map::BuildError>(())
```

## License

Licensed under either of Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)) or MIT license ([LICENSE-MIT](LICENSE-MIT)) at your option.

## Features

Default builds use `std` and standard global `Vec` storage.

For `no_std`, disable default features and enable `libm` for construction-time floating-point math:

```toml
static-fuse-map = { version = "0.1", default-features = false, features = ["libm"] }
```

Optional features:

- `serde`: serialize/deserialize map tables using Serde.
- `allocator-api2`: expose allocator-aware constructors and map types backed by `allocator-api2` vectors.

## Value types

Values are stored internally as `u64`. Implement `StaticFuseValue` for ID/newtype values that round-trip through `u64`.

## Benchmarks

On an Apple M5 Max with 1 million `key-{i}` strings, the current Rust port measured approximately:

| Data structure | Lookup time |
|---|---:|
| `StaticFuseMap` | 4.7 ns/op |
| `VerifiedStaticFuseMap` | 9.3 ns/op |
| `std::collections::HashMap` | 34.7 ns/op |

The Go source implementation measured in the same session:

| Data structure | Lookup time |
|---|---:|
| Go `ConstMap` | 5.9 ns/op |
| Go `VerifiedConstMap` | 10.9 ns/op |
| Go `map[string]uint64` | 18.8 ns/op |

Benchmark numbers are workload- and machine-dependent.
