//! WIT-aware debugger integration
//!
//! This module extends the runtime debugger with WIT source mapping
//! capabilities, allowing debugging at the WIT source level rather than just
//! binary level.

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, collections::BTreeMap, format, vec::Vec};
#[cfg(feature = "std")]
use std::{boxed::Box, collections::BTreeMap, format, vec::Vec};

use wrt_error::{Error, Result};
use wrt_foundation::{BoundedString, CrateId, NoStdProvider, prelude::*, safe_managed_alloc};

use crate::bounded_debug_infra;
// Import WIT source mapping
#[cfg(any(feature = "wit-integration", feature = "std"))]
use crate::wit_source_map::{
    ComponentBoundary, ComponentId, FunctionId, SourceSpan, TypeId, WitDiagnostic, WitSourceMap,
    WitTypeInfo,
};
// Import from existing modules
#[cfg(feature = "runtime-debug")]
use crate::{
    Breakpoint, DebugAction, DebugError, DebugMemory, DebuggableRuntime, RuntimeDebugger,
    RuntimeState,
};

/// Component error for WIT debugging
#[derive(Debug, Clone)]
pub struct ComponentError {
    /// Error message
    pub message: BoundedString<512>,
    /// Binary offset where error occurred
    pub binary_offset: Option<u32>,
    /// Component that generated the error
    pub component_id: Option<ComponentId>,
    /// Function that generated the error
    pub function_id: Option<FunctionId>,
}

/// WIT-aware debugger trait that extends RuntimeDebugger
#[cfg(feature = "wit-integration")]
pub trait WitAwareDebugger: RuntimeDebugger {
    /// Get source location for a runtime error
    fn source_location_for_error(&self, error: &ComponentError) -> Option<SourceSpan>;

    /// Get WIT type information at a binary offset
    fn wit_type_at_offset(&self, binary_offset: u32) -> Option<WitTypeInfo>;

    /// Get component boundary information for an address
    fn component_boundary_info(&self, addr: u32) -> Option<ComponentBoundary>;

    /// Map a runtime error to a WIT diagnostic
    fn map_to_wit_diagnostic(&self, error: &ComponentError) -> Option<WitDiagnostic>;

    /// Get WIT function name for a function ID
    fn wit_function_name(&self, function_id: FunctionId) -> Option<BoundedString<64>>;

    /// Get WIT type name for a type ID
    fn wit_type_name(&self, type_id: TypeId) -> Option<BoundedString<64>>;
}

/// Implementation of WIT-aware debugger
#[cfg(feature = "wit-integration")]
#[derive(Debug)]
pub struct WitDebugger {
    /// WIT source mapping
    source_map: WitSourceMap,

    /// Component metadata
    components: BTreeMap<ComponentId, ComponentMetadata>,

    /// Function metadata
    functions: BTreeMap<FunctionId, FunctionMetadata>,

    /// Type metadata
    types: BTreeMap<TypeId, TypeMetadata>,

    /// Current execution context
    current_component: Option<ComponentId>,

    /// Breakpoints by source location
    source_breakpoints: BTreeMap<SourceSpan, u32>,

    /// Step mode for source-level stepping
    step_mode: WitStepMode,
}

/// Metadata about a component for debugging
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone)]
pub struct ComponentMetadata {
    /// Component name
    pub name: BoundedString<64>,

    /// Source span in WIT
    pub source_span: SourceSpan,

    /// Binary start offset
    pub binary_start: u32,

    /// Binary end offset
    pub binary_end: u32,

    /// Exported functions
    pub exports: Vec<FunctionId>,

    /// Imported functions
    pub imports: Vec<FunctionId>,
}

/// Metadata about a function for debugging
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone)]
pub struct FunctionMetadata {
    /// Function name
    pub name: BoundedString<64>,

    /// Source span in WIT
    pub source_span: SourceSpan,

    /// Binary offset
    pub binary_offset: u32,

    /// Parameter types
    pub param_types: Vec<TypeId>,

    /// Return types
    pub return_types: Vec<TypeId>,

    /// Whether function is async
    pub is_async: bool,
}

/// Metadata about a type for debugging
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone)]
pub struct TypeMetadata {
    /// Type name
    pub name: BoundedString<64>,

    /// Source span in WIT
    pub source_span: SourceSpan,

    /// Type kind (record, variant, etc.)
    pub kind: WitTypeKind,

    /// Size in bytes (if known)
    pub size: Option<u32>,
}

/// WIT type kind for debugging
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone, PartialEq)]
pub enum WitTypeKind {
    /// Primitive type
    Primitive,
    /// Record type
    Record,
    /// Variant type
    Variant,
    /// Enum type
    Enum,
    /// Flags type
    Flags,
    /// Resource type
    Resource,
    /// Function type
    Function,
    /// Interface type
    Interface,
    /// World type
    World,
}

/// Step mode for WIT debugging
#[cfg(feature = "wit-integration")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WitStepMode {
    /// Step instruction by instruction
    Instruction,
    /// Step line by line in WIT source
    SourceLine,
    /// Step over WIT function calls
    SourceStepOver,
    /// Step out of current WIT function
    SourceStepOut,
    /// Continue execution
    Continue,
}

#[cfg(feature = "wit-integration")]
impl WitDebugger {
    /// Create a new WIT-aware debugger
    pub fn new() -> Self {
        Self {
            source_map: WitSourceMap::new(),
            components: BTreeMap::new(),
            functions: BTreeMap::new(),
            types: BTreeMap::new(),
            current_component: None,
            source_breakpoints: BTreeMap::new(),
            step_mode: WitStepMode::Continue,
        }
    }

    /// Add component metadata
    pub fn add_component(&mut self, id: ComponentId, metadata: ComponentMetadata) {
        // Add to source map
        self.source_map.add_component_boundary(id, metadata.source_span);

        // Store metadata
        self.components.insert(id, metadata);
    }

    /// Add function metadata
    pub fn add_function(&mut self, id: FunctionId, metadata: FunctionMetadata) {
        // Add to source map
        self.source_map.add_function_definition(id, metadata.source_span);
        self.source_map.add_binary_mapping(metadata.binary_offset, metadata.source_span);

        // Store metadata
        self.functions.insert(id, metadata);
    }

    /// Add type metadata
    pub fn add_type(&mut self, id: TypeId, metadata: TypeMetadata) {
        // Add to source map
        self.source_map.add_type_definition(id, metadata.source_span);

        // Store metadata
        self.types.insert(id, metadata);
    }

    /// Set source file
    pub fn add_source_file(&mut self, file_id: u32, path: &str, content: &str) -> Result<()> {
        use crate::wit_source_map::WitSourceFile;
        let source_file = WitSourceFile::new(path, content)?;
        self.source_map.add_source_file(file_id, source_file);
        Ok(())
    }

    /// Add a source-level breakpoint
    pub fn add_source_breakpoint(&mut self, span: SourceSpan) -> Result<u32, DebugError> {
        // Find binary offset for this source location
        let binary_offset = self
            .source_map
            .binary_offset_for_source(span)
            .ok_or(DebugError::InvalidAddress)?;

        // Generate breakpoint ID
        let bp_id = self.source_breakpoints.len() as u32 + 1;
        self.source_breakpoints.insert(span, bp_id);

        Ok(bp_id)
    }

    /// Remove a source-level breakpoint
    pub fn remove_source_breakpoint(&mut self, span: SourceSpan) -> Result<(), DebugError> {
        self.source_breakpoints
            .remove(&span)
            .map(|_| ())
            .ok_or(DebugError::BreakpointNotFound)
    }

    /// Set step mode
    pub fn set_step_mode(&mut self, mode: WitStepMode) {
        self.step_mode = mode;
    }

    /// Get current step mode
    pub fn step_mode(&self) -> WitStepMode {
        self.step_mode
    }

    /// Find component containing a binary address
    pub fn find_component_for_address(&self, addr: u32) -> Option<ComponentId> {
        for (id, metadata) in &self.components {
            if addr >= metadata.binary_start && addr < metadata.binary_end {
                return Some(*id);
            }
        }
        None
    }

    /// Find function containing a binary address
    pub fn find_function_for_address(&self, addr: u32) -> Option<FunctionId> {
        // Look for the closest function at or before this address
        let mut best_func = None;
        let mut best_distance = u32::MAX;

        for (id, metadata) in &self.functions {
            if metadata.binary_offset <= addr {
                let distance = addr - metadata.binary_offset;
                if distance < best_distance {
                    best_distance = distance;
                    best_func = Some(*id);
                }
            }
        }

        best_func
    }

    /// Get source context for an address
    pub fn source_context_for_address(
        &self,
        addr: u32,
        context_lines: u32,
    ) -> Option<crate::wit_source_map::SourceContext> {
        let span = self.source_map.source_location_for_offset(addr)?;
        self.source_map.source_context(span, context_lines)
    }
}

#[cfg(feature = "wit-integration")]
impl Default for WitDebugger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "wit-integration")]
impl RuntimeDebugger for WitDebugger {
    fn on_breakpoint(
        &mut self,
        bp: &Breakpoint,
        state: &(dyn RuntimeState + 'static),
    ) -> DebugAction {
        // Update current context
        let pc = state.pc();
        self.current_component = self.find_component_for_address(pc);

        // Check if this is a source-level breakpoint
        if let Some(span) = self.source_map.source_location_for_offset(pc) {
            if self.source_breakpoints.contains_key(&span) {
                // This is a source-level breakpoint
                return DebugAction::Break;
            }
        }

        // Default behavior
        DebugAction::Break
    }

    fn on_instruction(&mut self, pc: u32, state: &(dyn RuntimeState + 'static)) -> DebugAction {
        // Update current context
        self.current_component = self.find_component_for_address(pc);

        match self.step_mode {
            WitStepMode::Instruction => DebugAction::StepInstruction,
            WitStepMode::SourceLine => {
                // Step to next WIT source line
                if let Some(_span) = self.source_map.source_location_for_offset(pc) {
                    // Could implement more sophisticated line stepping here
                    DebugAction::StepLine
                } else {
                    DebugAction::StepInstruction
                }
            },
            WitStepMode::SourceStepOver => DebugAction::StepOver,
            WitStepMode::SourceStepOut => DebugAction::StepOut,
            WitStepMode::Continue => DebugAction::Continue,
        }
    }

    fn on_function_entry(&mut self, func_idx: u32, state: &(dyn RuntimeState + 'static)) {
        let pc = state.pc();
        self.current_component = self.find_component_for_address(pc);

        // Could log WIT function entry here
    }

    fn on_function_exit(&mut self, func_idx: u32, state: &(dyn RuntimeState + 'static)) {
        let pc = state.pc();
        self.current_component = self.find_component_for_address(pc);

        // Could log WIT function exit here
    }

    fn on_trap(&mut self, trap_code: u32, state: &(dyn RuntimeState + 'static)) {
        let pc = state.pc();
        self.current_component = self.find_component_for_address(pc);

        // Could generate WIT-level diagnostic here
    }
}

#[cfg(feature = "wit-integration")]
impl WitAwareDebugger for WitDebugger {
    fn source_location_for_error(&self, error: &ComponentError) -> Option<SourceSpan> {
        if let Some(offset) = error.binary_offset {
            self.source_map.source_location_for_offset(offset)
        } else {
            None
        }
    }

    fn wit_type_at_offset(&self, binary_offset: u32) -> Option<WitTypeInfo> {
        // Find the source location for this offset
        let span = self.source_map.source_location_for_offset(binary_offset)?;

        // Look for type definitions that contain this span
        for (type_id, type_span) in &self.source_map.type_definitions {
            if span.start >= type_span.start && span.end <= type_span.end {
                // Found a containing type definition
                let metadata = self.types.get(type_id)?;
                let guard = safe_managed_alloc!(8192, CrateId::Debug)?;
                let provider = guard.provider().clone();

                return Some(WitTypeInfo {
                    id: *type_id,
                    name: metadata.name.clone(),
                    kind: match metadata.kind {
                        WitTypeKind::Primitive => crate::wit_source_map::WitTypeKind::Primitive(
                            BoundedString::try_from_str("primitive", provider).unwrap(),
                        ),
                        WitTypeKind::Record => crate::wit_source_map::WitTypeKind::Record(0),
                        WitTypeKind::Variant => crate::wit_source_map::WitTypeKind::Variant(0),
                        WitTypeKind::Enum => crate::wit_source_map::WitTypeKind::Enum(0),
                        WitTypeKind::Flags => crate::wit_source_map::WitTypeKind::Flags(0),
                        WitTypeKind::Resource => crate::wit_source_map::WitTypeKind::Resource,
                        WitTypeKind::Function => crate::wit_source_map::WitTypeKind::Function,
                        WitTypeKind::Interface => crate::wit_source_map::WitTypeKind::Interface,
                        WitTypeKind::World => crate::wit_source_map::WitTypeKind::World,
                    },
                    definition_span: *type_span,
                    usage_spans: Vec::new(),
                });
            }
        }

        None
    }

    fn component_boundary_info(&self, addr: u32) -> Option<ComponentBoundary> {
        let component_id = self.find_component_for_address(addr)?;
        let metadata = self.components.get(&component_id)?;

        Some(ComponentBoundary {
            id: component_id,
            name: Some(metadata.name.clone()),
            start_offset: metadata.binary_start,
            end_offset: metadata.binary_end,
            source_span: metadata.source_span,
            memory_regions: Vec::new(), // Could be populated from component metadata
        })
    }

    fn map_to_wit_diagnostic(&self, error: &ComponentError) -> Option<WitDiagnostic> {
        #[cfg(feature = "std")]
        {
            let error_str = error.message.as_str().unwrap_or("Unknown error");
            let runtime_error = Error::runtime_error(error_str);
            self.source_map.map_error_to_diagnostic(&runtime_error, error.binary_offset)
        }
        #[cfg(not(feature = "std"))]
        {
            // Binary std/no_std choice
            None
        }
    }

    fn wit_function_name(&self, function_id: FunctionId) -> Option<BoundedString<64>> {
        self.functions.get(&function_id).map(|metadata| metadata.name.clone())
    }

    fn wit_type_name(&self, type_id: TypeId) -> Option<BoundedString<64>> {
        self.types.get(&type_id).map(|metadata| metadata.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "wit-integration")]
    #[test]
    fn test_wit_debugger_creation() {
        let debugger = WitDebugger::new();
        assert_eq!(debugger.step_mode(), WitStepMode::Continue);
        assert!(debugger.components.is_empty());
        assert!(debugger.functions.is_empty());
        assert!(debugger.types.is_empty());
    }

    #[cfg(feature = "wit-integration")]
    #[test]
    fn test_component_metadata() {
        let mut debugger = WitDebugger::new();
        let guard = safe_managed_alloc!(8192, CrateId::Debug)?;
        let provider = guard.provider().clone();

        let metadata = ComponentMetadata {
            name: BoundedString::try_from_str("test-component", provider).unwrap(),
            source_span: SourceSpan::new(0, 100, 0),
            binary_start: 1000,
            binary_end: 2000,
            exports: Vec::new(),
            imports: Vec::new(),
        };

        let id = ComponentId(1);
        debugger.add_component(id, metadata);

        assert_eq!(debugger.find_component_for_address(1500), Some(id));
        assert_eq!(debugger.find_component_for_address(500), None);
        assert_eq!(debugger.find_component_for_address(2500), None);
    }

    #[cfg(feature = "wit-integration")]
    #[test]
    fn test_function_metadata() {
        let mut debugger = WitDebugger::new();
        let guard = safe_managed_alloc!(8192, CrateId::Debug)?;
        let provider = guard.provider().clone();

        let metadata = FunctionMetadata {
            name: BoundedString::try_from_str("test-function", provider).unwrap(),
            source_span: SourceSpan::new(10, 50, 0),
            binary_offset: 1200,
            param_types: Vec::new(),
            return_types: Vec::new(),
            is_async: false,
        };

        let id = FunctionId(1);
        debugger.add_function(id, metadata);

        assert_eq!(debugger.find_function_for_address(1200), Some(id));
        assert_eq!(debugger.find_function_for_address(1250), Some(id)); // Should find closest
        assert_eq!(debugger.find_function_for_address(1100), None); // Before function
    }

    #[cfg(feature = "wit-integration")]
    #[test]
    fn test_step_mode() {
        let mut debugger = WitDebugger::new();

        assert_eq!(debugger.step_mode(), WitStepMode::Continue);

        debugger.set_step_mode(WitStepMode::SourceLine);
        assert_eq!(debugger.step_mode(), WitStepMode::SourceLine);

        debugger.set_step_mode(WitStepMode::SourceStepOver);
        assert_eq!(debugger.step_mode(), WitStepMode::SourceStepOver);
    }
}
