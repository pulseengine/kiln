use std::sync::Arc;

use wrt_component::{Component, ComponentType};
use wrt_error::Result;
use wrt_foundation::values::Value;
use wrt_host::{CallbackRegistry, function::CloneableFn};
use wrt_intercept::{
    LinkInterceptor,
    strategies::{FirewallBuilder, LoggingStrategy, StatisticsStrategy},
};
use wrt_runtime::RuntimeInstance;

fn main() -> Result<()> {
    // Create a mock component type
    let component_type = ComponentType::new();

    // Create a callback registry with a test function
    let mut registry = CallbackRegistry::new();
    registry.register_host_function(
        "test",
        "double",
        CloneableFn::new(|_ctx| {
            println!("Host function 'test.double' called");
            Ok(vec![Value::I32(84)]) // Double 42
        }),
    );

    // Create a logging interceptor
    let log_sink = Arc::new(|log_entry: &str| println!("[LOG] {}", log_entry));
    let logging_strategy = LoggingStrategy::new(log_sink);

    // Create a firewall interceptor
    let firewall = FirewallBuilder::new(false)
        .allow_function("component", "test::double", "test.double")
        .build();

    // Create a statistics interceptor
    let stats = StatisticsStrategy::new();
    let stats_rc = Arc::new(stats);

    // Create an interceptor and add strategies
    let mut interceptor = LinkInterceptor::new("example_interceptor");
    interceptor.add_strategy(Arc::new(logging_strategy));
    interceptor.add_strategy(Arc::new(firewall));
    interceptor.add_strategy(stats_rc.clone());

    // Create a component with the interceptor
    let component = Component::new(component_type)
        .with_callback_registry(Arc::new(registry))
        .with_runtime(RuntimeInstance::new())
        .with_interceptor(Arc::new(interceptor));

    // Call a host function through the component
    println!("\nCalling test.double function...");
    let result = component.call_host_function("test.double", &[Value::I32(42)])?;
    println!("Result: {:?}", result);

    // Print statistics
    println!("\nFunction call statistics:");
    for (name, stats) in stats_rc.get_all_stats() {
        println!(
            "{}: {} calls, {} successful, {} errors, avg time: {:.2}ms",
            name, stats.call_count, stats.success_count, stats.error_count, stats.avg_time_ms
        );
    }

    // Try calling a disallowed function
    println!("\nTrying to call a disallowed function...");
    let result = component.call_host_function("forbidden.function", &[Value::I32(42)]);
    println!("Result: {:?}", result);

    Ok(())
}
