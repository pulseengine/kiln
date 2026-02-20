use crate::{
    canonical_options::CanonicalOptions,
    execution_engine::{ComponentExecutionEngine, ExecutionContext, ExecutionState},
    post_return::{CleanupTask, CleanupTaskType, PostReturnRegistry},
    ComponentInstanceId, ResourceHandle, ValType,
};
use core::{fmt, time::Duration};
use wrt_foundation::{
    BoundedVec,
    bounded_collections::BoundedMap,
    component_value::ComponentValue,
    safe_memory::{SafeMemory, NoStdProvider},
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

const MAX_START_FUNCTION_VALIDATIONS: usize = 256;
const MAX_START_FUNCTION_EXPORTS: usize = 64;
const MAX_START_FUNCTION_PARAMS: usize = 16;
const DEFAULT_START_TIMEOUT_MS: u64 = 5000;

#[derive(Debug, Clone, PartialEq)]
pub struct StartFunctionError {
    pub kind: StartFunctionErrorKind,
    pub message: String,
    pub component_id: Option<ComponentInstanceId>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StartFunctionErrorKind {
    StartFunctionNotFound,
    InvalidSignature,
    ValidationFailed,
    ExecutionTimeout,
    ExecutionFailed,
    ResourceLimitExceeded,
    DependencyNotMet,
    InitializationFailed,
}

impl fmt::Display for StartFunctionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StartFunctionError {}

pub type StartFunctionResult<T> = Result<T, StartFunctionError>;

#[derive(Debug, Clone)]
pub struct StartFunctionDescriptor {
    pub name: String,
    pub parameters: BoundedVec<StartFunctionParam, MAX_START_FUNCTION_PARAMS>,
    pub return_type: Option<ValType>,
    pub required: bool,
    pub timeout_ms: u64,
    pub validation_level: ValidationLevel,
    pub dependencies: BoundedVec<String, MAX_START_FUNCTION_EXPORTS>,
}

#[derive(Debug, Clone)]
pub struct StartFunctionParam {
    pub name: String,
    pub param_type: ValType,
    pub required: bool,
    pub default_value: Option<ComponentValue>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValidationLevel {
    None,
    Basic,
    Standard,
    Strict,
    Complete,
}

#[derive(Debug, Clone)]
pub struct StartFunctionValidation {
    pub component_id: ComponentInstanceId,
    pub descriptor: StartFunctionDescriptor,
    pub validation_state: ValidationState,
    pub execution_result: Option<StartFunctionExecutionResult>,
    pub validated_at: u64,
    pub validation_duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValidationState {
    Pending,
    InProgress,
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct StartFunctionExecutionResult {
    pub success: bool,
    pub return_value: Option<ComponentValue>,
    pub execution_time_ms: u64,
    pub memory_usage: usize,
    pub error_message: Option<String>,
    pub side_effects: BoundedVec<SideEffect, 32>,
}

#[derive(Debug, Clone)]
pub struct SideEffect {
    pub effect_type: SideEffectType,
    pub description: String,
    pub severity: SideEffectSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SideEffectType {
    MemoryAllocation,
    ResourceCreation,
    ExternalCall,
    StateModification,
    FileSystemAccess,
    NetworkAccess,
    TimeAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum SideEffectSeverity {
    Info = 0,
    Warning = 1,
    Error = 2,
    Critical = 3,
}

pub struct StartFunctionValidator {
    validations: BoundedMap<
        ComponentInstanceId,
        StartFunctionValidation,
        MAX_START_FUNCTION_VALIDATIONS,
    >,
    execution_engine: ComponentExecutionEngine,
    post_return_registry: PostReturnRegistry,
    default_timeout_ms: u64,
    default_validation_level: ValidationLevel,
    strict_mode: bool,
}

impl StartFunctionValidator {
    pub fn new() -> Self {
        Self {
            validations: BoundedMap::new(),
            execution_engine: ComponentExecutionEngine::new(),
            post_return_registry: PostReturnRegistry::new(),
            default_timeout_ms: DEFAULT_START_TIMEOUT_MS,
            default_validation_level: ValidationLevel::Standard,
            strict_mode: false,
        }
    }

    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = strict;
        self
    }

    pub fn with_default_timeout(mut self, timeout_ms: u64) -> Self {
        self.default_timeout_ms = timeout_ms;
        self
    }

    pub fn with_default_validation_level(mut self, level: ValidationLevel) -> Self {
        self.default_validation_level = level;
        self
    }

    pub fn register_start_function(
        &mut self,
        component_id: ComponentInstanceId,
        descriptor: StartFunctionDescriptor,
    ) -> StartFunctionResult<()> {
        self.validate_descriptor(&descriptor)?;

        let validation = StartFunctionValidation {
            component_id,
            descriptor,
            validation_state: ValidationState::Pending,
            execution_result: None,
            validated_at: 0,
            validation_duration_ms: 0,
        };

        self.validations.insert(component_id, validation).map_err(|_| StartFunctionError {
            kind: StartFunctionErrorKind::ResourceLimitExceeded,
            message: "Too many start function validations".to_owned(),
            component_id: Some(component_id),
        })?;

        Ok(()
    }

    pub fn validate_start_function(
        &mut self,
        component_id: ComponentInstanceId,
    ) -> StartFunctionResult<ValidationState> {
        let validation =
            self.validations.get_mut(&component_id).ok_or_else(|| StartFunctionError {
                kind: StartFunctionErrorKind::StartFunctionNotFound,
                message: "No start function registered for component".to_owned(),
                component_id: Some(component_id),
            })?;

        if validation.validation_state != ValidationState::Pending {
            return Ok(validation.validation_state;
        }

        validation.validation_state = ValidationState::InProgress;
        let start_time = self.get_current_time);

        let result = self.perform_validation(component_id, &validation.descriptor;

        let end_time = self.get_current_time);
        let duration = end_time.saturating_sub(start_time;

        match result {
            Ok(execution_result) => {
                validation.validation_state = if execution_result.success {
                    ValidationState::Passed
                } else {
                    ValidationState::Failed
                };
                validation.execution_result = Some(execution_result;
            }
            Err(_) => {
                validation.validation_state = ValidationState::Failed;
            }
        }

        validation.validated_at = end_time;
        validation.validation_duration_ms = duration;

        Ok(validation.validation_state)
    }

    pub fn validate_all_pending(
        &mut self,
    ) -> StartFunctionResult<
        BoundedVec<(ComponentInstanceId, ValidationState), MAX_START_FUNCTION_VALIDATIONS>,
    > {
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let mut results = BoundedVec::new();

        let pending_components: Vec<ComponentInstanceId> = self
            .validations
            .iter()
            .filter(|(_, v)| v.validation_state == ValidationState::Pending)
            .map(|(id, _)| *id)
            .collect();

        for component_id in pending_components {
            let state = self.validate_start_function(component_id)?;
            results.push((component_id, state)).map_err(|_| StartFunctionError {
                kind: StartFunctionErrorKind::ResourceLimitExceeded,
                message: "Too many validation results".to_owned(),
                component_id: Some(component_id),
            })?;
        }

        Ok(results)
    }

    pub fn get_validation_result(
        &self,
        component_id: ComponentInstanceId,
    ) -> Option<&StartFunctionValidation> {
        self.validations.get(&component_id)
    }

    pub fn get_validation_summary(&self) -> ValidationSummary {
        let mut summary = ValidationSummary::default());

        for validation in self.validations.values() {
            summary.total += 1;
            match validation.validation_state {
                ValidationState::Pending => summary.pending += 1,
                ValidationState::InProgress => summary.in_progress += 1,
                ValidationState::Passed => summary.passed += 1,
                ValidationState::Failed => summary.failed += 1,
                ValidationState::Skipped => summary.skipped += 1,
            }

            if let Some(result) = validation.execution_result {
                summary.total_execution_time_ms += result.execution_time_ms;
                summary.total_memory_usage += result.memory_usage;
            }
        }

        summary
    }

    pub fn reset_validation(
        &mut self,
        component_id: ComponentInstanceId,
    ) -> StartFunctionResult<()> {
        if let Some(validation) = self.validations.get_mut(&component_id) {
            validation.validation_state = ValidationState::Pending;
            validation.execution_result = None;
            validation.validated_at = 0;
            validation.validation_duration_ms = 0;
            Ok(()
        } else {
            Err(StartFunctionError {
                kind: StartFunctionErrorKind::StartFunctionNotFound,
                message: "No validation found for component".to_owned(),
                component_id: Some(component_id),
            })
        }
    }

    pub fn remove_validation(
        &mut self,
        component_id: ComponentInstanceId,
    ) -> StartFunctionResult<()> {
        self.validations.remove(&component_id;
        Ok(()
    }

    fn validate_descriptor(&self, descriptor: &StartFunctionDescriptor) -> StartFunctionResult<()> {
        if descriptor.name.is_empty() {
            return Err(StartFunctionError {
                kind: StartFunctionErrorKind::InvalidSignature,
                message: "Start function name cannot be empty".to_owned(),
                component_id: None,
            };
        }

        if descriptor.timeout_ms == 0 {
            return Err(StartFunctionError {
                kind: StartFunctionErrorKind::InvalidSignature,
                message: "Timeout must be greater than zero".to_owned(),
                component_id: None,
            };
        }

        // Validate parameter types
        for param in descriptor.parameters.iter() {
            if param.name.is_empty() {
                return Err(StartFunctionError {
                    kind: StartFunctionErrorKind::InvalidSignature,
                    message: "Parameter name cannot be empty".to_owned(),
                    component_id: None,
                };
            }

            if param.required && param.default_value.is_some() {
                return Err(StartFunctionError {
                    kind: StartFunctionErrorKind::InvalidSignature,
                    message: "Required parameters cannot have default values".to_owned(),
                    component_id: None,
                };
            }
        }

        Ok(()
    }

    fn perform_validation(
        &mut self,
        component_id: ComponentInstanceId,
        descriptor: &StartFunctionDescriptor,
    ) -> StartFunctionResult<StartFunctionExecutionResult> {
        let start_time = self.get_current_time);

        // Check dependencies first
        self.validate_dependencies(component_id, descriptor)?;

        // Prepare execution context
        let mut execution_context = ExecutionContext::new(component_id;

        // Prepare arguments
        let arguments = self.prepare_arguments(descriptor)?;

        // Execute the start function
        let execution_result = self.execute_start_function(
            component_id,
            &descriptor.name,
            &arguments,
            descriptor.timeout_ms,
        )?;

        let end_time = self.get_current_time);
        let execution_time = end_time.saturating_sub(start_time;

        // Analyze side effects
        let side_effects = self.analyze_side_effects(&execution_context)?;

        // Validate result based on validation level
        let success = self.validate_execution_result(
            &execution_result,
            descriptor.validation_level,
            &side_effects,
        )?;

        Ok(StartFunctionExecutionResult {
            success,
            return_value: execution_result,
            execution_time_ms: execution_time,
            memory_usage: execution_context.memory_usage(),
            error_message: None,
            side_effects,
        })
    }

    fn validate_dependencies(
        &self,
        component_id: ComponentInstanceId,
        descriptor: &StartFunctionDescriptor,
    ) -> StartFunctionResult<()> {
        for dependency in descriptor.dependencies.iter() {
            if !self.check_dependency_available(component_id, dependency) {
                return Err(StartFunctionError {
                    kind: StartFunctionErrorKind::DependencyNotMet,
                    message: format!("Dependency '{}' not available for component", dependency),
                    component_id: Some(component_id),
                };
            }
        }
        Ok(()
    }

    fn prepare_arguments(
        &self,
        descriptor: &StartFunctionDescriptor,
    ) -> StartFunctionResult<BoundedVec<ComponentValue<ComponentProvider>, MAX_START_FUNCTION_PARAMS>> {
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let mut arguments = BoundedVec::new();

        for param in descriptor.parameters.iter() {
            let value = if let Some(default) = param.default_value {
                default.clone()
            } else if param.required {
                return Err(StartFunctionError {
                    kind: StartFunctionErrorKind::ValidationFailed,
                    message: format!("Required parameter '{}' has no value", param.name),
                    component_id: None,
                };
            } else {
                self.get_default_value_for_type(&param.param_type)
            };

            arguments.push(value).map_err(|_| StartFunctionError {
                kind: StartFunctionErrorKind::ResourceLimitExceeded,
                message: "Too many start function parameters".to_owned(),
                component_id: None,
            })?;
        }

        Ok(arguments)
    }

    fn execute_start_function(
        &mut self,
        component_id: ComponentInstanceId,
        function_name: &str,
        arguments: &[ComponentValue],
        timeout_ms: u64,
    ) -> StartFunctionResult<Option<ComponentValue>> {
        // Create execution state
        let mut execution_state = ExecutionState::new();
        execution_state.set_timeout(Duration::from_millis(timeout_ms;

        // Execute through the execution engine
        match self.execution_engine.call_function(
            component_id,
            function_name,
            arguments,
            &mut execution_state,
        ) {
            Ok(result) => Ok(result),
            Err(e) => Err(StartFunctionError {
                kind: StartFunctionErrorKind::ExecutionFailed,
                message: format!("Failed to execute start function '{}'", function_name),
                component_id: Some(component_id),
            }),
        }
    }

    fn analyze_side_effects(
        &self,
        execution_context: &ExecutionContext,
    ) -> StartFunctionResult<BoundedVec<SideEffect, 32>> {
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let mut side_effects = BoundedVec::new();

        // Binary std/no_std choice
        if execution_context.memory_allocations() > 0 {
            let effect = SideEffect {
                effect_type: SideEffectType::MemoryAllocation,
                description: "Memory allocated during start function execution".to_owned(),
                severity: if execution_context.memory_usage() > 1024 * 1024 {
                    SideEffectSeverity::Warning
                } else {
                    SideEffectSeverity::Info
                },
            };
            side_effects.push(effect).map_err(|_| StartFunctionError {
                kind: StartFunctionErrorKind::ResourceLimitExceeded,
                message: "Too many side effects".to_owned(),
                component_id: None,
            })?;
        }

        // Analyze resource creations
        if execution_context.resources_created() > 0 {
            let effect = SideEffect {
                effect_type: SideEffectType::ResourceCreation,
                description: "Resources created during start function execution".to_owned(),
                severity: SideEffectSeverity::Info,
            };
            side_effects.push(effect).map_err(|_| StartFunctionError {
                kind: StartFunctionErrorKind::ResourceLimitExceeded,
                message: "Too many side effects".to_owned(),
                component_id: None,
            })?;
        }

        Ok(side_effects)
    }

    fn validate_execution_result(
        &self,
        result: &Option<ComponentValue>,
        validation_level: ValidationLevel,
        side_effects: &[SideEffect],
    ) -> StartFunctionResult<bool> {
        match validation_level {
            ValidationLevel::None => Ok(true),
            ValidationLevel::Basic => {
                // Just check if execution completed
                Ok(true)
            }
            ValidationLevel::Standard => {
                // Check for critical side effects
                let has_critical = side_effects
                    .iter()
                    .any(|effect| effect.severity == SideEffectSeverity::Critical;
                Ok(!has_critical)
            }
            ValidationLevel::Strict => {
                // Check for any error-level side effects
                let has_errors =
                    side_effects.iter().any(|effect| effect.severity >= SideEffectSeverity::Error;
                Ok(!has_errors)
            }
            ValidationLevel::Complete => {
                // Check for any warnings or above
                let has_warnings = side_effects
                    .iter()
                    .any(|effect| effect.severity >= SideEffectSeverity::Warning;
                Ok(!has_warnings && result.is_some()
            }
        }
    }

    fn check_dependency_available(
        &self,
        _component_id: ComponentInstanceId,
        _dependency: &str,
    ) -> bool {
        // For now, assume all dependencies are available
        // In a real implementation, this would check if the dependency is actually available
        true
    }

    fn get_default_value_for_type(&self, val_type: &ValType) -> ComponentValue {
        match val_type {
            ValType::I32 => ComponentValue::I32(0),
            ValType::I64 => ComponentValue::I64(0),
            ValType::F32 => ComponentValue::F32(0.0),
            ValType::F64 => ComponentValue::F64(0.0),
            ValType::String => ComponentValue::String(String::new()),
            ValType::Bool => ComponentValue::Bool(false),
            _ => ComponentValue::I32(0), // Default fallback
        }
    }

    fn get_current_time(&self) -> u64 {
        #[cfg(feature = "std")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
        }
        #[cfg(not(feature = "std"))]
        {
            0
        }
    }
}

impl Default for StartFunctionValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ValidationSummary {
    pub total: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub total_execution_time_ms: u64,
    pub total_memory_usage: usize,
}

pub fn create_start_function_descriptor(name: &str) -> StartFunctionResult<StartFunctionDescriptor> {
    let parameters_provider = safe_managed_alloc!(65536, CrateId::Component)?;
    let parameters = BoundedVec::new(parameters_provider).map_err(|_| StartFunctionError {
        kind: StartFunctionErrorKind::ResourceLimitExceeded,
        message: "Failed to create parameters vector".to_owned(),
        component_id: None,
    })?;
    
    let dependencies_provider = safe_managed_alloc!(65536, CrateId::Component)?;
    let dependencies = BoundedVec::new(dependencies_provider).map_err(|_| StartFunctionError {
        kind: StartFunctionErrorKind::ResourceLimitExceeded,
        message: "Failed to create dependencies vector".to_owned(),
        component_id: None,
    })?;
    
    Ok(StartFunctionDescriptor {
        name: name.to_string(),
        parameters,
        return_type: None,
        required: true,
        timeout_ms: DEFAULT_START_TIMEOUT_MS,
        validation_level: ValidationLevel::Standard,
        dependencies,
    })
}

pub fn create_start_function_param(name: &str, param_type: ValType) -> StartFunctionParam {
    StartFunctionParam { name: name.to_string(), param_type, required: false, default_value: None }
}

}
