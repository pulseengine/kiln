// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! DWARF type definitions

use wrt_foundation::prelude::*;

/// Section offset information
#[derive(Debug, Clone, Copy)]
pub struct DebugSectionRef {
    /// Offset in module bytes
    pub offset: u32,
    /// Size of the section
    pub size: u32,
}

/// DWARF section offsets
#[derive(Debug, Default)]
pub struct DwarfSections {
    /// .debug_info section
    pub debug_info: Option<DebugSectionRef>,
    /// .debug_abbrev section
    pub debug_abbrev: Option<DebugSectionRef>,
    /// .debug_line section
    pub debug_line: Option<DebugSectionRef>,
    /// .debug_str section
    pub debug_str: Option<DebugSectionRef>,
    /// .debug_line_str section
    pub debug_line_str: Option<DebugSectionRef>,
}

/// Debug section representation for storage
#[derive(Debug, Clone)]
pub struct DebugSection {
    /// Section name (e.g., ".debug_line")
    pub name: &'static str,
    /// Offset in the module
    pub offset: u32,
    /// Size of the section
    pub size: u32,
}
