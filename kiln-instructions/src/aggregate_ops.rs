//! WebAssembly 3.0 Aggregate type operations implementation.
//!
//! This module implements WebAssembly 3.0 aggregate type instructions
//! including:
//! - struct.new: Create a new struct instance
//! - struct.get: Get a field from a struct
//! - struct.set: Set a field in a struct
//! - array.new: Create a new array instance
//! - array.get: Get an element from an array
//! - array.set: Set an element in an array
//! - array.len: Get the length of an array
//!
//! These operations support the WebAssembly 3.0 GC proposal
//! and work across std, `no_std+alloc`, and pure `no_std` environments.

// WebAssembly value semantics require bit reinterpretation casts.
// These casts implement correct WASM behavior, not bugs.
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]

use wrt_error::{
    Error,
    Result,
};
use wrt_foundation::{
    traits::DefaultMemoryProvider,
    types::ValueType,
    values::{
        ArrayRef,
        StructRef,
        Value,
    },
};

use crate::{
    prelude::{
        Debug,
        Eq,
        PartialEq,
    },
    validation::{
        Validate,
        ValidationContext,
    },
};

/// Struct new operation - creates a new struct instance
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructNew {
    /// Type index of the struct to create
    pub type_index: u32,
}

impl StructNew {
    /// Create a new struct.new instruction
    #[must_use]
    pub fn new(type_index: u32) -> Self {
        Self { type_index }
    }

    /// Execute the struct.new instruction
    /// Takes field values from the stack and creates a new struct
    ///
    /// # Errors
    ///
    /// Returns an error if struct creation or field assignment fails
    pub fn execute(&self, field_values: &[Value]) -> Result<Value> {
        let mut struct_ref = StructRef::new(self.type_index, DefaultMemoryProvider::default())?;

        // Add all field values to the struct
        for value in field_values {
            struct_ref.add_field(value.clone())?;
        }

        Ok(Value::StructRef(Some(struct_ref)))
    }
}

/// Struct get operation - gets a field from a struct
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructGet {
    /// Type index of the struct
    pub type_index:  u32,
    /// Field index to get
    pub field_index: u32,
}

impl StructGet {
    /// Create a new struct.get instruction
    #[must_use]
    pub fn new(type_index: u32, field_index: u32) -> Self {
        Self {
            type_index,
            field_index,
        }
    }

    /// Execute the struct.get instruction
    ///
    /// # Errors
    ///
    /// Returns an error if value is not a struct or field index is invalid
    pub fn execute(&self, struct_value: Value) -> Result<Value> {
        match struct_value {
            Value::StructRef(Some(struct_ref)) => {
                // Verify type index matches
                if struct_ref.type_index != self.type_index {
                    return Err(Error::type_error("Struct type index mismatch"));
                }

                // Get the field value
                struct_ref.get_field(self.field_index as usize)
            },
            Value::StructRef(None) => Err(Error::runtime_error(
                "Cannot get field from null struct reference",
            )),
            _ => Err(Error::type_error("struct.get requires a struct reference")),
        }
    }
}

/// Struct set operation - sets a field in a struct
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructSet {
    /// Type index of the struct
    pub type_index:  u32,
    /// Field index to set
    pub field_index: u32,
}

impl StructSet {
    /// Create a new struct.set instruction
    #[must_use]
    pub fn new(type_index: u32, field_index: u32) -> Self {
        Self {
            type_index,
            field_index,
        }
    }

    /// Execute the struct.set instruction
    ///
    /// # Errors
    ///
    /// Returns an error if value is not a struct or field update fails
    pub fn execute(&self, struct_value: Value, new_value: Value) -> Result<Value> {
        match struct_value {
            Value::StructRef(Some(mut struct_ref)) => {
                // Verify type index matches
                if struct_ref.type_index != self.type_index {
                    return Err(Error::type_error("Struct type index mismatch"));
                }

                // Set the field value
                struct_ref.set_field(self.field_index as usize, new_value)?;

                Ok(Value::StructRef(Some(struct_ref)))
            },
            Value::StructRef(None) => Err(Error::runtime_error(
                "Cannot set field on null struct reference",
            )),
            _ => Err(Error::type_error("struct.set requires a struct reference")),
        }
    }
}

/// Array new operation - creates a new array instance
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayNew {
    /// Type index of the array to create
    pub type_index: u32,
}

impl ArrayNew {
    /// Create a new array.new instruction
    #[must_use]
    pub fn new(type_index: u32) -> Self {
        Self { type_index }
    }

    /// Execute the array.new instruction
    /// Takes size and initial value from the stack
    ///
    /// # Errors
    ///
    /// Returns an error if array creation fails
    pub fn execute(&self, size: u32, init_value: Value) -> Result<Value> {
        let array_ref = ArrayRef::with_size(
            self.type_index,
            size as usize,
            init_value,
            DefaultMemoryProvider::default(),
        )?;

        Ok(Value::ArrayRef(Some(array_ref)))
    }
}

/// Array get operation - gets an element from an array
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayGet {
    /// Type index of the array
    pub type_index: u32,
}

impl ArrayGet {
    /// Create a new array.get instruction
    #[must_use]
    pub fn new(type_index: u32) -> Self {
        Self { type_index }
    }

    /// Execute the array.get instruction
    ///
    /// # Errors
    ///
    /// Returns an error if value is not an array or index is out of bounds
    pub fn execute(&self, array_value: Value, index: u32) -> Result<Value> {
        match array_value {
            Value::ArrayRef(Some(array_ref)) => {
                // Verify type index matches
                if array_ref.type_index != self.type_index {
                    return Err(Error::type_error("Array type index mismatch"));
                }

                // Get the element value
                array_ref.get(index as usize)
            },
            Value::ArrayRef(None) => Err(Error::runtime_error(
                "Cannot get element from null array reference",
            )),
            _ => Err(Error::type_error("array.get requires an array reference")),
        }
    }
}

/// Array set operation - sets an element in an array
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArraySet {
    /// Type index of the array
    pub type_index: u32,
}

impl ArraySet {
    /// Create a new array.set instruction
    #[must_use]
    pub fn new(type_index: u32) -> Self {
        Self { type_index }
    }

    /// Execute the array.set instruction
    ///
    /// # Errors
    ///
    /// Returns an error if value is not an array or index is out of bounds
    pub fn execute(&self, array_value: Value, index: u32, new_value: Value) -> Result<Value> {
        match array_value {
            Value::ArrayRef(Some(mut array_ref)) => {
                // Verify type index matches
                if array_ref.type_index != self.type_index {
                    return Err(Error::type_error("Array type index mismatch"));
                }

                // Set the element value
                array_ref.set(index as usize, new_value)?;

                Ok(Value::ArrayRef(Some(array_ref)))
            },
            Value::ArrayRef(None) => Err(Error::runtime_error(
                "Cannot set element on null array reference",
            )),
            _ => Err(Error::type_error("array.set requires an array reference")),
        }
    }
}

/// Array length operation - gets the length of an array
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayLen {
    /// Type index of the array
    pub type_index: u32,
}

impl ArrayLen {
    /// Create a new array.len instruction
    #[must_use]
    pub fn new(type_index: u32) -> Self {
        Self { type_index }
    }

    /// Execute the array.len instruction
    ///
    /// # Errors
    ///
    /// Returns an error if value is not an array
    pub fn execute(&self, array_value: Value) -> Result<Value> {
        match array_value {
            Value::ArrayRef(Some(array_ref)) => {
                // Verify type index matches
                if array_ref.type_index != self.type_index {
                    return Err(Error::type_error("Array type index mismatch"));
                }

                // Return the array length as i32
                Ok(Value::I32(array_ref.len() as i32))
            },
            Value::ArrayRef(None) => Err(Error::runtime_error(
                "Cannot get length of null array reference",
            )),
            _ => Err(Error::type_error("array.len requires an array reference")),
        }
    }
}

/// Trait for aggregate type operations that can be implemented by execution
/// contexts
pub trait AggregateOperations {
    /// Get struct type information by type index
    ///
    /// # Errors
    ///
    /// Returns an error if type index is invalid
    fn get_struct_type(&self, type_index: u32) -> Result<Option<u32>>; // For now, just validate existence

    /// Get array type information by type index
    ///
    /// # Errors
    ///
    /// Returns an error if type index is invalid
    fn get_array_type(&self, type_index: u32) -> Result<Option<u32>>; // For now, just validate existence

    /// Validate that a struct type index exists
    ///
    /// # Errors
    ///
    /// Returns an error if struct type doesn't exist
    fn validate_struct_type(&self, type_index: u32) -> Result<()>;

    /// Validate that an array type index exists
    ///
    /// # Errors
    ///
    /// Returns an error if array type doesn't exist
    fn validate_array_type(&self, type_index: u32) -> Result<()>;
}

/// Aggregate operation enum for unified handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregateOp {
    /// struct.new operation
    StructNew(StructNew),
    /// struct.get operation
    StructGet(StructGet),
    /// struct.set operation
    StructSet(StructSet),
    /// array.new operation
    ArrayNew(ArrayNew),
    /// array.get operation
    ArrayGet(ArrayGet),
    /// array.set operation
    ArraySet(ArraySet),
    /// array.len operation
    ArrayLen(ArrayLen),
}

impl AggregateOp {
    /// Execute the aggregate operation with the given context and stack values
    ///
    /// # Errors
    ///
    /// Returns an error if operation execution fails
    pub fn execute<C: AggregateOperations>(
        &self,
        context: &C,
        operands: &[Value],
    ) -> Result<Value> {
        match self {
            AggregateOp::StructNew(op) => {
                // Validate struct type exists
                context.validate_struct_type(op.type_index)?;
                op.execute(operands)
            },
            AggregateOp::StructGet(op) => {
                if operands.is_empty() {
                    return Err(Error::runtime_error("struct.get requires one operand"));
                }
                // Validate struct type exists
                context.validate_struct_type(op.type_index)?;
                op.execute(operands[0].clone())
            },
            AggregateOp::StructSet(op) => {
                if operands.len() < 2 {
                    return Err(Error::runtime_error("struct.set requires two operands"));
                }
                // Validate struct type exists
                context.validate_struct_type(op.type_index)?;
                op.execute(operands[0].clone(), operands[1].clone())
            },
            AggregateOp::ArrayNew(op) => {
                if operands.len() < 2 {
                    return Err(Error::runtime_error(
                        "array.new requires two operands (size, init_value)",
                    ));
                }
                // Validate array type exists
                context.validate_array_type(op.type_index)?;

                // Extract size and init value
                let size = operands[0]
                    .as_u32()
                    .ok_or_else(|| Error::type_error("array.new size must be i32"))?;
                let init_value = operands[1].clone();

                op.execute(size, init_value)
            },
            AggregateOp::ArrayGet(op) => {
                if operands.len() < 2 {
                    return Err(Error::runtime_error("array.get requires two operands"));
                }
                // Validate array type exists
                context.validate_array_type(op.type_index)?;

                // Extract index
                let index = operands[1]
                    .as_u32()
                    .ok_or_else(|| Error::type_error("array.get index must be i32"))?;

                op.execute(operands[0].clone(), index)
            },
            AggregateOp::ArraySet(op) => {
                if operands.len() < 3 {
                    return Err(Error::runtime_error("array.set requires three operands"));
                }
                // Validate array type exists
                context.validate_array_type(op.type_index)?;

                // Extract index
                let index = operands[1]
                    .as_u32()
                    .ok_or_else(|| Error::type_error("array.set index must be i32"))?;

                op.execute(operands[0].clone(), index, operands[2].clone())
            },
            AggregateOp::ArrayLen(op) => {
                if operands.is_empty() {
                    return Err(Error::runtime_error("array.len requires one operand"));
                }
                // Validate array type exists
                context.validate_array_type(op.type_index)?;
                op.execute(operands[0].clone())
            },
        }
    }
}

// Validation implementations
impl Validate for StructNew {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // struct.new: [field_types...] -> [structref]
        // For now, we don't have access to the struct type definition in validation
        // context In a full implementation, this would validate field types
        // against the struct definition
        ctx.push_type(ValueType::StructRef(self.type_index))?;
        Ok(())
    }
}

impl Validate for StructGet {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // struct.get: [structref] -> [field_type]
        if !ctx.is_unreachable() {
            let struct_type = ctx.pop_type()?;
            match struct_type {
                ValueType::StructRef(type_idx) if type_idx == self.type_index => {
                    // In a full implementation, this would push the actual field type
                    // For now, we'll push I32 as a placeholder
                    ctx.push_type(ValueType::I32)?;
                },
                ValueType::StructRef(_) => {
                    return Err(Error::type_error("struct.get type index mismatch"));
                },
                _ => {
                    return Err(Error::type_error("struct.get expects struct reference"));
                },
            }
        }
        Ok(())
    }
}

impl Validate for StructSet {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // struct.set: [structref field_value] -> []
        if !ctx.is_unreachable() {
            let _field_value_type = ctx.pop_type()?; // In full implementation, validate against field type
            let struct_type = ctx.pop_type()?;
            match struct_type {
                ValueType::StructRef(type_idx) if type_idx == self.type_index => {
                    // struct.set doesn't push anything to the stack
                },
                ValueType::StructRef(_) => {
                    return Err(Error::type_error("struct.set type index mismatch"));
                },
                _ => {
                    return Err(Error::type_error("struct.set expects struct reference"));
                },
            }
        }
        Ok(())
    }
}

impl Validate for ArrayNew {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // array.new: [i32 init_value] -> [arrayref]
        if !ctx.is_unreachable() {
            let _init_value_type = ctx.pop_type()?; // In full implementation, validate against array element type
            let size_type = ctx.pop_type()?;
            match size_type {
                ValueType::I32 => {
                    ctx.push_type(ValueType::ArrayRef(self.type_index))?;
                },
                _ => {
                    return Err(Error::type_error("array.new expects i32 size"));
                },
            }
        }
        Ok(())
    }
}

impl Validate for ArrayGet {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // array.get: [arrayref i32] -> [element_type]
        if !ctx.is_unreachable() {
            let index_type = ctx.pop_type()?;
            let array_type = ctx.pop_type()?;
            match (array_type, index_type) {
                (ValueType::ArrayRef(type_idx), ValueType::I32) if type_idx == self.type_index => {
                    // In a full implementation, this would push the actual element type
                    // For now, we'll push I32 as a placeholder
                    ctx.push_type(ValueType::I32)?;
                },
                (ValueType::ArrayRef(_), ValueType::I32) => {
                    return Err(Error::type_error("array.get type index mismatch"));
                },
                _ => {
                    return Err(Error::type_error(
                        "array.get expects array reference and i32 index",
                    ));
                },
            }
        }
        Ok(())
    }
}

impl Validate for ArraySet {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // array.set: [arrayref i32 element_value] -> []
        if !ctx.is_unreachable() {
            let _element_value_type = ctx.pop_type()?; // In full implementation, validate against array element type
            let index_type = ctx.pop_type()?;
            let array_type = ctx.pop_type()?;
            match (array_type, index_type) {
                (ValueType::ArrayRef(type_idx), ValueType::I32) if type_idx == self.type_index => {
                    // array.set doesn't push anything to the stack
                },
                (ValueType::ArrayRef(_), ValueType::I32) => {
                    return Err(Error::type_error("array.set type index mismatch"));
                },
                _ => {
                    return Err(Error::type_error(
                        "array.set expects array reference and i32 index",
                    ));
                },
            }
        }
        Ok(())
    }
}

impl Validate for ArrayLen {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // array.len: [arrayref] -> [i32]
        if !ctx.is_unreachable() {
            let array_type = ctx.pop_type()?;
            match array_type {
                ValueType::ArrayRef(type_idx) if type_idx == self.type_index => {
                    ctx.push_type(ValueType::I32)?;
                },
                ValueType::ArrayRef(_) => {
                    return Err(Error::type_error("array.len type index mismatch"));
                },
                _ => {
                    return Err(Error::type_error("array.len expects array reference"));
                },
            }
        }
        Ok(())
    }
}

impl Validate for AggregateOp {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        match self {
            AggregateOp::StructNew(op) => op.validate(ctx),
            AggregateOp::StructGet(op) => op.validate(ctx),
            AggregateOp::StructSet(op) => op.validate(ctx),
            AggregateOp::ArrayNew(op) => op.validate(ctx),
            AggregateOp::ArrayGet(op) => op.validate(ctx),
            AggregateOp::ArraySet(op) => op.validate(ctx),
            AggregateOp::ArrayLen(op) => op.validate(ctx),
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use wrt_foundation::values::V128;

    use super::*;

    struct MockAggregateContext;

    impl AggregateOperations for MockAggregateContext {
        fn get_struct_type(&self, type_index: u32) -> Result<Option<u32>> {
            // Mock: struct types 0-9 exist
            if type_index < 10 {
                Ok(Some(type_index))
            } else {
                Ok(None)
            }
        }

        fn get_array_type(&self, type_index: u32) -> Result<Option<u32>> {
            // Mock: array types 0-9 exist
            if type_index < 10 {
                Ok(Some(type_index))
            } else {
                Ok(None)
            }
        }

        fn validate_struct_type(&self, type_index: u32) -> Result<()> {
            if type_index < 10 {
                Ok(())
            } else {
                Err(Error::runtime_error("Struct type index out of bounds"))
            }
        }

        fn validate_array_type(&self, type_index: u32) -> Result<()> {
            if type_index < 10 {
                Ok(())
            } else {
                Err(Error::runtime_error("Array type index out of bounds"))
            }
        }
    }

    #[test]
    fn test_struct_new() {
        let op = StructNew::new(1);
        let field_values = vec![Value::I32(42), Value::I64(123)];
        let result = op.execute(&field_values).unwrap();

        match result {
            Value::StructRef(Some(struct_ref)) => {
                assert_eq!(struct_ref.type_index, 1);
                assert_eq!(struct_ref.get_field(0).unwrap(), Value::I32(42));
                assert_eq!(struct_ref.get_field(1).unwrap(), Value::I64(123));
            },
            _ => panic!("Expected struct reference"),
        }
    }

    #[test]
    fn test_struct_get() {
        let op = StructGet::new(1, 0);

        // Create a struct to test with
        let mut struct_ref = StructRef::new(1, DefaultMemoryProvider::default()).unwrap();
        struct_ref.add_field(Value::I32(42)).unwrap();
        let struct_value = Value::StructRef(Some(struct_ref));

        let result = op.execute(struct_value).unwrap();
        assert_eq!(result, Value::I32(42));
    }

    #[test]
    fn test_struct_get_null() {
        let op = StructGet::new(1, 0);
        let result = op.execute(Value::StructRef(None));
        assert!(result.is_err());
    }

    #[test]
    fn test_struct_set() {
        let op = StructSet::new(1, 0);

        // Create a struct to test with
        let mut struct_ref = StructRef::new(1, DefaultMemoryProvider::default()).unwrap();
        struct_ref.add_field(Value::I32(42)).unwrap();
        let struct_value = Value::StructRef(Some(struct_ref));

        let result = op.execute(struct_value, Value::I32(100)).unwrap();

        match result {
            Value::StructRef(Some(struct_ref)) => {
                assert_eq!(struct_ref.get_field(0).unwrap(), Value::I32(100));
            },
            _ => panic!("Expected struct reference"),
        }
    }

    #[test]
    fn test_array_new() {
        let op = ArrayNew::new(2);
        let result = op.execute(3, Value::I32(42)).unwrap();

        match result {
            Value::ArrayRef(Some(array_ref)) => {
                assert_eq!(array_ref.type_index, 2);
                assert_eq!(array_ref.len(), 3);
                assert_eq!(array_ref.get(0).unwrap(), Value::I32(42));
                assert_eq!(array_ref.get(1).unwrap(), Value::I32(42));
                assert_eq!(array_ref.get(2).unwrap(), Value::I32(42));
            },
            _ => panic!("Expected array reference"),
        }
    }

    #[test]
    fn test_array_get() {
        let op = ArrayGet::new(2);

        // Create an array to test with
        let array_ref =
            ArrayRef::with_size(2, 2, Value::I32(42), DefaultMemoryProvider::default()).unwrap();
        let array_value = Value::ArrayRef(Some(array_ref));

        let result = op.execute(array_value, 1).unwrap();
        assert_eq!(result, Value::I32(42));
    }

    #[test]
    fn test_array_set() {
        let op = ArraySet::new(2);

        // Create an array to test with
        let array_ref =
            ArrayRef::with_size(2, 2, Value::I32(42), DefaultMemoryProvider::default()).unwrap();
        let array_value = Value::ArrayRef(Some(array_ref));

        let result = op.execute(array_value, 1, Value::I32(100)).unwrap();

        match result {
            Value::ArrayRef(Some(array_ref)) => {
                assert_eq!(array_ref.get(1).unwrap(), Value::I32(100));
            },
            _ => panic!("Expected array reference"),
        }
    }

    #[test]
    fn test_array_len() {
        let op = ArrayLen::new(2);

        // Create an array to test with
        let array_ref =
            ArrayRef::with_size(2, 5, Value::I32(42), DefaultMemoryProvider::default()).unwrap();
        let array_value = Value::ArrayRef(Some(array_ref));

        let result = op.execute(array_value).unwrap();
        assert_eq!(result, Value::I32(5));
    }

    #[test]
    fn test_aggregate_op_enum() {
        let context = MockAggregateContext;

        // Test StructNew
        let struct_new_op = AggregateOp::StructNew(StructNew::new(1));
        let result = struct_new_op.execute(&context, &[Value::I32(42)]).unwrap();
        assert!(if let Value::StructRef(Some(_)) = result { true } else { false });

        // Test ArrayNew
        let array_new_op = AggregateOp::ArrayNew(ArrayNew::new(2));
        let result = array_new_op.execute(&context, &[Value::I32(3), Value::I32(42)]).unwrap();
        assert!(if let Value::ArrayRef(Some(_)) = result { true } else { false });

        // Test invalid type index
        let invalid_struct_op = AggregateOp::StructNew(StructNew::new(15));
        let result = invalid_struct_op.execute(&context, &[]);
        assert!(result.is_err());
    }
}
