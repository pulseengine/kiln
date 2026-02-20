//! Threading and concurrency support
//!
//! This module provides threading primitives, task management, and
//! concurrency control for the WebAssembly Component Model.

pub mod advanced_threading_builtins;
pub mod task_builtins;
pub mod task_cancellation;
pub mod task_manager;
pub mod thread_builtins;
pub mod thread_spawn;
pub mod thread_spawn_fuel;
pub mod waitable_set_builtins;

pub use advanced_threading_builtins::*;
pub use task_builtins::*;
pub use task_cancellation::*;
pub use task_manager::*;
pub use thread_builtins::*;
pub use thread_spawn::*;
pub use thread_spawn_fuel::*;
pub use waitable_set_builtins::*;
