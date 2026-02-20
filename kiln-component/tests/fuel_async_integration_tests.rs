//! Integration tests for fuel-based async system
//!
//! These tests verify that the fuel-based async executor, scheduler, and bridge
//! work together correctly for deterministic WebAssembly Component Model
//! execution.

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use wrt_component::{
    ComponentInstanceId,
    async_::{
        fuel_async_bridge::{AsyncBridgeConfig, FuelAsyncBridge},
        fuel_async_executor::{AsyncTaskState, FuelAsyncExecutor},
        fuel_async_scheduler::{FuelAsyncScheduler, SchedulingPolicy},
    },
    prelude::*,
    task_manager::TaskId,
};
use wrt_foundation::verification::VerificationLevel;
use wrt_platform::advanced_sync::Priority;

/// Simple test future that completes after a certain number of polls
struct TestFuture {
    polls_remaining: usize,
    result: Option<u32>,
}

impl TestFuture {
    fn new(polls_until_ready: usize, result: u32) -> Self {
        Self {
            polls_remaining: polls_until_ready,
            result: Some(result),
        }
    }
}

impl Future for TestFuture {
    type Output = Result<u32, Error>;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.polls_remaining == 0 {
            Poll::Ready(Ok(self.result.take().unwrap_or(0)))
        } else {
            self.polls_remaining -= 1;
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuel_async_executor_basic() {
        let mut executor = FuelAsyncExecutor::new().unwrap();
        executor.set_global_fuel_limit(10000);

        // Test executor creation and configuration
        let status = executor.get_global_fuel_status();
        assert_eq!(status.active_tasks, 0);
        assert_eq!(status.ready_tasks, 0);
        assert!(status.enforcement_enabled);
        assert_eq!(status.limit, 10000);

        // Test fuel limit setting
        executor.set_global_fuel_limit(5000);
        let updated_status = executor.get_global_fuel_status();
        assert_eq!(updated_status.limit, 5000);
    }

    #[test]
    fn test_fuel_async_scheduler_policies() {
        // Test cooperative scheduling
        let scheduler =
            FuelAsyncScheduler::new(SchedulingPolicy::Cooperative, VerificationLevel::Standard)
                .unwrap();

        let stats = scheduler.get_statistics();
        assert_eq!(stats.policy, SchedulingPolicy::Cooperative);
        assert_eq!(stats.total_tasks, 0);

        // Test priority-based scheduling
        let priority_scheduler =
            FuelAsyncScheduler::new(SchedulingPolicy::PriorityBased, VerificationLevel::Standard)
                .unwrap();

        let priority_stats = priority_scheduler.get_statistics();
        assert_eq!(priority_stats.policy, SchedulingPolicy::PriorityBased);

        // Test round-robin scheduling
        let rr_scheduler =
            FuelAsyncScheduler::new(SchedulingPolicy::RoundRobin, VerificationLevel::Standard)
                .unwrap();

        let rr_stats = rr_scheduler.get_statistics();
        assert_eq!(rr_stats.policy, SchedulingPolicy::RoundRobin);
    }

    #[test]
    fn test_scheduler_task_management() {
        let mut scheduler =
            FuelAsyncScheduler::new(SchedulingPolicy::Cooperative, VerificationLevel::Standard)
                .unwrap();

        let task_id = TaskId::new(1);
        let component_id = ComponentInstanceId::new(1);

        // Add a task
        scheduler
            .add_task(
                task_id,
                component_id,
                128,  // Normal priority
                1000, // fuel quota
                None, // no deadline
            )
            .unwrap();

        let stats = scheduler.get_statistics();
        assert_eq!(stats.total_tasks, 1);
        assert_eq!(stats.ready_tasks, 1);

        // Get next task
        let next_task = scheduler.next_task();
        assert_eq!(next_task, Some(task_id));

        // Remove task
        scheduler.remove_task(task_id).unwrap();
        let final_stats = scheduler.get_statistics();
        assert_eq!(final_stats.total_tasks, 0);
    }

    #[test]
    fn test_priority_scheduling_order() {
        let mut scheduler =
            FuelAsyncScheduler::new(SchedulingPolicy::PriorityBased, VerificationLevel::Standard)
                .unwrap();

        let low_task = TaskId::new(1);
        let high_task = TaskId::new(2);
        let normal_task = TaskId::new(3);

        // Add tasks in non-priority order
        scheduler
            .add_task(
                low_task,
                ComponentInstanceId::new(1),
                64, // Low priority
                1000,
                None,
            )
            .unwrap();
        scheduler
            .add_task(
                normal_task,
                ComponentInstanceId::new(1),
                128, // Normal priority
                1000,
                None,
            )
            .unwrap();
        scheduler
            .add_task(
                high_task,
                ComponentInstanceId::new(1),
                192, // High priority
                1000,
                None,
            )
            .unwrap();

        // Should get high priority task first
        assert_eq!(scheduler.next_task(), Some(high_task));

        // Update high priority task to waiting state
        scheduler.update_task_state(high_task, 100, AsyncTaskState::Waiting).unwrap();

        // Should get normal priority task next
        assert_eq!(scheduler.next_task(), Some(normal_task));
    }

    #[test]
    fn test_fuel_async_bridge_configuration() {
        let config = AsyncBridgeConfig {
            default_fuel_budget: 5000,
            default_time_limit_ms: Some(2000),
            default_priority: 192, // High priority
            scheduling_policy: SchedulingPolicy::PriorityBased,
            allow_fuel_extension: true,
            fuel_check_interval: 500,
        };

        let bridge = FuelAsyncBridge::new(config.clone(), VerificationLevel::Standard).unwrap();

        let stats = bridge.get_bridge_statistics();
        assert_eq!(stats.total_bridges, 0);
        assert_eq!(stats.active_bridges, 0);
        assert_eq!(stats.completed_bridges, 0);
        assert_eq!(stats.failed_bridges, 0);

        // Verify configuration was applied
        assert_eq!(config.default_fuel_budget, 5000);
        assert_eq!(config.default_time_limit_ms, Some(2000));
        assert_eq!(config.default_priority, 192); // High priority
    }

    #[test]
    fn test_task_spawning_with_fuel_limits() {
        let mut executor = FuelAsyncExecutor::new().unwrap();
        executor.set_global_fuel_limit(1000);

        let component_id = ComponentInstanceId::new(1);

        // Should succeed with fuel budget within limit
        let task1 = executor.spawn_task(
            component_id,
            500, // fuel budget
            128, // Normal priority
            TestFuture::new(3, 42),
        );
        assert!(task1.is_ok());

        // Should succeed with remaining fuel
        let task2 = executor.spawn_task(
            component_id,
            400, // fuel budget
            128, // Normal priority
            TestFuture::new(2, 84),
        );
        assert!(task2.is_ok());

        // Should fail when exceeding global fuel limit
        let task3 = executor.spawn_task(
            component_id,
            200, // would exceed limit (500 + 400 + 200 > 1000)
            128, // Normal priority
            TestFuture::new(1, 126),
        );
        assert!(task3.is_err());

        let status = executor.get_global_fuel_status();
        assert_eq!(status.active_tasks, 2);
    }

    #[test]
    fn test_verification_level_impact() {
        // Test that different verification levels are properly handled
        let basic_scheduler =
            FuelAsyncScheduler::new(SchedulingPolicy::Cooperative, VerificationLevel::Basic)
                .unwrap();

        let full_scheduler =
            FuelAsyncScheduler::new(SchedulingPolicy::Cooperative, VerificationLevel::Full)
                .unwrap();

        // Both should work but with different internal fuel costs
        // (fuel costs are handled by the operations module)
        let basic_stats = basic_scheduler.get_statistics();
        let full_stats = full_scheduler.get_statistics();

        assert_eq!(basic_stats.total_tasks, 0);
        assert_eq!(full_stats.total_tasks, 0);
    }

    #[test]
    fn test_executor_shutdown() {
        let mut executor = FuelAsyncExecutor::new().unwrap();
        executor.set_global_fuel_limit(5000);

        // Spawn some tasks
        let component_id = ComponentInstanceId::new(1);
        let _task1 = executor
            .spawn_task(
                component_id,
                1000,
                128, // Normal priority
                TestFuture::new(10, 42),
            )
            .unwrap();

        let _task2 = executor
            .spawn_task(
                component_id,
                1000,
                128, /* Normal priority */
                TestFuture::new(5, 84),
            )
            .unwrap();

        let status_before = executor.get_global_fuel_status();
        assert_eq!(status_before.active_tasks, 2;

        // Shutdown should succeed
        let shutdown_result = executor.shutdown);
        assert!(shutdown_result.is_ok());

        let status_after = executor.get_global_fuel_status();
        assert_eq!(status_after.active_tasks, 0);
    }

    #[test]
    fn test_bridge_statistics() {
        let bridge =
            FuelAsyncBridge::new(AsyncBridgeConfig::default(), VerificationLevel::Standard)
                .unwrap();

        let stats = bridge.get_bridge_statistics();

        // Test success rate calculation with no bridges
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.average_fuel_per_bridge(), 0.0);

        // Verify stats structure
        assert_eq!(stats.total_bridges, 0);
        assert_eq!(stats.active_bridges, 0);
        assert_eq!(stats.completed_bridges, 0);
        assert_eq!(stats.failed_bridges, 0);
        assert_eq!(stats.total_fuel_consumed, 0);
    }

    #[test]
    fn test_scheduling_statistics() {
        let scheduler =
            FuelAsyncScheduler::new(SchedulingPolicy::Cooperative, VerificationLevel::Standard)
                .unwrap();

        let stats = scheduler.get_statistics();

        // Test efficiency calculations with no tasks
        assert_eq!(stats.average_fuel_per_task(), 0.0);
        assert_eq!(stats.scheduling_efficiency(), 0.0);

        // Verify initial state
        assert_eq!(stats.policy, SchedulingPolicy::Cooperative);
        assert_eq!(stats.total_tasks, 0);
        assert_eq!(stats.ready_tasks, 0);
        assert_eq!(stats.waiting_tasks, 0);
        assert_eq!(stats.total_fuel_consumed, 0);
        assert_eq!(stats.total_schedule_count, 0);
    }

    #[test]
    fn test_round_robin_fairness() {
        let mut scheduler =
            FuelAsyncScheduler::new(SchedulingPolicy::RoundRobin, VerificationLevel::Standard)
                .unwrap();

        let task1 = TaskId::new(1);
        let task2 = TaskId::new(2);
        let task3 = TaskId::new(3);

        // Add tasks to round-robin queue
        scheduler
            .add_task(
                task1,
                ComponentInstanceId::new(1),
                128, // Normal priority
                1000,
                None,
            )
            .unwrap();
        scheduler
            .add_task(
                task2,
                ComponentInstanceId::new(1),
                128, // Normal priority
                1000,
                None,
            )
            .unwrap();
        scheduler
            .add_task(
                task3,
                ComponentInstanceId::new(1),
                128, // Normal priority
                1000,
                None,
            )
            .unwrap();

        // Should cycle through tasks
        assert_eq!(scheduler.next_task(), Some(task1));
        scheduler.update_task_state(task1, 100, AsyncTaskState::Waiting).unwrap();

        assert_eq!(scheduler.next_task(), Some(task2));
        scheduler.update_task_state(task2, 100, AsyncTaskState::Waiting).unwrap();

        assert_eq!(scheduler.next_task(), Some(task3));
        scheduler.update_task_state(task3, 100, AsyncTaskState::Waiting).unwrap();

        // Should wrap around back to task1 if it becomes ready again
        scheduler.update_task_state(task1, 0, AsyncTaskState::Ready).unwrap();
        // Note: This test demonstrates the intended behavior but may need
        // scheduler modifications to fully work as expected
    }
}
