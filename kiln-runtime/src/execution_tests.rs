//! Test module for ExecutionStats
//!
//! This module provides comprehensive tests for the ExecutionStats struct
//! to ensure ASIL-B compliance and proper functionality.

#[cfg(test)]
mod tests {
    use super::super::execution::ExecutionStats;

    #[test]
    fn test_execution_stats_creation() {
        let stats = ExecutionStats::new();
        assert_eq!(stats.instructions_executed, 0);
        assert_eq!(stats.memory_usage, 0);
        assert_eq!(stats.max_stack_depth, 0);
        assert_eq!(stats.function_calls, 0);
        assert_eq!(stats.memory_reads, 0);
        assert_eq!(stats.memory_writes, 0);
        assert_eq!(stats.execution_time_us, 0);
        assert_eq!(stats.gas_used, 0);
        assert_eq!(stats.gas_limit, 0);
        assert_eq!(stats.simd_operations_executed, 0);
    }

    #[test]
    fn test_execution_stats_reset() {
        let mut stats = ExecutionStats::new();

        // Modify some values
        stats.instructions_executed = 100;
        stats.memory_usage = 1024;
        stats.max_stack_depth = 10;

        // Reset
        stats.reset();

        // Verify all values are zero
        assert_eq!(stats.instructions_executed, 0);
        assert_eq!(stats.memory_usage, 0);
        assert_eq!(stats.max_stack_depth, 0);
    }

    #[test]
    fn test_increment_instructions() {
        let mut stats = ExecutionStats::new();

        stats.increment_instructions(10);
        assert_eq!(stats.instructions_executed, 10);

        stats.increment_instructions(5);
        assert_eq!(stats.instructions_executed, 15);
    }

    #[test]
    fn test_increment_instructions_overflow_protection() {
        let mut stats = ExecutionStats::new();
        stats.instructions_executed = u64::MAX - 5;

        // Should use saturating add to prevent overflow
        stats.increment_instructions(10);
        assert_eq!(stats.instructions_executed, u64::MAX);
    }

    #[test]
    fn test_update_memory_usage() {
        let mut stats = ExecutionStats::new();

        stats.update_memory_usage(1024);
        assert_eq!(stats.memory_usage, 1024);

        stats.update_memory_usage(512);
        assert_eq!(stats.memory_usage, 1536);
    }

    #[test]
    fn test_update_memory_usage_overflow_protection() {
        let mut stats = ExecutionStats::new();
        stats.memory_usage = usize::MAX - 100;

        // Should use saturating add to prevent overflow
        stats.update_memory_usage(200);
        assert_eq!(stats.memory_usage, usize::MAX);
    }

    #[test]
    fn test_update_stack_depth() {
        let mut stats = ExecutionStats::new();

        stats.update_stack_depth(5);
        assert_eq!(stats.max_stack_depth, 5);

        stats.update_stack_depth(3);
        assert_eq!(stats.max_stack_depth, 5); // Should keep maximum

        stats.update_stack_depth(10);
        assert_eq!(stats.max_stack_depth, 10); // Should update to new maximum
    }

    #[test]
    fn test_increment_function_calls() {
        let mut stats = ExecutionStats::new();

        stats.increment_function_calls(1);
        assert_eq!(stats.function_calls, 1);

        stats.increment_function_calls(5);
        assert_eq!(stats.function_calls, 6);
    }

    #[test]
    fn test_increment_memory_reads() {
        let mut stats = ExecutionStats::new();

        stats.increment_memory_reads(10);
        assert_eq!(stats.memory_reads, 10);

        stats.increment_memory_reads(5);
        assert_eq!(stats.memory_reads, 15);
    }

    #[test]
    fn test_increment_memory_writes() {
        let mut stats = ExecutionStats::new();

        stats.increment_memory_writes(7);
        assert_eq!(stats.memory_writes, 7);

        stats.increment_memory_writes(3);
        assert_eq!(stats.memory_writes, 10);
    }

    #[test]
    fn test_update_execution_time() {
        let mut stats = ExecutionStats::new();

        stats.update_execution_time(1000);
        assert_eq!(stats.execution_time_us, 1000);

        stats.update_execution_time(500);
        assert_eq!(stats.execution_time_us, 1500);
    }

    #[test]
    fn test_consume_gas() {
        let mut stats = ExecutionStats::new();
        stats.gas_limit = 1000;

        // Normal consumption
        assert!(stats.use_gas(100).is_ok());
        assert_eq!(stats.gas_used, 100);

        assert!(stats.use_gas(200).is_ok());
        assert_eq!(stats.gas_used, 300);
    }

    #[test]
    fn test_consume_gas_overflow_protection() {
        let mut stats = ExecutionStats::new();
        stats.gas_limit = 1000;
        stats.gas_used = u64::MAX - 100;

        // Should use saturating add
        assert!(stats.use_gas(200).is_err());
        assert_eq!(stats.gas_used, u64::MAX);
    }

    #[test]
    fn test_consume_gas_exceeds_limit() {
        let mut stats = ExecutionStats::new();
        stats.gas_limit = 1000;
        stats.gas_used = 900;

        // Should fail when exceeding limit
        assert!(stats.use_gas(200).is_err());
        assert_eq!(stats.gas_used, 1100); // Gas was consumed even though it
                                          // exceeded limit
    }

    #[test]
    fn test_simd_operations_field() {
        let stats = ExecutionStats::new();

        // Verify field exists and defaults to 0
        assert_eq!(stats.simd_operations_executed, 0);
    }

    #[test]
    fn test_set_gas_limit() {
        let mut stats = ExecutionStats::new();

        stats.set_gas_limit(5000);
        assert_eq!(stats.gas_limit, 5000);

        // Can update gas limit
        stats.set_gas_limit(10000);
        assert_eq!(stats.gas_limit, 10000);
    }
}
