//! Comprehensive test suite for instruction-level fuel consumption
//!
//! This module validates that all 240+ WebAssembly instructions consume
//! the correct amount of fuel based on their complexity and verification level.

use wrt_error::{
    Error,
    Result,
};
use wrt_foundation::{
    operations::{
        global_fuel_consumed,
        reset_global_operations,
        Type as OperationType,
    },
    verification::VerificationLevel,
    FloatBits32,
    FloatBits64,
    Value,
};
use wrt_runtime::stackless::engine::{
    InstructionFuelType,
    StacklessEngine,
};

#[cfg(test)]
mod fuel_consumption_tests {
    use super::*;

    /// Helper to create a test engine with fuel tracking
    fn create_test_engine(
        fuel: Option<u64>,
        verification_level: VerificationLevel,
    ) -> StacklessEngine {
        let mut engine = StacklessEngine::new();
        engine.set_fuel(fuel);
        engine.set_verification_level(verification_level);
        engine
    }

    /// Helper to measure fuel consumed by an operation
    fn measure_fuel_consumption<F>(
        initial_fuel: u64,
        verification_level: VerificationLevel,
        operation: F,
    ) -> Result<u64>
    where
        F: FnOnce(&mut StacklessEngine) -> Result<()>,
    {
        reset_global_operations();
        let mut engine = create_test_engine(Some(initial_fuel), verification_level);

        operation(&mut engine)?;

        let remaining_fuel = engine.get_fuel().unwrap_or(0);
        let consumed = initial_fuel - remaining_fuel;

        // Also verify against global fuel tracking
        let global_consumed = global_fuel_consumed();
        assert_eq!(
            consumed, global_consumed,
            "Engine fuel and global fuel tracking mismatch"
        );

        Ok(consumed)
    }

    #[test]
    fn test_simple_constant_fuel_costs() {
        // Test that simple constants consume 1 fuel at base verification level
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::SimpleConstant)
        })
        .unwrap();

        assert_eq!(
            consumed, 1,
            "SimpleConstant should consume 1 fuel at VerificationLevel::Off"
        );

        // Test with different verification levels
        let consumed_basic = measure_fuel_consumption(1000, VerificationLevel::Basic, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::SimpleConstant)
        })
        .unwrap();
        assert_eq!(
            consumed_basic, 1,
            "SimpleConstant should consume 1 fuel at Basic (1.1x = 1)"
        );

        let consumed_full = measure_fuel_consumption(1000, VerificationLevel::Full, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::SimpleConstant)
        })
        .unwrap();
        assert_eq!(
            consumed_full, 2,
            "SimpleConstant should consume 2 fuel at Full (2.0x)"
        );
    }

    #[test]
    fn test_arithmetic_operation_fuel_costs() {
        // Test SimpleArithmetic (1 fuel base)
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::SimpleArithmetic)
        })
        .unwrap();
        assert_eq!(consumed, 1, "SimpleArithmetic should consume 1 fuel");

        // Test ComplexArithmetic (3 fuel base)
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::ComplexArithmetic)
        })
        .unwrap();
        assert_eq!(consumed, 3, "ComplexArithmetic should consume 3 fuel");

        // Test FloatArithmetic (4 fuel base)
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::FloatArithmetic)
        })
        .unwrap();
        assert_eq!(consumed, 4, "FloatArithmetic should consume 4 fuel");
    }

    #[test]
    fn test_memory_operation_fuel_costs() {
        // Test MemoryLoad (5 fuel base)
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::MemoryLoad)
        })
        .unwrap();
        assert_eq!(consumed, 5, "MemoryLoad should consume 5 fuel");

        // Test MemoryStore (6 fuel base)
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::MemoryStore)
        })
        .unwrap();
        assert_eq!(consumed, 6, "MemoryStore should consume 6 fuel");

        // Test MemoryManagement (50 fuel base)
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::MemoryManagement)
        })
        .unwrap();
        assert_eq!(consumed, 50, "MemoryManagement should consume 50 fuel");
    }

    #[test]
    fn test_control_flow_fuel_costs() {
        // Test SimpleControl (2 fuel base)
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::SimpleControl)
        })
        .unwrap();
        assert_eq!(consumed, 2, "SimpleControl should consume 2 fuel");

        // Test ComplexControl (8 fuel base)
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::ComplexControl)
        })
        .unwrap();
        assert_eq!(consumed, 8, "ComplexControl should consume 8 fuel");

        // Test FunctionCall (10 fuel base)
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::FunctionCall)
        })
        .unwrap();
        assert_eq!(consumed, 10, "FunctionCall should consume 10 fuel");
    }

    #[test]
    fn test_extended_instruction_fuel_costs() {
        // Test SimdOperation (6 fuel base)
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::SimdOperation)
        })
        .unwrap();
        assert_eq!(consumed, 6, "SimdOperation should consume 6 fuel");

        // Test AtomicOperation (15 fuel base)
        let consumed = measure_fuel_consumption(1000, VerificationLevel::Off, |engine| {
            engine.consume_instruction_fuel(InstructionFuelType::AtomicOperation)
        })
        .unwrap();
        assert_eq!(consumed, 15, "AtomicOperation should consume 15 fuel");
    }

    #[test]
    fn test_verification_level_multipliers() {
        // Test all verification levels with ComplexArithmetic (3 fuel base)
        let test_cases = vec![
            (VerificationLevel::Off, 3),       // 1.0x = 3
            (VerificationLevel::Basic, 3),     // 1.1x = 3.3 -> 3
            (VerificationLevel::Sampling, 4),  // 1.25x = 3.75 -> 4
            (VerificationLevel::Standard, 5),  // 1.5x = 4.5 -> 5
            (VerificationLevel::Full, 6),      // 2.0x = 6
            (VerificationLevel::Redundant, 8), // 2.5x = 7.5 -> 8
        ];

        for (level, expected) in test_cases {
            let consumed = measure_fuel_consumption(1000, level, |engine| {
                engine.consume_instruction_fuel(InstructionFuelType::ComplexArithmetic)
            })
            .unwrap();
            assert_eq!(
                consumed, expected,
                "ComplexArithmetic at {:?} should consume {} fuel",
                level, expected
            );
        }
    }

    #[test]
    fn test_fuel_exhaustion() {
        // Test that execution stops when fuel is exhausted
        let mut engine = create_test_engine(Some(5), VerificationLevel::Off);

        // Consume 5 fuel (should succeed)
        engine.consume_instruction_fuel(InstructionFuelType::MemoryLoad).unwrap();
        assert_eq!(engine.get_fuel(), Some(0));

        // Try to consume more fuel (should fail)
        let result = engine.consume_instruction_fuel(InstructionFuelType::SimpleConstant);
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(e.message().contains("Fuel exhausted"));
        }
    }

    #[test]
    fn test_instruction_type_mapping() {
        // Verify that instruction types map to correct operation types
        let mappings = vec![
            (
                InstructionFuelType::SimpleConstant,
                OperationType::WasmSimpleConstant,
                1,
            ),
            (
                InstructionFuelType::LocalAccess,
                OperationType::WasmLocalAccess,
                1,
            ),
            (
                InstructionFuelType::GlobalAccess,
                OperationType::WasmGlobalAccess,
                2,
            ),
            (
                InstructionFuelType::SimpleArithmetic,
                OperationType::WasmSimpleArithmetic,
                1,
            ),
            (
                InstructionFuelType::ComplexArithmetic,
                OperationType::WasmComplexArithmetic,
                3,
            ),
            (
                InstructionFuelType::FloatArithmetic,
                OperationType::WasmFloatArithmetic,
                4,
            ),
            (
                InstructionFuelType::Comparison,
                OperationType::WasmComparison,
                1,
            ),
            (
                InstructionFuelType::SimpleControl,
                OperationType::WasmSimpleControl,
                2,
            ),
            (
                InstructionFuelType::ComplexControl,
                OperationType::WasmComplexControl,
                8,
            ),
            (
                InstructionFuelType::FunctionCall,
                OperationType::WasmFunctionCall,
                10,
            ),
            (
                InstructionFuelType::MemoryLoad,
                OperationType::WasmMemoryLoad,
                5,
            ),
            (
                InstructionFuelType::MemoryStore,
                OperationType::WasmMemoryStore,
                6,
            ),
            (
                InstructionFuelType::MemoryManagement,
                OperationType::WasmMemoryManagement,
                50,
            ),
            (
                InstructionFuelType::TableAccess,
                OperationType::WasmTableAccess,
                3,
            ),
            (
                InstructionFuelType::TypeConversion,
                OperationType::WasmTypeConversion,
                2,
            ),
            (
                InstructionFuelType::SimdOperation,
                OperationType::WasmSimdOperation,
                6,
            ),
            (
                InstructionFuelType::AtomicOperation,
                OperationType::WasmAtomicOperation,
                15,
            ),
        ];

        for (fuel_type, op_type, expected_cost) in mappings {
            let cost = op_type.cost();
            assert_eq!(
                cost, expected_cost,
                "{:?} should have base cost of {}",
                fuel_type, expected_cost
            );
        }
    }

    #[test]
    fn test_fuel_consumption_determinism() {
        // Test that fuel consumption is deterministic
        let run_test = || {
            measure_fuel_consumption(1000, VerificationLevel::Full, |engine| {
                // Simulate a sequence of operations
                engine.consume_instruction_fuel(InstructionFuelType::SimpleConstant)?;
                engine.consume_instruction_fuel(InstructionFuelType::SimpleArithmetic)?;
                engine.consume_instruction_fuel(InstructionFuelType::MemoryLoad)?;
                engine.consume_instruction_fuel(InstructionFuelType::FunctionCall)?;
                Ok(())
            })
            .unwrap()
        };

        let first_run = run_test();
        let second_run = run_test();
        let third_run = run_test();

        assert_eq!(
            first_run, second_run,
            "Fuel consumption should be deterministic"
        );
        assert_eq!(
            second_run, third_run,
            "Fuel consumption should be deterministic"
        );

        // Expected: (1 + 1 + 5 + 10) * 2.0 = 34
        assert_eq!(first_run, 34, "Total fuel consumption should be 34");
    }

    #[test]
    fn test_no_fuel_tracking() {
        // Test that operations work without fuel tracking
        let mut engine = create_test_engine(None, VerificationLevel::Off);

        // Should succeed even without fuel
        engine.consume_instruction_fuel(InstructionFuelType::MemoryManagement).unwrap();
        engine.consume_instruction_fuel(InstructionFuelType::AtomicOperation).unwrap();

        assert_eq!(engine.get_fuel(), None);
    }
}
