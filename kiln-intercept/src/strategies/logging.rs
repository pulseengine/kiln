//! Logging strategy for intercepting component function calls
//!
//! This strategy logs function calls between components and hosts.
//! It can be configured to log arguments, results, timing, etc.

#[cfg(feature = "std")]
use std::{
    sync::{
        Arc,
        Mutex,
    },
    time::Instant,
};

use wrt_error::Result;

// Import the prelude for unified access to standard types
use crate::prelude::{
    str,
    Debug,
    Value,
};
use crate::LinkInterceptorStrategy;

/// Trait for formatting values in logging output
#[cfg(feature = "std")]
pub trait ValueFormatter: Clone + Send + Sync {
    /// Format a value for logging
    fn format_value(&self, value: &Value) -> String;
}

/// Default formatter for values
#[cfg(feature = "std")]
#[derive(Clone)]
pub struct DefaultValueFormatter;

#[cfg(feature = "std")]
impl ValueFormatter for DefaultValueFormatter {
    fn format_value(&self, value: &Value) -> String {
        match value {
            Value::I32(v) => format!("I32({v})"),
            Value::I64(v) => format!("I64({v})"),
            Value::F32(v) => format!("F32({})", f32::from_bits(v.0)),
            Value::F64(v) => format!("F64({})", f64::from_bits(v.0)),
            // Add other value types as needed
            _ => format!("{value:?}"),
        }
    }
}

/// A trait for receiving log entries
#[cfg(feature = "std")]
pub trait LogSink: Send + Sync {
    /// Write a log entry
    fn write_log(&self, entry: &str);
}

/// Configuration for the logging strategy
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Whether to log arguments
    pub log_args:    bool,
    /// Whether to log results
    pub log_results: bool,
    /// Whether to log timing information
    pub log_timing:  bool,
    /// Maximum number of arguments to log (0 for unlimited)
    pub max_args:    usize,
    /// Maximum number of results to log (0 for unlimited)
    pub max_results: usize,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_args:    true,
            log_results: true,
            log_timing:  true,
            max_args:    10,
            max_results: 10,
        }
    }
}

/// A strategy that logs function calls
#[cfg(feature = "std")]
pub struct LoggingStrategy<S: LogSink, F: ValueFormatter = DefaultValueFormatter> {
    /// Log sink to write logs to
    sink:      Arc<S>,
    /// Value formatter
    formatter: F,
    /// Configuration
    config:    LoggingConfig,
    /// Thread-local storage for timing information
    #[cfg(feature = "std")]
    timing:    Arc<Mutex<Option<Instant>>>,
}

/// A simple logging strategy for `no_std` environments
#[cfg(not(feature = "std"))]
pub struct LoggingStrategy {
    /// Configuration
    config: LoggingConfig,
}

#[cfg(feature = "std")]
impl<S: LogSink> LoggingStrategy<S> {
    /// Create a new logging strategy with default formatter
    pub fn new(sink: Arc<S>) -> Self {
        Self {
            sink,
            formatter: DefaultValueFormatter,
            config: LoggingConfig::default(),
            timing: Arc::new(Mutex::new(None)),
        }
    }
}

#[cfg(feature = "std")]
impl<S: LogSink, F: ValueFormatter> LoggingStrategy<S, F> {
    /// Create a new logging strategy with custom formatter
    pub fn with_formatter(sink: Arc<S>, formatter: F) -> Self {
        Self {
            sink,
            formatter,
            config: LoggingConfig::default(),
            timing: Arc::new(Mutex::new(None)),
        }
    }

    /// Configure the logging strategy
    #[must_use]
    pub fn with_config(mut self, config: LoggingConfig) -> Self {
        self.config = config;
        self
    }
}

#[cfg(feature = "std")]
impl<S: LogSink + 'static, F: ValueFormatter + 'static> LinkInterceptorStrategy
    for LoggingStrategy<S, F>
{
    fn before_call(
        &self,
        source: &str,
        target: &str,
        function: &str,
        args: &[Value],
    ) -> Result<Vec<Value>> {
        use core::fmt::Write;

        // Format the function call
        let mut log_entry = format!("CALL: {source}->{target}::{function}");

        // Add arguments if configured
        if self.config.log_args && !args.is_empty() {
            let mut args_str = String::new();
            let limit = if self.config.max_args > 0 {
                self.config.max_args.min(args.len())
            } else {
                args.len()
            };

            for (i, arg) in args.iter().take(limit).enumerate() {
                if i > 0 {
                    args_str.push_str(", ");
                }
                args_str.push_str(&self.formatter.format_value(arg));
            }

            if limit < args.len() {
                let _ = write!(args_str, ", ... ({} more)", args.len() - limit);
            }

            let _ = write!(log_entry, " args: [{args_str}]");
        }

        // Write the log entry
        self.sink.write_log(&log_entry);

        // Store start time if timing is enabled
        if self.config.log_timing {
            if let Ok(mut timing) = self.timing.lock() {
                *timing = Some(Instant::now());
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
        use core::fmt::Write;

        // Format the return
        let mut log_entry = format!("RETURN: {source}->{target}::{function}");

        // Add timing information if configured
        if self.config.log_timing {
            if let Ok(mut timing) = self.timing.lock() {
                if let Some(start_time) = timing.take() {
                    let elapsed = start_time.elapsed();
                    let _ = write!(log_entry, " elapsed: {elapsed:?}");
                }
            }
        }

        // Add results if configured
        if self.config.log_results {
            match &result {
                Ok(values) => {
                    if values.is_empty() {
                        log_entry.push_str(" result: []");
                    } else {
                        let mut result_str = String::new();
                        let limit = if self.config.max_results > 0 {
                            self.config.max_results.min(values.len())
                        } else {
                            values.len()
                        };

                        for (i, value) in values.iter().take(limit).enumerate() {
                            if i > 0 {
                                result_str.push_str(", ");
                            }
                            result_str.push_str(&self.formatter.format_value(value));
                        }

                        if limit < values.len() {
                            let _ = write!(result_str, ", ... ({} more)", values.len() - limit);
                        }

                        let _ = write!(log_entry, " result: [{result_str}]");
                    }
                },
                Err(e) => {
                    let _ = write!(log_entry, " error: {e}");
                },
            }
        }

        // Write the log entry
        self.sink.write_log(&log_entry);

        // Return unmodified result
        result
    }

    fn clone_strategy(&self) -> Arc<dyn LinkInterceptorStrategy> {
        Arc::new(Self {
            sink:      self.sink.clone(),
            formatter: self.formatter.clone(),
            config:    self.config.clone(),
            timing:    self.timing.clone(),
        })
    }
}

// Helper implementation for using a closure as a LogSink
#[cfg(feature = "std")]
impl<F> LogSink for F
where
    F: Fn(&str) + Send + Sync,
{
    fn write_log(&self, entry: &str) {
        self(entry);
    }
}

// Binary std/no_std choice
#[cfg(not(feature = "std"))]
impl Default for LoggingStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "std"))]
impl LoggingStrategy {
    /// Create a new logging strategy for `no_std` environments
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: LoggingConfig::default(),
        }
    }

    /// Configure the logging strategy
    #[must_use]
    pub fn with_config(mut self, config: LoggingConfig) -> Self {
        self.config = config;
        self
    }
}

#[cfg(not(feature = "std"))]
impl LinkInterceptorStrategy for LoggingStrategy {
    fn before_call(
        &self,
        _source: &str,
        _target: &str,
        _function: &str,
        _args: &[Value],
    ) -> Result<()> {
        // In no_std, we can't log, just pass through
        Ok(())
    }

    fn after_call(
        &self,
        _source: &str,
        _target: &str,
        _function: &str,
        _args: &[Value],
        result: Result<()>,
    ) -> Result<()> {
        // In no_std, we can't log, just pass through
        result
    }
}

#[cfg(feature = "std")]
/// A simple file log sink
pub struct FileLogSink {
    file: Mutex<std::fs::File>,
}

#[cfg(feature = "std")]
#[allow(dead_code)]
impl FileLogSink {
    /// Create a new file log sink
    fn new(path: &std::path::Path) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;

        Ok(Self {
            file: Mutex::new(file),
        })
    }
}

#[cfg(feature = "std")]
impl LogSink for FileLogSink {
    fn write_log(&self, entry: &str) {
        use std::io::Write;

        if let Ok(mut file) = self.file.lock() {
            // Intentionally ignore write errors in logging
            drop(writeln!(file, "{entry}"));
        }
    }
}

#[cfg(feature = "log")]
/// A log sink that uses the log crate
pub struct LogCrateSink {
    module: &'static str,
}

#[cfg(feature = "log")]
#[allow(dead_code)]
impl LogCrateSink {
    /// Create a new log crate sink
    fn new(module: &'static str) -> Self {
        Self { module }
    }
}

#[cfg(feature = "log")]
impl LogSink for LogCrateSink {
    fn write_log(&self, entry: &str) {
        log::debug!(target: self.module, "{entry}");
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use std::sync::{
        Arc,
        Mutex,
    };

    use super::*;

    struct TestSink {
        logs: Mutex<Vec<String>>,
    }

    impl LogSink for TestSink {
        fn write_log(&self, entry: &str) {
            if let Ok(mut logs) = self.logs.lock() {
                logs.push(entry.to_string());
            }
        }
    }

    #[test]
    fn test_logging_strategy() {
        let sink = Arc::new(TestSink {
            logs: Mutex::new(Vec::new()),
        });
        let strategy = LoggingStrategy::new(sink.clone());

        // Test before_call
        let args = vec![Value::I32(42), Value::I64(123)];
        let result = strategy.before_call("source", "target", "function", &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), args);

        // Check log entry
        let logs = sink.logs.lock().unwrap();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].contains("CALL: source->target::function"));
        assert!(logs[0].contains("I32(42)"));
        assert!(logs[0].contains("I64(123)"));
    }

    #[test]
    fn test_logging_strategy_after_call() {
        let sink = Arc::new(TestSink {
            logs: Mutex::new(Vec::new()),
        });
        let strategy = LoggingStrategy::new(sink.clone());

        // Test after_call with success
        let args = vec![Value::I32(42)];
        let result = Ok(vec![Value::I64(123)]);
        let after_result = strategy.after_call("source", "target", "function", &args, result);
        assert!(after_result.is_ok());
        assert_eq!(after_result.unwrap(), vec![Value::I64(123)]);

        // Check log entry
        let logs = sink.logs.lock().unwrap();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].contains("RETURN: source->target::function"));
        assert!(logs[0].contains("I64(123)"));
    }

    #[test]
    fn test_logging_strategy_config() {
        let sink = Arc::new(TestSink {
            logs: Mutex::new(Vec::new()),
        });
        let config = LoggingConfig {
            log_args:    false,
            log_results: true,
            log_timing:  false,
            max_args:    5,
            max_results: 5,
        };
        let strategy = LoggingStrategy::new(sink.clone()).with_config(config);

        // Test before_call with custom config
        let args = vec![Value::I32(42), Value::I64(123)];
        let _ = strategy.before_call("source", "target", "function", &args);

        // Check log entry - should not contain args
        let logs = sink.logs.lock().unwrap();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].contains("CALL: source->target::function"));
        assert!(!logs[0].contains("I32(42)"));
        assert!(!logs[0].contains("I64(123)"));
    }
}
