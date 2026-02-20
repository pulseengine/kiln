#\!/bin/bash

# Fix corrupted error message patterns in async files
files=(
    "wrt-component/src/async_/fuel_preemption_support.rs"
    "wrt-component/src/async_/async_task_executor.rs"
    "wrt-component/src/async_/timer_integration.rs"
    "wrt-component/src/async_/fuel_debt_credit.rs"
    "wrt-component/src/async_/fuel_stream_handler.rs"
    "wrt-component/src/async_/fuel_deadline_scheduler.rs"
    "wrt-component/src/async_/async_combinators.rs"
    "wrt-component/src/async_/fuel_async_runtime.rs"
    "wrt-component/src/async_/fuel_dynamic_manager.rs"
    "wrt-component/src/async_/fuel_error_context.rs"
    "wrt-component/src/async_/async_canonical_abi_support.rs"
    "wrt-component/src/async_/optimized_async_channels.rs"
    "wrt-component/src/async_/fuel_resource_lifetime.rs"
    "wrt-component/src/async_/fuel_handle_table.rs"
    "wrt-component/src/async_/fuel_future_combinators.rs"
    "wrt-component/src/async_/fuel_aware_waker.rs"
    "wrt-component/src/async_/component_async_bridge.rs"
    "wrt-component/src/async_/fuel_resource_cleanup.rs"
    "wrt-component/src/async_/resource_async_operations.rs"
    "wrt-component/src/async_/task_manager_async_bridge.rs"
    "wrt-component/src/async_/component_model_async_ops.rs"
    "wrt-component/src/async_/fuel_wcet_analyzer.rs"
)

for file in "${files[@]}"; do
    if [ -f "$file" ]; then
        echo "Fixing $file..."
        # Pattern 1: "Error occurred"MessageMissing message" -> "Message"
        sed -i.bak -E 's/"Error occurred"([^"]*)(Missing message)+/"/' "$file"
        # Pattern 2: format\!(Missing message), -> format\!("Format error"),
        sed -i.bak2 -E 's/format\!\(Missing message\),/format\!("Format error"),/g' "$file"
        # Pattern 3: Missing message), -> "Error message"),
        sed -i.bak3 -E 's/Missing message\),/"Error message"),/g' "$file"
        # Pattern 4: log::warn\!(Missing message); -> log::warn\!("Warning message");
        sed -i.bak4 -E 's/log::warn\!\(Missing message\);/log::warn\!("Warning message");/g' "$file"
        # Clean up backup files
        rm -f "$file.bak" "$file.bak2" "$file.bak3" "$file.bak4"
    fi
done

echo "Fixed all corrupted error messages"
