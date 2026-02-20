//! WIT Component Integration for enhanced component lowering
//!
//! This module provides integration between WIT (WebAssembly Interface Types)
//! and the component model, enabling improved component lowering and lifting.

#[cfg(feature = "std")]
use std::{collections::BTreeMap, vec::Vec};
#[cfg(all(not(feature = "std")))]
use std::{collections::BTreeMap, vec::Vec};

use wrt_foundation::{
    bounded::{BoundedString},
    prelude::*,
    traits::{ToBytes, FromBytes, Checksummable, WriteStream, ReadStream},
    verification::Checksum,
};
use wrt_error::{Error, Result};

// Platform-aware type aliases for WIT component integration
type ComponentProvider = wrt_foundation::budget_provider::BudgetProvider<4096>;  // 4KB for component data
type WitBoundedString<const N: usize> = BoundedString<N>;

// Re-export WIT AST types for convenience
pub use wrt_format::ast::{
    WitDocument, InterfaceDecl, FunctionDecl, TypeDecl, WorldDecl,
    TypeExpr, PrimitiveKind, SourceSpan,
};

/// WIT Component lowering context
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct WitComponentContext {
    /// Parsed WIT document
    pub document: WitDocument,
    
    /// Component interface mappings
    interface_mappings: BTreeMap<String, InterfaceMapping>,
    
    /// Type mappings between WIT and component model
    type_mappings: BTreeMap<String, TypeMapping>,
    
    /// Function mappings
    function_mappings: BTreeMap<String, FunctionMapping>,
    
    /// Component configuration
    config: ComponentConfig,
}

/// Interface mapping between WIT and component model
#[derive(Debug, Clone)]
pub struct InterfaceMapping {
    /// WIT interface name
    pub wit_name: WitBoundedString<64>,
    
    /// Component interface ID
    pub component_id: u32,
    
    /// Interface functions
    pub functions: Vec<FunctionMapping>,
    
    /// Interface types
    pub types: Vec<TypeMapping>,
    
    /// Source location in WIT
    pub source_span: SourceSpan,
}

/// Type mapping between WIT and component model
#[derive(Debug, Clone)]
pub struct TypeMapping {
    /// WIT type name
    pub wit_name: WitBoundedString<64>,
    
    /// Component type representation
    pub component_type: ComponentType,
    
    /// Size in bytes (if known)
    pub size: Option<u32>,
    
    /// Alignment requirements
    pub alignment: Option<u32>,
    
    /// Source location in WIT
    pub source_span: SourceSpan,
}

/// Function mapping between WIT and component model
#[derive(Debug, Clone)]
pub struct FunctionMapping {
    /// WIT function name
    pub wit_name: WitBoundedString<64>,
    
    /// Component function index
    pub function_index: u32,
    
    /// Parameter types
    pub param_types: Vec<TypeMapping>,
    
    /// Return types
    pub return_types: Vec<TypeMapping>,
    
    /// Whether function is async
    pub is_async: bool,
    
    /// Source location in WIT
    pub source_span: SourceSpan,
}

/// Component type representation
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentType {
    /// Primitive types
    U8, U16, U32, U64,
    S8, S16, S32, S64,
    F32, F64,
    Bool,
    Char,
    String,
    
    /// Composite types
    Record(RecordType),
    Variant(VariantType),
    Enum(EnumType),
    Flags(FlagsType),
    
    /// Special types
    Option(Box<ComponentType>),
    Result(Box<ComponentType>, Box<ComponentType>),
    List(Box<ComponentType>),
    
    /// Resources
    Resource(ResourceType),
    
    /// Function type
    Function(FunctionType),
}

/// Record type definition
#[derive(Debug, Clone, PartialEq)]
pub struct RecordType {
    /// Record fields
    pub fields: Vec<FieldType>,
}

/// Field in a record
#[derive(Debug, Clone, PartialEq)]
pub struct FieldType {
    /// Field name
    pub name: WitBoundedString<32>,
    /// Field type
    pub field_type: Box<ComponentType>,
}

/// Variant type definition
#[derive(Debug, Clone, PartialEq)]
pub struct VariantType {
    /// Variant cases
    pub cases: Vec<CaseType>,
}

/// Case in a variant
#[derive(Debug, Clone, PartialEq)]
pub struct CaseType {
    /// Case name
    pub name: WitBoundedString<32>,
    /// Optional case type
    pub case_type: Option<Box<ComponentType>>,
}

/// Enum type definition
#[derive(Debug, Clone, PartialEq)]
pub struct EnumType {
    /// Enum values
    pub values: Vec<WitBoundedString<32>>,
}

/// Flags type definition
#[derive(Debug, Clone, PartialEq)]
pub struct FlagsType {
    /// Flag names
    pub flags: Vec<WitBoundedString<32>>,
}

/// Resource type definition
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceType {
    /// Resource name
    pub name: WitBoundedString<64>,
    /// Resource methods
    pub methods: Vec<FunctionType>,
}

/// Function type definition
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionType {
    /// Parameter types
    pub params: Vec<ComponentType>,
    /// Return types
    pub returns: Vec<ComponentType>,
}

// Trait implementations for ComponentType
impl Default for ComponentType {
    fn default() -> Self {
        ComponentType::Bool
    }
}

impl ToBytes for ComponentType {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        // Write discriminant for the type
        let discriminant: u8 = match self {
            ComponentType::U8 => 0,
            ComponentType::U16 => 1,
            ComponentType::U32 => 2,
            ComponentType::U64 => 3,
            ComponentType::S8 => 4,
            ComponentType::S16 => 5,
            ComponentType::S32 => 6,
            ComponentType::S64 => 7,
            ComponentType::F32 => 8,
            ComponentType::F64 => 9,
            ComponentType::Bool => 10,
            ComponentType::Char => 11,
            ComponentType::String => 12,
            _ => 255, // Complex types
        };
        writer.write_u8(discriminant)?;
        Ok(())
    }
}

impl FromBytes for ComponentType {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        let discriminant = reader.read_u8()?;
        match discriminant {
            0 => Ok(ComponentType::U8),
            1 => Ok(ComponentType::U16),
            2 => Ok(ComponentType::U32),
            3 => Ok(ComponentType::U64),
            4 => Ok(ComponentType::S8),
            5 => Ok(ComponentType::S16),
            6 => Ok(ComponentType::S32),
            7 => Ok(ComponentType::S64),
            8 => Ok(ComponentType::F32),
            9 => Ok(ComponentType::F64),
            10 => Ok(ComponentType::Bool),
            11 => Ok(ComponentType::Char),
            12 => Ok(ComponentType::String),
            _ => Ok(ComponentType::Bool), // Default fallback
        }
    }
}

impl Checksummable for ComponentType {
    fn update_checksum(&self, checksum: &mut Checksum) {
        let discriminant: u8 = match self {
            ComponentType::U8 => 0,
            ComponentType::U16 => 1,
            ComponentType::U32 => 2,
            ComponentType::U64 => 3,
            ComponentType::S8 => 4,
            ComponentType::S16 => 5,
            ComponentType::S32 => 6,
            ComponentType::S64 => 7,
            ComponentType::F32 => 8,
            ComponentType::F64 => 9,
            ComponentType::Bool => 10,
            ComponentType::Char => 11,
            ComponentType::String => 12,
            _ => 255,
        };
        checksum.update(discriminant);
    }
}

/// Component configuration
#[derive(Debug, Clone)]
pub struct ComponentConfig {
    /// Enable debug information
    pub debug_info: bool,
    
    /// Enable optimization
    pub optimize: bool,
    
    /// Memory limits
    pub memory_limit: Option<u32>,
    
    /// Maximum stack size
    pub stack_limit: Option<u32>,
    
    /// Enable async support
    pub async_support: bool,
}

impl Default for ComponentConfig {
    fn default() -> Self {
        Self {
            debug_info: true,
            optimize: false,
            memory_limit: Some(1024 * 1024), // 1MB
            stack_limit: Some(64 * 1024),    // 64KB
            async_support: false,
        }
    }
}

#[cfg(feature = "std")]
impl WitComponentContext {
    /// Create a new WIT component context
    pub fn new(document: WitDocument) -> Self {
        Self {
            document,
            interface_mappings: BTreeMap::new(),
            type_mappings: BTreeMap::new(),
            function_mappings: BTreeMap::new(),
            config: ComponentConfig::default(),
        }
    }
    
    /// Create context with custom configuration
    pub fn with_config(document: WitDocument, config: ComponentConfig) -> Self {
        Self {
            document,
            interface_mappings: BTreeMap::new(),
            type_mappings: BTreeMap::new(),
            function_mappings: BTreeMap::new(),
            config,
        }
    }
    
    /// Build component mappings from WIT document
    pub fn build_mappings(&mut self) -> Result<()> {
        // Process interfaces
        for item in &self.document.items {
            match item {
                wrt_format::ast::TopLevelItem::Interface(interface) => {
                    self.process_interface(interface)?;
                }
                wrt_format::ast::TopLevelItem::World(world) => {
                    self.process_world(world)?;
                }
                wrt_format::ast::TopLevelItem::Type(type_decl) => {
                    self.process_type_declaration(type_decl)?;
                }
            }
        }
        
        Ok(()
    }
    
    /// Process an interface declaration
    fn process_interface(&mut self, interface: &InterfaceDecl) -> Result<()> {
        let mut functions = Vec::new();
        let mut types = Vec::new();
        
        // Process interface items
        for item in &interface.items {
            match item {
                wrt_format::ast::InterfaceItem::Function(func) => {
                    let mapping = self.process_function(func)?;
                    functions.push(mapping);
                }
                wrt_format::ast::InterfaceItem::Type(type_decl) => {
                    let mapping = self.process_type_declaration(type_decl)?;
                    types.push(mapping);
                }
                wrt_format::ast::InterfaceItem::Use(_) => {
                    // Handle use declarations if needed
                }
            }
        }
        
        // Create interface mapping
        let interface_name = interface.name.name.as_str()
            .map_err(|_| Error::parse_error("Error occurred"))?
            .to_string());
        
        let mapping = InterfaceMapping {
            wit_name: interface.name.name.clone(),
            component_id: self.interface_mappings.len() as u32,
            functions,
            types,
            source_span: interface.span,
        };
        
        self.interface_mappings.insert(interface_name, mapping;
        
        Ok(()
    }
    
    /// Process a world declaration
    fn process_world(&mut self, _world: &WorldDecl) -> Result<()> {
        // Process world imports and exports
        // This would involve mapping world items to component imports/exports
        Ok(()
    }
    
    /// Process a function declaration
    fn process_function(&mut self, func: &FunctionDecl) -> Result<FunctionMapping> {
        let mut param_types = Vec::new();
        let mut return_types = Vec::new();
        
        // Process parameters
        for param in &func.func.params {
            let type_mapping = self.convert_wit_type(&param.ty)?;
            param_types.push(type_mapping);
        }
        
        // Process return types
        match &func.func.results {
            wrt_format::ast::FunctionResults::None => {
                // No return types
            }
            wrt_format::ast::FunctionResults::Type(ty) => {
                let type_mapping = self.convert_wit_type(ty)?;
                return_types.push(type_mapping);
            }
            wrt_format::ast::FunctionResults::Named(_named) => {
                // Handle named results
            }
        }
        
        Ok(FunctionMapping {
            wit_name: func.name.name.clone(),
            function_index: self.function_mappings.len() as u32,
            param_types,
            return_types,
            is_async: func.func.is_async,
            source_span: func.span,
        })
    }
    
    /// Process a type declaration
    fn process_type_declaration(&mut self, type_decl: &TypeDecl) -> Result<TypeMapping> {
        let component_type = self.convert_wit_type(&type_decl.ty)?;
        
        let mapping = TypeMapping {
            wit_name: type_decl.name.name.clone(),
            component_type: component_type.clone(),
            size: self.calculate_type_size(&component_type),
            alignment: self.calculate_type_alignment(&component_type),
            source_span: type_decl.span,
        };
        
        let type_name = type_decl.name.name.as_str()
            .map_err(|_| Error::parse_error("Error occurred"))?
            .to_string());
        
        self.type_mappings.insert(type_name, mapping.clone();
        
        Ok(mapping)
    }
    
    /// Convert WIT type to component type
    fn convert_wit_type(&self, wit_type: &TypeExpr) -> Result<ComponentType> {
        match wit_type {
            TypeExpr::Primitive(prim) => {
                Ok(match prim.kind {
                    PrimitiveKind::U8 => ComponentType::U8,
                    PrimitiveKind::U16 => ComponentType::U16,
                    PrimitiveKind::U32 => ComponentType::U32,
                    PrimitiveKind::U64 => ComponentType::U64,
                    PrimitiveKind::S8 => ComponentType::S8,
                    PrimitiveKind::S16 => ComponentType::S16,
                    PrimitiveKind::S32 => ComponentType::S32,
                    PrimitiveKind::S64 => ComponentType::S64,
                    PrimitiveKind::F32 => ComponentType::F32,
                    PrimitiveKind::F64 => ComponentType::F64,
                    PrimitiveKind::Bool => ComponentType::Bool,
                    PrimitiveKind::Char => ComponentType::Char,
                    PrimitiveKind::String => ComponentType::String,
                })
            }
            TypeExpr::Named(named) => {
                // Look up named type
                let type_name = named.name.name.as_str()
                    .map_err(|_| Error::parse_error("Error occurred"))?;
                
                if let Some(mapping) = self.type_mappings.get(type_name) {
                    Ok(mapping.component_type.clone()
                } else {
                    Err(Error::parse_error(&"Component not found")
                }
            }
            TypeExpr::List(inner) => {
                let inner_type = self.convert_wit_type(inner)?;
                Ok(ComponentType::List(Box::new(inner_type))
            }
            TypeExpr::Option(inner) => {
                let inner_type = self.convert_wit_type(inner)?;
                Ok(ComponentType::Option(Box::new(inner_type))
            }
        }
    }
    
    /// Calculate type size in bytes
    fn calculate_type_size(&self, ty: &ComponentType) -> Option<u32> {
        match ty {
            ComponentType::U8 | ComponentType::S8 => Some(1),
            ComponentType::U16 | ComponentType::S16 => Some(2),
            ComponentType::U32 | ComponentType::S32 | ComponentType::F32 => Some(4),
            ComponentType::U64 | ComponentType::S64 | ComponentType::F64 => Some(8),
            ComponentType::Bool | ComponentType::Char => Some(1),
            ComponentType::String => None, // Variable size
            ComponentType::List(_) => None, // Variable size
            ComponentType::Option(_) => None, // Variable size
            ComponentType::Record(record) => {
                let mut total_size = 0u32;
                for field in &record.fields {
                    if let Some(field_size) = self.calculate_type_size(&field.field_type) {
                        total_size += field_size;
                    } else {
                        return None; // Contains variable size field
                    }
                }
                Some(total_size)
            }
            _ => None, // Complex types have variable or unknown sizes
        }
    }
    
    /// Calculate type alignment
    fn calculate_type_alignment(&self, ty: &ComponentType) -> Option<u32> {
        match ty {
            ComponentType::U8 | ComponentType::S8 | ComponentType::Bool | ComponentType::Char => Some(1),
            ComponentType::U16 | ComponentType::S16 => Some(2),
            ComponentType::U32 | ComponentType::S32 | ComponentType::F32 => Some(4),
            ComponentType::U64 | ComponentType::S64 | ComponentType::F64 => Some(8),
            ComponentType::String => Some(4), // Pointer alignment
            ComponentType::List(_) => Some(4), // Pointer alignment
            ComponentType::Option(_) => Some(4), // Discriminant + pointer
            ComponentType::Record(record) => {
                let mut max_alignment = 1u32;
                for field in &record.fields {
                    if let Some(field_align) = self.calculate_type_alignment(&field.field_type) {
                        max_alignment = max_alignment.max(field_align;
                    }
                }
                Some(max_alignment)
            }
            _ => Some(4), // Default pointer alignment
        }
    }
    
    /// Get interface mapping by name
    pub fn get_interface(&self, name: &str) -> Option<&InterfaceMapping> {
        self.interface_mappings.get(name)
    }
    
    /// Get type mapping by name
    pub fn get_type(&self, name: &str) -> Option<&TypeMapping> {
        self.type_mappings.get(name)
    }
    
    /// Get function mapping by name
    pub fn get_function(&self, name: &str) -> Option<&FunctionMapping> {
        self.function_mappings.get(name)
    }
    
    /// Get all interface mappings
    pub fn interfaces(&self) -> &BTreeMap<String, InterfaceMapping> {
        &self.interface_mappings
    }
    
    /// Get all type mappings
    pub fn types(&self) -> &BTreeMap<String, TypeMapping> {
        &self.type_mappings
    }
    
    /// Get all function mappings
    pub fn functions(&self) -> &BTreeMap<String, FunctionMapping> {
        &self.function_mappings
    }
    
    /// Get configuration
    pub fn config(&self) -> &ComponentConfig {
        &self.config
    }
}

/// Component lowering utilities
pub struct ComponentLowering;

impl ComponentLowering {
    /// Lower WIT document to component representation
    #[cfg(feature = "std")]
    pub fn lower_document(document: WitDocument) -> Result<WitComponentContext> {
        let mut context = WitComponentContext::new(document;
        context.build_mappings()?;
        Ok(context)
    }
    
    /// Lower WIT document with custom configuration
    #[cfg(feature = "std")]
    pub fn lower_document_with_config(document: WitDocument, config: ComponentConfig) -> Result<WitComponentContext> {
        let mut context = WitComponentContext::with_config(document, config;
        context.build_mappings()?;
        Ok(context)
    }
    
    /// Validate component mappings
    pub fn validate_mappings(context: &WitComponentContext) -> Result<()> {
        // Validate that all types are resolvable
        for (name, mapping) in context.types() {
            Self::validate_type_mapping(name, mapping, context)?;
        }
        
        // Validate that all functions have valid signatures
        for (name, mapping) in context.functions() {
            Self::validate_function_mapping(name, mapping)?;
        }
        
        Ok(()
    }
    
    /// Validate a single type mapping
    fn validate_type_mapping(name: &str, mapping: &TypeMapping, context: &WitComponentContext) -> Result<()> {
        // Check that the type is well-formed
        Self::validate_component_type(&mapping.component_type, context)?;
        
        // Check size/alignment consistency
        if let (Some(size), Some(alignment)) = (mapping.size, mapping.alignment) {
            if size % alignment != 0 {
                return Err(Error::validation_error(&format!(
                    "Type {} has inconsistent size {} and alignment {}", 
                    name, size, alignment
                ;
            }
        }
        
        Ok(()
    }
    
    /// Validate a component type
    fn validate_component_type(ty: &ComponentType, context: &WitComponentContext) -> Result<()> {
        match ty {
            ComponentType::Record(record) => {
                for field in &record.fields {
                    Self::validate_component_type(&field.field_type, context)?;
                }
            }
            ComponentType::Variant(variant) => {
                for case in &variant.cases {
                    if let Some(case_type) = case.case_type {
                        Self::validate_component_type(case_type, context)?;
                    }
                }
            }
            ComponentType::Option(inner) | ComponentType::List(inner) => {
                Self::validate_component_type(inner, context)?;
            }
            ComponentType::Result(ok_type, err_type) => {
                Self::validate_component_type(ok_type, context)?;
                Self::validate_component_type(err_type, context)?;
            }
            ComponentType::Function(func_type) => {
                for param in &func_type.params {
                    Self::validate_component_type(param, context)?;
                }
                for ret in &func_type.returns {
                    Self::validate_component_type(ret, context)?;
                }
            }
            _ => {} // Primitive types are always valid
        }
        
        Ok(()
    }
    
    /// Validate a function mapping
    fn validate_function_mapping(_name: &str, _mapping: &FunctionMapping) -> Result<()> {
        // Validate function signature
        // Check parameter and return type consistency
        // This would involve more detailed validation in a real implementation
        Ok(()
    }

}
