# WebAssembly Runtime Interception Layer (wrt-intercept)

This crate provides a flexible interception mechanism for WebAssembly component linking in the WebAssembly Runtime (WRT). It allows intercepting function calls between components and between components and the host.

## Features

- **Flexible Strategy Pattern**: Add multiple interception strategies to modify or monitor function calls
- **Transparent Interception**: Hook into function calls without modifying component or host code
- **Built-in Strategies**:
  - **Logging**: Log function calls with arguments, results, and timing
  - **Firewall**: Enforce security policies on component interactions
  - **Statistics**: Collect metrics on function usage and performance

## Usage

### Basic Usage

```rust
use std::sync::Arc;
use wrt_intercept::{LinkInterceptor, strategies::LoggingStrategy};

// Create a logging strategy
let log_sink = Arc::new(|entry: &str| println!("{}", entry));
let logging = LoggingStrategy::new(log_sink);

// Create an interceptor and add the strategy
let mut interceptor = LinkInterceptor::new("my_interceptor");
interceptor.add_strategy(Arc::new(logging));

// Attach to a component
// component.with_interceptor(Arc::new(interceptor));
```

### Using the Firewall Strategy

```rust
use wrt_intercept::strategies::{FirewallBuilder, FirewallRule};

// Create a firewall that denies by default
let firewall = FirewallBuilder::new(false)
    .allow_function("component_a", "component_b", "allowed_function")
    .allow_source("trusted_component", "component_b")
    .deny_function("any_component", "sensitive_component", "dangerous_function")
    .build();

// Add to interceptor
let mut interceptor = LinkInterceptor::new("security");
interceptor.add_strategy(Arc::new(firewall));
```

### Collecting Statistics

```rust
use wrt_intercept::strategies::{StatisticsStrategy, StatisticsConfig};

// Create a statistics collector with custom config
let config = StatisticsConfig {
    track_timing: true,
    track_errors: true,
    max_functions: 100,
};

let stats = StatisticsStrategy::with_config(config);
let stats_arc = Arc::new(stats.clone());

// Add to interceptor
let mut interceptor = LinkInterceptor::new("metrics");
interceptor.add_strategy(stats_arc.clone());

// Later, retrieve statistics
let all_stats = stats_arc.get_all_stats();
for (func_name, func_stats) in all_stats {
    println!("{}: {} calls, avg {}ms", func_name, func_stats.call_count, func_stats.avg_time_ms);
}
```

## Creating Custom Strategies

To create a custom strategy, implement the `LinkInterceptorStrategy` trait:

```rust
use std::sync::Arc;
use wrt_intercept::{LinkInterceptorStrategy, LinkInterceptor};
use wrt_error::Result;
use wrt_foundation::values::Value;

struct MyCustomStrategy {
    // Your strategy's state
}

impl LinkInterceptorStrategy for MyCustomStrategy {
    fn before_call(
        &self,
        source: &str,
        target: &str,
        function: &str,
        args: &[Value],
    ) -> Result<Vec<Value>> {
        // Pre-process the function call
        // You can modify arguments or perform checks here
        println!("Call from {} to {}::{}", source, target, function);
        
        // Return original or modified arguments
        Ok(args.to_vec())
    }

    fn after_call(
        &self,
        source: &str,
        target: &str,
        function: &str,
        args: &[Value],
        result: Result<Vec<Value>>,
    ) -> Result<Vec<Value>> {
        // Post-process the function call
        // You can modify results here
        println!("Return from {}::{} to {}", target, function, source);
        
        // Return original or modified result
        result
    }

    fn should_bypass(&self) -> bool {
        // Return true to skip the actual function call
        false
    }

    fn clone_strategy(&self) -> Arc<dyn LinkInterceptorStrategy> {
        // Create a clone of this strategy
        Arc::new(Self {
            // Clone state
        })
    }
}
```

## Feature Flags

- `std` (default): Enables standard library features
- `no_std`: Supports environments without the standard library
- `kani`: Enables Kani verification proofs
- `log`: Enables integration with the `log` crate

## No-std Support

This crate can be used in no-std environments by disabling the default `std` feature and enabling the `no_std` feature:

```toml
[dependencies]
wrt-intercept = { version = "0.2.0", default-features = false, features = ["no_std"] }
```

Note that some strategies (like `StatisticsStrategy`) have limited functionality in no-std mode.

## License

This project is licensed under the MIT License.