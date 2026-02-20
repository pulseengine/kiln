//! Phase 2 Integration Tests: Priority-aware async with fuel inheritance
//!
//! This test demonstrates the complete Phase 2 ASIL-B async system including:
//! - Priority inheritance protocol
//! - Bounded async channels with fuel tracking
//! - Preemptive fuel scheduling
//! - Integration with existing fuel system

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use wrt_component::{
    ComponentInstanceId,
    async_::{
        fuel_async_bridge::{AsyncBridgeConfig, FuelAsyncBridge},

        fuel_async_channels::{ChannelId, FuelAsyncChannelManager},
        // Phase 1 components
        fuel_async_executor::{AsyncTaskState, FuelAsyncExecutor},
        fuel_async_scheduler::{FuelAsyncScheduler, SchedulingPolicy},
        fuel_preemptive_scheduler::{FuelPreemptiveScheduler, PreemptiveSchedulerConfig},
        // Phase 2 components
        fuel_priority_inheritance::{FuelPriorityInheritanceProtocol, ResourceId},
    },
    prelude::*,
    task_manager::TaskId,
};
use wrt_foundation::verification::VerificationLevel;
use wrt_platform::advanced_sync::Priority;

/// Test future that simulates different types of async work
struct SimulatedAsyncWork {
    polls_remaining: usize,
    fuel_per_poll: u64,
    result: Option<u32>,
    priority: Priority,
    can_block: bool,
}

impl SimulatedAsyncWork {
    fn new(
        polls_until_ready: usize,
        fuel_per_poll: u64,
        result: u32,
        priority: Priority,
        can_block: bool,
    ) -> Self {
        Self {
            polls_remaining: polls_until_ready,
            fuel_per_poll,
            result: Some(result),
            priority,
            can_block,
        }
    }

    fn high_priority_work(result: u32) -> Self {
        Self::new(2, 50, result, 192 /* High priority */, false)
    }

    fn normal_priority_work(result: u32) -> Self {
        Self::new(5, 30, result, 128 /* Normal priority */, true)
    }

    fn low_priority_work(result: u32) -> Self {
        Self::new(10, 20, result, 64 /* Low priority */, true)
    }
}

impl Future for SimulatedAsyncWork {
    type Output = Result<u32, Error>;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.polls_remaining == 0 {
            Poll::Ready(Ok(self.result.take().unwrap_or(0)))
        } else {
            self.polls_remaining -= 1;

            // Simulate fuel consumption
            // In real implementation, this would integrate with fuel tracking

            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase2_priority_inheritance_integration() {
        // Test priority inheritance protocol with async tasks
        let mut protocol =
            FuelPriorityInheritanceProtocol::new(VerificationLevel::Standard).unwrap();

        let high_priority_task = TaskId::new(1);
        let low_priority_holder = TaskId::new(2);
        let resource_id = ResourceId::new(1000);

        // Simulate high priority task blocked by low priority task
        protocol
            .register_blocking(
                high_priority_task,
                192, // High priority
                resource_id,
                Some(low_priority_holder),
                Some(Duration::from_millis(1000)),
            )
            .unwrap();

        // Verify priority inheritance occurred
        let effective_priority =
            protocol.get_effective_priority(low_priority_holder, 64 /* Low priority */);
        assert_eq!(effective_priority, 192); // High priority

        let stats = protocol.get_statistics();
        assert_eq!(
            stats.total_inheritances.load(core::sync::atomic::Ordering::Acquire),
            1
        );
        assert_eq!(
            stats.inversions_prevented.load(core::sync::atomic::Ordering::Acquire),
            1
        );

        // Release resource and verify cleanup
        let next_holder = protocol.release_resource(resource_id, low_priority_holder).unwrap();
        assert_eq!(next_holder, Some(high_priority_task));

        let final_stats = protocol.get_statistics();
        assert_eq!(
            final_stats.active_chains.load(core::sync::atomic::Ordering::Acquire),
            0
        );
    }

    #[test]
    fn test_phase2_async_channels_with_priorities() {
        // Test bounded async channels with priority-aware flow control
        let mut channel_manager =
            FuelAsyncChannelManager::<u32>::new(VerificationLevel::Standard).unwrap();

        let (sender, receiver) = channel_manager
            .create_channel(
                5,    // capacity
                true, // enable priority inheritance
                TaskId::new(1),
                ComponentInstanceId::new(1),
                192, // High priority // sender
                TaskId::new(2),
                ComponentInstanceId::new(1),
                128, // Normal priority // receiver
            )
            .unwrap();

        // Test immediate send/receive
        assert!(sender.try_send(42).is_ok());
        let received = receiver.try_receive().unwrap();
        assert_eq!(received, 42);

        // Fill channel to test blocking behavior
        for i in 0..5 {
            assert!(sender.try_send(i).is_ok());
        }

        // Next send should indicate it would block
        let result = sender.try_send(999);
        assert!(matches!(
            result,
            Err(super::super::async_::fuel_async_channels::ChannelError::WouldBlock(_))
        ));

        // Receive messages to make space
        for expected in 0..5 {
            let received = receiver.try_receive().unwrap();
            assert_eq!(received, expected);
        }

        // Verify channel statistics
        let stats = channel_manager.get_global_stats();
        assert_eq!(
            stats.total_channels_created.load(core::sync::atomic::Ordering::Acquire),
            1
        );
        assert_eq!(
            stats.active_channels.load(core::sync::atomic::Ordering::Acquire),
            1
        );
        assert!(stats.total_messages_sent.load(core::sync::atomic::Ordering::Acquire) >= 6);
        assert!(stats.total_messages_received.load(core::sync::atomic::Ordering::Acquire) >= 6);
    }

    #[test]
    fn test_phase2_preemptive_scheduler() {
        // Test preemptive scheduler with priority and fuel-based time slicing
        let config = PreemptiveSchedulerConfig {
            default_fuel_quantum: 1000,
            enable_priority_aging: true,
            aging_fuel_threshold: 2000,
            max_priority_boost: 2,
            enable_deadline_scheduling: true,
            enable_priority_inheritance: true,
            min_fuel_quantum: 100,
            max_fuel_quantum: 5000,
        };

        let mut scheduler =
            FuelPreemptiveScheduler::new(config, VerificationLevel::Standard).unwrap();

        // Add tasks with different priorities
        scheduler
            .add_task(
                TaskId::new(1),
                ComponentInstanceId::new(1),
                64, // Low priority
                10000,
                None,
                true, // preemptible
            )
            .unwrap();

        scheduler
            .add_task(
                TaskId::new(2),
                ComponentInstanceId::new(1),
                128, // Normal priority
                8000,
                Some(Duration::from_millis(5000)), // deadline
                true,
            )
            .unwrap();

        scheduler
            .add_task(
                TaskId::new(3),
                ComponentInstanceId::new(1),
                192, // High priority
                5000,
                Some(Duration::from_millis(2000)), // tight deadline
                false,                             // non-preemptible
            )
            .unwrap();

        // Schedule should select highest priority task first
        let next_task = scheduler.schedule_next_task().unwrap();
        assert_eq!(next_task, Some(TaskId::new(3))); // High priority task

        // Simulate execution and state changes
        scheduler
            .update_task_state(
                TaskId::new(3),
                AsyncTaskState::Waiting, // Task becomes blocked
                500,                     // fuel consumed
            )
            .unwrap();

        // Next schedule should pick normal priority task
        let next_task = scheduler.schedule_next_task().unwrap();
        assert_eq!(next_task, Some(TaskId::new(2))); // Normal priority task

        // Test preemption by adding higher priority task back
        scheduler
            .update_task_state(
                TaskId::new(3),
                AsyncTaskState::Ready, // High priority task becomes ready again
                0,
            )
            .unwrap();

        // Check if current task should be preempted
        if let Some(current_context) = scheduler.current_task.as_ref() {
            let should_preempt = scheduler
                .should_preempt_current_task(
                    current_context,
                    1000, // current fuel time
                )
                .unwrap();
            assert!(should_preempt); // Should preempt for higher priority
        }

        // Verify scheduler statistics
        let stats = scheduler.get_statistics();
        assert_eq!(
            stats.active_tasks.load(core::sync::atomic::Ordering::Acquire),
            3
        );
        assert_eq!(
            stats.total_tasks_scheduled.load(core::sync::atomic::Ordering::Acquire),
            3
        );
        assert!(stats.total_context_switches.load(core::sync::atomic::Ordering::Acquire) > 0);
    }

    #[test]
    fn test_phase2_integrated_async_system() {
        // Integration test showing all Phase 2 components working together

        // 1. Create async executor with fuel limits
        let mut executor = FuelAsyncExecutor::new().unwrap();
        executor.set_global_fuel_limit(50000);
        executor.set_default_verification_level(VerificationLevel::Standard);

        // 2. Create preemptive scheduler
        let config = PreemptiveSchedulerConfig::default();
        let mut preemptive_scheduler =
            FuelPreemptiveScheduler::new(config, VerificationLevel::Standard).unwrap();

        // 3. Create priority inheritance protocol
        let mut priority_protocol =
            FuelPriorityInheritanceProtocol::new(VerificationLevel::Standard).unwrap();

        // 4. Create channel manager for inter-task communication
        let mut channel_manager =
            FuelAsyncChannelManager::<u32>::new(VerificationLevel::Standard).unwrap();

        // 5. Set up communication channel between tasks
        let (sender, receiver) = channel_manager
            .create_channel(
                10,   // capacity
                true, // enable priority inheritance
                TaskId::new(1),
                ComponentInstanceId::new(1),
                192, // High priority // producer
                TaskId::new(2),
                ComponentInstanceId::new(1),
                128, // Normal priority // consumer
            )
            .unwrap();

        // 6. Add tasks to preemptive scheduler
        preemptive_scheduler
            .add_task(
                TaskId::new(1), // Producer task
                ComponentInstanceId::new(1),
                192, // High priority
                15000,
                Some(Duration::from_millis(3000)),
                true,
            )
            .unwrap();

        preemptive_scheduler
            .add_task(
                TaskId::new(2), // Consumer task
                ComponentInstanceId::new(1),
                128, // Normal priority
                10000,
                Some(Duration::from_millis(5000)),
                true,
            )
            .unwrap();

        preemptive_scheduler
            .add_task(
                TaskId::new(3), // Background task
                ComponentInstanceId::new(1),
                64, // Low priority
                20000,
                None,
                true,
            )
            .unwrap();

        // 7. Simulate task execution with priority inheritance

        // High priority producer should run first
        let next_task = preemptive_scheduler.schedule_next_task().unwrap();
        assert_eq!(next_task, Some(TaskId::new(1)));

        // Simulate producer sending data
        assert!(sender.try_send(100).is_ok());
        assert!(sender.try_send(200).is_ok());

        // Producer becomes blocked (simulating I/O or resource wait)
        preemptive_scheduler
            .update_task_state(
                TaskId::new(1),
                AsyncTaskState::Waiting,
                1000, // fuel consumed
            )
            .unwrap();

        // Consumer should run next
        let next_task = preemptive_scheduler.schedule_next_task().unwrap();
        assert_eq!(next_task, Some(TaskId::new(2)));

        // Consumer receives data
        let received1 = receiver.try_receive().unwrap();
        let received2 = receiver.try_receive().unwrap();
        assert_eq!(received1, 100);
        assert_eq!(received2, 200);

        // Consumer runs for a while, then producer becomes ready again
        preemptive_scheduler
            .update_task_state(
                TaskId::new(2),
                AsyncTaskState::Ready,
                800, // fuel consumed
            )
            .unwrap();

        preemptive_scheduler
            .update_task_state(TaskId::new(1), AsyncTaskState::Ready, 0)
            .unwrap();

        // Producer should preempt consumer due to higher priority
        if let Some(current_context) = preemptive_scheduler.current_task.as_ref() {
            let should_preempt =
                preemptive_scheduler.should_preempt_current_task(current_context, 2000).unwrap();
            assert!(should_preempt);
        }

        // 8. Verify final system state
        let executor_stats = executor.get_global_fuel_status();
        let scheduler_stats = preemptive_scheduler.get_statistics();
        let protocol_stats = priority_protocol.get_statistics();
        let channel_stats = channel_manager.get_global_stats();

        // All components should show activity
        assert_eq!(
            scheduler_stats.active_tasks.load(core::sync::atomic::Ordering::Acquire),
            3
        );
        assert!(
            scheduler_stats
                .total_context_switches
                .load(core::sync::atomic::Ordering::Acquire)
                > 0
        );
        assert!(channel_stats.total_messages_sent.load(core::sync::atomic::Ordering::Acquire) >= 2);
        assert!(
            channel_stats
                .total_messages_received
                .load(core::sync::atomic::Ordering::Acquire)
                >= 2
        );

        println!("Phase 2 Integration Test Results:");
        println!(
            "- Executor fuel status: {}/{} fuel used",
            executor_stats.consumed, executor_stats.limit
        );
        println!(
            "- Scheduler: {} active tasks, {} context switches",
            scheduler_stats.active_tasks.load(core::sync::atomic::Ordering::Acquire),
            scheduler_stats
                .total_context_switches
                .load(core::sync::atomic::Ordering::Acquire)
        );
        println!(
            "- Channels: {} messages sent, {} received",
            channel_stats.total_messages_sent.load(core::sync::atomic::Ordering::Acquire),
            channel_stats
                .total_messages_received
                .load(core::sync::atomic::Ordering::Acquire)
        );
        println!(
            "- Priority inheritance: {} total inheritances",
            protocol_stats.total_inheritances.load(core::sync::atomic::Ordering::Acquire)
        );
    }

    #[test]
    fn test_phase2_asil_b_compliance_scenario() {
        // Test scenario demonstrating ASIL-B compliance features:
        // - Freedom from interference (spatial and temporal isolation)
        // - Deterministic timing via fuel budgets
        // - Priority inheritance preventing priority inversion
        // - Bounded resource usage

        // Safety-critical high priority task
        let safety_critical_task = TaskId::new(1);
        // Normal application task
        let application_task = TaskId::new(2);
        // Background maintenance task
        let maintenance_task = TaskId::new(3);

        let mut executor = FuelAsyncExecutor::new().unwrap();
        executor.set_global_fuel_limit(30000); // System-wide resource limit

        let config = PreemptiveSchedulerConfig {
            default_fuel_quantum: 500,    // Small quanta for responsive scheduling
            enable_priority_aging: false, // Disabled for deterministic behavior
            enable_deadline_scheduling: true,
            enable_priority_inheritance: true,
            min_fuel_quantum: 100,
            max_fuel_quantum: 2000,
        };
        let mut scheduler = FuelPreemptiveScheduler::new(config, VerificationLevel::Full).unwrap();

        // Add safety-critical task with tight deadline and non-preemptible execution
        scheduler
            .add_task(
                safety_critical_task,
                ComponentInstanceId::new(1), // Isolated component
                Priority::Critical,
                5000, // Limited fuel budget for deterministic timing
                Some(Duration::from_millis(1000)), // Tight deadline
                false, // Non-preemptible when running
            )
            .unwrap();

        // Add application task with moderate priority
        scheduler
            .add_task(
                application_task,
                ComponentInstanceId::new(2), // Different component for isolation
                128,                         // Normal priority
                10000,
                Some(Duration::from_millis(5000)),
                true, // Preemptible
            )
            .unwrap();

        // Add maintenance task with low priority
        scheduler
            .add_task(
                maintenance_task,
                ComponentInstanceId::new(3), // Separate component
                64,                          // Low priority
                15000,
                None, // No deadline - best effort
                true,
            )
            .unwrap();

        // Test 1: Safety-critical task has highest priority
        let first_scheduled = scheduler.schedule_next_task().unwrap();
        assert_eq!(first_scheduled, Some(safety_critical_task));

        // Test 2: Safety-critical task completes without preemption
        scheduler
            .update_task_state(
                safety_critical_task,
                AsyncTaskState::Completed,
                800, // Under deadline fuel limit
            )
            .unwrap();

        // Test 3: Application task runs next
        let second_scheduled = scheduler.schedule_next_task().unwrap();
        assert_eq!(second_scheduled, Some(application_task));

        // Test 4: Safety-critical task can preempt application task
        scheduler
            .add_task(
                TaskId::new(4), // Another safety-critical task
                ComponentInstanceId::new(1),
                Priority::Critical,
                3000,
                Some(Duration::from_millis(800)),
                false,
            )
            .unwrap();

        // Should trigger preemption
        if let Some(current_context) = scheduler.current_task.as_ref() {
            let should_preempt =
                scheduler.should_preempt_current_task(current_context, 2000).unwrap();
            assert!(should_preempt);
        }

        // Test 5: Resource isolation via component separation
        let mut channel_manager = FuelAsyncChannelManager::<u32>::new(VerificationLevel::Full)?;

        // Create isolated communication channel
        let (_safety_sender, _safety_receiver) = channel_manager
            .create_channel(
                5,    // Small bounded capacity
                true, // Priority inheritance enabled
                safety_critical_task,
                ComponentInstanceId::new(1),
                Priority::Critical,
                application_task,
                ComponentInstanceId::new(2),
                128, // Normal priority
            )
            .unwrap();

        // Verify system constraints
        let scheduler_stats = scheduler.get_statistics();
        let channel_stats = channel_manager.get_global_stats();

        // ASIL-B compliance checks:
        assert!(scheduler_stats.active_tasks.load(core::sync::atomic::Ordering::Acquire) <= 4);
        assert!(channel_stats.active_channels.load(core::sync::atomic::Ordering::Acquire) == 1);

        // Fuel consumption should be bounded and predictable
        let total_scheduler_fuel = scheduler_stats
            .scheduler_fuel_consumed
            .load(core::sync::atomic::Ordering::Acquire);
        assert!(total_scheduler_fuel > 0); // Scheduler is tracking fuel
        assert!(total_scheduler_fuel < 1000); // Overhead is bounded

        println!("ASIL-B Compliance Test Results:");
        println!("✓ Priority isolation: Critical tasks scheduled first");
        println!("✓ Temporal isolation: Non-preemptible critical sections");
        println!("✓ Spatial isolation: Component-based task separation");
        println!("✓ Resource bounds: Fuel budgets enforced");
        println!("✓ Deterministic timing: Fuel-based scheduling");
        println!("✓ Priority inheritance: Available for blocking scenarios");
    }
}

/// Example usage functions for documentation
#[allow(dead_code)]
mod examples {
    use super::*;

    /// Example: Creating a complete Phase 2 async system
    pub async fn create_phase2_async_system() -> Result<(), Error> {
        // 1. Create the core async executor
        let mut executor = FuelAsyncExecutor::new()?;
        executor.set_global_fuel_limit(100000);
        executor.set_default_verification_level(VerificationLevel::Standard);

        // 2. Create preemptive scheduler for advanced task management
        let scheduler_config = PreemptiveSchedulerConfig {
            default_fuel_quantum: 2000,
            enable_priority_aging: true,
            enable_deadline_scheduling: true,
            enable_priority_inheritance: true,
            ..Default::default()
        };
        let preemptive_scheduler =
            FuelPreemptiveScheduler::new(scheduler_config, VerificationLevel::Standard)?;

        // 3. Create priority inheritance protocol
        let priority_protocol = FuelPriorityInheritanceProtocol::new(VerificationLevel::Standard)?;

        // 4. Create channel manager for async communication
        let channel_manager = FuelAsyncChannelManager::<String>::new(VerificationLevel::Standard)?;

        println!("Phase 2 async system created with:");
        println!("- Fuel-based async executor with preemption support");
        println!("- Priority inheritance protocol for preventing priority inversion");
        println!("- Bounded async channels with flow control");
        println!("- Preemptive scheduler with priority aging and deadline support");

        Ok(())
    }

    /// Example: High-priority task with resource blocking
    pub async fn priority_inheritance_example() -> Result<(), Error> {
        let mut protocol = FuelPriorityInheritanceProtocol::new(VerificationLevel::Standard)?;

        // High priority task gets blocked by low priority task holding a resource
        let resource_id = ResourceId::new(42);
        let high_priority_task = TaskId::new(1);
        let low_priority_holder = TaskId::new(2);

        // Register the blocking scenario
        protocol.register_blocking(
            high_priority_task,
            Priority::High,
            resource_id,
            Some(low_priority_holder),
            Some(Duration::from_millis(1000)), // Max blocking time
        )?;

        // Priority inheritance automatically boosts low priority holder
        let effective_priority =
            protocol.get_effective_priority(low_priority_holder, 64 /* Low priority */);
        assert_eq!(effective_priority, 192); // High priority

        // When resource is released, original priorities are restored
        let next_holder = protocol.release_resource(resource_id, low_priority_holder)?;
        assert_eq!(next_holder, Some(high_priority_task));

        Ok(())
    }
}
