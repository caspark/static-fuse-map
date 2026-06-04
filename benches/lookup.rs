use criterion::{black_box, criterion_group, criterion_main, Criterion};
use static_fuse_map::{StaticFuseMap, VerifiedStaticFuseMap};
use std::collections::HashMap;

const BENCH_N: usize = 1_000_000;

fn make_bench_data(n: usize) -> (Vec<String>, Vec<u64>) {
    let mut keys = Vec::with_capacity(n);
    let mut values = Vec::with_capacity(n);
    for i in 0..n {
        keys.push(format!("key-{i}"));
        values.push(i as u64);
    }
    (keys, values)
}

fn lookup_benches(c: &mut Criterion) {
    let (keys, values) = make_bench_data(BENCH_N);

    let map = StaticFuseMap::new(&keys, &values).unwrap();
    c.bench_function("static_fuse_map", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let value = map.get(&keys[i % BENCH_N]);
            i = i.wrapping_add(1);
            black_box(value);
        });
    });

    let verified = VerifiedStaticFuseMap::new(&keys, &values).unwrap();
    c.bench_function("verified_static_fuse_map", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let value = verified.get(&keys[i % BENCH_N]);
            i = i.wrapping_add(1);
            black_box(value);
        });
    });

    let std_map: HashMap<String, u64> = keys.iter().cloned().zip(values.iter().copied()).collect();
    c.bench_function("std_hash_map", |b| {
        let mut i = 0usize;
        b.iter(|| {
            let value = std_map.get(&keys[i % BENCH_N]);
            i = i.wrapping_add(1);
            black_box(value);
        });
    });
}

criterion_group!(benches, lookup_benches);
criterion_main!(benches);
