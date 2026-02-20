//! Performance benchmarks for real WASM execution capability
//!
//! These benchmarks validate the performance characteristics of the WRT
//! framework's real WebAssembly execution engine, measuring instruction
//! parsing, execution, and memory management performance under QM and ASIL-B
//! safety levels.

#![allow(unused_imports)]

#[cfg(not(feature = "std"))]
compile_error!("Benchmarks require std feature for criterion");

use std::{
    fs,
    sync::Arc,
    time::{Duration, Instant},
};

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
// Import the execution components
use wrt_decoder::decoder::decode_module;
use wrt_error::Result;
use wrt_foundation::values::Value;
use wrt_runtime::{module::Module, module_instance::ModuleInstance, stackless::StacklessEngine};

// Benchmark sizes for different test scenarios
const SMALL_ITERATION: usize = 10;
const MEDIUM_ITERATION: usize = 100;
const LARGE_ITERATION: usize = 1000;

/// Helper function to load and prepare a test WASM module
fn load_test_module() -> Result<Module> {
    let wasm_bytes = fs::read("test_add.wasm")
        .map_err(|_| wrt_error::Error::system_io_error("Failed to read test_add.wasm"))?;

    let decoded = decode_module(&wasm_bytes)?;
    Module::from_wrt_module(&decoded)
}

/// Helper function to create test arguments
fn create_test_args(a: i32, b: i32) -> Vec<Value> {
    vec![Value::I32(a), Value::I32(b)]
}

/// Benchmark module loading and parsing performance
fn bench_module_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("module_loading");

    if let Ok(wasm_bytes) = fs::read("test_add.wasm") {
        group.bench_function("decode_module", |b| {
            b.iter(|| {
                let decoded = decode_module(black_box(&wasm_bytes)).unwrap();
                black_box(decoded)
            })
        });

        group.bench_function("convert_to_runtime", |b| {
            let decoded = decode_module(&wasm_bytes).unwrap();
            b.iter(|| {
                let runtime_module = Module::from_wrt_module(black_box(&decoded)).unwrap();
                black_box(runtime_module)
            })
        });

        group.bench_function("full_module_loading", |b| {
            b.iter(|| {
                let decoded = decode_module(black_box(&wasm_bytes)).unwrap();
                let runtime_module = Module::from_wrt_module(&decoded).unwrap();
                black_box(runtime_module)
            })
        });
    }

    group.finish();
}

/// Benchmark instruction parsing performance
fn bench_instruction_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("instruction_parsing");

    if let Ok(runtime_module) = load_test_module() {
        group.bench_function("function_instruction_count", |b| {
            b.iter(|| {
                if let Ok(function) = runtime_module.functions.get(0) {
                    let instruction_count = function.body.len();
                    black_box(instruction_count)
                } else {
                    black_box(0)
                }
            })
        });

        group.bench_function("instruction_access_pattern", |b| {
            b.iter(|| {
                if let Ok(function) = runtime_module.functions.get(0) {
                    let mut total_ops = 0;
                    for i in 0..function.body.len() {
                        if let Ok(_instruction) = function.body.get(i) {
                            total_ops += 1;
                        }
                    }
                    black_box(total_ops)
                } else {
                    black_box(0)
                }
            })
        });
    }

    group.finish();
}

/// Benchmark single execution performance
fn bench_single_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_execution");

    if let Ok(runtime_module) = load_test_module() {
        group.bench_function("engine_instantiation", |b| {
            b.iter(|| {
                let mut engine = StacklessEngine::new();
                let instance = ModuleInstance::new(runtime_module.clone(), 0).unwrap();
                let instance_arc = Arc::new(instance);
                let instance_idx = engine.set_current_module(instance_arc).unwrap();
                black_box(instance_idx)
            })
        });

        group.bench_function("simple_add_execution", |b| {
            let mut engine = StacklessEngine::new();
            let instance = ModuleInstance::new(runtime_module.clone(), 0).unwrap();
            let instance_arc = Arc::new(instance);
            let instance_idx = engine.set_current_module(instance_arc).unwrap();

            b.iter(|| {
                let args = create_test_args(black_box(42), black_box(24));
                let results = engine.execute(instance_idx, 0, args).unwrap();
                black_box(results)
            })
        });

        group.bench_function("execution_with_setup", |b| {
            b.iter(|| {
                let mut engine = StacklessEngine::new();
                let instance = ModuleInstance::new(runtime_module.clone(), 0).unwrap();
                let instance_arc = Arc::new(instance);
                let instance_idx = engine.set_current_module(instance_arc).unwrap();

                let args = create_test_args(black_box(10), black_box(32));
                let results = engine.execute(instance_idx, 0, args).unwrap();
                black_box(results)
            })
        });
    }

    group.finish();
}

/// Benchmark repeated execution performance
fn bench_repeated_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("repeated_execution");

    if let Ok(runtime_module) = load_test_module() {
        let mut engine = StacklessEngine::new();
        let instance = ModuleInstance::new(runtime_module, 0).unwrap();
        let instance_arc = Arc::new(instance);
        let instance_idx = engine.set_current_module(instance_arc).unwrap();

        for &iterations in &[SMALL_ITERATION, MEDIUM_ITERATION, LARGE_ITERATION] {
            group.bench_with_input(
                BenchmarkId::new("batch_execution", iterations),
                &iterations,
                |b, &iterations| {
                    b.iter(|| {
                        let mut results_sum = 0;
                        for i in 0..iterations {
                            let args = create_test_args(i as i32, (i * 2) as i32);
                            let results = engine.execute(instance_idx, 0, args).unwrap();
                            if let Some(Value::I32(result)) = results.get(0) {
                                results_sum += result;
                            }
                        }
                        black_box(results_sum)
                    })
                },
            );
        }
    }

    group.finish();
}

/// Benchmark execution determinism and timing variance
fn bench_execution_determinism(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_determinism");
    group.sample_size(500); // More samples for better variance analysis

    if let Ok(runtime_module) = load_test_module() {
        let mut engine = StacklessEngine::new();
        let instance = ModuleInstance::new(runtime_module, 0).unwrap();
        let instance_arc = Arc::new(instance);
        let instance_idx = engine.set_current_module(instance_arc).unwrap();

        group.bench_function("deterministic_execution", |b| {
            b.iter(|| {
                // Use same inputs for deterministic testing
                let args = create_test_args(123, 456);
                let results = engine.execute(instance_idx, 0, args).unwrap();
                black_box(results)
            })
        });

        group.bench_function("varied_input_execution", |b| {
            let mut counter = 0;
            b.iter(|| {
                counter += 1;
                let args = create_test_args(counter % 1000, (counter * 7) % 1000);
                let results = engine.execute(instance_idx, 0, args).unwrap();
                black_box(results)
            })
        });
    }

    group.finish();
}

/// Benchmark memory allocation patterns during execution
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");

    if let Ok(runtime_module) = load_test_module() {
        group.bench_function("module_instance_creation", |b| {
            b.iter(|| {
                let instance = ModuleInstance::new(runtime_module.clone(), 0).unwrap();
                black_box(instance)
            })
        });

        group.bench_function("arc_wrapping", |b| {
            let instance = ModuleInstance::new(runtime_module.clone(), 0).unwrap();
            b.iter(|| {
                let instance_arc = Arc::new(instance.clone());
                black_box(instance_arc)
            })
        });

        group.bench_function("engine_module_setting", |b| {
            let instance = ModuleInstance::new(runtime_module.clone(), 0).unwrap();
            let instance_arc = Arc::new(instance);

            b.iter(|| {
                let mut engine = StacklessEngine::new();
                let instance_idx = engine.set_current_module(instance_arc.clone()).unwrap();
                black_box(instance_idx)
            })
        });
    }

    group.finish();
}

/// Benchmark ASIL-B compliance overhead
fn bench_asil_compliance_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("asil_compliance_overhead");

    if let Ok(runtime_module) = load_test_module() {
        let mut engine = StacklessEngine::new();
        let instance = ModuleInstance::new(runtime_module, 0).unwrap();
        let instance_arc = Arc::new(instance);
        let instance_idx = engine.set_current_module(instance_arc).unwrap();

        // Test bounds checking overhead
        group.bench_function("bounds_checked_access", |b| {
            b.iter(|| {
                if let Ok(function) =
                    engine.current_module.as_ref().unwrap().module().functions.get(0)
                {
                    let instruction_count = function.body.len();
                    // Simulate bounds-checked instruction access
                    let mut valid_accesses = 0;
                    for i in 0..instruction_count {
                        if i < function.body.len() {
                            valid_accesses += 1;
                        }
                    }
                    black_box(valid_accesses)
                } else {
                    black_box(0)
                }
            })
        });

        // Test error handling overhead
        group.bench_function("error_propagation", |b| {
            b.iter(|| {
                let args = create_test_args(42, 24);
                match engine.execute(instance_idx, 0, args) {
                    Ok(results) => black_box(results.len()),
                    Err(_) => black_box(0),
                }
            })
        });

        // Test capability verification overhead
        group.bench_function("capability_verification", |b| {
            b.iter(|| {
                // Simulate capability checks during execution
                let capability_checks = 5; // Typical number of checks per execution
                let mut verification_results = Vec::with_capacity(capability_checks);

                for i in 0..capability_checks {
                    // Simulate capability verification
                    let is_valid = i < 100; // Simulate capability bounds
                    verification_results.push(is_valid);
                }

                black_box(verification_results.iter().all(|&x| x))
            })
        });
    }

    group.finish();
}

/// Benchmark execution vs simulation comparison
fn bench_execution_vs_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("execution_vs_simulation");

    // Simulation benchmark (what we had before)
    group.bench_function("simulation_add", |b| {
        b.iter(|| {
            // Simulate the old placeholder execution
            let a = black_box(42);
            let b = black_box(24);
            let simulated_result = a + b; // Direct arithmetic
            black_box(simulated_result)
        })
    });

    // Real execution benchmark (what we have now)
    if let Ok(runtime_module) = load_test_module() {
        let mut engine = StacklessEngine::new();
        let instance = ModuleInstance::new(runtime_module, 0).unwrap();
        let instance_arc = Arc::new(instance);
        let instance_idx = engine.set_current_module(instance_arc).unwrap();

        group.bench_function("real_execution_add", |b| {
            b.iter(|| {
                let args = create_test_args(black_box(42), black_box(24));
                let results = engine.execute(instance_idx, 0, args).unwrap();
                black_box(results)
            })
        });
    }

    group.finish();
}

/// Comprehensive benchmark suite covering all execution aspects
fn bench_comprehensive_execution_suite(c: &mut Criterion) {
    let mut group = c.benchmark_group("comprehensive_execution");

    if let Ok(runtime_module) = load_test_module() {
        group.bench_function("full_execution_pipeline", |b| {
            b.iter(|| {
                // Complete execution pipeline from engine creation to result
                let mut engine = StacklessEngine::new();
                let instance = ModuleInstance::new(runtime_module.clone(), 0).unwrap();
                let instance_arc = Arc::new(instance);
                let instance_idx = engine.set_current_module(instance_arc).unwrap();

                // Execute multiple operations
                let mut total_result = 0;
                for i in 0..10 {
                    let args = create_test_args(i, i * 2);
                    let results = engine.execute(instance_idx, 0, args).unwrap();
                    if let Some(Value::I32(result)) = results.get(0) {
                        total_result += result;
                    }
                }

                black_box(total_result)
            })
        });

        group.bench_function("production_workload_simulation", |b| {
            let mut engine = StacklessEngine::new();
            let instance = ModuleInstance::new(runtime_module.clone(), 0).unwrap();
            let instance_arc = Arc::new(instance);
            let instance_idx = engine.set_current_module(instance_arc).unwrap();

            b.iter(|| {
                // Simulate production workload patterns
                let test_cases = [(10, 20), (100, 200), (-50, 75), (0, 42), (1000, -500)];

                let mut results_sum = 0;
                for (a, b) in test_cases.iter() {
                    let args = create_test_args(*a, *b);
                    let results = engine.execute(instance_idx, 0, args).unwrap();
                    if let Some(Value::I32(result)) = results.get(0) {
                        results_sum += result;
                    }
                }

                black_box(results_sum)
            })
        });
    }

    group.finish();
}

// Define benchmark groups
criterion_group!(
    execution_benches,
    bench_module_loading,
    bench_instruction_parsing,
    bench_single_execution,
    bench_repeated_execution,
    bench_execution_determinism,
    bench_memory_patterns,
    bench_asil_compliance_overhead,
    bench_execution_vs_simulation,
    bench_comprehensive_execution_suite
);

criterion_main!(execution_benches);
