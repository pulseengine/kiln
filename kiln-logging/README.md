# wrt-logging

> Logging infrastructure for WebAssembly components

## Overview

Enables WebAssembly components to log messages to the host environment. Provides different log levels and extensible handlers, supporting both standard and no_std environments.

## Features

- **Component logging** - WebAssembly to host message logging
- **Log levels** - Debug, Info, Warning, Error support
- **Custom handlers** - Extensible logging architecture  
- **Cross-environment** - Works in std and no_std

## Quick Start

```toml
[dependencies]
wrt-logging = "0.1"
```

```rust
use wrt_logging::{LogHandler, LogLevel};

struct ConsoleLogger;

impl LogHandler for ConsoleLogger {
    fn handle_log(&self, level: LogLevel, message: &str) -> wrt_logging::Result<()> {
        println!("{:?}: {}", level, message);
        Ok(())
    }
}

// Register with component runtime
let handler = Box::new(ConsoleLogger);
runtime.register_log_handler(handler);
```

## See Also

- [API Documentation](https://docs.rs/wrt-logging)
- [Component Model Guide](../docs/source/user_guide/component_model.rst)