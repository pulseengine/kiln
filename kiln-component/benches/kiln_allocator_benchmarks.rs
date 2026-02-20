//! Benchmarks comparing WRT allocator vs standard library collections
//!
//! This module benchmarks the performance of WRT's safety-critical allocator
//! against standard library collections to validate performance parity.

#![allow(unused_imports)]

#[cfg(not(feature = "std"))]
compile_error!("Benchmarks require std feature for criterion");

use std::{collections::HashMap as StdHashMap, vec::Vec as StdVec};

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
#[cfg(feature = "safety-critical")]
use kiln_foundation::allocator::{CrateId, KilnHashMap, KilnVec};

// Benchmark sizes
const SMALL_SIZE: usize = 10;
const MEDIUM_SIZE: usize = 100;
const LARGE_SIZE: usize = 1000;

// Memory limits for WRT collections
const VEC_LIMIT: usize = 2048;
const MAP_LIMIT: usize = 2048;

/// Benchmark vector push operations
fn bench_vec_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_push");

    for size in &[SMALL_SIZE, MEDIUM_SIZE, LARGE_SIZE] {
        // Standard Vec benchmark
        group.bench_with_input(BenchmarkId::new("std_vec", size), size, |b, &size| {
            b.iter(|| {
                let mut vec = StdVec::with_capacity(size);
                for i in 0..size {
                    vec.push(black_box(i));
                }
                vec
            })
        });

        // WRT Vec benchmark
        #[cfg(feature = "safety-critical")]
        group.bench_with_input(BenchmarkId::new("kiln_vec", size), size, |b, &size| {
            b.iter(|| {
                let mut vec: KilnVec<usize, { CrateId::Component as u8 }, VEC_LIMIT> = KilnVec::new();
                for i in 0..size {
                    let _ = vec.push(black_box(i));
                }
                vec
            })
        });
    }

    group.finish();
}

/// Benchmark vector iteration
fn bench_vec_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_iteration");

    for size in &[SMALL_SIZE, MEDIUM_SIZE, LARGE_SIZE] {
        // Setup data
        let mut std_vec = StdVec::with_capacity(*size);
        #[cfg(feature = "safety-critical")]
        let mut kiln_vec: KilnVec<usize, { CrateId::Component as u8 }, VEC_LIMIT> = KilnVec::new();

        for i in 0..*size {
            std_vec.push(i);
            #[cfg(feature = "safety-critical")]
            let _ = kiln_vec.push(i);
        }

        // Standard Vec iteration
        group.bench_with_input(BenchmarkId::new("std_vec", size), &std_vec, |b, vec| {
            b.iter(|| {
                let mut sum = 0;
                for &val in vec {
                    sum += black_box(val);
                }
                sum
            })
        });

        // WRT Vec iteration
        #[cfg(feature = "safety-critical")]
        group.bench_with_input(BenchmarkId::new("kiln_vec", size), &kiln_vec, |b, vec| {
            b.iter(|| {
                let mut sum = 0;
                for &val in vec.iter() {
                    sum += black_box(val);
                }
                sum
            })
        });
    }

    group.finish();
}

/// Benchmark map insertion
fn bench_map_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_insert");

    for size in &[SMALL_SIZE, MEDIUM_SIZE, LARGE_SIZE] {
        // Standard HashMap benchmark
        group.bench_with_input(BenchmarkId::new("std_hashmap", size), size, |b, &size| {
            b.iter(|| {
                let mut map = StdHashMap::with_capacity(size);
                for i in 0..size {
                    map.insert(black_box(i as u32), black_box(i * 2));
                }
                map
            })
        });

        // WRT HashMap benchmark
        #[cfg(feature = "safety-critical")]
        group.bench_with_input(BenchmarkId::new("kiln_hashmap", size), size, |b, &size| {
            b.iter(|| {
                let mut map: KilnHashMap<u32, usize, { CrateId::Component as u8 }, MAP_LIMIT> =
                    KilnHashMap::new();
                for i in 0..size {
                    let _ = map.insert(black_box(i as u32), black_box(i * 2));
                }
                map
            })
        });
    }

    group.finish();
}

/// Benchmark map lookup
fn bench_map_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_lookup");

    for size in &[SMALL_SIZE, MEDIUM_SIZE, LARGE_SIZE] {
        // Setup data
        let mut std_map = StdHashMap::with_capacity(*size);
        #[cfg(feature = "safety-critical")]
        let mut kiln_map: KilnHashMap<u32, usize, { CrateId::Component as u8 }, MAP_LIMIT> =
            KilnHashMap::new();

        for i in 0..*size {
            std_map.insert(i as u32, i * 2);
            #[cfg(feature = "safety-critical")]
            let _ = kiln_map.insert(i as u32, i * 2);
        }

        // Standard HashMap lookup
        group.bench_with_input(BenchmarkId::new("std_hashmap", size), &std_map, |b, map| {
            b.iter(|| {
                let mut sum = 0;
                for i in 0..*size {
                    if let Some(&val) = map.get(&(i as u32)) {
                        sum += black_box(val);
                    }
                }
                sum
            })
        });

        // WRT HashMap lookup
        #[cfg(feature = "safety-critical")]
        group.bench_with_input(BenchmarkId::new("kiln_hashmap", size), &kiln_map, |b, map| {
            b.iter(|| {
                let mut sum = 0;
                for i in 0..*size {
                    if let Some(&val) = map.get(&(i as u32)) {
                        sum += black_box(val);
                    }
                }
                sum
            })
        });
    }

    group.finish();
}

/// Benchmark mixed operations simulating component workloads
fn bench_component_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("component_workload");

    // Simulate typical component operations
    group.bench_function("std_component_ops", |b| {
        b.iter(|| {
            let mut exports = StdHashMap::new();
            let mut imports = StdVec::new();
            let mut resources = StdHashMap::new();

            // Simulate component initialization
            for i in 0..50 {
                exports.insert(format!("export_{}", i), i);
                imports.push(format!("import_{}", i));
                resources.insert(i as u32, format!("resource_{}", i));
            }

            // Simulate runtime operations
            let mut sum = 0;
            for i in 0..100 {
                // Export lookup
                if let Some(&val) = exports.get(&format!("export_{}", i % 50)) {
                    sum += val;
                }

                // Import access
                if i < imports.len() {
                    sum += imports[i].len();
                }

                // Resource management
                if i % 10 == 0 {
                    resources.insert(100 + i as u32, format!("new_resource_{}", i));
                }
                if i % 20 == 0 {
                    resources.remove(&(i as u32 / 2));
                }
            }

            black_box((exports, imports, resources, sum))
        })
    });

    #[cfg(feature = "safety-critical")]
    group.bench_function("kiln_component_ops", |b| {
        b.iter(|| {
            let mut exports: KilnHashMap<String, usize, { CrateId::Component as u8 }, 256> =
                KilnHashMap::new();
            let mut imports: KilnVec<String, { CrateId::Component as u8 }, 256> = KilnVec::new();
            let mut resources: KilnHashMap<u32, String, { CrateId::Component as u8 }, 1024> =
                KilnHashMap::new();

            // Simulate component initialization
            for i in 0..50 {
                let _ = exports.insert(format!("export_{}", i), i);
                let _ = imports.push(format!("import_{}", i));
                let _ = resources.insert(i as u32, format!("resource_{}", i));
            }

            // Simulate runtime operations
            let mut sum = 0;
            for i in 0..100 {
                // Export lookup
                if let Some(&val) = exports.get(&format!("export_{}", i % 50)) {
                    sum += val;
                }

                // Import access
                if i < imports.len() {
                    sum += imports[i].len();
                }

                // Resource management
                if i % 10 == 0 {
                    let _ = resources.insert(100 + i as u32, format!("new_resource_{}", i));
                }
                if i % 20 == 0 {
                    resources.remove(&(i as u32 / 2));
                }
            }

            black_box((exports, imports, resources, sum))
        })
    });

    group.finish();
}

/// Benchmark capacity error handling
#[cfg(feature = "safety-critical")]
fn bench_capacity_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("capacity_handling");

    // Small capacity for testing overflow
    const SMALL_CAPACITY: usize = 32;

    group.bench_function("kiln_vec_capacity_check", |b| {
        b.iter(|| {
            let mut vec: KilnVec<usize, { CrateId::Component as u8 }, SMALL_CAPACITY> =
                KilnVec::new();
            let mut errors = 0;

            for i in 0..50 {
                if vec.push(i).is_err() {
                    errors += 1;
                }
            }

            black_box((vec, errors))
        })
    });

    group.bench_function("kiln_map_capacity_check", |b| {
        b.iter(|| {
            let mut map: KilnHashMap<u32, usize, { CrateId::Component as u8 }, SMALL_CAPACITY> =
                KilnHashMap::new();
            let mut errors = 0;

            for i in 0..50 {
                if map.insert(i, i as usize).is_err() {
                    errors += 1;
                }
            }

            black_box((map, errors))
        })
    });

    group.finish();
}

// Define benchmark groups
#[cfg(not(feature = "safety-critical"))]
criterion_group!(
    benches,
    bench_vec_push,
    bench_vec_iteration,
    bench_map_insert,
    bench_map_lookup,
    bench_component_workload
);

#[cfg(feature = "safety-critical")]
criterion_group!(
    benches,
    bench_vec_push,
    bench_vec_iteration,
    bench_map_insert,
    bench_map_lookup,
    bench_component_workload,
    bench_capacity_handling
);

criterion_main!(benches);
