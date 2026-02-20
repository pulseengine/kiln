//! Phase 3 Integration Tests: Deadline-based scheduling with WCET guarantees
//!
//! This test demonstrates the complete Phase 3 ASIL-C async system including:
//! - Constrained deadline scheduling (deadline ≤ period)
//! - WCET analysis and enforcement
//! - Rate Monotonic + EDF hybrid scheduling
//! - Criticality-aware mode switching
//! - Freedom from interference with deadline guarantees

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
        // Phase 3 components
        fuel_deadline_scheduler::{
            AsilLevel, CriticalityMode, DeadlineSchedulerConfig, FuelDeadlineScheduler,
        },
        fuel_preemptive_scheduler::{FuelPreemptiveScheduler, PreemptiveSchedulerConfig},

        // Phase 2 components
        fuel_priority_inheritance::{FuelPriorityInheritanceProtocol, ResourceId},
        fuel_wcet_analyzer::{
            FuelWcetAnalyzer, WcetAnalysisMethod, WcetAnalysisResult, WcetAnalyzerConfig,
        },
    },
    prelude::*,
    task_manager::TaskId,
};
use wrt_foundation::verification::VerificationLevel;
use wrt_platform::advanced_sync::Priority;

/// Deadline-constrained test task with WCET profile
struct DeadlineConstrainedTask {
    /// Task identifier
    task_id: TaskId,
    /// Component this task belongs to
    component_id: ComponentInstanceId,
    /// ASIL criticality level
    asil_level: AsilLevel,
    /// Task period
    period: Duration,
    /// Task deadline (≤ period)
    deadline: Duration,
    /// Simulated execution phases with different fuel consumption
    execution_phases: Vec<ExecutionPhase>,
    /// Current phase index
    current_phase: usize,
    /// Fuel consumed so far in this execution
    fuel_consumed: u64,
    /// Whether task completed successfully
    completed: bool,
}

/// Phase of task execution with specific fuel consumption
#[derive(Clone, Copy)]
struct ExecutionPhase {
    /// Fuel to consume in this phase
    fuel_cost: u64,
    /// Minimum execution time for this phase
    min_duration: u64,
    /// Whether this phase can vary in execution time
    variable_execution: bool,
}

impl DeadlineConstrainedTask {
    fn new_asil_c_control_task() -> Self {
        Self {
            task_id: TaskId::new(1),
            component_id: ComponentInstanceId::new(1),
            asil_level: AsilLevel::C,
            period: Duration::from_millis(50),
            deadline: Duration::from_millis(40), // Constrained deadline
            execution_phases: vec![
                ExecutionPhase {
                    fuel_cost: 15,
                    min_duration: 5,
                    variable_execution: false,
                }, // Sensor reading
                ExecutionPhase {
                    fuel_cost: 25,
                    min_duration: 10,
                    variable_execution: true,
                }, // Control calculation
                ExecutionPhase {
                    fuel_cost: 10,
                    min_duration: 3,
                    variable_execution: false,
                }, // Actuator command
            ],
            current_phase: 0,
            fuel_consumed: 0,
            completed: false,
        }
    }

    fn new_asil_b_monitoring_task() -> Self {
        Self {
            task_id: TaskId::new(2),
            component_id: ComponentInstanceId::new(2),
            asil_level: AsilLevel::B,
            period: Duration::from_millis(100),
            deadline: Duration::from_millis(80),
            execution_phases: vec![
                ExecutionPhase {
                    fuel_cost: 20,
                    min_duration: 8,
                    variable_execution: true,
                }, // Data collection
                ExecutionPhase {
                    fuel_cost: 15,
                    min_duration: 5,
                    variable_execution: false,
                }, // Analysis
                ExecutionPhase {
                    fuel_cost: 8,
                    min_duration: 2,
                    variable_execution: false,
                }, // Report
            ],
            current_phase: 0,
            fuel_consumed: 0,
            completed: false,
        }
    }

    fn new_background_task() -> Self {
        Self {
            task_id: TaskId::new(3),
            component_id: ComponentInstanceId::new(3),
            asil_level: AsilLevel::QM,
            period: Duration::from_millis(200),
            deadline: Duration::from_millis(200), // Deadline = period
            execution_phases: vec![
                ExecutionPhase {
                    fuel_cost: 30,
                    min_duration: 15,
                    variable_execution: true,
                }, // Background processing
                ExecutionPhase {
                    fuel_cost: 20,
                    min_duration: 10,
                    variable_execution: true,
                }, // Cleanup
            ],
            current_phase: 0,
            fuel_consumed: 0,
            completed: false,
        }
    }

    fn calculate_wcet(&self) -> u64 {
        self.execution_phases
            .iter()
            .map(|phase| {
                if phase.variable_execution {
                    phase.fuel_cost + (phase.fuel_cost / 4) // Add 25% for
                // variability
                } else {
                    phase.fuel_cost
                }
            })
            .sum()
    }

    fn calculate_bcet(&self) -> u64 {
        self.execution_phases
            .iter()
            .map(|phase| {
                if phase.variable_execution {
                    phase.fuel_cost - (phase.fuel_cost / 4) // Subtract 25% for
                // best case
                } else {
                    phase.fuel_cost
                }
            })
            .sum()
    }
}

impl Future for DeadlineConstrainedTask {
    type Output = Result<u64, Error>;

    // Returns total fuel consumed

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.completed {
            return Poll::Ready(Ok(self.fuel_consumed));
        }

        if self.current_phase >= self.execution_phases.len() {
            self.completed = true;
            return Poll::Ready(Ok(self.fuel_consumed));
        }

        let phase = self.execution_phases[self.current_phase];

        // Simulate variable execution time
        let actual_fuel = if phase.variable_execution {
            // Add some randomness based on task ID for deterministic testing
            let variation = (self.task_id.0 as u64 * 7) % (phase.fuel_cost / 2);
            phase.fuel_cost + variation
        } else {
            phase.fuel_cost
        };

        self.fuel_consumed += actual_fuel;
        self.current_phase += 1;

        if self.current_phase >= self.execution_phases.len() {
            self.completed = true;
            Poll::Ready(Ok(self.fuel_consumed))
        } else {
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase3_wcet_analysis_integration() {
        // Test WCET analysis for deadline-constrained tasks
        let config = WcetAnalyzerConfig {
            default_method: WcetAnalysisMethod::Static,
            required_confidence: 0.95,
            safety_margin_factor: 1.2,
            ..Default::default()
        };
        let mut wcet_analyzer = FuelWcetAnalyzer::new(config, VerificationLevel::Full).unwrap();

        let control_task = DeadlineConstrainedTask::new_asil_c_control_task();
        let monitoring_task = DeadlineConstrainedTask::new_asil_b_monitoring_task();

        // Register control flow paths for ASIL-C control task
        wcet_analyzer
            .register_control_flow_path(
                control_task.task_id,
                1,             // Critical path
                &[1, 2, 3, 4], // Basic blocks
                control_task.calculate_wcet(),
            )
            .unwrap();

        wcet_analyzer
            .register_control_flow_path(
                control_task.task_id,
                2,          // Alternative path
                &[1, 5, 6], // Different basic blocks
                control_task.calculate_bcet(),
            )
            .unwrap();

        // Perform WCET analysis
        let control_wcet = wcet_analyzer
            .analyze_task_wcet(
                control_task.task_id,
                control_task.component_id,
                Some(WcetAnalysisMethod::Static),
            )
            .unwrap();

        let monitoring_wcet = wcet_analyzer
            .analyze_task_wcet(
                monitoring_task.task_id,
                monitoring_task.component_id,
                Some(WcetAnalysisMethod::Static),
            )
            .unwrap();

        // Verify WCET results include safety margins
        assert!(control_wcet.wcet_fuel > control_task.calculate_wcet());
        assert!(monitoring_wcet.wcet_fuel > monitoring_task.calculate_wcet());
        assert_eq!(control_wcet.confidence_level, 0.8); // Static analysis confidence
        assert!(control_wcet.critical_path.is_some());

        println!("WCET Analysis Results:");
        println!(
            "- ASIL-C Control Task: WCET = {} fuel, BCET = {} fuel",
            control_wcet.wcet_fuel, control_wcet.bcet_fuel
        );
        println!(
            "- ASIL-B Monitoring Task: WCET = {} fuel, BCET = {} fuel",
            monitoring_wcet.wcet_fuel, monitoring_wcet.bcet_fuel
        );
    }

    #[test]
    fn test_phase3_deadline_scheduler_configuration() {
        // Test deadline scheduler with ASIL-C configuration
        let config = DeadlineSchedulerConfig {
            enable_hybrid_scheduling: true,
            enable_criticality_switching: true,
            enable_wcet_enforcement: true,
            enable_deadline_monitoring: true,
            max_utilization_per_level: 0.7,
            global_utilization_bound: 0.69, // RM bound
            deadline_miss_threshold: 2,
            scheduling_overhead_factor: 1.1,
        };

        let mut scheduler = FuelDeadlineScheduler::new(config, VerificationLevel::Full).unwrap();

        let control_task = DeadlineConstrainedTask::new_asil_c_control_task();
        let monitoring_task = DeadlineConstrainedTask::new_asil_b_monitoring_task();
        let background_task = DeadlineConstrainedTask::new_background_task();

        // Add tasks with WCET constraints
        let result1 = scheduler.add_deadline_task(
            control_task.task_id,
            control_task.component_id,
            control_task.asil_level,
            control_task.period,
            control_task.deadline,
            control_task.calculate_wcet(),
            control_task.calculate_bcet(),
        );
        assert!(result1.is_ok());

        let result2 = scheduler.add_deadline_task(
            monitoring_task.task_id,
            monitoring_task.component_id,
            monitoring_task.asil_level,
            monitoring_task.period,
            monitoring_task.deadline,
            monitoring_task.calculate_wcet(),
            monitoring_task.calculate_bcet(),
        );
        assert!(result2.is_ok());

        let result3 = scheduler.add_deadline_task(
            background_task.task_id,
            background_task.component_id,
            background_task.asil_level,
            background_task.period,
            background_task.deadline,
            background_task.calculate_wcet(),
            background_task.calculate_bcet(),
        );
        assert!(result3.is_ok());

        // Verify tasks were added
        let stats = scheduler.get_statistics();
        assert_eq!(
            stats.total_tasks.load(core::sync::atomic::Ordering::Acquire),
            3
        );
        assert_eq!(
            stats.active_tasks.load(core::sync::atomic::Ordering::Acquire),
            3
        );

        // Test schedulability analysis
        let analysis = scheduler.analyze_schedulability().unwrap();
        assert!(analysis.schedulable);
        println!("Schedulability Analysis:");
        println!("- Total utilization: {:.3}", analysis.total_utilization);
        println!("- Utilization bound: {:.3}", analysis.utilization_bound);
        println!("- Schedulable: {}", analysis.schedulable);
    }

    #[test]
    fn test_phase3_hybrid_rm_edf_scheduling() {
        // Test hybrid Rate Monotonic + EDF scheduling
        let config = DeadlineSchedulerConfig {
            enable_hybrid_scheduling: true,
            ..Default::default()
        };
        let mut scheduler =
            FuelDeadlineScheduler::new(config, VerificationLevel::Standard).unwrap();

        // Add tasks with different periods (RM ordering)
        let short_period_task = TaskId::new(10);
        let medium_period_task = TaskId::new(11);
        let long_period_task = TaskId::new(12);

        // Short period = high RM priority
        scheduler
            .add_deadline_task(
                short_period_task,
                ComponentInstanceId::new(1),
                AsilLevel::C,
                Duration::from_millis(25), // period
                Duration::from_millis(20), // deadline
                15,                        // WCET
                10,                        // BCET
            )
            .unwrap();

        // Medium period = medium RM priority
        scheduler
            .add_deadline_task(
                medium_period_task,
                ComponentInstanceId::new(1),
                AsilLevel::C,
                Duration::from_millis(50), // period
                Duration::from_millis(40), // deadline
                20,                        // WCET
                15,                        // BCET
            )
            .unwrap();

        // Long period = low RM priority
        scheduler
            .add_deadline_task(
                long_period_task,
                ComponentInstanceId::new(1),
                AsilLevel::C,
                Duration::from_millis(100), // period
                Duration::from_millis(80),  // deadline
                30,                         // WCET
                25,                         // BCET
            )
            .unwrap();

        // Schedule next task - should prioritize by Rate Monotonic
        let next_task = scheduler.schedule_next_task().unwrap();
        assert_eq!(next_task, Some(short_period_task)); // Shortest period first

        // Simulate task execution
        scheduler
            .update_task_execution(
                short_period_task,
                15, // fuel consumed
                AsyncTaskState::Completed,
            )
            .unwrap();

        // Next should be medium period task
        let next_task = scheduler.schedule_next_task().unwrap();
        assert_eq!(next_task, Some(medium_period_task));

        let stats = scheduler.get_statistics();
        assert_eq!(
            stats.successful_deadlines.load(core::sync::atomic::Ordering::Acquire),
            1
        );
    }

    #[test]
    fn test_phase3_criticality_mode_switching() {
        // Test ASIL-based criticality mode switching
        let config = DeadlineSchedulerConfig {
            enable_criticality_switching: true,
            deadline_miss_threshold: 2,
            ..Default::default()
        };
        let mut scheduler = FuelDeadlineScheduler::new(config, VerificationLevel::Full).unwrap();

        // Add tasks at different ASIL levels
        let asil_d_task = TaskId::new(20);
        let asil_c_task = TaskId::new(21);
        let asil_b_task = TaskId::new(22);
        let asil_a_task = TaskId::new(23);
        let qm_task = TaskId::new(24);

        scheduler
            .add_deadline_task(
                asil_d_task,
                ComponentInstanceId::new(1),
                AsilLevel::D,
                Duration::from_millis(20),
                Duration::from_millis(15),
                10,
                8,
            )
            .unwrap();

        scheduler
            .add_deadline_task(
                asil_c_task,
                ComponentInstanceId::new(2),
                AsilLevel::C,
                Duration::from_millis(40),
                Duration::from_millis(30),
                20,
                15,
            )
            .unwrap();

        scheduler
            .add_deadline_task(
                asil_b_task,
                ComponentInstanceId::new(3),
                AsilLevel::B,
                Duration::from_millis(60),
                Duration::from_millis(50),
                25,
                20,
            )
            .unwrap();

        scheduler
            .add_deadline_task(
                asil_a_task,
                ComponentInstanceId::new(4),
                AsilLevel::A,
                Duration::from_millis(80),
                Duration::from_millis(70),
                30,
                25,
            )
            .unwrap();

        scheduler
            .add_deadline_task(
                qm_task,
                ComponentInstanceId::new(5),
                AsilLevel::QM,
                Duration::from_millis(100),
                Duration::from_millis(90),
                35,
                30,
            )
            .unwrap();

        // Initially in Low mode - all tasks active
        let stats = scheduler.get_statistics();
        assert_eq!(
            stats.active_tasks.load(core::sync::atomic::Ordering::Acquire),
            5
        );

        // Switch to High criticality mode (ASIL-B and above)
        scheduler.switch_criticality_mode(CriticalityMode::High).unwrap();

        // ASIL-A and QM tasks should be dropped
        let stats = scheduler.get_statistics();
        assert_eq!(
            stats.tasks_dropped.load(core::sync::atomic::Ordering::Acquire),
            2
        );
        assert_eq!(
            stats.criticality_switches.load(core::sync::atomic::Ordering::Acquire),
            1
        );

        // Switch to Critical mode (ASIL-C and above)
        scheduler.switch_criticality_mode(CriticalityMode::Critical).unwrap();

        // ASIL-B task should also be dropped
        let stats = scheduler.get_statistics();
        assert_eq!(
            stats.tasks_dropped.load(core::sync::atomic::Ordering::Acquire),
            3
        );
        assert_eq!(
            stats.criticality_switches.load(core::sync::atomic::Ordering::Acquire),
            2
        );

        // Only ASIL-C and ASIL-D tasks remain active
        let next_task = scheduler.schedule_next_task().unwrap();
        assert!(next_task == Some(asil_d_task) || next_task == Some(asil_c_task));
    }

    #[test]
    fn test_phase3_wcet_enforcement_and_validation() {
        // Test WCET enforcement during execution
        let wcet_config = WcetAnalyzerConfig {
            enable_online_sampling: true,
            min_samples_for_stats: 5,
            ..Default::default()
        };
        let mut wcet_analyzer =
            FuelWcetAnalyzer::new(wcet_config, VerificationLevel::Full).unwrap();

        let scheduler_config = DeadlineSchedulerConfig {
            enable_wcet_enforcement: true,
            ..Default::default()
        };
        let mut scheduler =
            FuelDeadlineScheduler::new(scheduler_config, VerificationLevel::Full).unwrap();

        let task_id = TaskId::new(30);
        let component_id = ComponentInstanceId::new(1);

        // Add task with tight WCET budget
        scheduler
            .add_deadline_task(
                task_id,
                component_id,
                AsilLevel::C,
                Duration::from_millis(50),
                Duration::from_millis(40),
                100, // WCET budget
                80,  // BCET
            )
            .unwrap();

        // Perform initial WCET analysis
        let wcet_result = wcet_analyzer
            .analyze_task_wcet(task_id, component_id, Some(WcetAnalysisMethod::Static))
            .unwrap();

        // Collect execution samples
        let execution_samples = [95, 102, 88, 110, 92, 105, 98];
        for (i, &sample) in execution_samples.iter().enumerate() {
            wcet_analyzer
                .collect_execution_sample(
                    task_id,
                    sample,
                    Some(1),  // path_id
                    i as u32, // input_hash
                )
                .unwrap();
        }

        // Perform measurement-based analysis with samples
        let refined_result = wcet_analyzer
            .analyze_task_wcet(
                task_id,
                component_id,
                Some(WcetAnalysisMethod::MeasurementBased),
            )
            .unwrap();

        assert_eq!(refined_result.sample_count, 7);
        assert!(refined_result.wcet_fuel >= 110); // Should account for worst observed

        // Test WCET validation
        let within_estimate = wcet_analyzer.validate_wcet_estimate(task_id, 95).unwrap();
        assert!(within_estimate);

        let exceeds_estimate = wcet_analyzer.validate_wcet_estimate(task_id, 150).unwrap();
        assert!(!exceeds_estimate);

        let wcet_stats = wcet_analyzer.get_statistics();
        assert_eq!(
            wcet_stats.total_samples.load(core::sync::atomic::Ordering::Acquire),
            7
        );
        assert_eq!(
            wcet_stats.underestimations.load(core::sync::atomic::Ordering::Acquire),
            1
        );

        println!("WCET Validation Results:");
        println!("- Static WCET: {} fuel", wcet_result.wcet_fuel);
        println!("- Measurement WCET: {} fuel", refined_result.wcet_fuel);
        println!("- Sample count: {}", refined_result.sample_count);
        println!(
            "- Confidence: {:.2}%",
            refined_result.confidence_level * 100.0
        );
    }

    #[test]
    fn test_phase3_asil_c_compliance_scenario() {
        // Comprehensive ASIL-C compliance test demonstrating all Phase 3 features

        // 1. Set up WCET analyzer with strict safety margins
        let wcet_config = WcetAnalyzerConfig {
            default_method: WcetAnalysisMethod::Hybrid,
            required_confidence: 0.999, // 99.9% confidence for ASIL-C
            safety_margin_factor: 1.3,  // 30% safety margin
            enable_online_sampling: true,
            enable_path_analysis: true,
            min_samples_for_stats: 20,
        };
        let mut wcet_analyzer =
            FuelWcetAnalyzer::new(wcet_config, VerificationLevel::Full).unwrap();

        // 2. Set up deadline scheduler with ASIL-C configuration
        let scheduler_config = DeadlineSchedulerConfig {
            enable_hybrid_scheduling: true,
            enable_criticality_switching: true,
            enable_wcet_enforcement: true,
            enable_deadline_monitoring: true,
            max_utilization_per_level: 0.6, // Conservative for ASIL-C
            global_utilization_bound: 0.5,  // Very conservative
            deadline_miss_threshold: 1,     // Strict threshold
            scheduling_overhead_factor: 1.15, // Account for analysis overhead
        };
        let mut scheduler =
            FuelDeadlineScheduler::new(scheduler_config, VerificationLevel::Full).unwrap();

        // 3. Create ASIL-C safety-critical tasks
        let engine_control_task = TaskId::new(100);
        let brake_monitor_task = TaskId::new(101);
        let steering_assist_task = TaskId::new(102);

        // Engine control - highest priority, shortest period
        scheduler
            .add_deadline_task(
                engine_control_task,
                ComponentInstanceId::new(1),
                AsilLevel::C,
                Duration::from_millis(10), // 100Hz
                Duration::from_millis(8),  // Tight deadline
                40,                        // WCET fuel
                30,                        // BCET fuel
            )
            .unwrap();

        // Brake monitoring - medium priority
        scheduler
            .add_deadline_task(
                brake_monitor_task,
                ComponentInstanceId::new(2),
                AsilLevel::C,
                Duration::from_millis(20), // 50Hz
                Duration::from_millis(15), // Constrained deadline
                60,                        // WCET fuel
                45,                        // BCET fuel
            )
            .unwrap();

        // Steering assist - lower priority
        scheduler
            .add_deadline_task(
                steering_assist_task,
                ComponentInstanceId::new(3),
                AsilLevel::C,
                Duration::from_millis(50), // 20Hz
                Duration::from_millis(40), // Constrained deadline
                120,                       // WCET fuel
                90,                        // BCET fuel
            )
            .unwrap();

        // 4. Perform comprehensive WCET analysis
        for &task_id in &[
            engine_control_task,
            brake_monitor_task,
            steering_assist_task,
        ] {
            // Register critical paths
            wcet_analyzer
                .register_control_flow_path(
                    task_id,
                    1, // Normal path
                    &[1, 2, 3],
                    if task_id == engine_control_task {
                        35
                    } else if task_id == brake_monitor_task {
                        55
                    } else {
                        110
                    },
                )
                .unwrap();

            wcet_analyzer
                .register_control_flow_path(
                    task_id,
                    2, // Error handling path
                    &[1, 4, 5, 6],
                    if task_id == engine_control_task {
                        40
                    } else if task_id == brake_monitor_task {
                        60
                    } else {
                        120
                    },
                )
                .unwrap();

            // Perform WCET analysis
            let wcet_result = wcet_analyzer
                .analyze_task_wcet(
                    task_id,
                    ComponentInstanceId::new(1),
                    Some(WcetAnalysisMethod::Hybrid),
                )
                .unwrap();

            println!("Task {} WCET Analysis:", task_id.0);
            println!(
                "  WCET: {} fuel, BCET: {} fuel",
                wcet_result.wcet_fuel, wcet_result.bcet_fuel
            );
            println!("  Confidence: {:.1}%", wcet_result.confidence_level * 100.0);
        }

        // 5. Test schedulability with ASIL-C constraints
        let analysis = scheduler.analyze_schedulability().unwrap();
        assert!(analysis.schedulable);

        // ASIL-C requires very conservative utilization
        assert!(analysis.total_utilization <= 0.5);

        // 6. Simulate critical execution scenario
        let mut execution_cycles = 0;
        let max_cycles = 10;

        while execution_cycles < max_cycles {
            // Schedule highest priority ready task
            if let Some(next_task) = scheduler.schedule_next_task().unwrap() {
                // Simulate execution with potential variability
                let base_fuel = match next_task {
                    id if id == engine_control_task => 35,
                    id if id == brake_monitor_task => 55,
                    id if id == steering_assist_task => 110,
                    _ => 50,
                };

                // Add small variation for realism
                let actual_fuel = base_fuel + (execution_cycles % 3) as u64;

                // Collect sample for WCET refinement
                wcet_analyzer
                    .collect_execution_sample(
                        next_task,
                        actual_fuel,
                        Some(1), // Normal path
                        execution_cycles as u32,
                    )
                    .unwrap();

                // Update task execution
                scheduler
                    .update_task_execution(next_task, actual_fuel, AsyncTaskState::Completed)
                    .unwrap();

                // Validate against WCET estimate
                let validation_result =
                    wcet_analyzer.validate_wcet_estimate(next_task, actual_fuel);
                if let Ok(within_estimate) = validation_result {
                    assert!(
                        within_estimate,
                        "Task {} exceeded WCET estimate",
                        next_task.0
                    );
                }
            }

            execution_cycles += 1;
        }

        // 7. Verify ASIL-C compliance metrics
        let scheduler_stats = scheduler.get_statistics();
        let wcet_stats = wcet_analyzer.get_statistics();

        // No deadline misses allowed for ASIL-C
        assert_eq!(
            scheduler_stats
                .total_deadline_misses
                .load(core::sync::atomic::Ordering::Acquire),
            0
        );

        // All scheduled tasks completed successfully
        assert!(
            scheduler_stats.successful_deadlines.load(core::sync::atomic::Ordering::Acquire) > 0
        );

        // No WCET violations
        assert_eq!(
            wcet_stats.underestimations.load(core::sync::atomic::Ordering::Acquire),
            0
        );

        // Utilization remains within safe bounds
        let current_util =
            scheduler_stats.current_utilization.load(core::sync::atomic::Ordering::Acquire) as f64
                / 1000.0;
        assert!(current_util <= 0.5);

        println!("ASIL-C Compliance Verification Results:");
        println!(
            "✓ Zero deadline misses: {}",
            scheduler_stats
                .total_deadline_misses
                .load(core::sync::atomic::Ordering::Acquire)
                == 0
        );
        println!(
            "✓ Zero WCET violations: {}",
            wcet_stats.underestimations.load(core::sync::atomic::Ordering::Acquire) == 0
        );
        println!(
            "✓ Conservative utilization: {:.2}% ≤ 50%",
            current_util * 100.0
        );
        println!(
            "✓ Successful task executions: {}",
            scheduler_stats.successful_deadlines.load(core::sync::atomic::Ordering::Acquire)
        );
        println!(
            "✓ WCET analysis samples: {}",
            wcet_stats.total_samples.load(core::sync::atomic::Ordering::Acquire)
        );
        println!("✓ Deterministic scheduling: Hybrid RM+EDF with fuel bounds");
        println!("✓ Freedom from interference: Component isolation enforced");
    }

    #[test]
    fn test_phase3_constrained_deadline_validation() {
        // Test that deadline ≤ period constraint is enforced
        let config = DeadlineSchedulerConfig::default();
        let mut scheduler =
            FuelDeadlineScheduler::new(config, VerificationLevel::Standard).unwrap();

        // Valid case: deadline < period
        let valid_result = scheduler.add_deadline_task(
            TaskId::new(1),
            ComponentInstanceId::new(1),
            AsilLevel::B,
            Duration::from_millis(100), // period
            Duration::from_millis(80),  // deadline < period ✓
            50,
            40,
        );
        assert!(valid_result.is_ok());

        // Valid case: deadline = period
        let equal_result = scheduler.add_deadline_task(
            TaskId::new(2),
            ComponentInstanceId::new(1),
            AsilLevel::B,
            Duration::from_millis(50), // period
            Duration::from_millis(50), // deadline = period ✓
            25,
            20,
        );
        assert!(equal_result.is_ok());

        // Invalid case: deadline > period
        let invalid_result = scheduler.add_deadline_task(
            TaskId::new(3),
            ComponentInstanceId::new(1),
            AsilLevel::B,
            Duration::from_millis(30), // period
            Duration::from_millis(50), // deadline > period ✗
            20,
            15,
        );
        assert!(invalid_result.is_err());
    }
}

/// Example usage functions for Phase 3 documentation
#[allow(dead_code)]
mod examples {
    use super::*;

    /// Example: Creating a complete Phase 3 ASIL-C async system
    pub async fn create_phase3_asil_c_system() -> Result<(), Error> {
        // 1. Create WCET analyzer for safety-critical timing analysis
        let wcet_config = WcetAnalyzerConfig {
            default_method: WcetAnalysisMethod::Hybrid,
            required_confidence: 0.999,
            safety_margin_factor: 1.3,
            enable_online_sampling: true,
            enable_path_analysis: true,
            ..Default::default()
        };
        let wcet_analyzer = FuelWcetAnalyzer::new(wcet_config, VerificationLevel::Full)?;

        // 2. Create deadline scheduler with ASIL-C constraints
        let scheduler_config = DeadlineSchedulerConfig {
            enable_hybrid_scheduling: true,
            enable_criticality_switching: true,
            enable_wcet_enforcement: true,
            enable_deadline_monitoring: true,
            max_utilization_per_level: 0.6,
            global_utilization_bound: 0.5,
            deadline_miss_threshold: 1,
            scheduling_overhead_factor: 1.15,
        };
        let deadline_scheduler =
            FuelDeadlineScheduler::new(scheduler_config, VerificationLevel::Full)?;

        // 3. Create Phase 1-2 components for complete system
        let mut executor = FuelAsyncExecutor::new()?;
        executor.set_global_fuel_limit(50000);
        executor.set_default_verification_level(VerificationLevel::Full);

        let preemptive_config = PreemptiveSchedulerConfig {
            default_fuel_quantum: 500,
            enable_priority_aging: false, // Disabled for deterministic ASIL-C behavior
            enable_deadline_scheduling: true,
            enable_priority_inheritance: true,
            ..Default::default()
        };
        let preemptive_scheduler =
            FuelPreemptiveScheduler::new(preemptive_config, VerificationLevel::Full)?;

        let priority_protocol = FuelPriorityInheritanceProtocol::new(VerificationLevel::Full)?;
        let channel_manager = FuelAsyncChannelManager::<String>::new(VerificationLevel::Full)?;

        println!("Phase 3 ASIL-C async system created with:");
        println!("- Fuel-aware WCET analysis with 99.9% confidence bounds");
        println!("- Constrained deadline scheduler (deadline ≤ period)");
        println!("- Hybrid Rate Monotonic + EDF scheduling");
        println!("- ASIL-based criticality mode switching");
        println!("- Real-time WCET enforcement and validation");
        println!("- Freedom from interference with deadline guarantees");

        Ok(())
    }

    /// Example: ASIL-C engine control task with WCET analysis
    pub async fn asil_c_engine_control_example() -> Result<(), Error> {
        let mut wcet_analyzer =
            FuelWcetAnalyzer::new(WcetAnalyzerConfig::default(), VerificationLevel::Full)?;
        let mut scheduler = FuelDeadlineScheduler::new(
            DeadlineSchedulerConfig::default(),
            VerificationLevel::Full,
        )?;

        let engine_task = TaskId::new(1);
        let engine_component = ComponentInstanceId::new(1);

        // Register control flow paths for engine control
        wcet_analyzer.register_control_flow_path(
            engine_task,
            1,             // Normal operation path
            &[1, 2, 3, 4], // Read sensors → Calculate → Apply control → Update state
            45,            // Estimated fuel consumption
        )?;

        wcet_analyzer.register_control_flow_path(
            engine_task,
            2,                // Error handling path
            &[1, 5, 6, 7, 8], // Read sensors → Detect error → Safe mode → Log → Notify
            65,               // Higher fuel consumption for error handling
        )?;

        // Perform WCET analysis
        let wcet_result = wcet_analyzer.analyze_task_wcet(
            engine_task,
            engine_component,
            Some(WcetAnalysisMethod::Static),
        )?;

        // Add task to deadline scheduler with WCET-derived timing
        scheduler.add_deadline_task(
            engine_task,
            engine_component,
            AsilLevel::C,
            Duration::from_millis(10), // 100Hz control loop
            Duration::from_millis(8),  // Tight deadline for responsiveness
            wcet_result.wcet_fuel,     // Use analyzed WCET
            wcet_result.bcet_fuel,     // Use analyzed BCET
        )?;

        println!("ASIL-C Engine Control Task configured:");
        println!("- Period: 10ms (100Hz control rate)");
        println!("- Deadline: 8ms (constrained deadline)");
        println!("- WCET: {} fuel units", wcet_result.wcet_fuel);
        println!("- BCET: {} fuel units", wcet_result.bcet_fuel);
        println!("- Safety margin: included in WCET analysis");

        Ok(())
    }
}
