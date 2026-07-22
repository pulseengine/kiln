//! Engine builder API for creating capability-aware engines with resource
//! limits
//!
//! This module provides a fluent builder interface for creating WebAssembly
//! engines with proper ASIL-level configuration and resource limits.

use kiln_decoder::resource_limits_section::extract_resource_limits_from_binary;
use kiln_error::{Error, Result};
use kiln_foundation::{
    capabilities::MemoryCapabilityContext,
    execution::ASILExecutionMode,
};

use crate::engine::{
    CapabilityAwareEngine,
    EnginePreset,
};

/// Builder for creating capability-aware WebAssembly engines
#[derive(Debug)]
pub struct EngineBuilder {
    /// Target ASIL level for the engine
    asil_level:     Option<ASILExecutionMode>,
    /// Engine preset (overrides ASIL level if set)
    preset:         Option<EnginePreset>,
    /// Custom capability context (overrides both ASIL level and preset)
    custom_context: Option<MemoryCapabilityContext>,
}

impl EngineBuilder {
    /// Create a new engine builder
    pub fn new() -> Self {
        Self {
            asil_level:     None,
            preset:         None,
            custom_context: None,
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

    /// Create an engine builder from a WebAssembly binary, selecting the ASIL
    /// level from the binary's `kiln.resource_limits` manifest (SR-45).
    ///
    /// - Manifest present with a qualified ASIL level: that level is selected.
    /// - Manifest present without a qualified level: QM (the manifest's
    ///   numeric limits are enforced separately by
    ///   `CapabilityAwareEngine::load_module`, regardless of level).
    /// - Manifest absent: QM.
    /// - Manifest present but malformed, or an unknown qualified level:
    ///   `Err` — never silently downgraded (fail loud).
    pub fn from_binary(binary: &[u8]) -> Result<Self> {
        let Some(section) = extract_resource_limits_from_binary(binary)? else {
            return Ok(Self::qm());
        };
        let Some(level) = section.qualified_asil_level() else {
            return Ok(Self::qm());
        };
        Ok(Self::new().with_asil_level(parse_qualified_asil_level(level)?))
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

/// Map a manifest's qualified ASIL level string to an execution mode.
///
/// Whitespace is trimmed and matching is ASCII-case-insensitive (section
/// authors have historically emitted levels like `"ASIL-D "`). An unknown
/// level is an error — a signed manifest claiming a qualification the runtime
/// does not recognize must not be silently reinterpreted (SR-45, fail loud).
fn parse_qualified_asil_level(level: &str) -> Result<ASILExecutionMode> {
    let normalized = level.trim();
    for (name, mode) in [
        ("QM", ASILExecutionMode::QM),
        ("ASIL-A", ASILExecutionMode::AsilA),
        ("ASIL-B", ASILExecutionMode::AsilB),
        ("ASIL-C", ASILExecutionMode::AsilC),
        ("ASIL-D", ASILExecutionMode::AsilD),
    ] {
        if normalized.eq_ignore_ascii_case(name) {
            return Ok(mode);
        }
    }
    Err(Error::parse_error(
        "Unknown qualified ASIL level in kiln.resource_limits manifest",
    ))
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}
