//! WIT source mapping for debugging integration
//!
//! This module provides source mapping capabilities between WIT source code,
//! AST nodes, and component binary representations for enhanced debugging.

#[cfg(feature = "std")]
use std::{boxed::Box, collections::BTreeMap, vec::Vec};
#[cfg(all(not(feature = "std")))]
use std::{boxed::Box, collections::BTreeMap, vec::Vec};

use wrt_error::{Error, Result};
/// Source location span (re-exported from wrt-format for consistency)
#[cfg(feature = "wit-integration")]
pub use wrt_format::ast::SourceSpan;
use wrt_foundation::{BoundedString, BoundedVec, NoStdProvider, prelude::*, safe_managed_alloc};

use crate::bounded_debug_infra;

/// Type identifier for mapping between AST and binary representations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeId(pub u32);

/// Function identifier for mapping between AST and binary representations  
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(pub u32);

/// Component identifier for tracking component boundaries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ComponentId(pub u32);

/// WIT source mapping information
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone)]
pub struct WitSourceMap {
    /// Maps binary offsets to source locations
    pub binary_to_source: BTreeMap<u32, SourceSpan>,

    /// Maps source locations to binary offsets
    pub source_to_binary: BTreeMap<SourceSpan, u32>,

    /// Maps type definitions to their source locations
    pub type_definitions: BTreeMap<TypeId, SourceSpan>,

    /// Maps function definitions to their source locations
    pub function_definitions: BTreeMap<FunctionId, SourceSpan>,

    /// Maps component boundaries to source locations
    pub component_boundaries: BTreeMap<ComponentId, SourceSpan>,

    /// Source file contents for displaying source context
    pub source_files: BTreeMap<u32, WitSourceFile>,
}

/// Information about a WIT source file
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone)]
pub struct WitSourceFile {
    /// File path or identifier
    pub path: BoundedString<256>,

    /// Source content lines for context display
    pub lines: Vec<BoundedString<1024>>,

    /// File size in bytes
    pub size: u32,
}

/// Type information for debugging
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone)]
pub struct WitTypeInfo {
    /// Type identifier
    pub id: TypeId,

    /// Type name
    pub name: BoundedString<64>,

    /// Type kind (record, variant, etc.)
    pub kind: WitTypeKind,

    /// Source location where type is defined
    pub definition_span: SourceSpan,

    /// Usage locations
    pub usage_spans: Vec<SourceSpan>,
}

/// Kind of WIT type for debugging display
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone, PartialEq)]
pub enum WitTypeKind {
    /// Primitive type (u32, string, etc.)
    Primitive(BoundedString<16>),

    /// Record type with field count
    Record(u32),

    /// Variant type with case count
    Variant(u32),

    /// Enum type with case count
    Enum(u32),

    /// Flags type with flag count
    Flags(u32),

    /// Resource type
    Resource,

    /// Function type
    Function,

    /// Interface type
    Interface,

    /// World type
    World,
}

/// Component boundary information for debugging
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone)]
pub struct ComponentBoundary {
    /// Component identifier
    pub id: ComponentId,

    /// Component name if available
    pub name: Option<BoundedString<64>>,

    /// Start offset in binary
    pub start_offset: u32,

    /// End offset in binary
    pub end_offset: u32,

    /// Source span in WIT
    pub source_span: SourceSpan,

    /// Memory regions owned by this component
    pub memory_regions: Vec<MemoryRegion>,
}

/// Memory region information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRegion {
    /// Start address
    pub start: u32,

    /// End address (exclusive)
    pub end: u32,

    /// Region type
    pub region_type: MemoryRegionType,
}

/// Types of memory regions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionType {
    /// Linear memory
    Linear,

    /// Table memory
    Table,

    /// Stack memory
    Stack,

    /// Component instance data
    Instance,
}

/// Diagnostic information mapped to source
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone)]
pub struct WitDiagnostic {
    /// Source location of the diagnostic
    pub span: SourceSpan,

    /// Diagnostic severity
    pub severity: DiagnosticSeverity,

    /// Error/warning message
    pub message: BoundedString<512>,

    /// Optional suggested fix
    pub suggestion: Option<BoundedString<256>>,

    /// Related locations (for multi-span diagnostics)
    pub related: Vec<SourceSpan>,
}

/// Diagnostic severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

#[cfg(feature = "wit-integration")]
impl WitSourceMap {
    /// Create a new empty source map
    pub fn new() -> Self {
        Self {
            binary_to_source: BTreeMap::new(),
            source_to_binary: BTreeMap::new(),
            type_definitions: BTreeMap::new(),
            function_definitions: BTreeMap::new(),
            component_boundaries: BTreeMap::new(),
            source_files: BTreeMap::new(),
        }
    }

    /// Add a mapping between binary offset and source location
    pub fn add_binary_mapping(&mut self, binary_offset: u32, source_span: SourceSpan) {
        self.binary_to_source.insert(binary_offset, source_span);
        self.source_to_binary.insert(source_span, binary_offset);
    }

    /// Add a type definition mapping
    pub fn add_type_definition(&mut self, type_id: TypeId, source_span: SourceSpan) {
        self.type_definitions.insert(type_id, source_span);
    }

    /// Add a function definition mapping
    pub fn add_function_definition(&mut self, function_id: FunctionId, source_span: SourceSpan) {
        self.function_definitions.insert(function_id, source_span);
    }

    /// Add a component boundary mapping
    pub fn add_component_boundary(&mut self, component_id: ComponentId, source_span: SourceSpan) {
        self.component_boundaries.insert(component_id, source_span);
    }

    /// Add a source file
    pub fn add_source_file(&mut self, file_id: u32, source_file: WitSourceFile) {
        self.source_files.insert(file_id, source_file);
    }

    /// Get source location for a binary offset
    pub fn source_location_for_offset(&self, binary_offset: u32) -> Option<SourceSpan> {
        // Find the closest mapping at or before the offset
        self.binary_to_source.range(..=binary_offset).next_back().map(|(_, span)| *span)
    }

    /// Get binary offset for a source location
    pub fn binary_offset_for_source(&self, source_span: SourceSpan) -> Option<u32> {
        self.source_to_binary.get(&source_span).copied()
    }

    /// Get type definition location
    pub fn type_definition_location(&self, type_id: TypeId) -> Option<SourceSpan> {
        self.type_definitions.get(&type_id).copied()
    }

    /// Get function definition location
    pub fn function_definition_location(&self, function_id: FunctionId) -> Option<SourceSpan> {
        self.function_definitions.get(&function_id).copied()
    }

    /// Get component boundary information
    pub fn component_boundary(&self, component_id: ComponentId) -> Option<SourceSpan> {
        self.component_boundaries.get(&component_id).copied()
    }

    /// Get source file by file ID
    pub fn source_file(&self, file_id: u32) -> Option<&WitSourceFile> {
        self.source_files.get(&file_id)
    }

    /// Get source context around a span (for error display)
    pub fn source_context(&self, span: SourceSpan, context_lines: u32) -> Option<SourceContext> {
        let file = self.source_file(span.file_id)?;

        // Calculate line numbers (assuming 1-based)
        let mut current_offset = 0u32;
        let mut start_line = 0u32;
        let mut end_line = 0u32;

        for (line_idx, line) in file.lines.iter().enumerate() {
            let line_len = line.as_str().map(|s| s.len()).unwrap_or(0) as u32 + 1; // +1 for newline

            if current_offset <= span.start && span.start < current_offset + line_len {
                start_line = line_idx as u32;
            }
            if current_offset <= span.end && span.end <= current_offset + line_len {
                end_line = line_idx as u32;
                break;
            }

            current_offset += line_len;
        }

        // Expand context
        let context_start = start_line.saturating_sub(context_lines);
        let context_end = (end_line + context_lines).min(file.lines.len() as u32);

        let mut context_lines_vec = Vec::new();
        for i in context_start..context_end {
            if let Some(line) = file.lines.get(i as usize) {
                context_lines_vec.push(ContextLine {
                    line_number: i + 1, // 1-based line numbers
                    content: line.clone(),
                    is_highlighted: i >= start_line && i <= end_line,
                });
            }
        }

        Some(SourceContext {
            file_path: file.path.clone(),
            lines: context_lines_vec,
            highlighted_span: span,
        })
    }

    /// Map a runtime error to a source diagnostic
    pub fn map_error_to_diagnostic(
        &self,
        error: &Error,
        binary_offset: Option<u32>,
    ) -> Option<WitDiagnostic> {
        let span = if let Some(offset) = binary_offset {
            self.source_location_for_offset(offset)?
        } else {
            // Use a default span if no offset provided
            SourceSpan::empty()
        };

        let provider = safe_managed_alloc!(8192, CrateId::Debug)?;
        let message =
            BoundedString::try_from_str(&format!("Runtime error: {}", error), provider.clone())
                .unwrap_or_else(|_| {
                    BoundedString::try_from_str(
                        "Runtime error (message too long)",
                        provider.clone(),
                    )
                    .unwrap()
                });

        Some(WitDiagnostic {
            span,
            severity: DiagnosticSeverity::Error,
            message,
            suggestion: None,
            related: Vec::new(),
        })
    }
}

/// Source context for display
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone)]
pub struct SourceContext {
    /// File path
    pub file_path: BoundedString<256>,

    /// Context lines
    pub lines: Vec<ContextLine>,

    /// The highlighted span
    pub highlighted_span: SourceSpan,
}

/// A single line of source context
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone)]
pub struct ContextLine {
    /// Line number (1-based)
    pub line_number: u32,

    /// Line content
    pub content: BoundedString<1024>,

    /// Whether this line is highlighted (contains the error)
    pub is_highlighted: bool,
}

#[cfg(feature = "wit-integration")]
impl Default for WitSourceMap {
    fn default() -> Self {
        Self::new()
    }
}

impl WitSourceFile {
    /// Create a new source file from content
    #[cfg(feature = "wit-integration")]
    pub fn new(path: &str, content: &str) -> Result<Self> {
        let provider = safe_managed_alloc!(8192, CrateId::Debug)?;
        let path_bounded = BoundedString::try_from_str(path, provider.clone())
            .map_err(|_| Error::parse_error("Path too long"))?;

        let mut lines = Vec::new();
        for line in content.lines() {
            let line_bounded = BoundedString::try_from_str(line, provider.clone())
                .map_err(|_| Error::parse_error("Line too long"))?;
            lines.push(line_bounded);
        }

        Ok(Self {
            path: path_bounded,
            lines,
            size: content.len() as u32,
        })
    }

    /// Get line by line number (1-based)
    #[cfg(feature = "wit-integration")]
    pub fn line(&self, line_number: u32) -> Option<&BoundedString<1024>> {
        if line_number == 0 {
            return None;
        }
        self.lines.get((line_number - 1) as usize)
    }

    /// Get total number of lines
    #[cfg(feature = "wit-integration")]
    pub fn line_count(&self) -> u32 {
        self.lines.len() as u32
    }
}

impl ComponentBoundary {
    /// Check if an address is within this component's memory regions
    pub fn contains_address(&self, address: u32) -> bool {
        self.memory_regions
            .iter()
            .any(|region| address >= region.start && address < region.end)
    }

    /// Get memory region containing the given address
    pub fn memory_region_for_address(&self, address: u32) -> Option<&MemoryRegion> {
        self.memory_regions
            .iter()
            .find(|region| address >= region.start && address < region.end)
    }
}

impl MemoryRegion {
    /// Create a new memory region
    pub const fn new(start: u32, end: u32, region_type: MemoryRegionType) -> Self {
        Self {
            start,
            end,
            region_type,
        }
    }

    /// Get the size of this memory region
    pub const fn size(&self) -> u32 {
        self.end - self.start
    }

    /// Check if this region contains the given address
    pub const fn contains(&self, address: u32) -> bool {
        address >= self.start && address < self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "wit-integration")]
    #[test]
    fn test_source_map_basic() {
        let mut source_map = WitSourceMap::new();

        let span = SourceSpan::new(10, 20, 0);
        source_map.add_binary_mapping(100, span);

        assert_eq!(source_map.source_location_for_offset(100), Some(span));
        assert_eq!(source_map.binary_offset_for_source(span), Some(100));
    }

    #[cfg(feature = "wit-integration")]
    #[test]
    fn test_source_file() {
        let content = "line 1\nline 2\nline 3";
        let file = WitSourceFile::new("test.wit", content).unwrap();

        assert_eq!(file.line_count(), 3);
        assert_eq!(file.line(1).unwrap().as_str().unwrap(), "line 1");
        assert_eq!(file.line(2).unwrap().as_str().unwrap(), "line 2");
        assert_eq!(file.line(3).unwrap().as_str().unwrap(), "line 3");
        assert!(file.line(4).is_none());
    }

    #[test]
    fn test_memory_region() {
        let region = MemoryRegion::new(100, 200, MemoryRegionType::Linear);

        assert_eq!(region.size(), 100);
        assert!(region.contains(150));
        assert!(!region.contains(250));
    }

    #[test]
    fn test_component_boundary() {
        let mut boundary = ComponentBoundary {
            id: ComponentId(1),
            name: None,
            start_offset: 0,
            end_offset: 1000,
            source_span: SourceSpan::empty(),
            memory_regions: vec![
                MemoryRegion::new(100, 200, MemoryRegionType::Linear),
                MemoryRegion::new(300, 400, MemoryRegionType::Stack),
            ],
        };

        assert!(boundary.contains_address(150));
        assert!(boundary.contains_address(350));
        assert!(!boundary.contains_address(250));

        let region = boundary.memory_region_for_address(150).unwrap();
        assert_eq!(region.region_type, MemoryRegionType::Linear);
    }
}
