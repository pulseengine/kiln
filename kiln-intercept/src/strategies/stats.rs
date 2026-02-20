//! Statistics strategy for intercepting component function calls
//!
//! This strategy collects metrics on function calls between components and
//! hosts. It can track call counts, error rates, performance metrics, etc.
//!
//! Note: This strategy requires the `std` feature.

// Statistics module requires floating-point arithmetic for timing metrics.
// Precision loss in large counts is acceptable for statistics.
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::float_arithmetic)]

#[cfg(all(feature = "std", test))]
use std::time::Duration;
#[cfg(feature = "std")]
use std::{
    sync::{
        Arc,
        Mutex,
        RwLock,
    },
    time::Instant,
};

#[cfg(feature = "std")]
use wrt_error::Result;

#[cfg(feature = "std")]
use crate::{
    prelude::{
        str,
        Debug,
        HashMap,
        Value,
    },
    LinkInterceptorStrategy,
};

/// Statistics collected for a function (`no_std` version - simplified)
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FunctionStats {
    /// Number of times the function was called
    pub call_count:    u64,
    /// Number of successful calls
    pub success_count: u64,
    /// Number of failed calls
    pub error_count:   u64,
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::Checksummable for FunctionStats {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        for byte in self.call_count.to_le_bytes() {
            checksum.update(byte);
        }
        for byte in self.success_count.to_le_bytes() {
            checksum.update(byte);
        }
        for byte in self.error_count.to_le_bytes() {
            checksum.update(byte);
        }
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::ToBytes for FunctionStats {
    fn serialized_size(&self) -> usize {
        24 // 3 * u64 = 24 bytes
    }

    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_u64_le(self.call_count)?;
        writer.write_u64_le(self.success_count)?;
        writer.write_u64_le(self.error_count)?;
        Ok(())
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::FromBytes for FunctionStats {
    fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let call_count = reader.read_u64_le()?;
        let success_count = reader.read_u64_le()?;
        let error_count = reader.read_u64_le()?;

        Ok(Self {
            call_count,
            success_count,
            error_count,
        })
    }
}

/// Statistics collected for a function
#[cfg(feature = "std")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FunctionStats {
    /// Number of times the function was called
    pub call_count:    u64,
    /// Number of successful calls
    pub success_count: u64,
    /// Number of failed calls
    pub error_count:   u64,
    /// Total execution time in milliseconds
    pub total_time_ms: f64,
    /// Minimum execution time in milliseconds
    pub min_time_ms:   Option<f64>,
    /// Maximum execution time in milliseconds
    pub max_time_ms:   Option<f64>,
    /// Average execution time in milliseconds
    pub avg_time_ms:   f64,
}

#[cfg(feature = "std")]
impl Eq for FunctionStats {}

#[cfg(feature = "std")]
impl wrt_foundation::traits::Checksummable for FunctionStats {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        for byte in self.call_count.to_le_bytes() {
            checksum.update(byte);
        }
        for byte in self.success_count.to_le_bytes() {
            checksum.update(byte);
        }
        for byte in self.error_count.to_le_bytes() {
            checksum.update(byte);
        }
        for byte in self.total_time_ms.to_le_bytes() {
            checksum.update(byte);
        }
        if let Some(min) = self.min_time_ms {
            for byte in min.to_le_bytes() {
                checksum.update(byte);
            }
        }
        if let Some(max) = self.max_time_ms {
            for byte in max.to_le_bytes() {
                checksum.update(byte);
            }
        }
        for byte in self.avg_time_ms.to_le_bytes() {
            checksum.update(byte);
        }
    }
}

#[cfg(feature = "std")]
impl wrt_foundation::traits::ToBytes for FunctionStats {
    fn serialized_size(&self) -> usize {
        // 5 * u64 + 2 * Option<f64> + f64 = approximately 56 bytes max
        56
    }

    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_u64_le(self.call_count)?;
        writer.write_u64_le(self.success_count)?;
        writer.write_u64_le(self.error_count)?;
        writer.write_f64_le(self.total_time_ms)?;
        writer.write_u8(u8::from(self.min_time_ms.is_some()))?;
        if let Some(min) = self.min_time_ms {
            writer.write_f64_le(min)?;
        }
        writer.write_u8(u8::from(self.max_time_ms.is_some()))?;
        if let Some(max) = self.max_time_ms {
            writer.write_f64_le(max)?;
        }
        writer.write_f64_le(self.avg_time_ms)?;
        Ok(())
    }
}

#[cfg(feature = "std")]
impl wrt_foundation::traits::FromBytes for FunctionStats {
    fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let call_count = reader.read_u64_le()?;
        let success_count = reader.read_u64_le()?;
        let error_count = reader.read_u64_le()?;
        let total_time_ms = reader.read_f64_le()?;
        let has_min = reader.read_u8()? != 0;
        let min_time_ms = if has_min { Some(reader.read_f64_le()?) } else { None };
        let has_max = reader.read_u8()? != 0;
        let max_time_ms = if has_max { Some(reader.read_f64_le()?) } else { None };
        let avg_time_ms = reader.read_f64_le()?;

        Ok(Self {
            call_count,
            success_count,
            error_count,
            total_time_ms,
            min_time_ms,
            max_time_ms,
            avg_time_ms,
        })
    }
}

/// Configuration for the statistics strategy
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct StatisticsConfig {
    /// Whether to track timings
    pub track_timing:  bool,
    /// Whether to track errors
    pub track_errors:  bool,
    /// Maximum number of functions to track (0 for unlimited)
    pub max_functions: usize,
}

#[cfg(feature = "std")]
impl Default for StatisticsConfig {
    fn default() -> Self {
        Self {
            track_timing:  true,
            track_errors:  true,
            max_functions: 1000,
        }
    }
}

/// A strategy that collects statistics on function calls
#[cfg(feature = "std")]
pub struct StatisticsStrategy {
    /// Configuration for this strategy
    config:    StatisticsConfig,
    /// Statistics for each function
    stats:     RwLock<HashMap<String, FunctionStats>>,
    /// Cache of currently executing functions with their start times
    executing: std::sync::Mutex<HashMap<String, std::time::Instant>>,
}

#[cfg(feature = "std")]
impl Default for StatisticsStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl StatisticsStrategy {
    /// Create a new statistics strategy with default configuration
    #[must_use] 
    pub fn new() -> Self {
        Self {
            config:    StatisticsConfig::default(),
            stats:     RwLock::new(HashMap::new()),
            executing: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Create a new statistics strategy with custom configuration
    #[must_use] 
    pub fn with_config(config: StatisticsConfig) -> Self {
        Self {
            config,
            stats: RwLock::new(HashMap::new()),
            executing: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Helper function to generate a unique key for a function call
    fn function_key(source: &str, target: &str, function: &str) -> String {
        format!("{source}->{target}::{function}")
    }

    /// Get statistics for all functions
    pub fn get_all_stats(&self) -> HashMap<String, FunctionStats> {
        match self.stats.read() {
            Ok(stats) => stats.clone(),
            _ => HashMap::new(),
        }
    }

    /// Get statistics for a specific function
    pub fn get_function_stats(
        &self,
        source: &str,
        target: &str,
        function: &str,
    ) -> Option<FunctionStats> {
        let key = Self::function_key(source, target, function);
        match self.stats.read() {
            Ok(stats) => stats.get(&key).cloned(),
            _ => None,
        }
    }

    /// Reset all statistics
    pub fn reset(&self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.clear();
        }
        if let Ok(mut executing) = self.executing.lock() {
            executing.clear();
        }
    }
}

#[cfg(feature = "std")]
impl LinkInterceptorStrategy for StatisticsStrategy {
    fn before_call(
        &self,
        source: &str,
        target: &str,
        function: &str,
        args: &[Value],
    ) -> Result<Vec<Value>> {
        if self.config.track_timing {
            let key = Self::function_key(source, target, function);
            if let Ok(mut executing) = self.executing.lock() {
                executing.insert(key, Instant::now());
            }
        }

        // Return unmodified arguments
        Ok(args.to_vec())
    }

    fn after_call(
        &self,
        source: &str,
        target: &str,
        function: &str,
        _args: &[Value],
        result: Result<Vec<Value>>,
    ) -> Result<Vec<Value>> {
        let key = Self::function_key(source, target, function);
        let is_success = result.is_ok();
        let elapsed_ms = if self.config.track_timing {
            match self.executing.lock() {
                Ok(mut executing) => {
                    executing.remove(&key).map(|start| start.elapsed().as_secs_f64() * 1000.0)
                },
                _ => None,
            }
        } else {
            None
        };

        // Update statistics
        if let Ok(mut stats_map) = self.stats.write() {
            // Check if we're at the limit and this is a new function
            if self.config.max_functions > 0
                && stats_map.len() >= self.config.max_functions
                && !stats_map.contains_key(&key)
            {
                // At limit, don't track a new function
                return result;
            }

            let stats = stats_map.entry(key).or_insert_with(FunctionStats::default);

            // Update call counts
            stats.call_count += 1;
            if is_success {
                stats.success_count += 1;
            } else if self.config.track_errors {
                stats.error_count += 1;
            }

            // Update timing information
            if let Some(elapsed) = elapsed_ms {
                stats.total_time_ms += elapsed;

                if let Some(min) = stats.min_time_ms {
                    if elapsed < min {
                        stats.min_time_ms = Some(elapsed);
                    }
                } else {
                    stats.min_time_ms = Some(elapsed);
                }

                if let Some(max) = stats.max_time_ms {
                    if elapsed > max {
                        stats.max_time_ms = Some(elapsed);
                    }
                } else {
                    stats.max_time_ms = Some(elapsed);
                }

                stats.avg_time_ms = stats.total_time_ms / stats.call_count as f64;
            }
        }

        // Return unmodified result
        result
    }

    fn clone_strategy(&self) -> Arc<dyn LinkInterceptorStrategy> {
        Arc::new(Self {
            config:    self.config.clone(),
            stats:     RwLock::new(HashMap::new()),
            executing: Mutex::new(HashMap::new()),
        })
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    fn test_statistics_strategy() {
        let strategy = StatisticsStrategy::new();

        // Test before_call and after_call
        let source = "source";
        let target = "target";
        let function = "test_function";
        let args = vec![Value::I32(42)];

        // First call (success)
        strategy.before_call(source, target, function, &args).unwrap();
        thread::sleep(Duration::from_millis(10)); // Simulate some work
        let result = Ok(vec![Value::I64(123)]);
        strategy.after_call(source, target, function, &args, result).unwrap();

        // Second call (error)
        strategy.before_call(source, target, function, &args).unwrap();
        thread::sleep(Duration::from_millis(5)); // Simulate some work
        let result = Err(wrt_error::Error::runtime_error("Test error"));
        let _ = strategy.after_call(source, target, function, &args, result);

        // Check statistics
        let key = StatisticsStrategy::function_key(source, target, function);
        let stats = strategy.get_all_stats();
        assert!(stats.contains_key(&key));

        let func_stats = stats.get(&key).unwrap();
        assert_eq!(func_stats.call_count, 2);
        assert_eq!(func_stats.success_count, 1);
        assert_eq!(func_stats.error_count, 1);
        assert!(func_stats.total_time_ms > 0.0);
        assert!(func_stats.min_time_ms.unwrap() > 0.0);
        assert!(func_stats.max_time_ms.unwrap() > 0.0);
        assert!(func_stats.avg_time_ms > 0.0);
    }

    #[test]
    fn test_statistics_config() {
        let config = StatisticsConfig {
            track_timing:  false,
            track_errors:  true,
            max_functions: 10,
        };
        let strategy = StatisticsStrategy::with_config(config);

        let source = "source";
        let target = "target";
        let function = "test_function";
        let args = vec![Value::I32(42)];

        // Make a call
        strategy.before_call(source, target, function, &args).unwrap();
        let result = Ok(vec![Value::I64(123)]);
        strategy.after_call(source, target, function, &args, result).unwrap();

        // Check statistics - timing should not be tracked
        let key = StatisticsStrategy::function_key(source, target, function);
        let stats = strategy.get_all_stats();
        let func_stats = stats.get(&key).unwrap();
        assert_eq!(func_stats.call_count, 1);
        assert_eq!(func_stats.success_count, 1);
        assert_eq!(func_stats.total_time_ms, 0.0);
        assert!(func_stats.min_time_ms.is_none());
        assert!(func_stats.max_time_ms.is_none());
        assert_eq!(func_stats.avg_time_ms, 0.0);
    }

    #[test]
    fn test_statistics_reset() {
        let strategy = StatisticsStrategy::new();

        // Make a call
        let source = "source";
        let target = "target";
        let function = "test_function";
        let args = vec![Value::I32(42)];

        strategy.before_call(source, target, function, &args).unwrap();
        let result = Ok(vec![Value::I64(123)]);
        strategy.after_call(source, target, function, &args, result).unwrap();

        // Verify we have stats
        assert_eq!(strategy.get_all_stats().len(), 1);

        // Reset and verify
        strategy.reset();
        assert_eq!(strategy.get_all_stats().len(), 0);
    }
}
