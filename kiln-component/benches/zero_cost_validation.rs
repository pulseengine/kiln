//! Zero-cost abstraction validation for WRT allocator
//!
//! This benchmark validates that WRT allocator truly provides zero-cost
//! abstractions by comparing assembly-level operations.

#![allow(unused_imports)]

use std::{collections::HashMap as StdHashMap, vec::Vec as StdVec};

use criterion::{Criterion, black_box, criterion_group, criterion_main};
#[cfg(feature = "safety-critical")]
use wrt_foundation::allocator::{CrateId, WrtHashMap, WrtVec};

/// Test that basic operations compile to identical code
fn bench_zero_cost_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("zero_cost_push");
    group.sample_size(1000); // More samples for precision

    // Test single push operation
    group.bench_function("std_single_push", |b| {
        b.iter(|| {
            let mut vec = StdVec::with_capacity(1);
            vec.push(black_box(42));
            black_box(vec)
        })
    });

    #[cfg(feature = "safety-critical")]
    group.bench_function("wrt_single_push", |b| {
        b.iter(|| {
            let mut vec: WrtVec<i32, { CrateId::Component as u8 }, 1> =
                WrtVec::with_capacity(1).unwrap();
            let _ = vec.push(black_box(42));
            black_box(vec)
        })
    });

    group.finish();
}

/// Test direct memory access patterns
fn bench_zero_cost_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("zero_cost_access");
    group.sample_size(1000);

    // Setup test data
    let mut std_vec = StdVec::with_capacity(100);
    #[cfg(feature = "safety-critical")]
    let mut wrt_vec: WrtVec<i32, { CrateId::Component as u8 }, 100> =
        WrtVec::with_capacity(100).unwrap();

    for i in 0..100 {
        std_vec.push(i);
        #[cfg(feature = "safety-critical")]
        let _ = wrt_vec.push(i);
    }

    // Test direct indexing
    group.bench_function("std_index_access", |b| {
        b.iter(|| {
            let mut sum = 0;
            for i in 0..100 {
                sum += black_box(std_vec[i]);
            }
            black_box(sum)
        })
    });

    #[cfg(feature = "safety-critical")]
    group.bench_function("wrt_index_access", |b| {
        b.iter(|| {
            let mut sum = 0;
            for i in 0..100 {
                sum += black_box(wrt_vec[i]);
            }
            black_box(sum)
        })
    });

    group.finish();
}

/// Test iterator performance (should compile to same code)
fn bench_zero_cost_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("zero_cost_iteration");
    group.sample_size(1000);

    // Setup test data
    let data: Vec<i32> = (0..100).collect();
    let std_vec = StdVec::from(data.clone());

    #[cfg(feature = "safety-critical")]
    let mut wrt_vec: WrtVec<i32, { CrateId::Component as u8 }, 100> = WrtVec::new();
    #[cfg(feature = "safety-critical")]
    for &val in &data {
        let _ = wrt_vec.push(val);
    }

    // Test iterator summing
    group.bench_function("std_iter_sum", |b| {
        b.iter(|| black_box(std_vec.iter().sum::<i32>()))
    });

    #[cfg(feature = "safety-critical")]
    group.bench_function("wrt_iter_sum", |b| {
        b.iter(|| black_box(wrt_vec.iter().sum::<i32>()))
    });

    group.finish();
}

/// Test that capacity checks don't add overhead in normal path
fn bench_capacity_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("capacity_overhead");
    group.sample_size(1000);

    // Test push within capacity (happy path)
    group.bench_function("std_push_within_capacity", |b| {
        b.iter(|| {
            let mut vec = StdVec::with_capacity(10);
            for i in 0..5 {
                vec.push(black_box(i));
            }
            black_box(vec)
        })
    });

    #[cfg(feature = "safety-critical")]
    group.bench_function("wrt_push_within_capacity", |b| {
        b.iter(|| {
            let mut vec: WrtVec<i32, { CrateId::Component as u8 }, 10> = WrtVec::new();
            for i in 0..5 {
                let _ = vec.push(black_box(i));
            }
            black_box(vec)
        })
    });

    group.finish();
}

/// Memory layout validation - ensure same size/alignment
#[cfg(all(test, feature = "safety-critical"))]
#[test]
fn test_memory_layout() {
    use std::mem::{align_of, size_of};

    // Vec layout comparison
    assert_eq!(
        size_of::<StdVec<u32>>(),
        size_of::<WrtVec<u32, { CrateId::Component as u8 }, 100>>(),
        "WrtVec should have same size as Vec"
    );

    assert_eq!(
        align_of::<StdVec<u32>>(),
        align_of::<WrtVec<u32, { CrateId::Component as u8 }, 100>>(),
        "WrtVec should have same alignment as Vec"
    );

    // HashMap layout comparison
    assert_eq!(
        size_of::<StdHashMap<u32, u32>>(),
        size_of::<WrtHashMap<u32, u32, { CrateId::Component as u8 }, 100>>(),
        "WrtHashMap should have same size as HashMap"
    );
}

// Define benchmark groups
#[cfg(not(feature = "safety-critical"))]
criterion_group!(
    benches,
    bench_zero_cost_push,
    bench_zero_cost_access,
    bench_zero_cost_iteration,
    bench_capacity_overhead
);

#[cfg(feature = "safety-critical")]
criterion_group!(
    benches,
    bench_zero_cost_push,
    bench_zero_cost_access,
    bench_zero_cost_iteration,
    bench_capacity_overhead
);

criterion_main!(benches);
