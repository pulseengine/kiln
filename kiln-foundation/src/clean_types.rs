//! Clean type definitions without provider parameters
//!
//! This module provides provider-agnostic core types for the WebAssembly
//! Component Model. These types are free from memory allocation concerns and
//! can be used in public APIs without leaking provider implementation details.
//!
//! Note: This module requires allocation capabilities (std or alloc feature).

// Only compile this module when allocation is available since clean types use
// Vec/String/Box
#[cfg(any(feature = "std", feature = "alloc"))]
pub use types::*;

#[cfg(any(feature = "std", feature = "alloc"))]
mod types {
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    use alloc::{
        boxed::Box,
        string::String,
        vec::Vec,
    };
    #[cfg(not(feature = "std"))]
    use core::fmt;
    #[cfg(feature = "std")]
    use std::fmt;
    #[cfg(feature = "std")]
    use std::{
        boxed::Box,
        string::String,
        vec::Vec,
    };

    /// Clean component model value type without provider parameters
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub enum ValType {
        /// Boolean type
        Bool,
        /// Signed 8-bit integer
        S8,
        /// Unsigned 8-bit integer  
        U8,
        /// Signed 16-bit integer
        S16,
        /// Unsigned 16-bit integer
        U16,
        /// Signed 32-bit integer
        S32,
        /// Unsigned 32-bit integer
        U32,
        /// Signed 64-bit integer
        S64,
        /// Unsigned 64-bit integer
        U64,
        /// 32-bit floating point
        F32,
        /// 64-bit floating point
        F64,
        /// Character type
        Char,
        /// String type
        String,
        /// List type with element type
        List(Box<ValType>),
        /// Record type with named fields
        Record(Record),
        /// Tuple type with element types
        Tuple(Tuple),
        /// Variant type with alternatives
        Variant(Variant),
        /// Enum type with cases
        Enum(Enum),
        /// Option type with payload
        Option(Box<ValType>),
        /// Result type with ok/error types
        Result(Result_),
        /// Flags type with bitfields
        Flags(Flags),
        /// Resource handle
        Own(u32),
        /// Borrowed resource
        Borrow(u32),
        /// Stream type with element type
        Stream(Box<ValType>),
        /// Future type with value type
        Future(Box<ValType>),
    }

    impl Default for ValType {
        fn default() -> Self {
            Self::Bool
        }
    }

    /// Record type definition
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Record {
        /// Record fields
        pub fields: Vec<Field>,
    }

    impl Default for Record {
        fn default() -> Self {
            Self { fields: Vec::new() }
        }
    }

    /// Field in a record
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Field {
        /// Field name
        pub name: String,
        /// Field type
        pub ty:   ValType,
    }

    impl Default for Field {
        fn default() -> Self {
            Self {
                name: String::new(),
                ty:   ValType::default(),
            }
        }
    }

    /// Tuple type definition
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Tuple {
        /// Tuple element types
        pub types: Vec<ValType>,
    }

    impl Default for Tuple {
        fn default() -> Self {
            Self { types: Vec::new() }
        }
    }

    /// Variant type definition
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Variant {
        /// Variant cases
        pub cases: Vec<Case>,
    }

    impl Default for Variant {
        fn default() -> Self {
            Self { cases: Vec::new() }
        }
    }

    /// Case in a variant
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Case {
        /// Case name
        pub name:    String,
        /// Case type (optional)
        pub ty:      Option<ValType>,
        /// Refinement information
        pub refines: Option<u32>,
    }

    impl Default for Case {
        fn default() -> Self {
            Self {
                name:    String::new(),
                ty:      None,
                refines: None,
            }
        }
    }

    /// Enum type definition
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Enum {
        /// Enum cases
        pub cases: Vec<String>,
    }

    impl Default for Enum {
        fn default() -> Self {
            Self { cases: Vec::new() }
        }
    }

    /// Result type definition (renamed to avoid conflict with
    /// std::result::Result)
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Result_ {
        /// Ok type (optional)
        pub ok:  Option<Box<ValType>>,
        /// Error type (optional)
        pub err: Option<Box<ValType>>,
    }

    impl Default for Result_ {
        fn default() -> Self {
            Self {
                ok:  None,
                err: None,
            }
        }
    }

    /// Flags type definition
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Flags {
        /// Flag labels
        pub labels: Vec<String>,
    }

    impl Default for Flags {
        fn default() -> Self {
            Self { labels: Vec::new() }
        }
    }

    /// Component model value
    #[derive(Debug, Clone, PartialEq)]
    pub enum Value {
        /// Boolean value
        Bool(bool),
        /// Signed 8-bit integer
        S8(i8),
        /// Unsigned 8-bit integer
        U8(u8),
        /// Signed 16-bit integer
        S16(i16),
        /// Unsigned 16-bit integer
        U16(u16),
        /// Signed 32-bit integer
        S32(i32),
        /// Unsigned 32-bit integer
        U32(u32),
        /// Signed 64-bit integer
        S64(i64),
        /// Unsigned 64-bit integer
        U64(u64),
        /// 32-bit floating point
        F32(f32),
        /// 64-bit floating point
        F64(f64),
        /// Character value
        Char(char),
        /// String value
        String(String),
        /// List value
        List(Vec<Value>),
        /// Record value
        Record(Vec<Value>),
        /// Tuple value
        Tuple(Vec<Value>),
        /// Variant value
        Variant {
            discriminant: u32,
            value:        Option<Box<Value>>,
        },
        /// Enum value
        Enum(u32),
        /// Option value
        Option(Option<Box<Value>>),
        /// Result value
        Result(core::result::Result<Option<Box<Value>>, Box<Value>>),
        /// Flags value
        Flags(u32),
        /// Owned resource
        Own(u32),
        /// Borrowed resource
        Borrow(u32),
    }

    impl Default for Value {
        fn default() -> Self {
            Self::Bool(false)
        }
    }

    impl Value {
        /// Get the type of this value
        pub fn value_type(&self) -> ValType {
            match self {
                Value::Bool(_) => ValType::Bool,
                Value::S8(_) => ValType::S8,
                Value::U8(_) => ValType::U8,
                Value::S16(_) => ValType::S16,
                Value::U16(_) => ValType::U16,
                Value::S32(_) => ValType::S32,
                Value::U32(_) => ValType::U32,
                Value::S64(_) => ValType::S64,
                Value::U64(_) => ValType::U64,
                Value::F32(_) => ValType::F32,
                Value::F64(_) => ValType::F64,
                Value::Char(_) => ValType::Char,
                Value::String(_) => ValType::String,
                Value::List(list) => {
                    // Determine element type from first element, or default to Bool
                    let element_type =
                        list.first().map(|v| v.value_type()).unwrap_or(ValType::Bool);
                    ValType::List(Box::new(element_type))
                },
                Value::Record(_) => {
                    // For records, we'd need more context to reconstruct the exact type
                    // For now, return a default empty record type
                    ValType::Record(Record::default())
                },
                Value::Tuple(tuple) => {
                    let types = tuple.iter().map(|v| v.value_type()).collect();
                    ValType::Tuple(Tuple { types })
                },
                Value::Variant { .. } => {
                    // For variants, we'd need more context to reconstruct the exact type
                    ValType::Variant(Variant::default())
                },
                Value::Enum(_) => ValType::Enum(Enum::default()),
                Value::Option(opt) => {
                    let inner_type = opt.as_ref().map(|v| v.value_type()).unwrap_or(ValType::Bool);
                    ValType::Option(Box::new(inner_type))
                },
                Value::Result(result) => {
                    let ok_type = result
                        .as_ref()
                        .ok()
                        .and_then(|opt| opt.as_ref())
                        .map(|v| Box::new(v.value_type()));
                    let err_type = result.as_ref().err().map(|v| Box::new(v.value_type()));
                    ValType::Result(Result_ {
                        ok:  ok_type,
                        err: err_type,
                    })
                },
                Value::Flags(_) => ValType::Flags(Flags::default()),
                Value::Own(handle) => ValType::Own(*handle),
                Value::Borrow(handle) => ValType::Borrow(*handle),
            }
        }
    }

    /// Clean runtime function type without provider parameters
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct FuncType {
        /// Function parameter types
        pub params:  Vec<ValType>,
        /// Function result types
        pub results: Vec<ValType>,
    }

    impl Default for FuncType {
        fn default() -> Self {
            Self {
                params:  Vec::new(),
                results: Vec::new(),
            }
        }
    }

    /// Clean runtime memory type without provider parameters
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct MemoryType {
        /// Memory limits
        pub limits: Limits,
        /// Memory is shared between threads
        pub shared: bool,
    }

    impl Default for MemoryType {
        fn default() -> Self {
            Self {
                limits: Limits::default(),
                shared: false,
            }
        }
    }

    /// Clean runtime table type without provider parameters
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct TableType {
        /// Element type
        pub element_type: RefType,
        /// Table limits
        pub limits:       Limits,
    }

    impl Default for TableType {
        fn default() -> Self {
            Self {
                element_type: RefType::FuncRef,
                limits:       Limits::default(),
            }
        }
    }

    /// Clean runtime global type without provider parameters
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct GlobalType {
        /// Value type
        pub value_type: ValType,
        /// Mutability
        pub mutable:    bool,
    }

    impl Default for GlobalType {
        fn default() -> Self {
            Self {
                value_type: ValType::Bool,
                mutable:    false,
            }
        }
    }

    /// Memory limits
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Limits {
        /// Minimum size
        pub min: u32,
        /// Maximum size (optional)
        pub max: Option<u32>,
    }

    impl Default for Limits {
        fn default() -> Self {
            Self { min: 0, max: None }
        }
    }

    /// Reference type
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub enum RefType {
        /// Function reference
        FuncRef,
        /// External reference
        ExternRef,
    }

    impl Default for RefType {
        fn default() -> Self {
            Self::FuncRef
        }
    }

    /// Clean component type without provider parameters
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ComponentType {
        /// Component imports
        pub imports:   Vec<(String, String, ExternType)>,
        /// Component exports
        pub exports:   Vec<(String, ExternType)>,
        /// Component instances
        pub instances: Vec<ComponentTypeDefinition>,
    }

    impl Default for ComponentType {
        fn default() -> Self {
            Self {
                imports:   Vec::new(),
                exports:   Vec::new(),
                instances: Vec::new(),
            }
        }
    }

    /// External type for imports and exports
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ExternType {
        /// Function type
        Function(FuncType),
        /// Table type
        Table(TableType),
        /// Memory type
        Memory(MemoryType),
        /// Global type
        Global(GlobalType),
        /// Component type
        Component(ComponentType),
        /// Instance type
        Instance(InstanceType),
    }

    impl Default for ExternType {
        fn default() -> Self {
            Self::Function(FuncType::default())
        }
    }

    /// Instance type definition
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct InstanceType {
        /// Instance exports
        pub exports: Vec<(String, ExternType)>,
    }

    impl Default for InstanceType {
        fn default() -> Self {
            Self {
                exports: Vec::new(),
            }
        }
    }

    /// Component type definition
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ComponentTypeDefinition {
        /// Type name
        pub name: String,
        /// Type definition
        pub ty:   ExternType,
    }

    impl Default for ComponentTypeDefinition {
        fn default() -> Self {
            Self {
                name: String::new(),
                ty:   ExternType::default(),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_valtype_creation() {
            let val_type = ValType::S32;
            assert_eq!(val_type, ValType::S32);
        }

        #[test]
        fn test_record_creation() {
            let record = Record {
                fields: vec![
                    Field {
                        name: "x".to_string(),
                        ty:   ValType::S32,
                    },
                    Field {
                        name: "y".to_string(),
                        ty:   ValType::F64,
                    },
                ],
            };
            assert_eq!(record.fields.len(), 2);
        }

        #[test]
        fn test_value_type_inference() {
            let value = Value::S32(42);
            assert_eq!(value.value_type(), ValType::S32);

            let list_value = Value::List(vec![Value::S32(1), Value::S32(2)]);
            assert_eq!(
                list_value.value_type(),
                ValType::List(Box::new(ValType::S32))
            );
        }

        #[test]
        fn test_func_type_creation() {
            let func_type = FuncType {
                params:  vec![ValType::S32, ValType::S32],
                results: vec![ValType::S32],
            };
            assert_eq!(func_type.params.len(), 2);
            assert_eq!(func_type.results.len(), 1);
        }
    }
}
