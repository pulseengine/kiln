.. _cpu_budgets:

CPU Resource Management and Budgets
====================================

This section documents CPU resource management in Pulseengine (Kiln Edition), including
execution budgets, performance monitoring, and resource allocation across different environments.

.. arch_component:: ARCH_COMP_CPU_001
   :title: CPU Resource Management System
   :status: implemented
   :version: 1.0
   :rationale: Ensure predictable CPU resource usage and prevent resource exhaustion

   CPU resource management system that provides execution budgets, performance monitoring,
   and fair resource allocation across components in all runtime environments.

CPU Budget Architecture
-----------------------

Execution Budget Framework
~~~~~~~~~~~~~~~~~~~~~~~~~~

CPU resources are managed through a budget-based system (``kiln-runtime/src/execution.rs:289-356``):

.. code-block:: rust

   /// CPU execution budget configuration
   #[derive(Debug, Clone, Copy)]
   pub struct CpuBudget {
       /// Maximum number of instructions per execution cycle
       pub max_instructions: u64,
       
       /// Maximum execution time per function call
       pub max_execution_time: Duration,
       
       /// Maximum stack depth
       pub max_stack_depth: usize,
       
       /// Maximum number of function calls
       pub max_function_calls: u64,
       
       /// Maximum number of host function calls
       pub max_host_calls: u64,
       
       /// CPU priority level (0 = highest, 255 = lowest)
       pub priority: u8,
   }

   /// Current CPU usage tracking
   #[derive(Debug, Clone, Copy, Default)]
   pub struct CpuUsage {
       /// Instructions executed in current cycle
       pub instructions_executed: u64,
       
       /// Execution time elapsed
       pub execution_time: Duration,
       
       /// Current stack depth
       pub current_stack_depth: usize,
       
       /// Function calls made
       pub function_calls_made: u64,
       
       /// Host calls made
       pub host_calls_made: u64,
       
       /// Number of budget violations
       pub budget_violations: u64,
   }

   /// CPU budget manager
   pub struct CpuBudgetManager {
       /// Budget configuration per component
       #[cfg(any(feature = "std", feature = "alloc"))]
       component_budgets: HashMap<ComponentId, CpuBudget>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       component_budgets: heapless::FnvIndexMap<ComponentId, CpuBudget, 256>,
       
       /// Current usage tracking
       #[cfg(any(feature = "std", feature = "alloc"))]
       component_usage: HashMap<ComponentId, CpuUsage>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       component_usage: heapless::FnvIndexMap<ComponentId, CpuUsage, 256>,
       
       /// Global CPU budget
       global_budget: CpuBudget,
       
       /// Budget reset interval
       reset_interval: Duration,
       
       /// Last reset timestamp
       last_reset: Timestamp,
   }

Environment-Specific CPU Management
-----------------------------------

CPU Resource Allocation by Environment
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Different environments have different CPU management capabilities:

.. list-table:: CPU Management by Environment
   :header-rows: 1
   :widths: 20 25 25 30

   * - Feature
     - std Environment
     - no_std+alloc Environment
     - no_std+no_alloc Environment
   * - Timing precision
     - Nanosecond (Instant)
     - Nanosecond (Instant)
     - Platform-dependent
   * - Thread scheduling
     - OS scheduler
     - OS scheduler
     - Single-threaded/RTOS
   * - Priority management
     - OS thread priorities
     - OS thread priorities
     - RTOS task priorities
   * - Budget enforcement
     - Preemptive
     - Preemptive
     - Cooperative
   * - Performance counters
     - Hardware counters
     - Hardware counters
     - Basic instruction counts

**Implementation Differences**:

.. code-block:: rust

   /// Environment-specific CPU management
   impl CpuBudgetManager {
       /// Check if budget allows continued execution
       pub fn check_budget_available(
           &self,
           component_id: ComponentId,
       ) -> Result<(), BudgetError> {
           let budget = self.get_component_budget(component_id)?;
           let usage = self.get_component_usage(component_id)?;
           
           // Check instruction count limit
           if usage.instructions_executed >= budget.max_instructions {
               return Err(BudgetError::InstructionLimitExceeded {
                   executed: usage.instructions_executed,
                   limit: budget.max_instructions,
               });
           }
           
           // Check execution time limit
           #[cfg(any(feature = "std", target_has_atomic = "64"))]
           {
               if usage.execution_time >= budget.max_execution_time {
                   return Err(BudgetError::TimeLimitExceeded {
                       elapsed: usage.execution_time,
                       limit: budget.max_execution_time,
                   });
               }
           }
           
           // Check stack depth limit
           if usage.current_stack_depth >= budget.max_stack_depth {
               return Err(BudgetError::StackLimitExceeded {
                   depth: usage.current_stack_depth,
                   limit: budget.max_stack_depth,
               });
           }
           
           Ok(())
       }
   }

Instruction-Level CPU Accounting
--------------------------------

Execution Profiling
~~~~~~~~~~~~~~~~~~~

CPU usage is tracked at the instruction level for precise accounting:

.. code-block:: rust

   /// Instruction execution tracker
   pub struct InstructionProfiler {
       /// Instructions executed per category
       instruction_counts: InstructionCounts,
       
       /// Execution time per instruction type
       #[cfg(feature = "std")]
       instruction_timings: HashMap<InstructionType, Duration>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       instruction_timings: heapless::FnvIndexMap<InstructionType, Duration, 64>,
       
       /// Current execution context
       current_context: ExecutionContext,
   }

   /// Instruction category counters
   #[derive(Debug, Clone, Copy, Default)]
   pub struct InstructionCounts {
       /// Control flow instructions (br, call, return)
       pub control_flow: u64,
       
       /// Memory access instructions (load, store)
       pub memory_access: u64,
       
       /// Arithmetic instructions (add, mul, div)
       pub arithmetic: u64,
       
       /// Comparison instructions (eq, ne, lt, gt)
       pub comparison: u64,
       
       /// Conversion instructions (wrap, extend, trunc)
       pub conversion: u64,
       
       /// Host function calls
       pub host_calls: u64,
       
       /// Total instructions
       pub total: u64,
   }

   impl InstructionProfiler {
       /// Record instruction execution
       pub fn record_instruction(
           &mut self,
           instruction: &Instruction,
           execution_time: Duration,
       ) {
           // Update instruction counts
           match instruction {
               Instruction::Br { .. } | 
               Instruction::BrIf { .. } | 
               Instruction::Call { .. } => {
                   self.instruction_counts.control_flow += 1;
               }
               Instruction::I32Load { .. } | 
               Instruction::I32Store { .. } => {
                   self.instruction_counts.memory_access += 1;
               }
               Instruction::I32Add | 
               Instruction::I32Mul | 
               Instruction::I32Div => {
                   self.instruction_counts.arithmetic += 1;
               }
               // ... other instruction categories
           }
           
           self.instruction_counts.total += 1;
           
           // Record timing information
           #[cfg(feature = "std")]
           {
               let instruction_type = InstructionType::from(instruction);
               let existing_time = self.instruction_timings
                   .get(&instruction_type)
                   .unwrap_or(&Duration::ZERO);
               self.instruction_timings.insert(
                   instruction_type, 
                   *existing_time + execution_time
               );
           }
       }
   }

Real-Time CPU Budget Enforcement
--------------------------------

Preemptive Budget Enforcement
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

In environments that support it, budget enforcement can be preemptive:

.. code-block:: rust

   /// Preemptive budget enforcement (std environments)
   #[cfg(feature = "std")]
   pub struct PreemptiveBudgetEnforcer {
       /// Budget violation handlers
       violation_handlers: Vec<Box<dyn BudgetViolationHandler>>,
       
       /// Execution timer
       execution_timer: Option<std::thread::JoinHandle<()>>,
       
       /// Interrupt signal
       interrupt_signal: Arc<AtomicBool>,
   }

   #[cfg(feature = "std")]
   impl PreemptiveBudgetEnforcer {
       /// Start budget enforcement for execution
       pub fn start_enforcement(
           &mut self,
           component_id: ComponentId,
           budget: &CpuBudget,
       ) -> Result<(), BudgetError> {
           let interrupt_signal = self.interrupt_signal.clone();
           let max_time = budget.max_execution_time;
           
           // Start timer thread
           self.execution_timer = Some(std::thread::spawn(move || {
               std::thread::sleep(max_time);
               interrupt_signal.store(true, Ordering::SeqCst);
           }));
           
           Ok(())
       }
       
       /// Check for budget interrupt
       pub fn check_interrupt(&self) -> bool {
           self.interrupt_signal.load(Ordering::SeqCst)
       }
   }

Cooperative Budget Enforcement
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

In no_alloc environments, budget enforcement is typically cooperative:

.. code-block:: rust

   /// Cooperative budget enforcement (no_alloc environments)
   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   pub struct CooperativeBudgetEnforcer {
       /// Instruction counter
       instruction_counter: u64,
       
       /// Check interval (instructions between budget checks)
       check_interval: u64,
       
       /// Last budget check time
       last_check_time: u64, // Platform-specific time units
   }

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   impl CooperativeBudgetEnforcer {
       /// Check budget at instruction boundaries
       pub fn check_budget_at_instruction(
           &mut self,
           budget_manager: &CpuBudgetManager,
           component_id: ComponentId,
       ) -> Result<(), BudgetError> {
           self.instruction_counter += 1;
           
           // Check budget every N instructions
           if self.instruction_counter % self.check_interval == 0 {
               budget_manager.check_budget_available(component_id)?;
               
               // Update timing if platform supports it
               #[cfg(target_has_atomic = "64")]
               {
                   let current_time = self.get_platform_time();
                   if current_time > self.last_check_time {
                       self.last_check_time = current_time;
                   }
               }
           }
           
           Ok(())
       }
   }

CPU Priority Management
-----------------------

Component Priority System
~~~~~~~~~~~~~~~~~~~~~~~~~

Components can be assigned different CPU priorities:

.. code-block:: rust

   /// CPU priority levels
   #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
   pub enum CpuPriority {
       Critical = 0,    // Highest priority (safety-critical components)
       High = 64,       // High priority (real-time components)
       Normal = 128,    // Normal priority (standard components)
       Low = 192,       // Low priority (background components)
       Idle = 255,      // Lowest priority (idle/cleanup components)
   }

   /// Priority-based scheduler
   pub struct PriorityScheduler {
       /// Priority queues for ready components
       #[cfg(any(feature = "std", feature = "alloc"))]
       priority_queues: BTreeMap<CpuPriority, VecDeque<ComponentId>>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       priority_queues: heapless::FnvIndexMap<CpuPriority, heapless::Deque<ComponentId, 64>, 5>,
       
       /// Currently executing component
       current_component: Option<ComponentId>,
       
       /// Time slice duration per priority level
       time_slices: [Duration; 5],
   }

   impl PriorityScheduler {
       /// Schedule next component for execution
       pub fn schedule_next(&mut self) -> Option<ComponentId> {
           // Find highest priority non-empty queue
           for priority in [
               CpuPriority::Critical,
               CpuPriority::High,
               CpuPriority::Normal,
               CpuPriority::Low,
               CpuPriority::Idle,
           ] {
               if let Some(queue) = self.priority_queues.get_mut(&priority) {
                   if let Some(component_id) = queue.pop_front() {
                       self.current_component = Some(component_id);
                       return Some(component_id);
                   }
               }
           }
           
           None
       }
       
       /// Yield current component back to appropriate queue
       pub fn yield_component(&mut self, component_id: ComponentId, priority: CpuPriority) {
           if let Some(queue) = self.priority_queues.get_mut(&priority) {
               let _ = queue.push_back(component_id); // May fail in no_alloc if queue full
           }
           
           if self.current_component == Some(component_id) {
               self.current_component = None;
           }
       }
   }

Performance Monitoring and Profiling
------------------------------------

CPU Performance Metrics
~~~~~~~~~~~~~~~~~~~~~~~

Comprehensive CPU performance monitoring across environments:

.. code-block:: rust

   /// CPU performance metrics
   #[derive(Debug, Clone, Default)]
   pub struct CpuPerformanceMetrics {
       /// Instructions per second
       pub instructions_per_second: f64,
       
       /// Average execution time per instruction
       pub avg_instruction_time: Duration,
       
       /// CPU utilization percentage
       pub cpu_utilization: f32,
       
       /// Cache hit rates (if available)
       #[cfg(feature = "std")]
       pub instruction_cache_hits: f32,
       #[cfg(feature = "std")]
       pub data_cache_hits: f32,
       
       /// Branch prediction accuracy (if available)
       #[cfg(feature = "std")]
       pub branch_prediction_accuracy: f32,
       
       /// Platform-specific metrics
       #[cfg(target_os = "linux")]
       pub context_switches: u64,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       pub interrupt_latency: Duration,
   }

   /// Performance monitoring system
   pub struct CpuPerformanceMonitor {
       /// Metrics collection
       metrics: CpuPerformanceMetrics,
       
       /// Sampling interval
       sampling_interval: Duration,
       
       /// Performance counters (platform-specific)
       #[cfg(feature = "std")]
       performance_counters: PerformanceCounterSet,
       
       /// Instruction profiler
       instruction_profiler: InstructionProfiler,
   }

   impl CpuPerformanceMonitor {
       /// Collect current performance metrics
       pub fn collect_metrics(&mut self) -> &CpuPerformanceMetrics {
           // Update instruction-based metrics
           let total_instructions = self.instruction_profiler.instruction_counts.total;
           let total_time = self.instruction_profiler.get_total_execution_time();
           
           if total_time.as_nanos() > 0 {
               self.metrics.instructions_per_second = 
                   (total_instructions as f64) / total_time.as_secs_f64();
               self.metrics.avg_instruction_time = 
                   total_time / (total_instructions as u32).max(1);
           }
           
           // Platform-specific metrics collection
           #[cfg(feature = "std")]
           {
               self.collect_hardware_metrics();
           }
           
           #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
           {
               self.collect_embedded_metrics();
           }
           
           &self.metrics
       }
       
       #[cfg(feature = "std")]
       fn collect_hardware_metrics(&mut self) {
           // Collect hardware performance counters
           if let Some(counters) = &mut self.performance_counters {
               self.metrics.instruction_cache_hits = counters.get_icache_hit_rate();
               self.metrics.data_cache_hits = counters.get_dcache_hit_rate();
               self.metrics.branch_prediction_accuracy = counters.get_branch_prediction_accuracy();
           }
       }
       
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       fn collect_embedded_metrics(&mut self) {
           // Collect embedded-specific metrics
           self.metrics.interrupt_latency = self.measure_interrupt_latency();
       }
   }

Budget Optimization and Tuning
------------------------------

Adaptive Budget Adjustment
~~~~~~~~~~~~~~~~~~~~~~~~~~

CPU budgets can be dynamically adjusted based on performance metrics:

.. code-block:: rust

   /// Budget optimization engine
   pub struct BudgetOptimizer {
       /// Performance history for budget adjustment
       #[cfg(any(feature = "std", feature = "alloc"))]
       performance_history: VecDeque<CpuPerformanceMetrics>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       performance_history: heapless::Deque<CpuPerformanceMetrics, 32>,
       
       /// Optimization strategy
       optimization_strategy: OptimizationStrategy,
       
       /// Adjustment parameters
       adjustment_params: AdjustmentParameters,
   }

   #[derive(Debug, Clone, Copy)]
   pub enum OptimizationStrategy {
       /// Maximize overall throughput
       Throughput,
       /// Minimize worst-case latency
       Latency,
       /// Balance throughput and latency
       Balanced,
       /// Optimize for power efficiency
       PowerEfficient,
   }

   #[derive(Debug, Clone, Copy)]
   pub struct AdjustmentParameters {
       /// Maximum budget increase per adjustment cycle
       pub max_increase_factor: f32,
       
       /// Maximum budget decrease per adjustment cycle
       pub max_decrease_factor: f32,
       
       /// Minimum adjustment threshold
       pub adjustment_threshold: f32,
       
       /// Adjustment smoothing factor
       pub smoothing_factor: f32,
   }

   impl BudgetOptimizer {
       /// Optimize budget based on performance history
       pub fn optimize_budget(
           &mut self,
           current_budget: &CpuBudget,
           recent_metrics: &CpuPerformanceMetrics,
       ) -> CpuBudget {
           let mut optimized_budget = *current_budget;
           
           // Add current metrics to history
           if self.performance_history.len() >= self.performance_history.capacity() {
               let _ = self.performance_history.pop_front();
           }
           let _ = self.performance_history.push_back(recent_metrics.clone());
           
           // Calculate optimization adjustments
           match self.optimization_strategy {
               OptimizationStrategy::Throughput => {
                   self.optimize_for_throughput(&mut optimized_budget, recent_metrics);
               }
               OptimizationStrategy::Latency => {
                   self.optimize_for_latency(&mut optimized_budget, recent_metrics);
               }
               OptimizationStrategy::Balanced => {
                   self.optimize_balanced(&mut optimized_budget, recent_metrics);
               }
               OptimizationStrategy::PowerEfficient => {
                   self.optimize_for_power(&mut optimized_budget, recent_metrics);
               }
           }
           
           optimized_budget
       }
       
       fn optimize_for_throughput(
           &self,
           budget: &mut CpuBudget,
           metrics: &CpuPerformanceMetrics,
       ) {
           // Increase instruction limit if CPU utilization is low
           if metrics.cpu_utilization < 0.8 {
               let increase_factor = 1.0 + (0.8 - metrics.cpu_utilization) * 0.1;
               budget.max_instructions = 
                   (budget.max_instructions as f64 * increase_factor as f64) as u64;
           }
       }
   }

Environment-Specific CPU Optimizations
--------------------------------------

Platform-Specific Optimizations
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Different platforms enable different CPU optimizations:

.. list-table:: CPU Optimizations by Platform
   :header-rows: 1
   :widths: 25 25 25 25

   * - Optimization
     - Linux/macOS (std)
     - QNX (no_std+alloc)
     - Embedded (no_alloc)
   * - Thread affinity
     - CPU core pinning
     - CPU core pinning
     - Not applicable
   * - NUMA optimization
     - Memory locality
     - Memory locality
     - Not applicable
   * - Cache optimization
     - Cache-friendly scheduling
     - Cache-friendly scheduling
     - Manual cache management
   * - Power management
     - DVFS support
     - Platform-dependent
     - Hardware-specific
   * - Real-time scheduling
     - RT priorities
     - QNX RT scheduling
     - Interrupt-based

Cross-References
-----------------

.. seealso::

   * :doc:`memory_budgets` for memory resource management
   * :doc:`io_constraints` for I/O resource constraints
   * :doc:`resource_overview` for overall resource management
   * :doc:`../04_dynamic_behavior/state_machines` for execution state management
   * :doc:`../01_architectural_design/patterns` for CPU management patterns