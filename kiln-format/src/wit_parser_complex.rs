//! WIT (WebAssembly Interface Types) parser with proper no_std support

#[cfg(feature = "std")]
use std::collections::BTreeMap;
#[cfg(feature = "std")]
use std::boxed::Box;

#[cfg(all(not(feature = "std")))]
use std::{boxed::Box, collections::BTreeMap, vec::Vec, string::String};

use core::fmt;

use wrt_foundation::{
    BoundedVec, BoundedString,
    bounded::MAX_GENERATIVE_TYPES,
    MemoryProvider, NoStdProvider,
};

// Component ValType import removed - using ValueType from types module
use wrt_error::{Error, ErrorCategory};

/// Default memory provider for WIT types
pub type DefaultWitProvider = NoStdProvider<1024>;

#[derive(Debug, Clone, PartialEq)]
pub struct WitWorld<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub name: BoundedString<64, P>,
    pub imports: BoundedVec<WitImport<P>, MAX_GENERATIVE_TYPES, P>,
    pub exports: BoundedVec<WitExport<P>, MAX_GENERATIVE_TYPES, P>,
    pub types: BoundedVec<WitTypeDef<P>, MAX_GENERATIVE_TYPES, P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitInterface<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub name: BoundedString<64, P>,
    pub functions: BoundedVec<WitFunction<P>, MAX_GENERATIVE_TYPES, P>,
    pub types: BoundedVec<WitTypeDef<P>, MAX_GENERATIVE_TYPES, P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitImport<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub name: BoundedString<64, P>,
    pub item: WitItem<P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitExport<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub name: BoundedString<64, P>,
    pub item: WitItem<P>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WitItem<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    Function(WitFunction<P>),
    Interface(WitInterface<P>),
    Type(WitType<P>),
    Instance(WitInstance<P>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitFunction<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub name: BoundedString<64, P>,
    pub params: BoundedVec<WitParam<P>, 32, P>,
    pub results: BoundedVec<WitResult<P>, 16, P>,
    pub is_async: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitParam<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub name: BoundedString<32, P>,
    pub ty: WitType<P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitResult<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub name: Option<BoundedString<32, P>>,
    pub ty: WitType<P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitInstance<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub interface_name: BoundedString<64, P>,
    pub args: BoundedVec<WitInstanceArg<P>, 32, P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitInstanceArg<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub name: BoundedString<32, P>,
    pub value: WitValue<P>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WitValue<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    Type(WitType<P>),
    Instance(BoundedString<64, P>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitTypeDef<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub name: BoundedString<64, P>,
    pub ty: WitType<P>,
    pub is_resource: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WitType<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    /// Basic primitive types
    Bool,
    U8,
    U16,
    U32,
    U64,
    S8,
    S16,
    S32,
    S64,
    F32,
    F64,
    Char,
    String,
    
    /// Compound types
    List(Box<WitType<P>>),
    Option(Box<WitType<P>>),
    Result {
        ok: Option<Box<WitType<P>>>,
        err: Option<Box<WitType<P>>>,
    },
    Tuple(BoundedVec<WitType<P>, 16, P>),
    Record(WitRecord<P>),
    Variant(WitVariant<P>),
    Enum(WitEnum<P>),
    Flags(WitFlags<P>),
    
    /// Resource types
    Own(BoundedString<64, P>),
    Borrow(BoundedString<64, P>),
    
    /// Named type reference
    Named(BoundedString<64, P>),
    
    /// Stream and Future for async support
    Stream(Box<WitType<P>>),
    Future(Box<WitType<P>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitRecord<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub fields: BoundedVec<WitRecordField<P>, 32, P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitRecordField<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub name: BoundedString<32, P>,
    pub ty: WitType<P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitVariant<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub cases: BoundedVec<WitVariantCase<P>, 32, P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitVariantCase<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub name: BoundedString<32, P>,
    pub ty: Option<WitType<P>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitEnum<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub cases: BoundedVec<BoundedString<32, P>, 64, P>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WitFlags<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    pub flags: BoundedVec<BoundedString<32, P>, 64, P>,
}

#[derive(Debug, Clone)]
pub struct WitParser<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    current_position: usize,
    type_definitions: BTreeMap<BoundedString<64, P>, WitType<P>>,
    provider: P,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WitParseError<P: MemoryProvider + Default + Clone + PartialEq + Eq = DefaultWitProvider> {
    UnexpectedEnd,
    InvalidSyntax(BoundedString<128, P>),
    UnknownType(BoundedString<64, P>),
    TooManyItems,
    InvalidIdentifier(BoundedString<64, P>),
    DuplicateDefinition(BoundedString<64, P>),
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> From<WitParseError<P>> for Error {
    fn from(err: WitParseError<P>) -> Self {
        match err {
            WitParseError::UnexpectedEnd => Error::parse_error("Unexpected end of WIT input"),
            WitParseError::InvalidSyntax(_) => Error::parse_error("Invalid WIT syntax"),
            WitParseError::UnknownType(_) => Error::parse_error("Unknown WIT type"),
            WitParseError::TooManyItems => Error::parse_error("Too many WIT items"),
            WitParseError::InvalidIdentifier(_) => Error::parse_error("Invalid WIT identifier"),
            WitParseError::DuplicateDefinition(_) => Error::parse_error("Duplicate WIT definition"),
        }
    }
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> WitParser<P> {
    pub fn new(provider: P) -> Self {
        Self {
            current_position: 0,
            type_definitions: BTreeMap::new(),
            provider,
        }
    }

    pub fn parse_world(&mut self, source: &str) -> Result<WitWorld<P>, WitParseError<P>> {
        let mut world = WitWorld {
            name: BoundedString::try_from_str("", self.provider.clone()).unwrap_or_default(),
            imports: BoundedVec::new(self.provider.clone()).unwrap_or_default(),
            exports: BoundedVec::new(self.provider.clone()).unwrap_or_default(),
            types: BoundedVec::new(self.provider.clone()).unwrap_or_default(),
        };

        #[cfg(feature = "std")]
        let lines: Vec<&str> = source.lines().collect();
        #[cfg(feature = "std")]
        let mut i = 0;

        #[cfg(feature = "std")]
        while i < lines.len() {
            let line = lines[i].trim);
            
            if line.is_empty() || line.starts_with("//") {
                i += 1;
                continue;
            }

            if line.starts_with("world ") {
                let name = self.extract_identifier(line, "world ")?;
                world.name = name;
            } else if line.starts_with("import ") {
                let import = self.parse_import(line)?;
                world.imports.push(import)
                    .map_err(|_| WitParseError::TooManyItems)?;
            } else if line.starts_with("export ") {
                let export = self.parse_export(line)?;
                world.exports.push(export)
                    .map_err(|_| WitParseError::TooManyItems)?;
            } else if line.starts_with("type ") {
                let type_def = self.parse_type_def(line)?;
                world.types.push(type_def)
                    .map_err(|_| WitParseError::TooManyItems)?;
            }

            i += 1;
        }

        Ok(world)
    }

    pub fn parse_interface(&mut self, source: &str) -> Result<WitInterface<P>, WitParseError<P>> {
        let mut interface = WitInterface {
            name: BoundedString::try_from_str("", self.provider.clone()).unwrap_or_default(),
            functions: BoundedVec::new(self.provider.clone()).unwrap_or_default(),
            types: BoundedVec::new(self.provider.clone()).unwrap_or_default(),
        };

        #[cfg(feature = "std")]
        let lines: Vec<&str> = source.lines().collect();
        #[cfg(feature = "std")]
        let mut i = 0;

        #[cfg(feature = "std")]
        while i < lines.len() {
            let line = lines[i].trim);
            
            if line.is_empty() || line.starts_with("//") {
                i += 1;
                continue;
            }

            if line.starts_with("interface ") {
                let name = self.extract_identifier(line, "interface ")?;
                interface.name = name;
            } else if line.contains(":") && (line.contains("func") || line.contains("->")) {
                let function = self.parse_function(line)?;
                interface.functions.push(function)
                    .map_err(|_| WitParseError::TooManyItems)?;
            } else if line.starts_with("type ") {
                let type_def = self.parse_type_def(line)?;
                interface.types.push(type_def)
                    .map_err(|_| WitParseError::TooManyItems)?;
            }

            i += 1;
        }

        Ok(interface)
    }

    fn parse_import(&mut self, line: &str) -> Result<WitImport<P>, WitParseError<P>> {
        #[cfg(feature = "std")]
        let parts: Vec<&str> = line.split_whitespace().collect();
        #[cfg(feature = "std")]
        if parts.len() < 3 {
            return Err(WitParseError::InvalidSyntax(
                BoundedString::try_from_str("Invalid import syntax", self.provider.clone()).unwrap()
            ;
        }

        #[cfg(feature = "std")]
        let name = BoundedString::try_from_str(parts[1], self.provider.clone())
            .map_err(|_| WitParseError::InvalidIdentifier(
                BoundedString::try_from_str(parts[1], self.provider.clone()).unwrap_or_default()
            ))?;

        #[cfg(feature = "std")]
        let item_type = parts[2];
        #[cfg(feature = "std")]
        let item = match item_type {
            "func" => {
                let func = self.parse_function(line)?;
                WitItem::Function(func)
            }
            _ => {
                return Err(WitParseError::InvalidSyntax(
                    BoundedString::try_from_str("Unsupported import type", self.provider.clone()).unwrap()
                ;
            }
        };

        #[cfg(feature = "std")]
        return Ok(WitImport { name, item };
        
        #[cfg(not(any(feature = "std", )))]
        Err(WitParseError::InvalidSyntax(
            BoundedString::try_from_str("Parsing not supported in no_std", self.provider.clone()).unwrap()
        ))
    }

    fn parse_export(&mut self, line: &str) -> Result<WitExport<P>, WitParseError<P>> {
        #[cfg(feature = "std")]
        let parts: Vec<&str> = line.split_whitespace().collect();
        #[cfg(feature = "std")]
        if parts.len() < 3 {
            return Err(WitParseError::InvalidSyntax(
                BoundedString::try_from_str("Invalid export syntax", self.provider.clone()).unwrap()
            ;
        }

        #[cfg(feature = "std")]
        let name = BoundedString::try_from_str(parts[1], self.provider.clone())
            .map_err(|_| WitParseError::InvalidIdentifier(
                BoundedString::try_from_str(parts[1], self.provider.clone()).unwrap_or_default()
            ))?;

        #[cfg(feature = "std")]
        let item_type = parts[2];
        #[cfg(feature = "std")]
        let item = match item_type {
            "func" => {
                let func = self.parse_function(line)?;
                WitItem::Function(func)
            }
            _ => {
                return Err(WitParseError::InvalidSyntax(
                    BoundedString::try_from_str("Unsupported export type", self.provider.clone()).unwrap()
                ;
            }
        };

        #[cfg(feature = "std")]
        return Ok(WitExport { name, item };
        
        #[cfg(not(any(feature = "std", )))]
        Err(WitParseError::InvalidSyntax(
            BoundedString::try_from_str("Parsing not supported in no_std", self.provider.clone()).unwrap()
        ))
    }

    fn parse_function(&mut self, line: &str) -> Result<WitFunction<P>, WitParseError<P>> {
        let mut function = WitFunction {
            name: BoundedString::try_from_str("", self.provider.clone()).unwrap_or_default(),
            params: BoundedVec::new(self.provider.clone()).unwrap_or_default(),
            results: BoundedVec::new(self.provider.clone()).unwrap_or_default(),
            is_async: line.contains("async"),
        };

        #[cfg(feature = "std")]
        if let Some(colon_pos) = line.find(':') {
            let name_part = &line[..colon_pos].trim);
            let parts: Vec<&str> = name_part.split_whitespace().collect();
            
            if let Some(name) = parts.last() {
                function.name = BoundedString::try_from_str(name, self.provider.clone())
                    .map_err(|_| WitParseError::InvalidIdentifier(
                        BoundedString::try_from_str(name, self.provider.clone()).unwrap_or_default()
                    ))?;
            }
        }

        Ok(function)
    }

    fn parse_type_def(&mut self, line: &str) -> Result<WitTypeDef<P>, WitParseError<P>> {
        #[cfg(feature = "std")]
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        #[cfg(feature = "std")]
        if parts.len() < 3 {
            return Err(WitParseError::InvalidSyntax(
                BoundedString::try_from_str("Invalid type definition", self.provider.clone()).unwrap()
            ;
        }

        #[cfg(feature = "std")]
        let name = BoundedString::try_from_str(parts[1], self.provider.clone())
            .map_err(|_| WitParseError::InvalidIdentifier(
                BoundedString::try_from_str(parts[1], self.provider.clone()).unwrap_or_default()
            ))?;

        #[cfg(feature = "std")]
        let type_str = parts[2];
        #[cfg(feature = "std")]
        let is_resource = type_str.starts_with("resource";
        
        #[cfg(feature = "std")]
        let ty = self.parse_type(type_str)?;

        #[cfg(feature = "std")]
        return Ok(WitTypeDef {
            name: name.clone(),
            ty: ty.clone(),
            is_resource,
        };
        
        #[cfg(not(any(feature = "std", )))]
        Err(WitParseError::InvalidSyntax(
            BoundedString::try_from_str("Parsing not supported in no_std", self.provider.clone()).unwrap()
        ))
    }

    fn parse_type(&mut self, type_str: &str) -> Result<WitType<P>, WitParseError<P>> {
        let type_str = type_str.trim);
        
        match type_str {
            "bool" => Ok(WitType::Bool),
            "u8" => Ok(WitType::U8),
            "u16" => Ok(WitType::U16),
            "u32" => Ok(WitType::U32),
            "u64" => Ok(WitType::U64),
            "s8" => Ok(WitType::S8),
            "s16" => Ok(WitType::S16),
            "s32" => Ok(WitType::S32),
            "s64" => Ok(WitType::S64),
            "f32" => Ok(WitType::F32),
            "f64" => Ok(WitType::F64),
            "char" => Ok(WitType::Char),
            "string" => Ok(WitType::String),
            _ => {
                #[cfg(feature = "std")]
                {
                    if type_str.starts_with("list<") && type_str.ends_with(">") {
                        let inner = &type_str[5..type_str.len()-1];
                        let inner_type = self.parse_type(inner)?;
                        Ok(WitType::List(Box::new(inner_type)))
                    } else if type_str.starts_with("option<") && type_str.ends_with(">") {
                        let inner = &type_str[7..type_str.len()-1];
                        let inner_type = self.parse_type(inner)?;
                        Ok(WitType::Option(Box::new(inner_type)))
                    } else if type_str.starts_with("stream<") && type_str.ends_with(">") {
                        let inner = &type_str[7..type_str.len()-1];
                        let inner_type = self.parse_type(inner)?;
                        Ok(WitType::Stream(Box::new(inner_type)))
                    } else if type_str.starts_with("future<") && type_str.ends_with(">") {
                        let inner = &type_str[7..type_str.len()-1];
                        let inner_type = self.parse_type(inner)?;
                        Ok(WitType::Future(Box::new(inner_type)))
                    } else {
                        let name = BoundedString::try_from_str(type_str, self.provider.clone())
                            .map_err(|_| WitParseError::InvalidIdentifier(
                                BoundedString::try_from_str(type_str, self.provider.clone()).unwrap_or_default()
                            ))?;
                        Ok(WitType::Named(name))
                    }
                }
                
                #[cfg(not(any(feature = "std", )))]
                {
                    let name = BoundedString::try_from_str(type_str, self.provider.clone())
                        .map_err(|_| WitParseError::InvalidIdentifier(
                            BoundedString::try_from_str(type_str, self.provider.clone()).unwrap_or_default()
                        ))?;
                    Ok(WitType::Named(name))
                }
            }
        }
    }

    fn extract_identifier(&self, line: &str, prefix: &str) -> Result<BoundedString<64, P>, WitParseError<P>> {
        let remaining = line.strip_prefix(prefix)
            .ok_or_else(|| WitParseError::InvalidSyntax(
                BoundedString::try_from_str("Missing prefix", self.provider.clone()).unwrap()
            ))?;
        
        let identifier = remaining.split_whitespace().next()
            .ok_or_else(|| WitParseError::InvalidSyntax(
                BoundedString::try_from_str("Missing identifier", self.provider.clone()).unwrap()
            ))?;

        BoundedString::try_from_str(identifier, self.provider.clone())
            .map_err(|_| WitParseError::InvalidIdentifier(
                BoundedString::try_from_str(identifier, self.provider.clone()).unwrap_or_default()
            ))
    }

    #[cfg(feature = "std")]
    pub fn convert_to_valtype(&self, wit_type: &WitType<P>) -> Result<crate::types::ValueType, Error> {
        match wit_type {
            WitType::Bool | WitType::U8 | WitType::U16 | WitType::U32 | WitType::U64 |
            WitType::S8 | WitType::S16 | WitType::S32 | WitType::S64 => {
                Ok(crate::types::ValueType::I32) // WIT integers map to i32 in core wasm
            },
            WitType::F32 => Ok(crate::types::ValueType::F32),
            WitType::F64 => Ok(crate::types::ValueType::F64),
            WitType::Char | WitType::String => {
                Ok(crate::types::ValueType::I32) // Strings/chars are represented as pointers
            },
            WitType::List(_) | WitType::Option(_) => {
                // Complex types are represented as references in core wasm
                Ok(crate::types::ValueType::I32)
            }
            _ => Err(Error::parse_error("Unsupported WIT type conversion")),
        }
    }
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Default for WitParser<P> {
    fn default() -> Self {
        Self::new(P::default())
    }
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> fmt::Display for WitParseError<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WitParseError::UnexpectedEnd => write!(f, "Unexpected end of input"),
            WitParseError::InvalidSyntax(msg) => write!(f, "Invalid syntax: {}", msg),
            WitParseError::UnknownType(name) => write!(f, "Unknown type: {}", name),
            WitParseError::TooManyItems => write!(f, "Too many items"),
            WitParseError::InvalidIdentifier(name) => write!(f, "Invalid identifier: {}", name),
            WitParseError::DuplicateDefinition(name) => write!(f, "Duplicate definition: {}", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wit_parser_creation() {
        let parser = WitParser::new(DefaultWitProvider::default());
        assert_eq!(parser.current_position, 0);
        assert_eq!(parser.type_definitions.len(), 0);
    }

    #[test]
    fn test_parse_basic_types() {
        let mut parser = WitParser::new(DefaultWitProvider::default());
        
        assert_eq!(parser.parse_type("bool").unwrap(), WitType::Bool);
        assert_eq!(parser.parse_type("u32").unwrap(), WitType::U32);
        assert_eq!(parser.parse_type("string").unwrap(), WitType::String);
        assert_eq!(parser.parse_type("f64").unwrap(), WitType::F64);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_parse_compound_types() {
        let mut parser = WitParser::new(DefaultWitProvider::default());
        
        let list_type = parser.parse_type("list<u32>").unwrap();
        match list_type {
            WitType::List(inner) => assert_eq!(*inner, WitType::U32),
            _ => panic!("Expected list type"),
        }

        let option_type = parser.parse_type("option<string>").unwrap();
        match option_type {
            WitType::Option(inner) => assert_eq!(*inner, WitType::String),
            _ => panic!("Expected option type"),
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_parse_async_types() {
        let mut parser = WitParser::new(DefaultWitProvider::default());
        
        let stream_type = parser.parse_type("stream<u8>").unwrap();
        match stream_type {
            WitType::Stream(inner) => assert_eq!(*inner, WitType::U8),
            _ => panic!("Expected stream type"),
        }

        let future_type = parser.parse_type("future<string>").unwrap();
        match future_type {
            WitType::Future(inner) => assert_eq!(*inner, WitType::String),
            _ => panic!("Expected future type"),
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_parse_simple_world() {
        let mut parser = WitParser::new(DefaultWitProvider::default());
        let source = r#"
            world test-world {
                import test-func: func()
                export result-func: func() -> u32
            }
        "#;

        let world = parser.parse_world(source);
        assert!(world.is_ok());
        
        let world = world.unwrap();
        assert_eq!(world.name.as_str(), "test-world");
        assert_eq!(world.imports.len(), 1);
        assert_eq!(world.exports.len(), 1);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_convert_to_valtype() {
        let parser = WitParser::new(DefaultWitProvider::default());
        
        assert_eq!(parser.convert_to_valtype(&WitType::Bool).unwrap(), crate::types::ValueType::I32);
        assert_eq!(parser.convert_to_valtype(&WitType::U32).unwrap(), crate::types::ValueType::I32);
        assert_eq!(parser.convert_to_valtype(&WitType::String).unwrap(), crate::types::ValueType::I32);
        
        let list_wit = WitType::List(Box::new(WitType::U32));
        let list_val = parser.convert_to_valtype(&list_wit).unwrap();
        assert_eq!(list_val, crate::types::ValueType::I32); // Lists are represented as pointers
    }
}