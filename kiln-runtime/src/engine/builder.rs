//! Engine builder API for creating capability-aware engines with resource
//! limits
//!
//! This module provides a fluent builder interface for creating WebAssembly
//! engines with proper ASIL-level configuration and resource limits.

use wrt_error::Result;
use wrt_foundation::{
    capabilities::MemoryCapabilityContext,
    execution::{
        extract_resource_limits_from_binary,
        ASILExecutionConfig,
        ASILExecutionMode,
    },
};

use crate::engine::{
    CapabilityAwareEngine,
    EnginePreset,
};

/// Builder for creating capability-aware WebAssembly engines
#[derive(Debug)]
pub struct EngineBuilder {
    /// Target ASIL level for the engine
    asil_level:      Option<ASILExecutionMode>,
    /// Engine preset (overrides ASIL level if set)
    preset:          Option<EnginePreset>,
    /// Custom capability context (overrides both ASIL level and preset)
    custom_context:  Option<MemoryCapabilityContext>,
    /// Resource limits configuration from binary
    resource_config: Option<ASILExecutionConfig>,
}

impl EngineBuilder {
    /// Create a new engine builder
    pub fn new() -> Self {
        Self {
            asil_level:      None,
            preset:          None,
            custom_context:  None,
            resource_config: None,
        }
    }

    /// Set the target ASIL level for the engine
    pub fn with_asil_level(mut self, level: ASILExecutionMode) -> Self {
        self.asil_level = Some(level);
        self
    }

    /// Set the engine preset (overrides ASIL level)
    pub fn with_preset(mut self, preset: EnginePreset) -> Self {
        self.preset = Some(preset);
        self
    }

    /// Set a custom capability context (overrides all other settings)
    pub fn with_custom_context(mut self, context: MemoryCapabilityContext) -> Self {
        self.custom_context = Some(context);
        self
    }

    /// Set resource limits configuration from a WebAssembly binary
    pub fn with_resource_config(mut self, config: ASILExecutionConfig) -> Self {
        self.resource_config = Some(config);
        self
    }

    /// Create an engine for QM (Quality Management) level
    pub fn qm() -> Self {
        Self::new().with_preset(EnginePreset::QM)
    }

    /// Create an engine for ASIL-A level
    pub fn asil_a() -> Self {
        Self::new().with_preset(EnginePreset::AsilA)
    }

    /// Create an engine for ASIL-B level
    pub fn asil_b() -> Self {
        Self::new().with_preset(EnginePreset::AsilB)
    }

    /// Create an engine for ASIL-C level
    pub fn asil_c() -> Self {
        Self::new().with_preset(EnginePreset::AsilC)
    }

    /// Create an engine for ASIL-D level
    pub fn asil_d() -> Self {
        Self::new().with_preset(EnginePreset::AsilD)
    }

    /// Create an engine from a WebAssembly binary with embedded resource limits
    pub fn from_binary(binary: &[u8]) -> Result<Self> {
        // Function is now imported at the top

        // Try to extract resource limits from the binary
        // Start with ASIL-D for maximum compatibility, then work down
        for asil_mode in &[
            ASILExecutionMode::AsilD,
            ASILExecutionMode::AsilC,
            ASILExecutionMode::AsilB,
            ASILExecutionMode::AsilA,
            ASILExecutionMode::QM,
        ] {
            if let Ok(Some(config)) = extract_resource_limits_from_binary(binary, *asil_mode) {
                return Ok(Self::new().with_asil_level(config.mode).with_resource_config(config));
            }
        }

        // No resource limits found, default to QM
        Ok(Self::qm())
    }

    /// Build the engine with the configured settings
    pub fn build(self) -> Result<CapabilityAwareEngine> {
        // Priority order: custom_context > preset > asil_level > default QM

        if let Some(context) = self.custom_context {
            let preset = self.preset.unwrap_or(EnginePreset::QM);
            return CapabilityAwareEngine::with_context_and_preset(context, preset);
        }

        if let Some(preset) = self.preset {
            return CapabilityAwareEngine::with_preset(preset);
        }

        if let Some(asil_level) = self.asil_level {
            let preset = match asil_level {
                ASILExecutionMode::QM => EnginePreset::QM,
                ASILExecutionMode::AsilA => EnginePreset::AsilA,
                ASILExecutionMode::AsilB => EnginePreset::AsilB,
                ASILExecutionMode::AsilC => EnginePreset::AsilC,
                ASILExecutionMode::AsilD => EnginePreset::AsilD,
            };
            return CapabilityAwareEngine::with_preset(preset);
        }

        // Default to QM
        CapabilityAwareEngine::with_preset(EnginePreset::QM)
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

