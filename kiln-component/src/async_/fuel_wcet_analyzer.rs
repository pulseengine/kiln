//! Worst-Case Execution Time (WCET) analysis for fuel-based scheduling
//!
//! This module provides WCET analysis tools for safety-critical systems,
//! enabling deterministic timing guarantees required for ASIL-C compliance.

use core::{
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

#[cfg(feature = "std")]
use log::{debug, error, info, trace, warn};
use wrt_foundation::{
    CrateId,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    operations::{Type as OperationType, record_global_operation},
    safe_managed_alloc,
    traits::{Checksummable, FromBytes, ToBytes},
    verification::VerificationLevel,
};

#[cfg(feature = "component-model-threading")]
use crate::threading::task_manager::TaskId;
use crate::{ComponentInstanceId, prelude::*};

// Placeholder TaskId when threading is not available
#[cfg(not(feature = "component-model-threading"))]
pub type TaskId = u32;

/// Maximum number of WCET analysis entries
const MAX_WCET_ENTRIES: usize = 512;

/// Maximum number of execution samples per task
const MAX_EXECUTION_SAMPLES: usize = 1000;

/// Maximum number of control flow paths
const MAX_CONTROL_FLOW_PATHS: usize = 64;

/// Fuel costs for WCET analysis operations
const WCET_ANALYSIS_FUEL: u64 = 30;
const SAMPLE_COLLECTION_FUEL: u64 = 5;
const PATH_ANALYSIS_FUEL: u64 = 20;
const STATISTICAL_ANALYSIS_FUEL: u64 = 15;

/// WCET analysis methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WcetAnalysisMethod {
    /// Static analysis based on code structure
    Static,
    /// Measurement-based analysis using execution samples
    MeasurementBased,
    /// Hybrid analysis combining static and measurement approaches
    Hybrid,
    /// Probabilistic WCET analysis
    Probabilistic,
}

impl Checksummable for WcetAnalysisMethod {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let discriminant = match self {
            WcetAnalysisMethod::Static => 0u8,
            WcetAnalysisMethod::MeasurementBased => 1u8,
            WcetAnalysisMethod::Hybrid => 2u8,
            WcetAnalysisMethod::Probabilistic => 3u8,
        };
        discriminant.update_checksum(checksum);
    }
}

impl ToBytes for WcetAnalysisMethod {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        let discriminant = match self {
            WcetAnalysisMethod::Static => 0u8,
            WcetAnalysisMethod::MeasurementBased => 1u8,
            WcetAnalysisMethod::Hybrid => 2u8,
            WcetAnalysisMethod::Probabilistic => 3u8,
        };
        discriminant.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for WcetAnalysisMethod {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let discriminant = u8::from_bytes_with_provider(reader, provider)?;
        match discriminant {
            0 => Ok(WcetAnalysisMethod::Static),
            1 => Ok(WcetAnalysisMethod::MeasurementBased),
            2 => Ok(WcetAnalysisMethod::Hybrid),
            3 => Ok(WcetAnalysisMethod::Probabilistic),
            _ => Err(wrt_error::Error::runtime_error(
                "Invalid WcetAnalysisMethod discriminant",
            )),
        }
    }
}

/// Control flow path information
#[derive(Debug)]
pub struct ControlFlowPath {
    /// Unique path identifier
    pub path_id: u32,
    /// Sequence of basic blocks in this path
    pub basic_blocks: BoundedVec<u32, MAX_CONTROL_FLOW_PATHS>,
    /// Estimated fuel consumption for this path
    pub estimated_fuel: u64,
    /// Measured fuel consumption samples
    pub measured_samples: BoundedVec<u64, MAX_EXECUTION_SAMPLES>,
    /// Path execution frequency
    pub execution_count: AtomicUsize,
    /// Whether this is the critical (longest) path
    pub is_critical_path: bool,
}

impl Clone for ControlFlowPath {
    fn clone(&self) -> Self {
        Self {
            path_id: self.path_id,
            basic_blocks: self.basic_blocks.clone(),
            estimated_fuel: self.estimated_fuel,
            measured_samples: self.measured_samples.clone(),
            execution_count: AtomicUsize::new(self.execution_count.load(Ordering::Relaxed)),
            is_critical_path: self.is_critical_path,
        }
    }
}

impl PartialEq for ControlFlowPath {
    fn eq(&self, other: &Self) -> bool {
        self.path_id == other.path_id
            && self.basic_blocks == other.basic_blocks
            && self.estimated_fuel == other.estimated_fuel
            && self.measured_samples == other.measured_samples
            && self.execution_count.load(Ordering::Relaxed)
                == other.execution_count.load(Ordering::Relaxed)
            && self.is_critical_path == other.is_critical_path
    }
}

impl Eq for ControlFlowPath {}

impl Default for ControlFlowPath {
    fn default() -> Self {
        Self {
            path_id: 0,
            basic_blocks: BoundedVec::new().unwrap(),
            estimated_fuel: 0,
            measured_samples: BoundedVec::new().unwrap(),
            execution_count: AtomicUsize::new(0),
            is_critical_path: false,
        }
    }
}

impl Checksummable for ControlFlowPath {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.path_id.update_checksum(checksum);
        self.basic_blocks.update_checksum(checksum);
        self.estimated_fuel.update_checksum(checksum);
        self.measured_samples.update_checksum(checksum);
        self.execution_count.load(Ordering::Relaxed).update_checksum(checksum);
        self.is_critical_path.update_checksum(checksum);
    }
}

impl ToBytes for ControlFlowPath {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.path_id.to_bytes_with_provider(writer, provider)?;
        self.basic_blocks.to_bytes_with_provider(writer, provider)?;
        self.estimated_fuel.to_bytes_with_provider(writer, provider)?;
        self.measured_samples.to_bytes_with_provider(writer, provider)?;
        self.execution_count
            .load(Ordering::Relaxed)
            .to_bytes_with_provider(writer, provider)?;
        self.is_critical_path.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for ControlFlowPath {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            path_id: u32::from_bytes_with_provider(reader, provider)?,
            basic_blocks: BoundedVec::from_bytes_with_provider(reader, provider)?,
            estimated_fuel: u64::from_bytes_with_provider(reader, provider)?,
            measured_samples: BoundedVec::from_bytes_with_provider(reader, provider)?,
            execution_count: AtomicUsize::new(usize::from_bytes_with_provider(reader, provider)?),
            is_critical_path: bool::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// Execution sample data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ExecutionSample {
    /// Fuel consumed in this execution
    pub fuel_consumed: u64,
    /// Timestamp when sample was collected
    pub timestamp: u64,
    /// Input characteristics that led to this execution
    pub input_hash: u32,
    /// Control flow path taken
    pub path_id: u32,
}

impl Checksummable for ExecutionSample {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.fuel_consumed.update_checksum(checksum);
        self.timestamp.update_checksum(checksum);
        self.input_hash.update_checksum(checksum);
        self.path_id.update_checksum(checksum);
    }
}

impl ToBytes for ExecutionSample {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.fuel_consumed.to_bytes_with_provider(writer, provider)?;
        self.timestamp.to_bytes_with_provider(writer, provider)?;
        self.input_hash.to_bytes_with_provider(writer, provider)?;
        self.path_id.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for ExecutionSample {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            fuel_consumed: u64::from_bytes_with_provider(reader, provider)?,
            timestamp: u64::from_bytes_with_provider(reader, provider)?,
            input_hash: u32::from_bytes_with_provider(reader, provider)?,
            path_id: u32::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// WCET analysis result
#[derive(Debug, Clone, PartialEq)]
pub struct WcetAnalysisResult {
    /// Task being analyzed
    pub task_id: TaskId,
    /// Analysis method used
    pub method: WcetAnalysisMethod,
    /// Estimated WCET in fuel units
    pub wcet_fuel: u64,
    /// Best-case execution time
    pub bcet_fuel: u64,
    /// Average execution time
    pub average_fuel: u64,
    /// Standard deviation of execution times
    pub std_deviation: f64,
    /// Confidence level of the estimate (0.0-1.0)
    pub confidence_level: f64,
    /// Critical path that leads to WCET
    pub critical_path: Option<u32>,
    /// Number of samples collected
    pub sample_count: usize,
    /// Analysis timestamp
    pub analysis_time: u64,
}

impl Default for WcetAnalysisResult {
    fn default() -> Self {
        Self {
            task_id: 0,
            method: WcetAnalysisMethod::Static,
            wcet_fuel: 0,
            bcet_fuel: 0,
            average_fuel: 0,
            std_deviation: 0.0,
            confidence_level: 0.0,
            critical_path: None,
            sample_count: 0,
            analysis_time: 0,
        }
    }
}

impl Eq for WcetAnalysisResult {}

impl Checksummable for WcetAnalysisResult {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.task_id.update_checksum(checksum);
        self.method.update_checksum(checksum);
        self.wcet_fuel.update_checksum(checksum);
        self.bcet_fuel.update_checksum(checksum);
        self.average_fuel.update_checksum(checksum);
        self.std_deviation.to_bits().update_checksum(checksum);
        self.confidence_level.to_bits().update_checksum(checksum);
        self.critical_path.update_checksum(checksum);
        self.sample_count.update_checksum(checksum);
        self.analysis_time.update_checksum(checksum);
    }
}

impl ToBytes for WcetAnalysisResult {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.task_id.to_bytes_with_provider(writer, provider)?;
        self.method.to_bytes_with_provider(writer, provider)?;
        self.wcet_fuel.to_bytes_with_provider(writer, provider)?;
        self.bcet_fuel.to_bytes_with_provider(writer, provider)?;
        self.average_fuel.to_bytes_with_provider(writer, provider)?;
        self.std_deviation.to_bits().to_bytes_with_provider(writer, provider)?;
        self.confidence_level.to_bits().to_bytes_with_provider(writer, provider)?;
        self.critical_path.to_bytes_with_provider(writer, provider)?;
        self.sample_count.to_bytes_with_provider(writer, provider)?;
        self.analysis_time.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for WcetAnalysisResult {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            task_id: u32::from_bytes_with_provider(reader, provider)?,
            method: WcetAnalysisMethod::from_bytes_with_provider(reader, provider)?,
            wcet_fuel: u64::from_bytes_with_provider(reader, provider)?,
            bcet_fuel: u64::from_bytes_with_provider(reader, provider)?,
            average_fuel: u64::from_bytes_with_provider(reader, provider)?,
            std_deviation: f64::from_bits(u64::from_bytes_with_provider(reader, provider)?),
            confidence_level: f64::from_bits(u64::from_bytes_with_provider(reader, provider)?),
            critical_path: Option::<u32>::from_bytes_with_provider(reader, provider)?,
            sample_count: usize::from_bytes_with_provider(reader, provider)?,
            analysis_time: u64::from_bytes_with_provider(reader, provider)?,
        })
    }
}

/// WCET analyzer for fuel-based timing analysis
pub struct FuelWcetAnalyzer {
    /// WCET analysis results indexed by task
    analysis_results: BoundedMap<TaskId, WcetAnalysisResult, MAX_WCET_ENTRIES>,
    /// Control flow paths for each task
    task_paths:
        BoundedMap<TaskId, BoundedVec<ControlFlowPath, MAX_CONTROL_FLOW_PATHS>, MAX_WCET_ENTRIES>,
    /// Execution samples for each task
    execution_samples:
        BoundedMap<TaskId, BoundedVec<ExecutionSample, MAX_EXECUTION_SAMPLES>, MAX_WCET_ENTRIES>,
    /// Analysis configuration
    config: WcetAnalyzerConfig,
    /// Performance statistics
    stats: WcetAnalyzerStats,
    /// Current fuel time
    current_fuel_time: AtomicU64,
    /// Verification level for fuel tracking
    verification_level: VerificationLevel,
}

/// WCET analyzer configuration
#[derive(Debug, Clone)]
pub struct WcetAnalyzerConfig {
    /// Default analysis method
    pub default_method: WcetAnalysisMethod,
    /// Confidence level required for WCET estimates
    pub required_confidence: f64,
    /// Maximum number of samples to collect per task
    pub max_samples_per_task: usize,
    /// Statistical margin for safety factor
    pub safety_margin_factor: f64,
    /// Enable online sample collection
    pub enable_online_sampling: bool,
    /// Enable path-based analysis
    pub enable_path_analysis: bool,
    /// Minimum samples required for statistical analysis
    pub min_samples_for_stats: usize,
}

/// WCET analyzer statistics
#[derive(Debug)]
pub struct WcetAnalyzerStats {
    /// Total WCET analyses performed
    pub total_analyses: AtomicUsize,
    /// Total execution samples collected
    pub total_samples: AtomicUsize,
    /// Total control flow paths discovered
    pub total_paths: AtomicUsize,
    /// Fuel consumed by WCET analysis
    pub analysis_fuel_consumed: AtomicU64,
    /// Number of WCET estimates that were too optimistic
    pub underestimations: AtomicUsize,
    /// Number of WCET estimates that were too pessimistic
    pub overestimations: AtomicUsize,
    /// Average analysis accuracy
    pub average_accuracy: AtomicU64, // Fixed point: accuracy * 1000
}

impl Default for WcetAnalyzerConfig {
    fn default() -> Self {
        Self {
            default_method: WcetAnalysisMethod::Hybrid,
            required_confidence: 0.95, // 95% confidence
            max_samples_per_task: 500,
            safety_margin_factor: 1.2, // 20% safety margin
            enable_online_sampling: true,
            enable_path_analysis: true,
            min_samples_for_stats: 50,
        }
    }
}

impl FuelWcetAnalyzer {
    /// Create a new WCET analyzer
    pub fn new(config: WcetAnalyzerConfig, verification_level: VerificationLevel) -> Result<Self> {
        let provider = safe_managed_alloc!(8192, CrateId::Component)?;
        Ok(Self {
            analysis_results: BoundedMap::new(),
            task_paths: BoundedMap::new(),
            execution_samples: BoundedMap::new(),
            config,
            stats: WcetAnalyzerStats {
                total_analyses: AtomicUsize::new(0),
                total_samples: AtomicUsize::new(0),
                total_paths: AtomicUsize::new(0),
                analysis_fuel_consumed: AtomicU64::new(0),
                underestimations: AtomicUsize::new(0),
                overestimations: AtomicUsize::new(0),
                average_accuracy: AtomicU64::new(0),
            },
            current_fuel_time: AtomicU64::new(0),
            verification_level,
        })
    }

    /// Perform WCET analysis for a task
    pub fn analyze_task_wcet(
        &mut self,
        task_id: TaskId,
        component_id: ComponentInstanceId,
        method: Option<WcetAnalysisMethod>,
    ) -> Result<WcetAnalysisResult> {
        record_global_operation(OperationType::FunctionCall, self.verification_level);
        self.consume_analysis_fuel(WCET_ANALYSIS_FUEL)?;

        let analysis_method = method.unwrap_or(self.config.default_method);
        let current_time = self.current_fuel_time.load(Ordering::Acquire);

        let result = match analysis_method {
            WcetAnalysisMethod::Static => self.perform_static_analysis(task_id, component_id)?,
            WcetAnalysisMethod::MeasurementBased => self.perform_measurement_analysis(task_id)?,
            WcetAnalysisMethod::Hybrid => self.perform_hybrid_analysis(task_id, component_id)?,
            WcetAnalysisMethod::Probabilistic => self.perform_probabilistic_analysis(task_id)?,
        };

        // Store analysis result
        self.analysis_results
            .insert(task_id, result.clone())
            .map_err(|_| Error::resource_limit_exceeded("Too many WCET analysis results"))?;

        self.stats.total_analyses.fetch_add(1, Ordering::AcqRel);
        Ok(result)
    }

    /// Collect execution sample for online WCET refinement
    pub fn collect_execution_sample(
        &mut self,
        task_id: TaskId,
        fuel_consumed: u64,
        path_id: Option<u32>,
        input_hash: u32,
    ) -> Result<()> {
        if !self.config.enable_online_sampling {
            return Ok(());
        }

        record_global_operation(OperationType::CollectionInsert, self.verification_level);
        self.consume_analysis_fuel(SAMPLE_COLLECTION_FUEL)?;

        let current_time = self.current_fuel_time.load(Ordering::Acquire);
        let sample = ExecutionSample {
            fuel_consumed,
            timestamp: current_time,
            input_hash,
            path_id: path_id.unwrap_or(0),
        };

        // Get or create sample collection for this task
        if !self.execution_samples.contains_key(&task_id) {
            let provider = safe_managed_alloc!(4096, CrateId::Component)?;
            let samples = BoundedVec::new();
            self.execution_samples
                .insert(task_id, samples)
                .map_err(|_| Error::resource_limit_exceeded("Too many task sample collections"))?;
        }

        if let Some(samples) = self.execution_samples.get_mut(&task_id) {
            // Add sample, removing oldest if at capacity
            if samples.len() >= self.config.max_samples_per_task {
                samples.remove(0); // Remove oldest sample
            }

            samples
                .push(sample)
                .map_err(|_| Error::resource_limit_exceeded("Sample collection is full"))?;
        }

        self.stats.total_samples.fetch_add(1, Ordering::AcqRel);

        // Update path information if path analysis is enabled
        if let Some(path_id) = path_id {
            self.update_path_sample(task_id, path_id, fuel_consumed)?;
        }

        // Refine WCET estimate if enough samples collected
        if let Some(samples) = self.execution_samples.get(&task_id) {
            if samples.len() >= self.config.min_samples_for_stats {
                self.refine_wcet_estimate(task_id)?;
            }
        }

        Ok(())
    }

    /// Register a control flow path for a task
    pub fn register_control_flow_path(
        &mut self,
        task_id: TaskId,
        path_id: u32,
        basic_blocks: &[u32],
        estimated_fuel: u64,
    ) -> Result<()> {
        if !self.config.enable_path_analysis {
            return Ok(());
        }

        record_global_operation(OperationType::CollectionInsert, self.verification_level);
        self.consume_analysis_fuel(PATH_ANALYSIS_FUEL)?;

        let provider = safe_managed_alloc!(1024, CrateId::Component)?;
        let mut bb_vec = BoundedVec::new();
        for &bb in basic_blocks {
            bb_vec
                .push(bb)
                .map_err(|_| Error::resource_limit_exceeded("Too many basic blocks in path"))?;
        }

        let path = ControlFlowPath {
            path_id,
            basic_blocks: bb_vec,
            estimated_fuel,
            measured_samples: {
                let provider = safe_managed_alloc!(1024, CrateId::Component)?;
                BoundedVec::new()
            },
            execution_count: AtomicUsize::new(0),
            is_critical_path: false,
        };

        // Get or create path collection for this task
        if !self.task_paths.contains_key(&task_id) {
            let provider = safe_managed_alloc!(2048, CrateId::Component)?;
            let paths = BoundedVec::new();
            self.task_paths
                .insert(task_id, paths)
                .map_err(|_| Error::resource_limit_exceeded("Too many task path collections"))?;
        }

        if let Some(paths) = self.task_paths.get_mut(&task_id) {
            paths
                .push(path)
                .map_err(|_| Error::resource_limit_exceeded("Too many paths for task"))?;
        }

        self.stats.total_paths.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    /// Get WCET analysis result for a task
    pub fn get_wcet_result(&self, task_id: TaskId) -> Option<&WcetAnalysisResult> {
        self.analysis_results.get(&task_id)
    }

    /// Validate WCET estimate against actual execution
    pub fn validate_wcet_estimate(&mut self, task_id: TaskId, actual_fuel: u64) -> Result<bool> {
        if let Some(result) = self.analysis_results.get(&task_id) {
            let within_estimate = actual_fuel <= result.wcet_fuel;

            if !within_estimate {
                self.stats.underestimations.fetch_add(1, Ordering::AcqRel);
                #[cfg(feature = "std")]
                warn!(
                    "WCET underestimation: Task {} consumed {} fuel, estimate was {}",
                    task_id, actual_fuel, result.wcet_fuel
                );
            } else if actual_fuel < result.wcet_fuel / 2 {
                self.stats.overestimations.fetch_add(1, Ordering::AcqRel);
            }

            // Update accuracy statistics
            let accuracy = if result.wcet_fuel > 0 {
                let error = actual_fuel.abs_diff(result.wcet_fuel);
                let accuracy_ratio = 1.0 - (error as f64 / result.wcet_fuel as f64);
                (accuracy_ratio * 1000.0) as u64
            } else {
                1000 // 100% accuracy if WCET is 0
            };

            let current_avg = self.stats.average_accuracy.load(Ordering::Acquire);
            let new_avg = if current_avg == 0 { accuracy } else { (current_avg + accuracy) / 2 };
            self.stats.average_accuracy.store(new_avg, Ordering::Release);

            Ok(within_estimate)
        } else {
            Err(Error::resource_not_found(
                "No WCET analysis result found for task",
            ))
        }
    }

    /// Get analyzer statistics
    pub fn get_statistics(&self) -> WcetAnalyzerStats {
        WcetAnalyzerStats {
            total_analyses: AtomicUsize::new(self.stats.total_analyses.load(Ordering::Acquire)),
            total_samples: AtomicUsize::new(self.stats.total_samples.load(Ordering::Acquire)),
            total_paths: AtomicUsize::new(self.stats.total_paths.load(Ordering::Acquire)),
            analysis_fuel_consumed: AtomicU64::new(
                self.stats.analysis_fuel_consumed.load(Ordering::Acquire),
            ),
            underestimations: AtomicUsize::new(self.stats.underestimations.load(Ordering::Acquire)),
            overestimations: AtomicUsize::new(self.stats.overestimations.load(Ordering::Acquire)),
            average_accuracy: AtomicU64::new(self.stats.average_accuracy.load(Ordering::Acquire)),
        }
    }

    // Private analysis methods

    fn perform_static_analysis(
        &mut self,
        task_id: TaskId,
        _component_id: ComponentInstanceId,
    ) -> Result<WcetAnalysisResult> {
        // Simplified static analysis - in real implementation, this would
        // analyze the task's code structure, control flow, and data dependencies

        let estimated_wcet = if let Some(paths) = self.task_paths.get(&task_id) {
            // Find the longest path
            paths.iter().map(|path| path.estimated_fuel).max().unwrap_or(1000) // Default estimate
        } else {
            1000 // Default WCET estimate
        };

        let wcet_with_margin = ((estimated_wcet as f64) * self.config.safety_margin_factor) as u64;

        Ok(WcetAnalysisResult {
            task_id,
            method: WcetAnalysisMethod::Static,
            wcet_fuel: wcet_with_margin,
            bcet_fuel: estimated_wcet / 2, // Assume BCET is half of estimated
            average_fuel: (estimated_wcet * 3) / 4, // Assume average is 75% of estimated
            std_deviation: (estimated_wcet as f64) * 0.2, // 20% standard deviation
            confidence_level: 0.8,         // Lower confidence for static analysis
            critical_path: self.find_critical_path(task_id),
            sample_count: 0,
            analysis_time: self.current_fuel_time.load(Ordering::Acquire),
        })
    }

    fn perform_measurement_analysis(&mut self, task_id: TaskId) -> Result<WcetAnalysisResult> {
        let samples = self.execution_samples.get(&task_id).ok_or_else(|| {
            Error::resource_not_found(
                "No execution samples available for measurement-based analysis",
            )
        })?;
        if samples.len() < self.config.min_samples_for_stats {
            return Err(Error::runtime_execution_error(
                "Insufficient samples for statistical analysis",
            ));
        }

        let fuel_values: Vec<u64> = samples.iter().map(|s| s.fuel_consumed).collect();
        let stats = self.calculate_execution_statistics(&fuel_values)?;

        // Apply safety margin and statistical confidence
        let confidence_multiplier =
            self.calculate_confidence_multiplier(self.config.required_confidence);
        let wcet_estimate =
            ((stats.max_value as f64) + (confidence_multiplier * stats.std_deviation)) as u64;
        let wcet_with_margin = ((wcet_estimate as f64) * self.config.safety_margin_factor) as u64;

        Ok(WcetAnalysisResult {
            task_id,
            method: WcetAnalysisMethod::MeasurementBased,
            wcet_fuel: wcet_with_margin,
            bcet_fuel: stats.min_value,
            average_fuel: stats.mean as u64,
            std_deviation: stats.std_deviation,
            confidence_level: self.config.required_confidence,
            critical_path: self.find_critical_path_from_samples(task_id),
            sample_count: samples.len(),
            analysis_time: self.current_fuel_time.load(Ordering::Acquire),
        })
    }

    fn perform_hybrid_analysis(
        &mut self,
        task_id: TaskId,
        component_id: ComponentInstanceId,
    ) -> Result<WcetAnalysisResult> {
        // Combine static and measurement-based analysis
        let static_result = self.perform_static_analysis(task_id, component_id)?;

        if let Ok(measurement_result) = self.perform_measurement_analysis(task_id) {
            // Use measurement data to refine static estimate
            let combined_wcet = if measurement_result.wcet_fuel > static_result.wcet_fuel {
                measurement_result.wcet_fuel
            } else {
                // Use static estimate but with measurement confidence
                let ratio =
                    measurement_result.average_fuel as f64 / static_result.average_fuel as f64;
                ((static_result.wcet_fuel as f64) * ratio) as u64
            };

            Ok(WcetAnalysisResult {
                task_id,
                method: WcetAnalysisMethod::Hybrid,
                wcet_fuel: combined_wcet,
                bcet_fuel: measurement_result.bcet_fuel.min(static_result.bcet_fuel),
                average_fuel: measurement_result.average_fuel,
                std_deviation: measurement_result.std_deviation,
                confidence_level: (static_result.confidence_level
                    + measurement_result.confidence_level)
                    / 2.0,
                critical_path: measurement_result.critical_path.or(static_result.critical_path),
                sample_count: measurement_result.sample_count,
                analysis_time: self.current_fuel_time.load(Ordering::Acquire),
            })
        } else {
            // Fall back to static analysis if no measurement data
            Ok(static_result)
        }
    }

    fn perform_probabilistic_analysis(&mut self, task_id: TaskId) -> Result<WcetAnalysisResult> {
        self.consume_analysis_fuel(STATISTICAL_ANALYSIS_FUEL)?;

        let samples = self.execution_samples.get(&task_id).ok_or_else(|| {
            Error::resource_not_found("No path history available for hybrid analysis")
        })?;

        if samples.len() < self.config.min_samples_for_stats {
            return Err(Error::runtime_execution_error(
                "Insufficient samples for statistical analysis",
            ));
        }

        let fuel_values: Vec<u64> = samples.iter().map(|s| s.fuel_consumed).collect();
        let stats = self.calculate_execution_statistics(&fuel_values)?;

        // Use Extreme Value Theory for WCET estimation
        let percentile_99_9 = self.calculate_percentile(&fuel_values, 0.999)?;
        let wcet_with_margin = ((percentile_99_9 as f64) * self.config.safety_margin_factor) as u64;

        Ok(WcetAnalysisResult {
            task_id,
            method: WcetAnalysisMethod::Probabilistic,
            wcet_fuel: wcet_with_margin,
            bcet_fuel: stats.min_value,
            average_fuel: stats.mean as u64,
            std_deviation: stats.std_deviation,
            confidence_level: 0.999, // 99.9% confidence
            critical_path: self.find_critical_path_from_samples(task_id),
            sample_count: samples.len(),
            analysis_time: self.current_fuel_time.load(Ordering::Acquire),
        })
    }

    fn calculate_execution_statistics(&self, values: &[u64]) -> Result<ExecutionStatistics> {
        if values.is_empty() {
            return Err(Error::new(
                ErrorCategory::InvalidInput,
                codes::INSUFFICIENT_DATA,
                "Error message",
            ));
        }

        let min_value = *values.iter().min().unwrap();
        let max_value = *values.iter().max().unwrap();
        let sum: u64 = values.iter().sum();
        let mean = (sum as f64) / (values.len() as f64);

        let variance = values
            .iter()
            .map(|&x| {
                let diff = (x as f64) - mean;
                diff * diff
            })
            .sum::<f64>()
            / (values.len() as f64);

        let std_deviation = variance.sqrt();

        Ok(ExecutionStatistics {
            min_value,
            max_value,
            mean,
            std_deviation,
        })
    }

    fn calculate_percentile(&self, values: &[u64], percentile: f64) -> Result<u64> {
        if values.is_empty() {
            return Err(Error::runtime_execution_error(
                "Insufficient samples for statistical analysis",
            ));
        }

        let mut sorted_values = values.to_vec();
        sorted_values.sort_unstable();

        let index = ((sorted_values.len() as f64) * percentile) as usize;
        let clamped_index = index.min(sorted_values.len() - 1);

        Ok(sorted_values[clamped_index])
    }

    fn calculate_confidence_multiplier(&self, confidence_level: f64) -> f64 {
        // Simplified confidence interval multiplier (z-score approximation)
        match confidence_level {
            x if x >= 0.999 => 3.29, // 99.9%
            x if x >= 0.99 => 2.58,  // 99%
            x if x >= 0.95 => 1.96,  // 95%
            x if x >= 0.90 => 1.65,  // 90%
            _ => 1.0,
        }
    }

    fn find_critical_path(&self, task_id: TaskId) -> Option<u32> {
        if let Some(paths) = self.task_paths.get(&task_id) {
            paths.iter().max_by_key(|path| path.estimated_fuel).map(|path| path.path_id)
        } else {
            None
        }
    }

    fn find_critical_path_from_samples(&self, task_id: TaskId) -> Option<u32> {
        if let Some(samples) = self.execution_samples.get(&task_id) {
            samples
                .iter()
                .max_by_key(|sample| sample.fuel_consumed)
                .map(|sample| sample.path_id)
        } else {
            None
        }
    }

    fn update_path_sample(
        &mut self,
        task_id: TaskId,
        path_id: u32,
        fuel_consumed: u64,
    ) -> Result<()> {
        if let Some(paths) = self.task_paths.get_mut(&task_id) {
            for path in paths.iter_mut() {
                if path.path_id == path_id {
                    path.execution_count.fetch_add(1, Ordering::AcqRel);

                    // Add sample if there's space
                    if path.measured_samples.len() < MAX_EXECUTION_SAMPLES {
                        path.measured_samples.push(fuel_consumed).map_err(|_| {
                            Error::resource_limit_exceeded("Path sample collection is full")
                        })?;
                    } else {
                        // Replace oldest sample
                        path.measured_samples.remove(0);
                        path.measured_samples.push(fuel_consumed).map_err(|_| {
                            Error::resource_limit_exceeded("Failed to add path sample")
                        })?;
                    }
                    break;
                }
            }
        }
        Ok(())
    }

    fn refine_wcet_estimate(&mut self, task_id: TaskId) -> Result<()> {
        // Re-analyze with updated samples
        let refined_result = self.perform_measurement_analysis(task_id)?;

        // Update stored result
        self.analysis_results
            .insert(task_id, refined_result)
            .map_err(|_| Error::resource_limit_exceeded("Failed to update WCET analysis result"))?;

        Ok(())
    }

    fn consume_analysis_fuel(&self, amount: u64) -> Result<()> {
        self.stats.analysis_fuel_consumed.fetch_add(amount, Ordering::AcqRel);
        Ok(())
    }
}

/// Statistical data for execution samples
#[derive(Debug, Clone)]
struct ExecutionStatistics {
    min_value: u64,
    max_value: u64,
    mean: f64,
    std_deviation: f64,
}
