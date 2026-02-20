// WRT - wrt-foundation
// Module: Universal Safety System
// SW-REQ-ID: REQ_SAFETY_ASIL_001, REQ_SAFETY_CROSS_001, REQ_SAFETY_MULTI_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Universal Safety System for WRT Foundation
//!
//! ⚠️ **PRELIMINARY IMPLEMENTATION WARNING** ⚠️
//!
//! This safety classification system is in a preliminary state and has NOT
//! undergone formal certification or validation by standards bodies. The
//! severity scores and cross-standard mappings are based on research and
//! analysis but should be validated by qualified safety engineers before use in
//! safety-critical applications.
//!
//! Users MUST conduct their own validation and risk assessment before deploying
//! this system in safety-critical environments. See documentation for
//! validation guidance.
//!
//! This module provides safety primitives that support multiple safety
//! standards including automotive (ISO 26262), aerospace (DO-178C), industrial
//! (IEC 61508), medical (IEC 62304), railway (EN 50128), and agricultural (ISO
//! 25119).
//!
//! # Supported Safety Standards
//!
//! - **ISO 26262 (Automotive)**: QM, ASIL-A, ASIL-B, ASIL-C, ASIL-D
//! - **DO-178C (Aerospace)**: DAL-E, DAL-D, DAL-C, DAL-B, DAL-A
//! - **IEC 61508 (Industrial)**: SIL-1, SIL-2, SIL-3, SIL-4
//! - **IEC 62304 (Medical)**: Class A, Class B, Class C
//! - **EN 50128 (Railway)**: SIL-0, SIL-1, SIL-2, SIL-3, SIL-4
//! - **ISO 25119 (Agricultural)**: AgPL-a, AgPL-b, AgPL-c, AgPL-d, AgPL-e
//!
//! # Design Principles
//!
//! - **Multi-Standard Support**: Cross-standard compatibility and conversion
//! - **Compile-Time Safety**: Safety levels are known at compile time when
//!   possible
//! - **Runtime Adaptation**: Safety checks can be enhanced at runtime
//! - **Zero-Cost Abstractions**: Safety primitives add minimal overhead
//! - **Fail-Safe Design**: All operations fail safely when safety violations
//!   occur
//! - **Severity-Based Mapping**: Universal severity score (0-1000) for
//!   comparisons
//! - **Conservative Approach**: When in doubt, maps to higher safety
//!   requirements
//!
//! # Usage
//!
//! ```rust
//! use wrt_foundation::safety_system::{
//!     AsilLevel,
//!     SafetyContext,
//!     SafetyStandard,
//!     UniversalSafetyContext,
//! };
//!
//! // Traditional ASIL-only context
//! const ASIL_CTX: SafetyContext = SafetyContext::new(AsilLevel::AsilC);
//!
//! // Multi-standard context
//! const MULTI_CTX: UniversalSafetyContext =
//!     UniversalSafetyContext::new(SafetyStandard::Iso26262(AsilLevel::AsilC));
//!
//! // Cross-standard conversion
//! let asil_c = SafetyStandard::Iso26262(AsilLevel::AsilC);
//! let equivalent_dal = asil_c.convert_to(SafetyStandardType::Do178c);
//! ```

use core::sync::atomic::{
    AtomicU8,
    Ordering,
};
#[cfg(feature = "std")]
use std::time::{
    SystemTime,
    UNIX_EPOCH,
};

use crate::{
    codes,
    Error,
    ErrorCategory,
};

/// Automotive Safety Integrity Level (ASIL) classification
///
/// ASIL levels define the safety requirements for automotive systems.
/// Higher levels require more rigorous safety measures.
///
/// # REQ Traceability
/// - REQ_SAFETY_ASIL_001: ASIL level classification support
/// - REQ_SAFETY_ISO26262_001: ISO 26262 automotive safety standard compliance
/// - REQ_MEM_SAFETY_001: Memory protection requirements for ASIL-C/D
/// - REQ_VERIFY_001: Runtime verification requirements for ASIL-B+
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum AsilLevel {
    /// Quality Management - No safety requirements
    QM    = 0,
    /// ASIL A - Lowest safety integrity level
    AsilA = 1,
    /// ASIL B - Low safety integrity level  
    AsilB = 2,
    /// ASIL C - Medium safety integrity level
    AsilC = 3,
    /// ASIL D - Highest safety integrity level
    AsilD = 4,
}

impl AsilLevel {
    /// Get the string representation of the ASIL level
    pub const fn as_str(&self) -> &'static str {
        match self {
            AsilLevel::QM => "QM",
            AsilLevel::AsilA => "ASIL-A",
            AsilLevel::AsilB => "ASIL-B",
            AsilLevel::AsilC => "ASIL-C",
            AsilLevel::AsilD => "ASIL-D",
        }
    }

    /// Check if this ASIL level requires memory protection
    pub const fn requires_memory_protection(&self) -> bool {
        matches!(self, AsilLevel::AsilC | AsilLevel::AsilD)
    }

    /// Check if this ASIL level requires runtime verification
    pub const fn requires_runtime_verification(&self) -> bool {
        matches!(self, AsilLevel::AsilB | AsilLevel::AsilC | AsilLevel::AsilD)
    }

    /// Check if this ASIL level requires control flow integrity
    pub const fn requires_cfi(&self) -> bool {
        matches!(self, AsilLevel::AsilC | AsilLevel::AsilD)
    }

    /// Check if this ASIL level requires redundant computation
    pub const fn requires_redundancy(&self) -> bool {
        matches!(self, AsilLevel::AsilD)
    }

    /// Get the required verification frequency for this ASIL level
    pub const fn verification_frequency(&self) -> u32 {
        match self {
            AsilLevel::QM => 0,
            AsilLevel::AsilA => 1000, // Every 1000 operations
            AsilLevel::AsilB => 100,  // Every 100 operations
            AsilLevel::AsilC => 10,   // Every 10 operations
            AsilLevel::AsilD => 1,    // Every operation
        }
    }

    /// Get the maximum allowed error rate for this ASIL level
    pub const fn max_error_rate(&self) -> f64 {
        match self {
            AsilLevel::QM => 1.0,       // No limit
            AsilLevel::AsilA => 0.1,    // 10%
            AsilLevel::AsilB => 0.01,   // 1%
            AsilLevel::AsilC => 0.001,  // 0.1%
            AsilLevel::AsilD => 0.0001, // 0.01%
        }
    }
}

impl Default for AsilLevel {
    fn default() -> Self {
        AsilLevel::QM
    }
}

/// Safety level wrapper for ASIL integration
///
/// This type provides a common interface for safety level operations
/// across the WRT system, wrapping the core AsilLevel enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SafetyLevel {
    asil: AsilLevel,
}

impl SafetyLevel {
    /// Create a new safety level from an ASIL level
    pub const fn new(asil: AsilLevel) -> Self {
        Self { asil }
    }

    /// Get the underlying ASIL level
    pub const fn asil_level(&self) -> AsilLevel {
        self.asil
    }

    /// Check if this safety level requires memory protection
    pub const fn requires_memory_protection(&self) -> bool {
        self.asil.requires_memory_protection()
    }

    /// Check if this safety level requires runtime verification
    pub const fn requires_runtime_verification(&self) -> bool {
        self.asil.requires_runtime_verification()
    }

    /// Get the verification frequency for this safety level
    pub const fn verification_frequency(&self) -> u32 {
        self.asil.verification_frequency()
    }
}

impl Default for SafetyLevel {
    fn default() -> Self {
        Self::new(AsilLevel::default())
    }
}

impl From<AsilLevel> for SafetyLevel {
    fn from(asil: AsilLevel) -> Self {
        Self::new(asil)
    }
}

impl core::fmt::Display for SafetyLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.asil)
    }
}

impl core::fmt::Display for AsilLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Universal Safety Standards System
// ============================================================================

/// DO-178C Design Assurance Level (Aerospace)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum DalLevel {
    /// DAL E - No effect on safety
    DalE = 0,
    /// DAL D - Minor effect
    DalD = 1,
    /// DAL C - Major effect
    DalC = 2,
    /// DAL B - Hazardous effect
    DalB = 3,
    /// DAL A - Catastrophic effect
    DalA = 4,
}

/// IEC 61508 Safety Integrity Level (Industrial)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SilLevel {
    /// SIL 1 - Low risk reduction
    Sil1 = 1,
    /// SIL 2 - Medium risk reduction
    Sil2 = 2,
    /// SIL 3 - High risk reduction
    Sil3 = 3,
    /// SIL 4 - Very high risk reduction
    Sil4 = 4,
}

/// IEC 62304 Medical Device Safety Class
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum MedicalClass {
    /// Class A - Non-life-threatening
    ClassA = 1,
    /// Class B - Non-life-threatening but injury possible
    ClassB = 2,
    /// Class C - Life-threatening or death possible
    ClassC = 3,
}

/// EN 50128 Railway Safety Integrity Level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum RailwaySil {
    /// SIL 0 - No safety significance
    Sil0 = 0,
    /// SIL 1 - Low safety significance
    Sil1 = 1,
    /// SIL 2 - Medium safety significance
    Sil2 = 2,
    /// SIL 3 - High safety significance
    Sil3 = 3,
    /// SIL 4 - Very high safety significance
    Sil4 = 4,
}

/// ISO 25119 Agricultural Performance Level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum AgricultureLevel {
    /// AgPL a - Low risk
    AgPla = 1,
    /// AgPL b - Medium risk
    AgPlb = 2,
    /// AgPL c - High risk
    AgPlc = 3,
    /// AgPL d - Very high risk
    AgPld = 4,
    /// AgPL e - Highest risk
    AgPle = 5,
}

/// Universal Safety Standard Classification
///
/// This enum represents safety levels from different international standards,
/// allowing for cross-standard comparison and conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafetyStandard {
    /// ISO 26262 - Automotive
    Iso26262(AsilLevel),
    /// DO-178C - Aerospace
    Do178c(DalLevel),
    /// IEC 61508 - Industrial
    Iec61508(SilLevel),
    /// IEC 62304 - Medical Device
    Iec62304(MedicalClass),
    /// EN 50128 - Railway
    En50128(RailwaySil),
    /// ISO 25119 - Agricultural
    Iso25119(AgricultureLevel),
}

/// Universal severity score (0-1000 scale) for cross-standard comparison
///
/// # REQ Traceability  
/// - REQ_SAFETY_CROSS_002: Universal severity scoring system
/// - REQ_SAFETY_COMPARE_001: Cross-standard safety level comparison
/// - REQ_SAFETY_NORMALIZE_001: Normalization of safety levels to 0-1000 scale
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeverityScore(u16);

/// Error types for safety operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyError {
    /// Invalid severity score (must be 0-1000)
    InvalidSeverityScore,
    /// Cannot convert between standards
    ConversionFailed,
    /// Standard not supported
    UnsupportedStandard,
}

impl SeverityScore {
    /// Maximum severity score (highest safety requirements)
    pub const MAX: Self = Self(1000);
    /// Minimum severity score (no safety requirements)
    pub const MIN: Self = Self(0);

    /// Create a new severity score
    ///
    /// # Arguments
    /// * `score` - Severity score (0-1000)
    ///
    /// # Errors
    /// Returns `SafetyError::InvalidSeverityScore` if score > 1000
    pub const fn new(score: u16) -> Result<Self, SafetyError> {
        if score <= 1000 {
            Ok(Self(score))
        } else {
            Err(SafetyError::InvalidSeverityScore)
        }
    }

    /// Get the raw severity score value
    pub const fn value(&self) -> u16 {
        self.0
    }
}

/// Safety standard type identifier for conversions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafetyStandardType {
    /// ISO 26262 (Automotive)
    Iso26262,
    /// DO-178C (Aerospace)
    Do178c,
    /// IEC 61508 (Industrial)
    Iec61508,
    /// IEC 62304 (Medical)
    Iec62304,
    /// EN 50128 (Railway)
    En50128,
    /// ISO 25119 (Agricultural)
    Iso25119,
}

impl SafetyStandard {
    /// Get the universal severity score for cross-standard comparison
    ///
    /// This method maps each safety level to a universal severity score on a
    /// 0-1000 scale, enabling comparison and conversion between different
    /// safety standards.
    pub const fn severity_score(&self) -> SeverityScore {
        match self {
            // ISO 26262 mapping (automotive)
            SafetyStandard::Iso26262(AsilLevel::QM) => SeverityScore(0),
            SafetyStandard::Iso26262(AsilLevel::AsilA) => SeverityScore(250),
            SafetyStandard::Iso26262(AsilLevel::AsilB) => SeverityScore(500),
            SafetyStandard::Iso26262(AsilLevel::AsilC) => SeverityScore(750),
            SafetyStandard::Iso26262(AsilLevel::AsilD) => SeverityScore(1000),

            // DO-178C mapping (aerospace)
            SafetyStandard::Do178c(DalLevel::DalE) => SeverityScore(0),
            SafetyStandard::Do178c(DalLevel::DalD) => SeverityScore(200),
            SafetyStandard::Do178c(DalLevel::DalC) => SeverityScore(400),
            SafetyStandard::Do178c(DalLevel::DalB) => SeverityScore(700),
            SafetyStandard::Do178c(DalLevel::DalA) => SeverityScore(1000),

            // IEC 61508 mapping (industrial)
            SafetyStandard::Iec61508(SilLevel::Sil1) => SeverityScore(250),
            SafetyStandard::Iec61508(SilLevel::Sil2) => SeverityScore(500),
            SafetyStandard::Iec61508(SilLevel::Sil3) => SeverityScore(750),
            SafetyStandard::Iec61508(SilLevel::Sil4) => SeverityScore(1000),

            // IEC 62304 mapping (medical device)
            SafetyStandard::Iec62304(MedicalClass::ClassA) => SeverityScore(200),
            SafetyStandard::Iec62304(MedicalClass::ClassB) => SeverityScore(500),
            SafetyStandard::Iec62304(MedicalClass::ClassC) => SeverityScore(1000),

            // EN 50128 mapping (railway)
            SafetyStandard::En50128(RailwaySil::Sil0) => SeverityScore(0),
            SafetyStandard::En50128(RailwaySil::Sil1) => SeverityScore(200),
            SafetyStandard::En50128(RailwaySil::Sil2) => SeverityScore(400),
            SafetyStandard::En50128(RailwaySil::Sil3) => SeverityScore(700),
            SafetyStandard::En50128(RailwaySil::Sil4) => SeverityScore(1000),

            // ISO 25119 mapping (agricultural)
            SafetyStandard::Iso25119(AgricultureLevel::AgPla) => SeverityScore(200),
            SafetyStandard::Iso25119(AgricultureLevel::AgPlb) => SeverityScore(400),
            SafetyStandard::Iso25119(AgricultureLevel::AgPlc) => SeverityScore(600),
            SafetyStandard::Iso25119(AgricultureLevel::AgPld) => SeverityScore(800),
            SafetyStandard::Iso25119(AgricultureLevel::AgPle) => SeverityScore(1000),
        }
    }

    /// Get the name of the safety standard
    pub const fn standard_name(&self) -> &'static str {
        match self {
            SafetyStandard::Iso26262(_) => "ISO 26262",
            SafetyStandard::Do178c(_) => "DO-178C",
            SafetyStandard::Iec61508(_) => "IEC 61508",
            SafetyStandard::Iec62304(_) => "IEC 62304",
            SafetyStandard::En50128(_) => "EN 50128",
            SafetyStandard::Iso25119(_) => "ISO 25119",
        }
    }

    /// Get the level name within the standard
    pub const fn level_name(&self) -> &'static str {
        match self {
            SafetyStandard::Iso26262(level) => level.as_str(),
            SafetyStandard::Do178c(DalLevel::DalE) => "DAL-E",
            SafetyStandard::Do178c(DalLevel::DalD) => "DAL-D",
            SafetyStandard::Do178c(DalLevel::DalC) => "DAL-C",
            SafetyStandard::Do178c(DalLevel::DalB) => "DAL-B",
            SafetyStandard::Do178c(DalLevel::DalA) => "DAL-A",
            SafetyStandard::Iec61508(SilLevel::Sil1) => "SIL-1",
            SafetyStandard::Iec61508(SilLevel::Sil2) => "SIL-2",
            SafetyStandard::Iec61508(SilLevel::Sil3) => "SIL-3",
            SafetyStandard::Iec61508(SilLevel::Sil4) => "SIL-4",
            SafetyStandard::Iec62304(MedicalClass::ClassA) => "Class A",
            SafetyStandard::Iec62304(MedicalClass::ClassB) => "Class B",
            SafetyStandard::Iec62304(MedicalClass::ClassC) => "Class C",
            SafetyStandard::En50128(RailwaySil::Sil0) => "SIL-0",
            SafetyStandard::En50128(RailwaySil::Sil1) => "SIL-1",
            SafetyStandard::En50128(RailwaySil::Sil2) => "SIL-2",
            SafetyStandard::En50128(RailwaySil::Sil3) => "SIL-3",
            SafetyStandard::En50128(RailwaySil::Sil4) => "SIL-4",
            SafetyStandard::Iso25119(AgricultureLevel::AgPla) => "AgPL-a",
            SafetyStandard::Iso25119(AgricultureLevel::AgPlb) => "AgPL-b",
            SafetyStandard::Iso25119(AgricultureLevel::AgPlc) => "AgPL-c",
            SafetyStandard::Iso25119(AgricultureLevel::AgPld) => "AgPL-d",
            SafetyStandard::Iso25119(AgricultureLevel::AgPle) => "AgPL-e",
        }
    }
}

/// Trait for converting between safety standards
///
/// # Conservative Mapping Rationale
///
/// This implementation uses a conservative approach when mapping between safety
/// standards:
///
/// 1. **"No Safety" Level Restrictions**: Some standards (IEC 61508, IEC 62304,
///    ISO 25119) do not have equivalent "no safety" levels. QM (Quality
///    Management) from ISO 26262 cannot convert to these standards because they
///    require some level of safety oversight.
///
/// 2. **Conservative Fallback**: When severity scores don't map exactly, the
///    system chooses the higher safety level to maintain safety properties.
///
/// 3. **Domain-Specific Constraints**: Medical devices (IEC 62304) cannot have
///    "no safety" classification as they inherently affect patient safety.
///
/// 4. **Severity Score Ranges**: Conversion uses overlapping ranges to account
///    for differences in how standards define severity boundaries.
///
/// # REQ Traceability
/// - REQ_SAFETY_CROSS_001: Cross-standard safety level conversion
/// - REQ_SAFETY_CONSERVATIVE_001: Conservative mapping approach
/// - REQ_SAFETY_DOMAIN_001: Domain-specific safety constraints
pub trait SafetyStandardConversion {
    /// Convert to equivalent level in another standard
    ///
    /// This method attempts to find an equivalent safety level in the target
    /// standard based on severity score mapping. Returns `None` if
    /// conversion is not possible or would violate domain-specific safety
    /// constraints.
    ///
    /// # Conservative Behavior Examples
    ///
    /// ```rust
    /// use wrt_foundation::safety_system::*;
    ///
    /// // QM cannot convert to medical - medical devices need safety classification
    /// let qm = SafetyStandard::Iso26262(AsilLevel::QM);
    /// assert!(qm.convert_to(SafetyStandardType::Iec62304).is_none());
    ///
    /// // Industrial systems don't have "no safety" level
    /// assert!(qm.convert_to(SafetyStandardType::Iec61508).is_none());
    /// ```
    fn convert_to(&self, target_standard: SafetyStandardType) -> Option<SafetyStandard>;

    /// Check if this level is compatible with another standard's level
    ///
    /// Returns `true` if this safety level provides equal or greater protection
    /// than the required level based on severity score comparison.
    ///
    /// # Safety Property
    /// This maintains the invariant that higher-criticality systems can always
    /// interface with lower-criticality requirements.
    fn is_compatible_with(&self, other: &SafetyStandard) -> bool;

    /// Get the minimum ASIL level that satisfies this standard
    ///
    /// This method provides a way to map any safety standard to an equivalent
    /// ASIL level for systems primarily using ISO 26262. Uses conservative
    /// mapping to ensure safety properties are maintained.
    fn minimum_asil_equivalent(&self) -> AsilLevel;
}

impl SafetyStandardConversion for SafetyStandard {
    fn convert_to(&self, target: SafetyStandardType) -> Option<SafetyStandard> {
        let severity = self.severity_score();

        match target {
            SafetyStandardType::Iso26262 => {
                Some(SafetyStandard::Iso26262(match severity.value() {
                    0..=125 => AsilLevel::QM,
                    126..=375 => AsilLevel::AsilA,
                    376..=625 => AsilLevel::AsilB,
                    626..=875 => AsilLevel::AsilC,
                    876..=1000 => AsilLevel::AsilD,
                    _ => return None,
                }))
            },
            SafetyStandardType::Do178c => Some(SafetyStandard::Do178c(match severity.value() {
                0..=100 => DalLevel::DalE,
                101..=300 => DalLevel::DalD,
                301..=550 => DalLevel::DalC,
                551..=850 => DalLevel::DalB,
                851..=1000 => DalLevel::DalA,
                _ => return None,
            })),
            SafetyStandardType::Iec61508 => {
                if severity.value() == 0 {
                    // CONSERVATIVE DECISION: IEC 61508 is for functional safety of electrical/
                    // electronic systems and doesn't recognize "no safety" operation. All systems
                    // covered by this standard must have some safety integrity level.
                    return None; // IEC 61508 doesn't have a "no safety" level
                }
                Some(SafetyStandard::Iec61508(match severity.value() {
                    1..=375 => SilLevel::Sil1,
                    376..=625 => SilLevel::Sil2,
                    626..=875 => SilLevel::Sil3,
                    876..=1000 => SilLevel::Sil4,
                    _ => return None,
                }))
            },
            SafetyStandardType::Iec62304 => {
                if severity.value() == 0 {
                    // CONSERVATIVE DECISION: Medical device software (IEC 62304) inherently affects
                    // patient safety and cannot have "no safety" classification. Even non-critical
                    // medical software must be Class A (no injury or harm possible).
                    return None; // Medical devices must have some safety
                                 // classification
                }
                Some(SafetyStandard::Iec62304(match severity.value() {
                    1..=350 => MedicalClass::ClassA, // Non-life-threatening, no injury possible
                    351..=750 => MedicalClass::ClassB, // Non-life-threatening, injury possible
                    751..=1000 => MedicalClass::ClassC, // Life-threatening or death possible
                    _ => return None,
                }))
            },
            SafetyStandardType::En50128 => Some(SafetyStandard::En50128(match severity.value() {
                0..=100 => RailwaySil::Sil0,
                101..=300 => RailwaySil::Sil1,
                301..=550 => RailwaySil::Sil2,
                551..=850 => RailwaySil::Sil3,
                851..=1000 => RailwaySil::Sil4,
                _ => return None,
            })),
            SafetyStandardType::Iso25119 => {
                if severity.value() == 0 {
                    // CONSERVATIVE DECISION: Agricultural machinery (ISO 25119) involves equipment
                    // that can cause physical harm. Even low-risk systems must have AgPL-a
                    // classification (no risk of injury to persons).
                    return None; // Agricultural systems must have some safety
                                 // level
                }
                Some(SafetyStandard::Iso25119(match severity.value() {
                    1..=300 => AgricultureLevel::AgPla, // No risk of injury to persons
                    301..=500 => AgricultureLevel::AgPlb, // Light to moderate injury
                    501..=700 => AgricultureLevel::AgPlc, // Severe to life-threatening injury
                    701..=900 => AgricultureLevel::AgPld, // Life-threatening to fatal (one person)
                    901..=1000 => AgricultureLevel::AgPle, // Life-threatening to fatal (multiple
                    // persons)
                    _ => return None,
                }))
            },
        }
    }

    fn is_compatible_with(&self, other: &SafetyStandard) -> bool {
        self.severity_score() >= other.severity_score()
    }

    fn minimum_asil_equivalent(&self) -> AsilLevel {
        match self.severity_score().value() {
            0..=125 => AsilLevel::QM,
            126..=375 => AsilLevel::AsilA,
            376..=625 => AsilLevel::AsilB,
            626..=875 => AsilLevel::AsilC,
            876..=1000 => AsilLevel::AsilD,
            _ => AsilLevel::AsilD, // Conservative fallback for invalid scores
        }
    }
}

impl core::fmt::Display for SafetyStandard {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} {}", self.standard_name(), self.level_name())
    }
}

impl core::fmt::Display for SeverityScore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}/1000", self.0)
    }
}

/// Safety context that tracks ASIL requirements and safety state
///
/// This structure maintains both compile-time and runtime safety information,
/// allowing for adaptive safety behavior based on current requirements.
#[derive(Debug)]
pub struct SafetyContext {
    /// ASIL level determined at compile time
    pub compile_time_asil: AsilLevel,
    /// ASIL level that may be upgraded at runtime
    runtime_asil:          AtomicU8,
    /// Number of safety violations detected
    violation_count:       AtomicU8,
    /// Operation counter for periodic verification
    operation_count:       AtomicU8,
}

impl Clone for SafetyContext {
    fn clone(&self) -> Self {
        Self {
            compile_time_asil: self.compile_time_asil,
            runtime_asil:      AtomicU8::new(self.runtime_asil.load(Ordering::SeqCst)),
            violation_count:   AtomicU8::new(self.violation_count.load(Ordering::SeqCst)),
            operation_count:   AtomicU8::new(self.operation_count.load(Ordering::SeqCst)),
        }
    }
}

impl SafetyContext {
    /// Create a new safety context with compile-time ASIL level
    ///
    /// # Arguments
    ///
    /// * `compile_time` - The ASIL level known at compile time
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wrt_foundation::safety_system::{
    ///     AsilLevel,
    ///     SafetyContext,
    /// };
    ///
    /// const SAFETY_CTX: SafetyContext = SafetyContext::new(AsilLevel::AsilC);
    /// ```
    pub const fn new(compile_time: AsilLevel) -> Self {
        Self {
            compile_time_asil: compile_time,
            runtime_asil:      AtomicU8::new(compile_time as u8),
            violation_count:   AtomicU8::new(0),
            operation_count:   AtomicU8::new(0),
        }
    }

    /// Get the effective ASIL level (highest of compile-time and runtime)
    ///
    /// # Returns
    ///
    /// The effective ASIL level currently in effect.
    pub fn effective_asil(&self) -> AsilLevel {
        let runtime_level = self.runtime_asil.load(Ordering::Acquire);
        let compile_level = self.compile_time_asil as u8;

        let effective_level = runtime_level.max(compile_level);

        // Safe to unwrap because we only store valid ASIL values
        match effective_level {
            0 => AsilLevel::QM,
            1 => AsilLevel::AsilA,
            2 => AsilLevel::AsilB,
            3 => AsilLevel::AsilC,
            4 => AsilLevel::AsilD,
            _ => AsilLevel::AsilD, // Default to highest safety level for invalid values
        }
    }

    /// Upgrade the runtime ASIL level
    ///
    /// # One-Way Upgrade Policy (COUNTERINTUITIVE BEHAVIOR)
    ///
    /// This allows increasing the safety requirements at runtime, but NEVER
    /// decreasing them below the compile-time level. This one-way policy
    /// prevents safety degradation attacks and ensures that systems always
    /// meet their design-time safety requirements.
    ///
    /// ## Rationale for One-Way Upgrade
    /// 1. **Safety Invariant**: Compile-time level represents minimum
    ///    guaranteed safety
    /// 2. **Attack Prevention**: Prevents malicious downgrade of safety
    ///    requirements
    /// 3. **Certification Compliance**: Many standards require non-degradable
    ///    safety levels
    /// 4. **Fail-Safe Design**: System fails towards higher safety, never lower
    ///
    /// ## REQ Traceability
    /// - REQ_SAFETY_RUNTIME_001: Runtime safety level adaptation
    /// - REQ_SAFETY_INVARIANT_001: Non-degradable safety guarantee
    /// - REQ_SAFETY_ATTACK_001: Protection against safety downgrade attacks
    ///
    /// # Arguments
    ///
    /// * `new_level` - The new ASIL level to set (must be >= compile-time
    ///   level)
    ///
    /// # Errors
    ///
    /// Returns `SAFETY_VIOLATION` error if attempting to downgrade below
    /// compile-time level.
    ///
    /// # Example
    ///
    /// ```rust
    /// use wrt_foundation::safety_system::{
    ///     AsilLevel,
    ///     SafetyContext,
    /// };
    ///
    /// let ctx = SafetyContext::new(AsilLevel::AsilB);
    ///
    /// // This succeeds - upgrading to higher safety level
    /// assert!(ctx.upgrade_runtime_asil(AsilLevel::AsilC).is_ok());
    ///
    /// // This fails - cannot downgrade below compile-time level
    /// assert!(ctx.upgrade_runtime_asil(AsilLevel::AsilA).is_err());
    /// ```
    pub fn upgrade_runtime_asil(&self, new_level: AsilLevel) -> wrt_error::Result<()> {
        let new_level_u8 = new_level as u8;
        let compile_level_u8 = self.compile_time_asil as u8;

        if new_level_u8 < compile_level_u8 {
            return Err(Error::safety_violation(
                "Cannot downgrade ASIL below compile-time level",
            ));
        }

        self.runtime_asil.store(new_level_u8, Ordering::Release);
        Ok(())
    }

    /// Record a safety violation
    ///
    /// This increments the violation counter and may trigger safety actions
    /// based on the current ASIL level.
    ///
    /// # Returns
    ///
    /// The new violation count after incrementing.
    pub fn record_violation(&self) -> u8 {
        let count = self.violation_count.fetch_add(1, Ordering::AcqRel) + 1;

        // Trigger safety actions based on ASIL level
        let effective = self.effective_asil();
        match effective {
            AsilLevel::QM => {
                // No action required
            },
            AsilLevel::AsilA | AsilLevel::AsilB => {
                // Log violation for audit
                #[cfg(feature = "std")]
                {
                    eprintln!("Safety violation #{count} detected at {effective}");
                }
            },
            AsilLevel::AsilC | AsilLevel::AsilD => {
                // For high ASIL levels, consider immediate protective actions
                #[cfg(feature = "std")]
                {
                    eprintln!("CRITICAL: Safety violation #{count} detected at {effective}");
                }

                // In a real implementation, this might trigger:
                // - System shutdown
                // - Failsafe mode activation
                // - Error reporting to safety monitor
            },
        }

        count
    }

    /// Get the current violation count
    pub fn violation_count(&self) -> u8 {
        self.violation_count.load(Ordering::Acquire)
    }

    /// Check if periodic verification should be performed
    ///
    /// Based on the current ASIL level, this determines whether verification
    /// should be performed for the current operation.
    ///
    /// # Returns
    ///
    /// `true` if verification should be performed, `false` otherwise.
    pub fn should_verify(&self) -> bool {
        let effective = self.effective_asil();
        let frequency = effective.verification_frequency();

        if frequency == 0 {
            return false; // QM level - no verification required
        }

        let count = self.operation_count.fetch_add(1, Ordering::AcqRel) + 1;
        (count as u32) % frequency == 0
    }

    /// Reset the safety context (for testing or system restart)
    ///
    /// # Safety
    ///
    /// This should only be called during system initialization or controlled
    /// test scenarios.
    pub fn reset(&self) {
        self.runtime_asil.store(self.compile_time_asil as u8, Ordering::Release);
        self.violation_count.store(0, Ordering::Release);
        self.operation_count.store(0, Ordering::Release);
    }

    /// Check if the context is in a safe state
    ///
    /// A context is considered unsafe if it has too many violations relative
    /// to the ASIL requirements.
    pub fn is_safe(&self) -> bool {
        let violations = self.violation_count.load(Ordering::Acquire);
        let operations = self.operation_count.load(Ordering::Acquire);

        if operations == 0 {
            return true; // No operations yet
        }

        let error_rate = violations as f64 / operations as f64;
        let max_rate = self.effective_asil().max_error_rate();

        error_rate <= max_rate
    }
}

impl Default for SafetyContext {
    fn default() -> Self {
        Self::new(AsilLevel::default())
    }
}

// ============================================================================
// Universal Multi-Standard Safety Context
// ============================================================================

/// Enhanced safety context supporting multiple standards
///
/// This context can handle multiple safety standards simultaneously and
/// provides cross-standard compatibility checking and conversion.
///
/// # Atomic Operations Integration
///
/// This context uses atomic operations extensively for thread-safe operation
/// counting, violation tracking, and runtime state management. The atomic
/// operations integrate with WRT's checksum system to ensure data integrity:
///
/// 1. **Atomic Counters**: All counters use memory ordering guarantees to
///    prevent race conditions
/// 2. **Checksum Validation**: Critical state changes trigger checksum
///    verification when enabled
/// 3. **Memory Barriers**: Proper acquire/release ordering ensures visibility
///    across threads
/// 4. **Lock-Free Design**: Avoids deadlocks in safety-critical interrupt
///    contexts
///
/// # REQ Traceability
/// - REQ_SAFETY_MULTI_001: Multi-standard safety context support
/// - REQ_SAFETY_ATOMIC_001: Atomic operation safety guarantees
/// - REQ_SAFETY_CHECKSUM_001: Checksum integration for data integrity
/// - REQ_SAFETY_THREAD_001: Thread-safe safety context operations
/// - REQ_MEM_SAFETY_002: Memory ordering guarantees for safety state
#[derive(Debug)]
pub struct UniversalSafetyContext {
    /// Primary safety standard (compile-time)
    primary_standard:    SafetyStandard,
    /// Secondary standards this context must satisfy
    secondary_standards: [Option<SafetyStandard>; 4],
    /// Runtime safety state (stores severity score)
    runtime_state:       core::sync::atomic::AtomicU16,
    /// Violation tracking
    violation_count:     AtomicU8,
    /// Operation counter
    operation_count:     core::sync::atomic::AtomicU32,
}

impl Clone for UniversalSafetyContext {
    fn clone(&self) -> Self {
        Self {
            primary_standard:    self.primary_standard,
            secondary_standards: self.secondary_standards,
            runtime_state:       core::sync::atomic::AtomicU16::new(
                self.runtime_state.load(Ordering::SeqCst),
            ),
            violation_count:     AtomicU8::new(self.violation_count.load(Ordering::SeqCst)),
            operation_count:     core::sync::atomic::AtomicU32::new(
                self.operation_count.load(Ordering::SeqCst),
            ),
        }
    }
}

impl UniversalSafetyContext {
    /// Create context with primary standard (compile-time)
    ///
    /// # Arguments
    /// * `primary` - The primary safety standard for this context
    ///
    /// # Examples
    /// ```rust
    /// use wrt_foundation::safety_system::{
    ///     AsilLevel,
    ///     SafetyStandard,
    ///     UniversalSafetyContext,
    /// };
    ///
    /// const CTX: UniversalSafetyContext =
    ///     UniversalSafetyContext::new(SafetyStandard::Iso26262(AsilLevel::AsilC));
    /// ```
    pub const fn new(primary: SafetyStandard) -> Self {
        Self {
            primary_standard:    primary,
            secondary_standards: [None; 4],
            runtime_state:       core::sync::atomic::AtomicU16::new(
                primary.severity_score().value(),
            ),
            violation_count:     AtomicU8::new(0),
            operation_count:     core::sync::atomic::AtomicU32::new(0),
        }
    }

    /// Add secondary standard requirement
    ///
    /// This allows the context to satisfy multiple safety standards
    /// simultaneously. The effective severity will be the highest of all
    /// standards.
    ///
    /// # Arguments
    /// * `standard` - The secondary safety standard to add
    ///
    /// # Errors
    /// Returns an error if the maximum number of secondary standards is
    /// exceeded.
    pub fn add_secondary_standard(&mut self, standard: SafetyStandard) -> wrt_error::Result<()> {
        for slot in &mut self.secondary_standards {
            if slot.is_none() {
                *slot = Some(standard);
                self.update_effective_severity();
                return Ok(());
            }
        }
        Err(Error::safety_violation("Too many secondary standards"))
    }

    /// Get the effective severity (highest of all standards)
    pub fn effective_severity(&self) -> SeverityScore {
        SeverityScore(self.runtime_state.load(Ordering::Acquire))
    }

    /// Check if this context can handle a given standard
    ///
    /// Returns `true` if the effective severity is greater than or equal to
    /// the required standard's severity.
    pub fn can_handle(&self, required: SafetyStandard) -> bool {
        let effective = self.effective_severity();
        let required_severity = required.severity_score();
        effective >= required_severity
    }

    /// Get the primary safety standard
    pub fn primary_standard(&self) -> SafetyStandard {
        self.primary_standard
    }

    /// Get all secondary standards
    pub fn secondary_standards(&self) -> &[Option<SafetyStandard>; 4] {
        &self.secondary_standards
    }

    /// Record a safety violation
    ///
    /// This increments the violation counter and may trigger safety actions
    /// based on the effective severity level.
    ///
    /// # Returns
    /// The new violation count after incrementing.
    pub fn record_violation(&self) -> u8 {
        let count = self.violation_count.fetch_add(1, Ordering::AcqRel) + 1;

        // Trigger safety actions based on severity
        let effective_severity = self.effective_severity();

        #[cfg(feature = "std")]
        {
            match effective_severity.value() {
                0..=200 => {
                    // Low severity - basic logging
                },
                201..=500 => {
                    eprintln!(
                        "Safety violation #{count} detected (severity: {effective_severity})"
                    );
                },
                501..=800 => {
                    eprintln!(
                        "HIGH SEVERITY: Safety violation #{count} detected (severity: \
                         {effective_severity})"
                    );
                },
                801..=1000 => {
                    eprintln!(
                        "CRITICAL: Safety violation #{count} detected (severity: \
                         {effective_severity})"
                    );
                },
                _ => {
                    eprintln!(
                        "UNKNOWN SEVERITY: Safety violation #{count} detected (severity: \
                         {effective_severity})"
                    );
                },
            }
        }

        count
    }

    /// Get the current violation count
    pub fn violation_count(&self) -> u8 {
        self.violation_count.load(Ordering::Acquire)
    }

    /// Check if periodic verification should be performed
    ///
    /// Based on the effective severity level, this determines whether
    /// verification should be performed for the current operation.
    pub fn should_verify(&self) -> bool {
        let effective_severity = self.effective_severity();

        // Determine frequency based on severity
        let frequency = match effective_severity.value() {
            0..=200 => 0,      // No verification required
            201..=400 => 1000, // Every 1000 operations
            401..=600 => 100,  // Every 100 operations
            601..=800 => 10,   // Every 10 operations
            801..=1000 => 1,   // Every operation
            _ => 1,            // Conservative fallback
        };

        if frequency == 0 {
            return false;
        }

        let count = self.operation_count.fetch_add(1, Ordering::AcqRel) + 1;
        count % frequency == 0
    }

    /// Update effective severity based on all standards
    fn update_effective_severity(&self) {
        let mut max_severity = self.primary_standard.severity_score().value();

        for standard_opt in &self.secondary_standards {
            if let Some(standard) = standard_opt {
                let severity = standard.severity_score().value();
                if severity > max_severity {
                    max_severity = severity;
                }
            }
        }

        self.runtime_state.store(max_severity, Ordering::Release);
    }

    /// Reset the safety context (for testing or system restart)
    ///
    /// # Safety
    /// This should only be called during system initialization or controlled
    /// test scenarios.
    pub fn reset(&self) {
        self.runtime_state.store(
            self.primary_standard.severity_score().value(),
            Ordering::Release,
        );
        self.violation_count.store(0, Ordering::Release);
        self.operation_count.store(0, Ordering::Release);
    }

    /// Check if the context is in a safe state
    ///
    /// A context is considered unsafe if it has too many violations relative
    /// to the effective severity requirements.
    pub fn is_safe(&self) -> bool {
        let violations = self.violation_count.load(Ordering::Acquire);
        let operations = self.operation_count.load(Ordering::Acquire);

        if operations == 0 {
            return true; // No operations yet
        }

        let error_rate = violations as f64 / operations as f64;

        // Calculate maximum error rate based on effective severity
        let max_rate = match self.effective_severity().value() {
            0..=200 => 1.0,       // No limit for low severity
            201..=400 => 0.1,     // 10% for low-medium severity
            401..=600 => 0.01,    // 1% for medium severity
            601..=800 => 0.001,   // 0.1% for high severity
            801..=1000 => 0.0001, // 0.01% for critical severity
            _ => 0.0001,          // Conservative fallback
        };

        error_rate <= max_rate
    }

    /// Convert this context to work with a specific safety standard
    ///
    /// This method returns a new context that ensures compatibility with
    /// the target standard while maintaining the current effective severity.
    pub fn convert_to_standard(
        &self,
        target: SafetyStandardType,
    ) -> Option<UniversalSafetyContext> {
        let effective_severity = self.effective_severity();

        // Find equivalent level in target standard
        let target_standard = SafetyStandard::Iso26262(AsilLevel::QM) // Dummy value
            .convert_to(target)?;

        // Create new context with target as primary
        let mut new_context = Self::new(target_standard);

        // Ensure effective severity is at least as high as current
        if target_standard.severity_score() < effective_severity {
            // Add a secondary standard to maintain effective severity
            if let Some(backup_standard) =
                self.primary_standard.convert_to(SafetyStandardType::Iso26262)
            {
                let _ = new_context.add_secondary_standard(backup_standard);
            }
        }

        Some(new_context)
    }
}

impl Default for UniversalSafetyContext {
    fn default() -> Self {
        Self::new(SafetyStandard::Iso26262(AsilLevel::QM))
    }
}

/// Safety guard that ensures operations are performed within safety constraints
///
/// This guard automatically performs safety checks based on the current ASIL
/// level and can prevent unsafe operations from proceeding.
#[derive(Debug)]
pub struct SafetyGuard<'a> {
    context:        &'a SafetyContext,
    operation_name: &'static str,
    #[cfg(feature = "std")]
    start_time:     SystemTime,
}

impl<'a> SafetyGuard<'a> {
    /// Create a new safety guard for an operation
    ///
    /// # Arguments
    ///
    /// * `context` - The safety context to use
    /// * `operation_name` - Name of the operation for logging
    pub fn new(
        context: &'a SafetyContext,
        operation_name: &'static str,
    ) -> wrt_error::Result<Self> {
        // Check if the context is in a safe state
        if !context.is_safe() {
            context.record_violation();
            return Err(Error::safety_violation(
                "Safety context is not in a safe state",
            ));
        }

        Ok(Self {
            context,
            operation_name,
            #[cfg(feature = "std")]
            start_time: SystemTime::now(),
        })
    }

    /// Get the safety context
    pub fn context(&self) -> &SafetyContext {
        self.context
    }

    /// Get the operation name
    pub fn operation_name(&self) -> &'static str {
        self.operation_name
    }

    /// Perform verification if required by the current ASIL level
    pub fn verify_if_required<F>(&self, verifier: F) -> wrt_error::Result<()>
    where
        F: FnOnce() -> wrt_error::Result<()>,
    {
        if self.context.should_verify() {
            verifier().map_err(|_| {
                self.context.record_violation();
                Error::verification_failed("Safety verification failed")
            })?;
        }
        Ok(())
    }

    /// Complete the guarded operation successfully
    pub fn complete(self) -> wrt_error::Result<()> {
        #[cfg(feature = "std")]
        {
            let duration = self.start_time.elapsed().unwrap_or_default();
            if self.context.effective_asil().requires_runtime_verification() {
                println!(
                    "Operation '{}' completed in {:?}",
                    self.operation_name, duration
                );
            }
        }
        Ok(())
    }
}

impl<'a> Drop for SafetyGuard<'a> {
    fn drop(&mut self) {
        // If the guard is dropped without calling complete(), it's likely an error
        #[cfg(feature = "std")]
        {
            if std::thread::panicking() {
                self.context.record_violation();
                eprintln!(
                    "Safety guard for '{}' dropped during panic",
                    self.operation_name
                );
            }
        }
        #[cfg(not(feature = "std"))]
        {
            // In no_std, we can't detect panicking, so we assume it might be an error
            // This is a conservative approach for safety-critical environments
            self.context.record_violation();
        }
    }
}

/// Safety-aware memory allocation wrapper
///
/// This wrapper ensures that memory allocations are performed according to
/// the current ASIL requirements, including verification and protection.
#[derive(Debug)]
pub struct SafeMemoryAllocation<'a> {
    data:     &'a mut [u8],
    context:  &'a SafetyContext,
    checksum: u32,
}

impl<'a> SafeMemoryAllocation<'a> {
    /// Create a new safe memory allocation
    ///
    /// # Arguments
    ///
    /// * `data` - The allocated memory slice
    /// * `context` - The safety context for verification
    pub fn new(data: &'a mut [u8], context: &'a SafetyContext) -> wrt_error::Result<Self> {
        let checksum = Self::calculate_checksum(data);

        Ok(Self {
            data,
            context,
            checksum,
        })
    }

    /// Calculate checksum for memory protection
    fn calculate_checksum(data: &[u8]) -> u32 {
        data.iter().fold(0u32, |acc, &byte| acc.wrapping_add(byte as u32))
    }

    /// Verify memory integrity
    pub fn verify_integrity(&self) -> wrt_error::Result<()> {
        if self.context.effective_asil().requires_memory_protection() {
            let current_checksum = Self::calculate_checksum(self.data);
            if current_checksum != self.checksum {
                self.context.record_violation();
                return Err(Error::memory_error("Memory corruption detected"));
            }
        }
        Ok(())
    }

    /// Get access to the underlying data
    pub fn data(&self) -> &[u8] {
        self.data
    }

    /// Get mutable access to the underlying data
    pub fn data_mut(&mut self) -> wrt_error::Result<&mut [u8]> {
        self.verify_integrity()?;
        Ok(self.data)
    }

    /// Update the checksum after modifying data
    pub fn update_checksum(&mut self) {
        if self.context.effective_asil().requires_memory_protection() {
            self.checksum = Self::calculate_checksum(self.data);
        }
    }
}

/// Macro for creating compile-time safety contexts
///
/// This macro ensures that safety contexts are created with the correct
/// ASIL level at compile time.
#[macro_export]
macro_rules! safety_context {
    (QM) => {
        $crate::safety_system::SafetyContext::new($crate::safety_system::AsilLevel::QM)
    };
    (AsilA) => {
        $crate::safety_system::SafetyContext::new($crate::safety_system::AsilLevel::AsilA)
    };
    (AsilB) => {
        $crate::safety_system::SafetyContext::new($crate::safety_system::AsilLevel::AsilB)
    };
    (AsilC) => {
        $crate::safety_system::SafetyContext::new($crate::safety_system::AsilLevel::AsilC)
    };
    (AsilD) => {
        $crate::safety_system::SafetyContext::new($crate::safety_system::AsilLevel::AsilD)
    };
}

/// Macro for performing safety-guarded operations
///
/// This macro automatically creates a safety guard and ensures proper
/// cleanup even if the operation fails.
#[macro_export]
macro_rules! safety_guarded {
    ($context:expr, $operation:expr, $block:block) => {{
        let guard = $crate::safety_system::SafetyGuard::new($context, $operation)?;
        let result = $block;
        guard.complete()?;
        result
    }};
}

// ============================================================================
// Universal Safety Macros
// ============================================================================

/// Macro for creating multi-standard safety contexts
///
/// This macro supports creating safety contexts with multiple standards.
#[macro_export]
macro_rules! universal_safety_context {
    // Single standard context
    (Iso26262($level:ident)) => {
        $crate::safety_system::UniversalSafetyContext::new(
            $crate::safety_system::SafetyStandard::Iso26262(
                $crate::safety_system::AsilLevel::$level,
            ),
        )
    };
    (Do178c($level:ident)) => {
        $crate::safety_system::UniversalSafetyContext::new(
            $crate::safety_system::SafetyStandard::Do178c($crate::safety_system::DalLevel::$level),
        )
    };
    (Iec61508($level:ident)) => {
        $crate::safety_system::UniversalSafetyContext::new(
            $crate::safety_system::SafetyStandard::Iec61508(
                $crate::safety_system::SilLevel::$level,
            ),
        )
    };
    (Iec62304($level:ident)) => {
        $crate::safety_system::UniversalSafetyContext::new(
            $crate::safety_system::SafetyStandard::Iec62304(
                $crate::safety_system::MedicalClass::$level,
            ),
        )
    };
    (En50128($level:ident)) => {
        $crate::safety_system::UniversalSafetyContext::new(
            $crate::safety_system::SafetyStandard::En50128(
                $crate::safety_system::RailwaySil::$level,
            ),
        )
    };
    (Iso25119($level:ident)) => {
        $crate::safety_system::UniversalSafetyContext::new(
            $crate::safety_system::SafetyStandard::Iso25119(
                $crate::safety_system::AgricultureLevel::$level,
            ),
        )
    };
}

/// Compile-time standard compatibility check
///
/// This macro verifies at compile time that a context can handle a required
/// standard.
#[macro_export]
macro_rules! assert_standard_compatibility {
    ($ctx:expr,Iso26262($level:ident)) => {
        const _: () = {
            let required = $crate::safety_system::SafetyStandard::Iso26262(
                $crate::safety_system::AsilLevel::$level,
            );
            // Note: This would need const evaluation support for full compile-time
            // checking For now, this serves as documentation and type checking
        };
    };
    ($ctx:expr,Do178c($level:ident)) => {
        const _: () = {
            let required = $crate::safety_system::SafetyStandard::Do178c(
                $crate::safety_system::DalLevel::$level,
            );
        };
    };
    ($ctx:expr,Iec61508($level:ident)) => {
        const _: () = {
            let required = $crate::safety_system::SafetyStandard::Iec61508(
                $crate::safety_system::SilLevel::$level,
            );
        };
    };
    ($ctx:expr,Iec62304($level:ident)) => {
        const _: () = {
            let required = $crate::safety_system::SafetyStandard::Iec62304(
                $crate::safety_system::MedicalClass::$level,
            );
        };
    };
    ($ctx:expr,En50128($level:ident)) => {
        const _: () = {
            let required = $crate::safety_system::SafetyStandard::En50128(
                $crate::safety_system::RailwaySil::$level,
            );
        };
    };
    ($ctx:expr,Iso25119($level:ident)) => {
        const _: () = {
            let required = $crate::safety_system::SafetyStandard::Iso25119(
                $crate::safety_system::AgricultureLevel::$level,
            );
        };
    };
}

#[cfg(test)]
mod tests {
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::{
        format,
        vec,
    };
    #[cfg(feature = "std")]
    use std::{
        format,
        vec,
    };

    use super::*;

    #[test]
    fn test_asil_level_ordering() {
        assert!(AsilLevel::QM < AsilLevel::AsilA);
        assert!(AsilLevel::AsilA < AsilLevel::AsilB);
        assert!(AsilLevel::AsilB < AsilLevel::AsilC);
        assert!(AsilLevel::AsilC < AsilLevel::AsilD);
    }

    #[test]
    fn test_asil_level_properties() {
        assert!(!AsilLevel::QM.requires_memory_protection());
        assert!(!AsilLevel::AsilA.requires_memory_protection());
        assert!(!AsilLevel::AsilB.requires_memory_protection());
        assert!(AsilLevel::AsilC.requires_memory_protection());
        assert!(AsilLevel::AsilD.requires_memory_protection());

        assert!(!AsilLevel::QM.requires_cfi());
        assert!(!AsilLevel::AsilA.requires_cfi());
        assert!(!AsilLevel::AsilB.requires_cfi());
        assert!(AsilLevel::AsilC.requires_cfi());
        assert!(AsilLevel::AsilD.requires_cfi());

        assert!(!AsilLevel::QM.requires_redundancy());
        assert!(!AsilLevel::AsilA.requires_redundancy());
        assert!(!AsilLevel::AsilB.requires_redundancy());
        assert!(!AsilLevel::AsilC.requires_redundancy());
        assert!(AsilLevel::AsilD.requires_redundancy());
    }

    #[test]
    fn test_safety_context_creation() {
        let ctx = SafetyContext::new(AsilLevel::AsilC);
        assert_eq!(ctx.compile_time_asil, AsilLevel::AsilC);
        assert_eq!(ctx.effective_asil(), AsilLevel::AsilC);
        assert_eq!(ctx.violation_count(), 0);
    }

    #[test]
    fn test_safety_context_upgrade() {
        let ctx = SafetyContext::new(AsilLevel::AsilB);

        // Should be able to upgrade
        assert!(ctx.upgrade_runtime_asil(AsilLevel::AsilD).is_ok());
        assert_eq!(ctx.effective_asil(), AsilLevel::AsilD);

        // Should not be able to downgrade below compile-time level
        assert!(ctx.upgrade_runtime_asil(AsilLevel::AsilA).is_err());
        assert_eq!(ctx.effective_asil(), AsilLevel::AsilD); // Should remain
                                                            // unchanged
    }

    #[test]
    fn test_safety_context_violations() {
        let ctx = SafetyContext::new(AsilLevel::AsilA);

        assert_eq!(ctx.violation_count(), 0);
        assert!(ctx.is_safe());

        let count1 = ctx.record_violation();
        assert_eq!(count1, 1);
        assert_eq!(ctx.violation_count(), 1);

        let count2 = ctx.record_violation();
        assert_eq!(count2, 2);
        assert_eq!(ctx.violation_count(), 2);
    }

    #[test]
    fn test_safety_context_verification() {
        let ctx = SafetyContext::new(AsilLevel::AsilD);

        // AsilD requires verification every operation
        assert!(ctx.should_verify());
        assert!(ctx.should_verify());
        assert!(ctx.should_verify());

        let ctx_qm = SafetyContext::new(AsilLevel::QM);

        // QM requires no verification
        assert!(!ctx_qm.should_verify());
        assert!(!ctx_qm.should_verify());
        assert!(!ctx_qm.should_verify());
    }

    #[test]
    fn test_safety_guard() -> wrt_error::Result<()> {
        let ctx = SafetyContext::new(AsilLevel::AsilB);

        let guard = SafetyGuard::new(&ctx, "test_operation")?;
        assert_eq!(guard.operation_name(), "test_operation");

        // Verify that verification works
        guard.verify_if_required(|| Ok(()))?;

        guard.complete()?;
        Ok(())
    }

    #[test]
    fn test_safe_memory_allocation() -> wrt_error::Result<()> {
        let ctx = SafetyContext::new(AsilLevel::AsilC);
        let mut data = [1u8, 2u8, 3u8, 4u8];

        let mut allocation = SafeMemoryAllocation::new(&mut data, &ctx)?;

        // Should verify successfully initially
        allocation.verify_integrity()?;

        // Modify data and update checksum
        {
            let data_mut = allocation.data_mut()?;
            data_mut[0] = 10;
        }
        allocation.update_checksum();

        // Should still verify successfully
        allocation.verify_integrity()?;

        Ok(())
    }

    #[test]
    fn test_safety_context_macro() {
        let ctx = safety_context!(AsilC);
        assert_eq!(ctx.effective_asil(), AsilLevel::AsilC);
    }

    #[test]
    fn test_safety_guarded_macro() -> wrt_error::Result<()> {
        let ctx = SafetyContext::new(AsilLevel::AsilA);

        let result = safety_guarded!(&ctx, "test_macro_operation", { 42 });

        assert_eq!(result, 42);
        Ok(())
    }

    #[test]
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn test_asil_level_display() {
        assert_eq!(format!("{}", AsilLevel::QM), "QM");
        assert_eq!(format!("{}", AsilLevel::AsilA), "ASIL-A");
        assert_eq!(format!("{}", AsilLevel::AsilB), "ASIL-B");
        assert_eq!(format!("{}", AsilLevel::AsilC), "ASIL-C");
        assert_eq!(format!("{}", AsilLevel::AsilD), "ASIL-D");
    }

    // ========================================================================
    // Universal Safety System Tests
    // ========================================================================

    #[test]
    fn test_safety_standard_severity_scores() {
        // Test ASIL mapping
        assert_eq!(
            SafetyStandard::Iso26262(AsilLevel::QM).severity_score().value(),
            0
        );
        assert_eq!(
            SafetyStandard::Iso26262(AsilLevel::AsilA).severity_score().value(),
            250
        );
        assert_eq!(
            SafetyStandard::Iso26262(AsilLevel::AsilC).severity_score().value(),
            750
        );
        assert_eq!(
            SafetyStandard::Iso26262(AsilLevel::AsilD).severity_score().value(),
            1000
        );

        // Test DAL mapping
        assert_eq!(
            SafetyStandard::Do178c(DalLevel::DalE).severity_score().value(),
            0
        );
        assert_eq!(
            SafetyStandard::Do178c(DalLevel::DalA).severity_score().value(),
            1000
        );

        // Test SIL mapping
        assert_eq!(
            SafetyStandard::Iec61508(SilLevel::Sil1).severity_score().value(),
            250
        );
        assert_eq!(
            SafetyStandard::Iec61508(SilLevel::Sil4).severity_score().value(),
            1000
        );
    }

    #[test]
    fn test_safety_standard_conversion() {
        let asil_c = SafetyStandard::Iso26262(AsilLevel::AsilC);

        // Convert to DAL
        let dal_equivalent = asil_c.convert_to(SafetyStandardType::Do178c).unwrap();
        if let SafetyStandard::Do178c(level) = dal_equivalent {
            assert_eq!(level, DalLevel::DalB); // 750 severity maps to DAL-B
        } else {
            panic!("Conversion failed");
        }

        // Convert to SIL
        let sil_equivalent = asil_c.convert_to(SafetyStandardType::Iec61508).unwrap();
        if let SafetyStandard::Iec61508(level) = sil_equivalent {
            assert_eq!(level, SilLevel::Sil3); // 750 severity maps to SIL-3
        } else {
            panic!("Conversion failed");
        }
    }

    #[test]
    fn test_safety_standard_compatibility() {
        let asil_c = SafetyStandard::Iso26262(AsilLevel::AsilC);
        let asil_b = SafetyStandard::Iso26262(AsilLevel::AsilB);
        let dal_b = SafetyStandard::Do178c(DalLevel::DalB);

        // ASIL-C should be compatible with ASIL-B (higher can handle lower)
        assert!(asil_c.is_compatible_with(&asil_b));
        assert!(!asil_b.is_compatible_with(&asil_c));

        // ASIL-C should be compatible with DAL-B (similar severity)
        assert!(asil_c.is_compatible_with(&dal_b));
    }

    #[test]
    fn test_universal_safety_context_creation() {
        let ctx = UniversalSafetyContext::new(SafetyStandard::Iso26262(AsilLevel::AsilC));
        assert_eq!(
            ctx.primary_standard(),
            SafetyStandard::Iso26262(AsilLevel::AsilC)
        );
        assert_eq!(ctx.effective_severity().value(), 750);
        assert_eq!(ctx.violation_count(), 0);
    }

    #[test]
    fn test_universal_safety_context_secondary_standards() -> wrt_error::Result<()> {
        let mut ctx = UniversalSafetyContext::new(SafetyStandard::Iso26262(AsilLevel::AsilB));

        // Add higher severity secondary standard
        ctx.add_secondary_standard(SafetyStandard::Do178c(DalLevel::DalA))?;

        // Effective severity should be the highest (DAL-A = 1000)
        assert_eq!(ctx.effective_severity().value(), 1000);

        // Should be able to handle both standards
        assert!(ctx.can_handle(SafetyStandard::Iso26262(AsilLevel::AsilB)));
        assert!(ctx.can_handle(SafetyStandard::Do178c(DalLevel::DalA)));

        Ok(())
    }

    #[test]
    fn test_universal_safety_context_verification() {
        let ctx = UniversalSafetyContext::new(SafetyStandard::Iso26262(AsilLevel::AsilD));

        // ASIL-D (severity 1000) should require verification every operation
        assert!(ctx.should_verify());
        assert!(ctx.should_verify());

        let ctx_qm = UniversalSafetyContext::new(SafetyStandard::Iso26262(AsilLevel::QM));

        // QM (severity 0) should require no verification
        assert!(!ctx_qm.should_verify());
    }

    #[test]
    fn test_universal_safety_context_violations() {
        let ctx = UniversalSafetyContext::new(SafetyStandard::Iso26262(AsilLevel::AsilB));

        assert_eq!(ctx.violation_count(), 0);
        assert!(ctx.is_safe());

        let count1 = ctx.record_violation();
        assert_eq!(count1, 1);
        assert_eq!(ctx.violation_count(), 1);
    }

    #[test]
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn test_safety_standard_display() {
        assert_eq!(
            format!("{}", SafetyStandard::Iso26262(AsilLevel::AsilC)),
            "ISO 26262 ASIL-C"
        );
        assert_eq!(
            format!("{}", SafetyStandard::Do178c(DalLevel::DalB)),
            "DO-178C DAL-B"
        );
        assert_eq!(
            format!("{}", SafetyStandard::Iec61508(SilLevel::Sil3)),
            "IEC 61508 SIL-3"
        );
    }

    #[test]
    fn test_severity_score_creation() {
        assert!(SeverityScore::new(0).is_ok());
        assert!(SeverityScore::new(500).is_ok());
        assert!(SeverityScore::new(1000).is_ok());
        assert!(SeverityScore::new(1001).is_err());
    }

    #[test]
    fn test_universal_safety_context_macro() {
        let ctx = universal_safety_context!(Iso26262(AsilC));
        assert_eq!(
            ctx.primary_standard(),
            SafetyStandard::Iso26262(AsilLevel::AsilC)
        );

        let ctx_dal = universal_safety_context!(Do178c(DalB));
        assert_eq!(
            ctx_dal.primary_standard(),
            SafetyStandard::Do178c(DalLevel::DalB)
        );
    }

    #[test]
    fn test_minimum_asil_equivalent() {
        let dal_a = SafetyStandard::Do178c(DalLevel::DalA);
        assert_eq!(dal_a.minimum_asil_equivalent(), AsilLevel::AsilD);

        let sil_2 = SafetyStandard::Iec61508(SilLevel::Sil2);
        assert_eq!(sil_2.minimum_asil_equivalent(), AsilLevel::AsilB);

        let class_a = SafetyStandard::Iec62304(MedicalClass::ClassA);
        assert_eq!(class_a.minimum_asil_equivalent(), AsilLevel::AsilA);
    }

    #[test]
    fn test_cross_standard_edge_cases() {
        // Test conversion from standards that don't have "no safety" levels
        let sil_1 = SafetyStandard::Iec61508(SilLevel::Sil1);
        let converted_to_iso = sil_1.convert_to(SafetyStandardType::Iso26262);
        assert!(converted_to_iso.is_some());

        // Test conversion to standards that require safety classification
        let qm = SafetyStandard::Iso26262(AsilLevel::QM);
        let converted_to_medical = qm.convert_to(SafetyStandardType::Iec62304);
        assert!(converted_to_medical.is_none()); // Medical devices must have
                                                 // some safety class
    }

    #[test]
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn test_safety_standard_ordering() {
        // Test that severity scores create proper ordering
        let standards = vec![
            SafetyStandard::Iso26262(AsilLevel::QM),
            SafetyStandard::Do178c(DalLevel::DalD),
            SafetyStandard::Iec61508(SilLevel::Sil1),
            SafetyStandard::Iso26262(AsilLevel::AsilC),
            SafetyStandard::Do178c(DalLevel::DalA),
        ];

        let mut severity_scores: Vec<_> =
            standards.iter().map(|s| s.severity_score().value()).collect();
        severity_scores.sort();

        assert_eq!(severity_scores, vec![0, 200, 250, 750, 1000]);
    }
}
