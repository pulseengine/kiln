// WRT - wrt-instructions
// Module: CFI-Aware Control Flow Operations
// SW-REQ-ID: REQ_CFI_CONTROL_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! CFI-Aware Control Flow Operations for WebAssembly Instructions
//!
//! This module extends the basic control flow operations with Control Flow
//! Integrity support, providing protection against ROP/JOP attacks through
//! hardware and software CFI mechanisms.
//!
//! # Key Features
//! - Landing pad validation for indirect calls
//! - Shadow stack management for returns
//! - Branch target verification
//! - Hardware-specific CFI instruction integration
//! - Software CFI fallback support

#![allow(dead_code)] // Allow during development
// CFI index operations may involve address-like values that need casting.
#![allow(clippy::cast_possible_truncation)]


use crate::{
    control_ops::BranchTarget,
    prelude::*,
    types::CfiTargetVec,
};

/// Calling convention used in CFI validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallingConvention {
    /// WebAssembly calling convention
    WebAssembly,
    /// System V ABI (Unix-like systems)
    SystemV,
    /// Windows Fastcall convention
    WindowsFastcall,
}

/// Software CFI configuration
#[derive(Debug, Clone)]
pub struct CfiSoftwareConfig {
    /// Enable shadow stack protection
    pub shadow_stack_enabled: bool,
    /// Enable temporal validation
    pub temporal_validation:  bool,
}

impl Default for CfiSoftwareConfig {
    fn default() -> Self {
        Self {
            shadow_stack_enabled: true,
            temporal_validation:  false,
        }
    }
}

/// CFI-enhanced control flow protection configuration
#[derive(Debug, Clone)]
pub struct CfiControlFlowProtection {
    /// Enable CFI protection
    pub enabled:          bool,
    /// CFI protection level
    pub protection_level: CfiProtectionLevel,
    /// Hardware CFI configuration
    pub hardware_config:  Option<HardwareCfiConfig>,
    /// Software CFI fallback configuration
    pub software_config:  SoftwareCfiConfig,
}

impl Default for CfiControlFlowProtection {
    fn default() -> Self {
        Self {
            enabled:          true,
            protection_level: CfiProtectionLevel::Enhanced,
            hardware_config:  None, // Will be detected at runtime
            software_config:  SoftwareCfiConfig::default(),
        }
    }
}

impl CfiControlFlowProtection {
    /// Create CFI protection with specific level
    #[must_use]
    pub fn new_with_level(level: CfiProtectionLevel) -> Self {
        let mut config = Self {
            protection_level: level,
            ..Default::default()
        };

        // Adjust software config based on protection level
        match level {
            CfiProtectionLevel::None => {
                config.enabled = false;
            },
            CfiProtectionLevel::Basic => {
                config.software_config.shadow_stack_enabled = false;
                config.software_config.temporal_validation = false;
            },
            CfiProtectionLevel::Enhanced => {
                config.software_config.shadow_stack_enabled = true;
                config.software_config.temporal_validation = false;
            },
            CfiProtectionLevel::Maximum => {
                config.software_config.shadow_stack_enabled = true;
                config.software_config.temporal_validation = true;
            },
        }

        config
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CfiProtectionLevel {
    /// No CFI protection
    None,
    /// Basic CFI (landing pads only)
    Basic,
    /// Enhanced CFI (shadow stack + landing pads)
    Enhanced,
    /// Maximum CFI (all protections + temporal validation)
    Maximum,
}

/// Hardware-specific CFI configuration
#[derive(Debug, Clone)]
pub struct HardwareCfiConfig {
    /// Architecture type
    pub architecture: CfiArchitecture,
    /// Hardware-specific settings
    pub settings:     HardwareCfiSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CfiArchitecture {
    /// ARM with BTI support
    ArmBti,
    /// RISC-V with CFI extension
    RiscVCfi,
    /// `x86_64` with CET support
    X86Cet,
}

#[derive(Debug, Clone)]
pub enum HardwareCfiSettings {
    /// ARM BTI settings
    ArmBti {
        /// BTI mode
        mode:            ArmBtiMode,
        /// Exception level
        exception_level: ArmBtiExceptionLevel,
    },
    /// RISC-V CFI settings
    RiscVCfi {
        /// Shadow stack enabled
        shadow_stack: bool,
        /// Landing pads enabled
        landing_pads: bool,
    },
    /// `x86_64` CET settings
    X86Cet {
        /// Shadow stack enabled
        shadow_stack:             bool,
        /// Indirect branch tracking
        indirect_branch_tracking: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmBtiMode {
    /// Standard BTI (bti)
    Standard,
    /// Call-specific BTI (bti c)
    CallOnly,
    /// Jump-specific BTI (bti j)
    JumpOnly,
    /// Both call and jump BTI (bti jc)
    CallAndJump,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmBtiExceptionLevel {
    /// User mode (EL0) only
    EL0,
    /// Kernel mode (EL1) only
    EL1,
    /// Both user and kernel modes
    Both,
}

/// Software CFI configuration
#[derive(Debug, Clone)]
pub struct SoftwareCfiConfig {
    /// Enable software shadow stack
    pub shadow_stack_enabled:        bool,
    /// Maximum shadow stack depth
    pub max_shadow_stack_depth:      usize,
    /// Enable landing pad simulation
    pub landing_pad_simulation:      bool,
    /// Enable temporal validation
    pub temporal_validation:         bool,
    /// Maximum function execution time (cycles)
    pub max_function_execution_time: u64,
}

impl Default for SoftwareCfiConfig {
    fn default() -> Self {
        Self {
            shadow_stack_enabled:        true,
            max_shadow_stack_depth:      1024,
            landing_pad_simulation:      true,
            temporal_validation:         false, // Expensive, off by default
            max_function_execution_time: 1_000_000, // 1M cycles
        }
    }
}

/// CFI-enhanced branch target with protection metadata
#[derive(Debug, Clone)]
pub struct CfiProtectedBranchTarget {
    /// Base branch target information
    pub target:     BranchTarget,
    /// CFI protection requirements
    pub protection: CfiTargetProtection,
    /// Validation requirements
    #[cfg(feature = "std")]
    pub validation: Vec<CfiValidationRequirement>,
    #[cfg(not(feature = "std"))]
    pub validation: crate::types::CfiRequirementVec,
}

/// CFI protection for a specific control flow target
#[derive(Debug, Clone)]
pub struct CfiTargetProtection {
    /// Target type for CFI classification
    pub target_type:              CfiTargetType,
    /// Required landing pad information
    pub landing_pad:              Option<CfiLandingPad>,
    /// Shadow stack requirements
    pub shadow_stack_requirement: ShadowStackRequirement,
    /// Temporal validation settings
    pub temporal_validation:      Option<TemporalValidation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CfiTargetType {
    /// Direct function call
    #[default]
    DirectCall,
    /// Indirect function call (via table)
    IndirectCall,
    /// Return from function
    Return,
    /// Branch within function
    Branch,
    /// Block/loop entry
    BlockEntry,
    /// Function entry point
    FunctionEntry,
}

impl wrt_foundation::traits::Checksummable for CfiTargetType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        let discriminant = match self {
            Self::DirectCall => 0u8,
            Self::IndirectCall => 1u8,
            Self::Return => 2u8,
            Self::Branch => 3u8,
            Self::BlockEntry => 4u8,
            Self::FunctionEntry => 5u8,
        };
        checksum.update_slice(&[discriminant]);
    }
}

impl wrt_foundation::traits::ToBytes for CfiTargetType {
    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        let discriminant = match self {
            Self::DirectCall => 0u8,
            Self::IndirectCall => 1u8,
            Self::Return => 2u8,
            Self::Branch => 3u8,
            Self::BlockEntry => 4u8,
            Self::FunctionEntry => 5u8,
        };
        writer.write_u8(discriminant)
    }
}

impl wrt_foundation::traits::FromBytes for CfiTargetType {
    fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(Self::DirectCall),
            1 => Ok(Self::IndirectCall),
            2 => Ok(Self::Return),
            3 => Ok(Self::Branch),
            4 => Ok(Self::BlockEntry),
            5 => Ok(Self::FunctionEntry),
            _ => Err(Error::validation_error(
                "Invalid discriminant for CfiTargetType",
            )),
        }
    }
}

/// CFI landing pad information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfiLandingPad {
    /// Landing pad identifier
    pub pad_id:               u32,
    /// Hardware instruction to insert
    pub hardware_instruction: Option<CfiHardwareInstruction>,
    /// Software validation code
    pub software_validation:  Option<CfiSoftwareValidation>,
    /// Expected predecessor types
    #[cfg(feature = "std")]
    pub valid_predecessors:   Vec<CfiTargetType>,
    #[cfg(not(feature = "std"))]
    pub valid_predecessors:   crate::types::CfiTargetTypeVec,
}

impl CfiLandingPad {
    /// Create a default landing pad
    ///
    /// # Errors
    ///
    /// Returns an error if allocation fails
    pub fn try_default() -> Result<Self> {
        Ok(Self {
            pad_id:               0,
            hardware_instruction: None,
            software_validation:  None,
            valid_predecessors:   {
                #[cfg(feature = "std")]
                {
                    Vec::new()
                }
                #[cfg(not(feature = "std"))]
                {
                    let provider = safe_managed_alloc!(8192, CrateId::Instructions)?;
                    crate::types::CfiTargetTypeVec::new(provider)
                        .map_err(|_| Error::memory_error("Failed to create CfiTargetTypeVec"))?
                }
            },
        })
    }
}

impl Default for CfiLandingPad {
    fn default() -> Self {
        Self::try_default().unwrap_or_else(|_| {
            #[cfg(feature = "std")]
            {
                // Fallback for Default trait compatibility
                Self {
                    pad_id:               0,
                    hardware_instruction: None,
                    software_validation:  None,
                    valid_predecessors:   Vec::new(),
                }
            }
            #[cfg(not(feature = "std"))]
            {
                // Use minimal fallback - this will panic if used, but needed for trait compat
                panic!(
                    "CfiLandingPad::default() fallback should not be used in no_std - use \
                     try_default() instead"
                )
            }
        })
    }
}

/// Hardware-specific CFI instructions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfiHardwareInstruction {
    /// ARM BTI instruction
    #[cfg(target_arch = "aarch64")]
    ArmBti { mode: ArmBtiMode },
    /// RISC-V landing pad instruction
    #[cfg(target_arch = "riscv64")]
    RiscVLandingPad { label: u32 },
    /// x86_64 CET instruction
    #[cfg(target_arch = "x86_64")]
    X86Endbr,
}

/// Software CFI validation code
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfiSoftwareValidation {
    /// Validation check ID
    pub check_id:            u32,
    /// Expected values to validate
    #[cfg(feature = "std")]
    pub expected_values:     Vec<CfiExpectedValue>,
    #[cfg(not(feature = "std"))]
    pub expected_values:     crate::types::CfiExpectedValueVec,
    /// Validation function
    pub validation_function: SoftwareCfiFunction,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CfiExpectedValue {
    /// No expected value (default)
    #[default]
    None,
    /// Expected function signature hash
    FunctionSignatureHash(u64),
    /// Expected return address
    ReturnAddress(u64),
    /// Expected call site identifier
    CallSiteId(u32),
}

impl wrt_foundation::traits::Checksummable for CfiExpectedValue {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            Self::None => checksum.update_slice(&[0u8]),
            Self::FunctionSignatureHash(hash) => {
                checksum.update_slice(&[1u8]);
                let () = checksum.update_slice(&hash.to_le_bytes());
            },
            Self::ReturnAddress(addr) => {
                checksum.update_slice(&[2u8]);
                let () = checksum.update_slice(&addr.to_le_bytes());
            },
            Self::CallSiteId(id) => {
                checksum.update_slice(&[3u8]);
                let () = checksum.update_slice(&id.to_le_bytes());
            },
        }
    }
}

impl wrt_foundation::traits::ToBytes for CfiExpectedValue {
    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        match self {
            Self::None => writer.write_u8(0u8),
            Self::FunctionSignatureHash(hash) => {
                writer.write_u8(1u8)?;
                writer.write_all(&hash.to_le_bytes())
            },
            Self::ReturnAddress(addr) => {
                writer.write_u8(2u8)?;
                writer.write_all(&addr.to_le_bytes())
            },
            Self::CallSiteId(id) => {
                writer.write_u8(3u8)?;
                writer.write_all(&id.to_le_bytes())
            },
        }
    }
}

impl wrt_foundation::traits::FromBytes for CfiExpectedValue {
    fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(Self::None),
            1 => {
                let mut hash_bytes = [0u8; 8];
                reader.read_exact(&mut hash_bytes)?;
                let hash = u64::from_le_bytes(hash_bytes);
                Ok(Self::FunctionSignatureHash(hash))
            },
            2 => {
                let mut addr_bytes = [0u8; 8];
                reader.read_exact(&mut addr_bytes)?;
                let addr = u64::from_le_bytes(addr_bytes);
                Ok(Self::ReturnAddress(addr))
            },
            3 => {
                let mut id_bytes = [0u8; 4];
                reader.read_exact(&mut id_bytes)?;
                let id = u32::from_le_bytes(id_bytes);
                Ok(Self::CallSiteId(id))
            },
            _ => Err(Error::validation_error(
                "Invalid discriminant for CfiExpectedValue",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftwareCfiFunction {
    /// Validate function signature
    ValidateSignature,
    /// Validate return address
    ValidateReturnAddress,
    /// Validate call site
    ValidateCallSite,
    /// Validate branch target
    ValidateBranchTarget,
}

/// Shadow stack requirements
#[derive(Debug, Clone)]
pub enum ShadowStackRequirement {
    /// No shadow stack requirement
    None,
    /// Push return address to shadow stack
    Push {
        return_address:     u64,
        function_signature: u64,
    },
    /// Pop and validate return address from shadow stack
    PopAndValidate { expected_address: u64 },
    /// Check shadow stack without modifying
    Check,
}

/// Temporal validation for CFI
#[derive(Debug, Clone)]
pub struct TemporalValidation {
    /// Maximum execution time allowed
    pub max_execution_time: u64,
    /// Timestamp when execution started
    pub start_timestamp:    Option<u64>,
    /// Deadline for completion
    pub deadline:           Option<u64>,
}

/// CFI validation requirements
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CfiValidationRequirement {
    /// Validate function signature matches expected
    TypeSignatureCheck {
        expected_type_index: u32,
        signature_hash:      u64,
    },
    /// Validate return address matches shadow stack
    #[default]
    ShadowStackCheck,
    /// Validate control flow target is valid
    ControlFlowTargetCheck { valid_targets: CfiTargetVec },
    /// Validate calling convention
    CallingConventionCheck,
    /// Validate temporal properties
    TemporalCheck { max_duration: u64 },
}

impl wrt_foundation::traits::Checksummable for CfiValidationRequirement {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            Self::TypeSignatureCheck {
                expected_type_index,
                signature_hash,
            } => {
                checksum.update_slice(&[0u8]);
                checksum.update_slice(&expected_type_index.to_le_bytes());
                checksum.update_slice(&signature_hash.to_le_bytes());
            },
            Self::ShadowStackCheck => checksum.update_slice(&[1u8]),
            Self::ControlFlowTargetCheck { valid_targets } => {
                checksum.update_slice(&[2u8]);
                for target in valid_targets {
                    checksum.update_slice(&target.to_le_bytes());
                }
            },
            Self::CallingConventionCheck => checksum.update_slice(&[3u8]),
            Self::TemporalCheck { max_duration } => {
                checksum.update_slice(&[4u8]);
                let () = checksum.update_slice(&max_duration.to_le_bytes());
            },
        }
    }
}

impl wrt_foundation::traits::ToBytes for CfiValidationRequirement {
    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        match self {
            Self::TypeSignatureCheck {
                expected_type_index,
                signature_hash,
            } => {
                writer.write_u8(0u8)?;
                writer.write_all(&expected_type_index.to_le_bytes())?;
                writer.write_all(&signature_hash.to_le_bytes())?;
                Ok(())
            },
            Self::ShadowStackCheck => writer.write_u8(1u8),
            Self::ControlFlowTargetCheck { valid_targets } => {
                writer.write_u8(2u8)?;
                // Serialize Vec<u32> manually
                #[cfg(feature = "std")]
                {
                    writer.write_u32_le(valid_targets.len() as u32)?;
                    for target in valid_targets {
                        writer.write_u32_le(*target)?;
                    }
                }
                #[cfg(not(feature = "std"))]
                {
                    writer.write_u32_le(valid_targets.len() as u32)?;
                    for i in 0..valid_targets.len() {
                        let target = valid_targets.get(i)?;
                        writer.write_u32_le(target)?;
                    }
                }
                Ok(())
            },
            Self::CallingConventionCheck => writer.write_u8(3u8),
            Self::TemporalCheck { max_duration } => {
                writer.write_u8(4u8)?;
                writer.write_all(&max_duration.to_le_bytes())?;
                Ok(())
            },
        }
    }
}

impl wrt_foundation::traits::FromBytes for CfiValidationRequirement {
    fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => {
                let mut index_bytes = [0u8; 4];
                reader.read_exact(&mut index_bytes)?;
                let expected_type_index = u32::from_le_bytes(index_bytes);

                let mut hash_bytes = [0u8; 8];
                reader.read_exact(&mut hash_bytes)?;
                let signature_hash = u64::from_le_bytes(hash_bytes);

                Ok(Self::TypeSignatureCheck {
                    expected_type_index,
                    signature_hash,
                })
            },
            1 => Ok(Self::ShadowStackCheck),
            2 => {
                // Deserialize CfiTargetVec manually
                let len = reader.read_u32_le()? as usize;
                #[cfg(feature = "std")]
                let mut valid_targets = Vec::with_capacity(len);
                #[cfg(not(feature = "std"))]
                let mut valid_targets = {
                    let provider = safe_managed_alloc!(8192, CrateId::Instructions)?;
                    BoundedVec::new(provider)?
                };

                for _ in 0..len {
                    #[cfg(feature = "std")]
                    valid_targets.push(reader.read_u32_le()?);
                    #[cfg(not(feature = "std"))]
                    valid_targets.push(reader.read_u32_le()?).map_err(|_| {
                        wrt_error::Error::validation_error("Failed to push to bounded vec")
                    })?;
                }
                Ok(Self::ControlFlowTargetCheck { valid_targets })
            },
            3 => Ok(Self::CallingConventionCheck),
            4 => {
                let mut duration_bytes = [0u8; 8];
                reader.read_exact(&mut duration_bytes)?;
                let max_duration = u64::from_le_bytes(duration_bytes);
                Ok(Self::TemporalCheck { max_duration })
            },
            _ => Err(Error::validation_error(
                "Invalid discriminant for CfiValidationRequirement",
            )),
        }
    }
}

/// CFI-aware control flow operations trait
pub trait CfiControlFlowOps {
    /// Execute indirect call with CFI protection
    ///
    /// # Errors
    ///
    /// Returns an error if CFI validation fails or call target is invalid
    fn call_indirect_with_cfi(
        &self,
        type_idx: u32,
        table_idx: u32,
        cfi_protection: &CfiControlFlowProtection,
        context: &mut CfiExecutionContext,
    ) -> Result<CfiProtectedBranchTarget>;

    /// Execute return with CFI protection
    ///
    /// # Errors
    ///
    /// Returns an error if CFI validation fails
    fn return_with_cfi(
        &self,
        cfi_protection: &CfiControlFlowProtection,
        context: &mut CfiExecutionContext,
    ) -> Result<()>;

    /// Execute branch with CFI protection
    ///
    /// # Errors
    ///
    /// Returns an error if CFI validation fails or branch target is invalid
    fn branch_with_cfi(
        &self,
        label_idx: u32,
        conditional: bool,
        cfi_protection: &CfiControlFlowProtection,
        context: &mut CfiExecutionContext,
    ) -> Result<CfiProtectedBranchTarget>;

    /// Insert CFI landing pad
    ///
    /// # Errors
    ///
    /// Returns an error if landing pad insertion fails
    fn insert_cfi_landing_pad(
        &self,
        target_type: CfiTargetType,
        cfi_protection: &CfiControlFlowProtection,
        context: &mut CfiExecutionContext,
    ) -> Result<CfiLandingPad>;

    /// Validate CFI requirements
    ///
    /// # Errors
    ///
    /// Returns an error if validation requirements are not met
    fn validate_cfi_requirements(
        &self,
        requirements: &[CfiValidationRequirement],
        context: &CfiExecutionContext,
    ) -> Result<()>;
}

/// CFI execution context for tracking state
#[derive(Debug, Clone)]
pub struct CfiExecutionContext {
    /// Current function index
    pub current_function:         u32,
    /// Current instruction offset
    pub current_instruction:      u32,
    /// Software shadow stack
    #[cfg(feature = "std")]
    pub shadow_stack:             Vec<ShadowStackEntry>,
    #[cfg(not(feature = "std"))]
    pub shadow_stack:             crate::types::ShadowStackVec,
    /// Active landing pad expectations
    #[cfg(feature = "std")]
    pub landing_pad_expectations: Vec<LandingPadExpectation>,
    #[cfg(not(feature = "std"))]
    pub landing_pad_expectations: crate::types::LandingPadExpectationVec,
    /// CFI violation count
    pub violation_count:          u32,
    /// Performance metrics
    pub metrics:                  CfiMetrics,
    /// Calling convention for platform-specific validation
    pub calling_convention:       CallingConvention,
    /// Current stack depth for alignment checking
    pub current_stack_depth:      u32,
    /// Software configuration for CFI
    pub software_config:          CfiSoftwareConfig,
    /// Timestamp of last checkpoint for temporal validation
    pub last_checkpoint_time:     u64,
    /// Maximum number of labels supported
    pub max_labels:               u32,
    /// Valid branch targets for indirect jumps
    #[cfg(feature = "std")]
    pub valid_branch_targets:     Vec<u32>,
    #[cfg(not(feature = "std"))]
    pub valid_branch_targets: wrt_foundation::bounded::BoundedVec<
        u32,
        256,
        wrt_foundation::safe_memory::NoStdProvider<1024>,
    >,
    /// Maximum number of types supported
    pub max_types:                u32,
    /// Type signatures for validation
    #[cfg(feature = "std")]
    pub type_signatures:          Vec<u64>,
    #[cfg(not(feature = "std"))]
    pub type_signatures: wrt_foundation::bounded::BoundedVec<
        u64,
        256,
        wrt_foundation::safe_memory::NoStdProvider<2048>,
    >,
    /// Maximum shadow stack depth
    pub max_shadow_stack_depth:   usize,
    /// Indirect branch targets for validation
    #[cfg(feature = "std")]
    pub indirect_branch_targets:  Vec<u32>,
    #[cfg(not(feature = "std"))]
    pub indirect_branch_targets: wrt_foundation::bounded::BoundedVec<
        u32,
        128,
        wrt_foundation::safe_memory::NoStdProvider<512>,
    >,
}

impl CfiExecutionContext {
    /// Create a new CFI execution context with default values
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails
    pub fn try_default() -> Result<Self> {
        // Remove unused providers - they were placeholders

        Ok(Self {
            current_function:         0,
            current_instruction:      0,
            shadow_stack:             {
                #[cfg(feature = "std")]
                {
                    Vec::new()
                }
                #[cfg(not(feature = "std"))]
                {
                    let provider = safe_managed_alloc!(65536, CrateId::Instructions)?;
                    crate::types::ShadowStackVec::new(provider)
                        .map_err(|_| Error::memory_error("Failed to create ShadowStackVec"))?
                }
            },
            landing_pad_expectations: {
                #[cfg(feature = "std")]
                {
                    Vec::new()
                }
                #[cfg(not(feature = "std"))]
                {
                    crate::types::LandingPadExpectationVec::new(safe_managed_alloc!(
                        8192,
                        CrateId::Instructions
                    )?)
                    .map_err(|_| Error::memory_error("Failed to create LandingPadExpectationVec"))?
                }
            },
            violation_count:          0,
            metrics:                  CfiMetrics::default(),
            calling_convention:       CallingConvention::WebAssembly,
            current_stack_depth:      0,
            software_config:          CfiSoftwareConfig::default(),
            last_checkpoint_time:     0,
            max_labels:               256,
            valid_branch_targets:     {
                #[cfg(feature = "std")]
                {
                    Vec::new()
                }
                #[cfg(not(feature = "std"))]
                {
                    let provider = safe_managed_alloc!(1024, CrateId::Instructions)?;
                    wrt_foundation::bounded::BoundedVec::new(provider).map_err(|_| {
                        Error::memory_error("Failed to create valid_branch_targets BoundedVec")
                    })?
                }
            },
            max_types:                256,
            type_signatures:          {
                #[cfg(feature = "std")]
                {
                    Vec::new()
                }
                #[cfg(not(feature = "std"))]
                {
                    let provider = safe_managed_alloc!(2048, CrateId::Instructions)?;
                    wrt_foundation::bounded::BoundedVec::new(provider).map_err(|_| {
                        Error::memory_error("Failed to create type_signatures BoundedVec")
                    })?
                }
            },
            max_shadow_stack_depth:   1024,
            indirect_branch_targets:  {
                #[cfg(feature = "std")]
                {
                    Vec::new()
                }
                #[cfg(not(feature = "std"))]
                {
                    let provider = safe_managed_alloc!(512, CrateId::Instructions)?;
                    wrt_foundation::bounded::BoundedVec::new(provider).map_err(|_| {
                        Error::memory_error("Failed to create indirect_branch_targets BoundedVec")
                    })?
                }
            },
        })
    }
}

impl Default for CfiExecutionContext {
    fn default() -> Self {
        Self::try_default().unwrap_or_else(|_| {
            #[cfg(feature = "std")]
            {
                // Fallback for Default trait compatibility - create minimal context
                Self {
                    current_function:         0,
                    current_instruction:      0,
                    shadow_stack:             Vec::new(),
                    landing_pad_expectations: Vec::new(),
                    violation_count:          0,
                    metrics:                  CfiMetrics::default(),
                    calling_convention:       CallingConvention::WebAssembly,
                    current_stack_depth:      0,
                    software_config:          CfiSoftwareConfig::default(),
                    last_checkpoint_time:     0,
                    max_labels:               256,
                    valid_branch_targets:     Vec::new(),
                    max_types:                256,
                    type_signatures:          Vec::new(),
                    max_shadow_stack_depth:   1024,
                    indirect_branch_targets:  Vec::new(),
                }
            }
            #[cfg(not(feature = "std"))]
            {
                panic!(
                    "CfiExecutionContext::default() fallback should not be used in no_std - use \
                     try_default() instead"
                )
            }
        })
    }
}

/// Shadow stack entry for software CFI
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShadowStackEntry {
    /// Return address (function index, instruction offset)
    pub return_address: (u32, u32),
    /// Function signature hash for validation
    pub signature_hash: u64,
    /// Timestamp when call was made
    pub timestamp:      u64,
    /// Call site metadata
    pub call_site_id:   u32,
}

impl wrt_foundation::traits::Checksummable for ShadowStackEntry {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&self.return_address.0.to_le_bytes());
        checksum.update_slice(&self.return_address.1.to_le_bytes());
        checksum.update_slice(&self.signature_hash.to_le_bytes());
        checksum.update_slice(&self.timestamp.to_le_bytes());
        checksum.update_slice(&self.call_site_id.to_le_bytes());
    }
}

impl wrt_foundation::traits::ToBytes for ShadowStackEntry {
    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_all(&self.return_address.0.to_le_bytes())?;
        writer.write_all(&self.return_address.1.to_le_bytes())?;
        writer.write_all(&self.signature_hash.to_le_bytes())?;
        writer.write_all(&self.timestamp.to_le_bytes())?;
        writer.write_all(&self.call_site_id.to_le_bytes())?;
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for ShadowStackEntry {
    fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let mut func_bytes = [0u8; 4];
        reader.read_exact(&mut func_bytes)?;
        let func_idx = u32::from_le_bytes(func_bytes);

        let mut offset_bytes = [0u8; 4];
        reader.read_exact(&mut offset_bytes)?;
        let offset = u32::from_le_bytes(offset_bytes);

        let mut hash_bytes = [0u8; 8];
        reader.read_exact(&mut hash_bytes)?;
        let signature_hash = u64::from_le_bytes(hash_bytes);

        let mut timestamp_bytes = [0u8; 8];
        reader.read_exact(&mut timestamp_bytes)?;
        let timestamp = u64::from_le_bytes(timestamp_bytes);

        let mut id_bytes = [0u8; 4];
        reader.read_exact(&mut id_bytes)?;
        let call_site_id = u32::from_le_bytes(id_bytes);

        Ok(Self {
            return_address: (func_idx, offset),
            signature_hash,
            timestamp,
            call_site_id,
        })
    }
}

/// Landing pad expectation for CFI validation
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub struct LandingPadExpectation {
    /// Expected function index
    pub function_index:     u32,
    /// Expected instruction offset
    pub instruction_offset: u32,
    /// Expected target type
    pub target_type:        CfiTargetType,
    /// Deadline for landing pad (for timeout detection)
    pub deadline:           Option<u64>,
    /// Associated metadata
    pub metadata:           CfiLandingPad,
}


impl wrt_foundation::traits::Checksummable for LandingPadExpectation {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&self.function_index.to_le_bytes());
        checksum.update_slice(&self.instruction_offset.to_le_bytes());
        self.target_type.update_checksum(checksum);
        if let Some(deadline) = self.deadline {
            checksum.update_slice(&[1u8]); // has deadline
            checksum.update_slice(&deadline.to_le_bytes());
        } else {
            checksum.update_slice(&[0u8]); // no deadline
        }
        // Skip metadata for now as it contains complex types
    }
}

impl wrt_foundation::traits::ToBytes for LandingPadExpectation {
    fn to_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_all(&self.function_index.to_le_bytes())?;
        writer.write_all(&self.instruction_offset.to_le_bytes())?;
        self.target_type.to_bytes_with_provider(writer, provider)?;
        if let Some(deadline) = self.deadline {
            writer.write_u8(1u8)?; // has deadline
            writer.write_all(&deadline.to_le_bytes())?;
        } else {
            writer.write_u8(0u8)?; // no deadline
        }
        // Skip metadata serialization for now
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for LandingPadExpectation {
    fn from_bytes_with_provider<PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let mut func_bytes = [0u8; 4];
        reader.read_exact(&mut func_bytes)?;
        let function_index = u32::from_le_bytes(func_bytes);

        let mut offset_bytes = [0u8; 4];
        reader.read_exact(&mut offset_bytes)?;
        let instruction_offset = u32::from_le_bytes(offset_bytes);

        let target_type = CfiTargetType::from_bytes_with_provider(reader, provider)?;

        let has_deadline = reader.read_u8()?;
        let deadline = if has_deadline == 1 {
            let mut deadline_bytes = [0u8; 8];
            reader.read_exact(&mut deadline_bytes)?;
            Some(u64::from_le_bytes(deadline_bytes))
        } else {
            None
        };

        Ok(Self {
            function_index,
            instruction_offset,
            target_type,
            deadline,
            metadata: CfiLandingPad::default(), // Default metadata for now
        })
    }
}

/// CFI performance metrics
#[derive(Debug, Clone, Default)]
pub struct CfiMetrics {
    /// Total indirect calls protected
    pub indirect_calls_protected: u64,
    /// Total returns protected
    pub returns_protected:        u64,
    /// Total branches protected
    pub branches_protected:       u64,
    /// Total CFI violations detected
    pub violations_detected:      u64,
    /// Total CFI overhead (nanoseconds)
    pub total_overhead_ns:        u64,
    /// Landing pads validated
    pub landing_pads_validated:   u64,
    /// Shadow stack operations
    pub shadow_stack_operations:  u64,
    /// Total execution time for temporal validation
    pub total_execution_time:     u64,
    /// Average instruction execution time
    pub average_instruction_time: Option<u64>,
    /// Last instruction execution time
    pub last_instruction_time:    u64,
    /// Count of indirect branches taken
    pub indirect_branches_taken:  u64,
}

/// Default implementation of CFI control flow operations
pub struct DefaultCfiControlFlowOps;

impl CfiControlFlowOps for DefaultCfiControlFlowOps {
    // Large function is intentional - implements full CFI validation logic.
    #[allow(clippy::too_many_lines)]
    fn call_indirect_with_cfi(
        &self,
        type_idx: u32,
        table_idx: u32,
        cfi_protection: &CfiControlFlowProtection,
        context: &mut CfiExecutionContext,
    ) -> Result<CfiProtectedBranchTarget> {
        if !cfi_protection.enabled {
            // CFI disabled, use regular branch target
            return Ok(CfiProtectedBranchTarget {
                target:     BranchTarget {
                    label_idx:   table_idx,
                    keep_values: 0, // Will be determined by type signature
                },
                protection: CfiTargetProtection {
                    target_type:              CfiTargetType::IndirectCall,
                    landing_pad:              None,
                    shadow_stack_requirement: ShadowStackRequirement::None,
                    temporal_validation:      None,
                },
                validation: {
                    #[cfg(feature = "std")]
                    {
                        Vec::new()
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        crate::types::CfiRequirementVec::new(safe_managed_alloc!(
                            8192,
                            CrateId::Instructions
                        )?)
                        .map_err(|_| Error::memory_error("Failed to create CfiRequirementVec"))?
                    }
                },
            });
        }

        // Create CFI-protected branch target
        let landing_pad =
            self.create_landing_pad_for_indirect_call(type_idx, cfi_protection, context)?;

        let shadow_stack_requirement = if matches!(
            cfi_protection.protection_level,
            CfiProtectionLevel::Enhanced | CfiProtectionLevel::Maximum
        ) {
            ShadowStackRequirement::Push {
                return_address:     self.compute_return_address(context),
                function_signature: self.compute_signature_hash(type_idx),
            }
        } else {
            ShadowStackRequirement::None
        };

        let temporal_validation =
            if matches!(cfi_protection.protection_level, CfiProtectionLevel::Maximum) {
                Some(TemporalValidation {
                    max_execution_time: cfi_protection.software_config.max_function_execution_time,
                    start_timestamp:    Some(self.get_current_timestamp()),
                    deadline:           Some(
                        self.get_current_timestamp()
                            + cfi_protection.software_config.max_function_execution_time,
                    ),
                })
            } else {
                None
            };

        // Create validation requirements
        #[cfg(feature = "std")]
        let validation_requirements = vec![
            CfiValidationRequirement::TypeSignatureCheck {
                expected_type_index: type_idx,
                signature_hash:      self.compute_signature_hash(type_idx),
            },
            CfiValidationRequirement::ControlFlowTargetCheck {
                valid_targets: {
                    #[cfg(feature = "std")]
                    {
                        vec![table_idx]
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        let provider = safe_managed_alloc!(65536, CrateId::Instructions)?;
                        let mut targets = CfiTargetVec::new(provider)
                            .map_err(|_| Error::memory_error("Failed to create CfiTargetVec"))?;
                        targets
                            .push(table_idx)
                            .map_err(|_| Error::memory_error("Failed to push to CfiTargetVec"))?;
                        targets
                    }
                }, // Table entry validation
            },
        ];

        #[cfg(not(feature = "std"))]
        let validation_requirements = {
            // For no_std environments, create minimal validation
            use crate::types::CfiRequirementVec;
            let mut reqs =
                CfiRequirementVec::new(safe_managed_alloc!(8192, CrateId::Instructions)?).map_err(
                    |_| Error::validation_error("Failed to create validation requirements"),
                )?;
            reqs.push(CfiValidationRequirement::TypeSignatureCheck {
                expected_type_index: type_idx,
                signature_hash:      self.compute_signature_hash(type_idx),
            })
            .map_err(|_| Error::validation_error("Failed to add validation requirement"))?;
            reqs
        };

        // Update metrics
        context.metrics.indirect_calls_protected += 1;

        Ok(CfiProtectedBranchTarget {
            target:     BranchTarget {
                label_idx:   table_idx,
                keep_values: 0, // Determined by function signature
            },
            protection: CfiTargetProtection {
                target_type: CfiTargetType::IndirectCall,
                landing_pad: Some(landing_pad),
                shadow_stack_requirement,
                temporal_validation,
            },
            validation: validation_requirements,
        })
    }

    fn return_with_cfi(
        &self,
        cfi_protection: &CfiControlFlowProtection,
        context: &mut CfiExecutionContext,
    ) -> Result<()> {
        if !cfi_protection.enabled {
            return Ok(());
        }

        // Validate shadow stack for enhanced/maximum protection
        if matches!(
            cfi_protection.protection_level,
            CfiProtectionLevel::Enhanced | CfiProtectionLevel::Maximum
        ) {
            self.validate_shadow_stack_return(context)?;
        }

        // Update metrics
        context.metrics.returns_protected += 1;

        Ok(())
    }

    fn branch_with_cfi(
        &self,
        label_idx: u32,
        _conditional: bool,
        cfi_protection: &CfiControlFlowProtection,
        context: &mut CfiExecutionContext,
    ) -> Result<CfiProtectedBranchTarget> {
        if !cfi_protection.enabled {
            return Ok(CfiProtectedBranchTarget {
                target:     BranchTarget {
                    label_idx,
                    keep_values: 0,
                },
                protection: CfiTargetProtection {
                    target_type:              CfiTargetType::Branch,
                    landing_pad:              None,
                    shadow_stack_requirement: ShadowStackRequirement::None,
                    temporal_validation:      None,
                },
                validation: {
                    #[cfg(feature = "std")]
                    {
                        Vec::new()
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        crate::types::CfiRequirementVec::new(safe_managed_alloc!(
                            8192,
                            CrateId::Instructions
                        )?)
                        .map_err(|_| Error::memory_error("Failed to create CfiRequirementVec"))?
                    }
                },
            });
        }

        // Create branch target validation
        let target_offset = self.resolve_label_to_offset(label_idx, context)?;

        let validation_requirements = {
            #[cfg(feature = "std")]
            {
                vec![CfiValidationRequirement::ControlFlowTargetCheck {
                    valid_targets: vec![target_offset],
                }]
            }
            #[cfg(not(feature = "std"))]
            {
                let mut reqs = crate::types::CfiRequirementVec::new(safe_managed_alloc!(
                    8192,
                    CrateId::Instructions
                )?)
                .map_err(|_| Error::memory_error("Failed to create CfiRequirementVec"))?;
                let mut targets = crate::types::CfiTargetVec::new(safe_managed_alloc!(
                    8192,
                    CrateId::Instructions
                )?)
                .map_err(|_| Error::memory_error("Failed to create CfiTargetVec"))?;
                targets
                    .push(target_offset)
                    .map_err(|_| Error::memory_error("Failed to push to CfiTargetVec"))?;
                reqs.push(CfiValidationRequirement::ControlFlowTargetCheck {
                    valid_targets: targets,
                })
                .map_err(|_| Error::memory_error("Failed to push to CfiRequirementVec"))?;
                reqs
            }
        };

        // Update metrics
        context.metrics.branches_protected += 1;

        Ok(CfiProtectedBranchTarget {
            target:     BranchTarget {
                label_idx,
                keep_values: 0,
            },
            protection: CfiTargetProtection {
                target_type:              CfiTargetType::Branch,
                landing_pad:              None,
                shadow_stack_requirement: ShadowStackRequirement::None,
                temporal_validation:      None,
            },
            validation: validation_requirements,
        })
    }

    fn insert_cfi_landing_pad(
        &self,
        target_type: CfiTargetType,
        cfi_protection: &CfiControlFlowProtection,
        context: &mut CfiExecutionContext,
    ) -> Result<CfiLandingPad> {
        let hardware_instruction = if let Some(ref hw_config) = cfi_protection.hardware_config {
            self.create_hardware_instruction(target_type, hw_config)
        } else {
            None
        };

        let software_validation = if cfi_protection.software_config.landing_pad_simulation {
            Some(self.create_software_validation(target_type, context)?)
        } else {
            None
        };

        let landing_pad = CfiLandingPad {
            pad_id: self.generate_landing_pad_id(context),
            hardware_instruction,
            software_validation,
            valid_predecessors: self.determine_valid_predecessors(target_type)?,
        };

        // Update metrics
        context.metrics.landing_pads_validated += 1;

        Ok(landing_pad)
    }

    fn validate_cfi_requirements(
        &self,
        requirements: &[CfiValidationRequirement],
        context: &CfiExecutionContext,
    ) -> Result<()> {
        for requirement in requirements {
            match requirement {
                CfiValidationRequirement::TypeSignatureCheck {
                    expected_type_index,
                    signature_hash,
                } => {
                    self.validate_type_signature(*expected_type_index, *signature_hash, context)?;
                },
                CfiValidationRequirement::ShadowStackCheck => {
                    self.validate_shadow_stack(context)?;
                },
                CfiValidationRequirement::ControlFlowTargetCheck { valid_targets: _ } => {
                    // Convert BoundedVec to slice - for validation, we can iterate
                    let targets: &[u32] = &[]; // Empty slice for now, proper implementation would iterate
                    self.validate_control_flow_target(targets, context)?;
                },
                CfiValidationRequirement::CallingConventionCheck => {
                    self.validate_calling_convention(context)?;
                },
                CfiValidationRequirement::TemporalCheck { max_duration } => {
                    self.validate_temporal_properties(*max_duration, context)?;
                },
            }
        }
        Ok(())
    }
}

impl DefaultCfiControlFlowOps {
    // Helper methods for CFI implementation

    fn create_landing_pad_for_indirect_call(
        &self,
        _type_idx: u32,
        cfi_protection: &CfiControlFlowProtection,
        context: &mut CfiExecutionContext,
    ) -> Result<CfiLandingPad> {
        self.insert_cfi_landing_pad(CfiTargetType::IndirectCall, cfi_protection, context)
    }

    fn compute_return_address(&self, context: &CfiExecutionContext) -> u64 {
        let _ = self; // Unused in current implementation
        // Combine function index and instruction offset into return address
        (u64::from(context.current_function) << 32) | u64::from(context.current_instruction)
    }

    fn compute_signature_hash(&self, type_idx: u32) -> u64 {
        let _ = self; // Unused in current implementation
        // Simple hash for now - real implementation would use proper type information
        u64::from(type_idx) * 0x9e37_79b9_7f4a_7c15
    }

    fn get_current_timestamp(&self) -> u64 {
        let _ = self; // Unused in current implementation
        // Software fallback - return 0 for deterministic behavior
        // Real implementation would use platform-specific timing APIs
        0
    }

    fn validate_shadow_stack_return(&self, context: &mut CfiExecutionContext) -> Result<()> {
        let _ = self; // Unused in current implementation
        #[cfg(feature = "std")]
        let shadow_entry_opt = context.shadow_stack.pop();
        #[cfg(not(feature = "std"))]
        let shadow_entry_opt = context.shadow_stack.pop().ok().flatten();

        if let Some(shadow_entry) = shadow_entry_opt {
            let expected_return = (context.current_function, context.current_instruction);
            if shadow_entry.return_address != expected_return {
                context.violation_count += 1;
                return Err(Error::cfi_violation("Shadow stack return address mismatch"));
            }
            context.metrics.shadow_stack_operations += 1;
        } else {
            context.violation_count += 1;
            return Err(Error::cfi_violation("Shadow stack underflow"));
        }
        Ok(())
    }

    fn resolve_label_to_offset(
        &self,
        label_idx: u32,
        context: &CfiExecutionContext,
    ) -> Result<u32> {
        let _ = self; // Unused in current implementation
        // ASIL-B: Validate label index bounds
        if label_idx >= context.max_labels {
            return Err(Error::validation_control_flow_error(
                "Label index out of bounds",
            ));
        }

        // ASIL-B: Check if label is in valid targets
        if !context.valid_branch_targets.is_empty() {
            #[cfg(feature = "std")]
            let contains_target = context.valid_branch_targets.contains(&label_idx);
            #[cfg(not(feature = "std"))]
            let contains_target =
                context.valid_branch_targets.contains(&label_idx).unwrap_or(false);

            if !contains_target {
                return Err(Error::validation_control_flow_error(
                    "Invalid branch target",
                ));
            }
        }

        // Return resolved label (in real implementation, would map to actual PC)
        Ok(label_idx)
    }

    fn create_hardware_instruction(
        &self,
        target_type: CfiTargetType,
        hw_config: &HardwareCfiConfig,
    ) -> Option<CfiHardwareInstruction> {
        let _ = self; // Unused except for generate_riscv_label call
        match &hw_config.settings {
            #[cfg(target_arch = "aarch64")]
            HardwareCfiSettings::ArmBti { mode, .. } => {
                let bti_mode = match target_type {
                    CfiTargetType::IndirectCall => ArmBtiMode::CallOnly,
                    CfiTargetType::Return => ArmBtiMode::Standard,
                    CfiTargetType::Branch => ArmBtiMode::JumpOnly,
                    _ => *mode,
                };
                Some(CfiHardwareInstruction::ArmBti { mode: bti_mode })
            },

            #[cfg(target_arch = "riscv64")]
            HardwareCfiSettings::RiscVCfi {
                landing_pads: true, ..
            } => Some(CfiHardwareInstruction::RiscVLandingPad {
                label: self.generate_riscv_label(),
            }),

            #[cfg(target_arch = "x86_64")]
            HardwareCfiSettings::X86Cet {
                indirect_branch_tracking: true,
                ..
            } => Some(CfiHardwareInstruction::X86Endbr),

            _ => None,
        }
    }

    // Result needed for no_std where safe_managed_alloc! can fail.
    #[allow(clippy::unnecessary_wraps)]
    fn create_software_validation(
        &self,
        target_type: CfiTargetType,
        context: &CfiExecutionContext,
    ) -> Result<CfiSoftwareValidation> {
        let validation_function = match target_type {
            CfiTargetType::IndirectCall => SoftwareCfiFunction::ValidateSignature,
            CfiTargetType::Return => SoftwareCfiFunction::ValidateReturnAddress,
            CfiTargetType::Branch => SoftwareCfiFunction::ValidateBranchTarget,
            _ => SoftwareCfiFunction::ValidateCallSite,
        };

        Ok(CfiSoftwareValidation {
            check_id: self.generate_cfi_check_id(context),
            expected_values: {
                #[cfg(feature = "std")]
                {
                    Vec::new()
                }
                #[cfg(not(feature = "std"))]
                {
                    crate::types::CfiExpectedValueVec::new(safe_managed_alloc!(
                        8192,
                        CrateId::Instructions
                    )?)
                    .map_err(|_| Error::memory_error("Failed to create CfiExpectedValueVec"))?
                }
            }, // Would be populated based on context
            validation_function,
        })
    }

    fn generate_landing_pad_id(&self, _context: &CfiExecutionContext) -> u32 {
        let _ = self; // Unused in current implementation
        // Generate unique landing pad ID
        0 // Placeholder
    }

    fn generate_cfi_check_id(&self, _context: &CfiExecutionContext) -> u32 {
        let _ = self; // Unused in current implementation
        // Generate unique CFI check ID
        0 // Placeholder
    }

    #[cfg(target_arch = "riscv64")]
    fn generate_riscv_label(&self) -> u32 {
        let _ = self; // Unused in current implementation
        // Generate unique RISC-V landing pad label
        0 // Placeholder
    }

    // Result needed for no_std where safe_managed_alloc! can fail.
    #[allow(clippy::unnecessary_wraps)]
    fn determine_valid_predecessors(
        &self,
        target_type: CfiTargetType,
    ) -> Result<crate::types::CfiTargetTypeVec> {
        let _ = self; // Unused in current implementation
        Ok(match target_type {
            CfiTargetType::IndirectCall => {
                #[cfg(feature = "std")]
                {
                    vec![CfiTargetType::IndirectCall]
                }
                #[cfg(not(feature = "std"))]
                {
                    let provider_8k = safe_managed_alloc!(8192, CrateId::Instructions)?;
                    let mut types = crate::types::CfiTargetTypeVec::new(provider_8k)
                        .map_err(|_| Error::memory_error("Failed to create CfiTargetTypeVec"))?;
                    types
                        .push(CfiTargetType::IndirectCall)
                        .map_err(|_| Error::memory_error("Failed to push to CfiTargetTypeVec"))?;
                    types
                }
            },
            CfiTargetType::Return | CfiTargetType::FunctionEntry => {
                #[cfg(feature = "std")]
                {
                    vec![CfiTargetType::DirectCall, CfiTargetType::IndirectCall]
                }
                #[cfg(not(feature = "std"))]
                {
                    let provider_8k = safe_managed_alloc!(8192, CrateId::Instructions)?;
                    let mut types = crate::types::CfiTargetTypeVec::new(provider_8k)
                        .map_err(|_| Error::memory_error("Failed to create CfiTargetTypeVec"))?;
                    types
                        .push(CfiTargetType::DirectCall)
                        .map_err(|_| Error::memory_error("Failed to push to CfiTargetTypeVec"))?;
                    types
                        .push(CfiTargetType::IndirectCall)
                        .map_err(|_| Error::memory_error("Failed to push to CfiTargetTypeVec"))?;
                    types
                }
            },
            CfiTargetType::Branch | CfiTargetType::BlockEntry => {
                #[cfg(feature = "std")]
                {
                    vec![CfiTargetType::Branch]
                }
                #[cfg(not(feature = "std"))]
                {
                    let provider_8k = safe_managed_alloc!(8192, CrateId::Instructions)?;
                    let mut types = crate::types::CfiTargetTypeVec::new(provider_8k)
                        .map_err(|_| Error::memory_error("Failed to create CfiTargetTypeVec"))?;
                    types
                        .push(CfiTargetType::Branch)
                        .map_err(|_| Error::memory_error("Failed to push to CfiTargetTypeVec"))?;
                    types
                }
            },
            CfiTargetType::DirectCall => {
                #[cfg(feature = "std")]
                {
                    Vec::new()
                }
                #[cfg(not(feature = "std"))]
                {
                    let provider = safe_managed_alloc!(8192, CrateId::Instructions)?;
                    crate::types::CfiTargetTypeVec::new(provider)
                        .map_err(|_| Error::memory_error("Failed to create CfiTargetTypeVec"))?
                }
            },
        })
    }

    // Validation helper methods

    fn validate_type_signature(
        &self,
        expected_type_index: u32,
        signature_hash: u64,
        context: &CfiExecutionContext,
    ) -> Result<()> {
        let _ = self; // Unused in current implementation
        // ASIL-B: Validate type index bounds
        if expected_type_index >= context.max_types {
            return Err(Error::validation_value_type_error(
                "Type index out of bounds",
            ));
        }

        // ASIL-B: Verify signature hash matches expected
        // In a real implementation, would compute hash from actual type
        #[cfg(feature = "std")]
        let expected_hash_val = *context.type_signatures.get(expected_type_index as usize).ok_or(
            Error::validation_value_type_error("Type signature not found"),
        )?;
        #[cfg(not(feature = "std"))]
        let expected_hash_val = context.type_signatures.get(expected_type_index as usize)?;

        if expected_hash_val != signature_hash {
            return Err(Error::security_runtime_error(
                "Type signature mismatch - potential CFI violation",
            ));
        }

        Ok(())
    }

    fn validate_shadow_stack(&self, context: &CfiExecutionContext) -> Result<()> {
        let _ = self; // Unused in current implementation
        // ASIL-B: Check shadow stack depth
        if context.shadow_stack.len() > context.max_shadow_stack_depth {
            return Err(Error::security_stack_overflow("Shadow stack overflow"));
        }

        // ASIL-B: Validate shadow stack integrity
        if context.shadow_stack.is_empty() && context.current_function != 0 {
            return Err(Error::security_runtime_error(
                "Shadow stack underflow - potential CFI violation",
            ));
        }

        // ASIL-B: Verify return address matches shadow stack
        #[cfg(feature = "std")]
        {
            if let Some(expected_return) = context.shadow_stack.last() {
                if expected_return.return_address
                    != (context.current_function, context.current_instruction)
                {
                    return Err(Error::runtime_error(
                        "Return address mismatch - potential ROP attack",
                    ));
                }
            }
        }
        #[cfg(not(feature = "std"))]
        {
            if let Ok(Some(expected_return)) = context.shadow_stack.last() {
                if expected_return.return_address
                    != (context.current_function, context.current_instruction)
                {
                    return Err(Error::runtime_error(
                        "Return address mismatch - potential ROP attack",
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_control_flow_target(
        &self,
        valid_targets: &[u32],
        context: &CfiExecutionContext,
    ) -> Result<()> {
        let _ = self; // Unused in current implementation
        // ASIL-B: Ensure valid targets list is not empty
        if valid_targets.is_empty() {
            return Err(Error::validation_control_flow_error(
                "No valid control flow targets specified",
            ));
        }

        // ASIL-B: Check current instruction is a valid target
        let current_target = context.current_instruction;
        if !valid_targets.contains(&current_target) {
            // Increment violation count for monitoring
            // Note: In real implementation, would update mutable context
            return Err(Error::security_runtime_error(
                "Invalid control flow target - potential CFI violation",
            ));
        }

        // ASIL-B: Additional check for indirect branches
        if context.metrics.indirect_branches_taken > 0 {
            // Verify target is marked as valid indirect branch target
            #[cfg(feature = "std")]
            let contains_target = context.indirect_branch_targets.contains(&current_target);
            #[cfg(not(feature = "std"))]
            let contains_target =
                context.indirect_branch_targets.contains(&current_target).unwrap_or(false);

            if !contains_target {
                return Err(Error::security_runtime_error(
                    "Invalid indirect branch target",
                ));
            }
        }

        Ok(())
    }

    fn validate_calling_convention(&self, context: &CfiExecutionContext) -> Result<()> {
        const REQUIRED_ALIGNMENT: u32 = 16; // Common for most ABIs
        let _ = self; // Unused in current implementation
        // ASIL-B: Validate stack alignment for calls
        if context.current_stack_depth % REQUIRED_ALIGNMENT != 0 {
            return Err(Error::runtime_unaligned_access(
                "Stack misaligned for function call",
            ));
        }

        // ASIL-B: Check calling convention constraints
        if context.calling_convention != CallingConvention::WebAssembly {
            // Verify platform-specific constraints
            match context.calling_convention {
                CallingConvention::SystemV => {
                    // Check red zone requirements
                    if context.current_stack_depth < 128 {
                        return Err(Error::runtime_error(
                            "Insufficient stack space for SystemV ABI",
                        ));
                    }
                },
                CallingConvention::WindowsFastcall => {
                    // Check shadow space requirements
                    if context.current_stack_depth < 32 {
                        return Err(Error::runtime_error(
                            "Insufficient shadow space for Windows ABI",
                        ));
                    }
                },
                CallingConvention::WebAssembly => {}, // WebAssembly has no additional requirements
            }
        }

        Ok(())
    }

    fn validate_temporal_properties(
        &self,
        max_duration: u64,
        context: &CfiExecutionContext,
    ) -> Result<()> {
        let _ = self; // Unused in current implementation
        // ASIL-B: Check if temporal validation is enabled
        if !context.software_config.temporal_validation {
            return Ok(()); // Skip if not enabled
        }

        // ASIL-B: Validate execution time bounds
        let current_time = context.metrics.total_execution_time;
        if current_time > max_duration {
            return Err(Error::runtime_error(
                "Execution time exceeded maximum allowed duration",
            ));
        }

        // ASIL-B: Check for timing anomalies that might indicate attacks
        if let Some(avg_instruction_time) = context.metrics.average_instruction_time {
            let current_instruction_time = context.metrics.last_instruction_time;

            // Flag if current instruction took >10x average time
            if current_instruction_time > avg_instruction_time * 10 {
                return Err(Error::security_runtime_error(
                    "Timing anomaly detected - potential side-channel attack",
                ));
            }
        }

        // ASIL-B: Validate monotonic time progression
        if context.metrics.total_execution_time < context.last_checkpoint_time {
            return Err(Error::security_runtime_error(
                "Time regression detected - potential clock manipulation",
            ));
        }

        Ok(())
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_cfi_control_flow_protection_default() {
        let protection = CfiControlFlowProtection::default();
        assert!(protection.enabled);
        assert_eq!(protection.protection_level, CfiProtectionLevel::Enhanced);
        assert!(protection.software_config.shadow_stack_enabled);
    }

    #[test]
    fn test_cfi_execution_context_default() {
        let context = CfiExecutionContext::default();
        assert_eq!(context.current_function, 0);
        assert_eq!(context.current_instruction, 0);
        assert!(context.shadow_stack.is_empty());
        assert_eq!(context.violation_count, 0);
    }

    #[test]
    fn test_default_cfi_ops_call_indirect() {
        let ops = DefaultCfiControlFlowOps;
        let protection = CfiControlFlowProtection::default();
        let mut context = CfiExecutionContext::default();

        let result = ops.call_indirect_with_cfi(1, 0, &protection, &mut context);
        assert!(result.is_ok());

        let protected_target = result.unwrap();
        assert_eq!(
            protected_target.protection.target_type,
            CfiTargetType::IndirectCall
        );
        assert!(protected_target.protection.landing_pad.is_some());
        assert_eq!(context.metrics.indirect_calls_protected, 1);
    }

    #[test]
    fn test_cfi_disabled() {
        let ops = DefaultCfiControlFlowOps;
        let mut protection = CfiControlFlowProtection::default();
        protection.enabled = false;
        let mut context = CfiExecutionContext::default();

        let result = ops.call_indirect_with_cfi(1, 0, &protection, &mut context);
        assert!(result.is_ok());

        let protected_target = result.unwrap();
        assert!(protected_target.protection.landing_pad.is_none());
        assert!(matches!(
            protected_target.protection.shadow_stack_requirement,
            ShadowStackRequirement::None
        ));
    }

    #[test]
    fn test_software_cfi_config_default() {
        let config = SoftwareCfiConfig::default();
        assert!(config.shadow_stack_enabled);
        assert_eq!(config.max_shadow_stack_depth, 1024);
        assert!(config.landing_pad_simulation);
        assert!(!config.temporal_validation); // Off by default due to cost
    }
}
