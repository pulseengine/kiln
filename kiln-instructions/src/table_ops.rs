// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Table operations for WebAssembly instructions.
//!
//! This module provides implementations for WebAssembly table access
//! instructions, including get, set, grow, size, fill, copy, and initialization
//! operations.
//!
//! # Table Operation Architecture
//!
//! This module separates table operations from the underlying table
//! implementation, allowing different execution engines to share the same
//! table access code. The key components are:
//!
//! - Individual operation structs: `TableGet`, `TableSet`, `TableSize`, etc.
//! - `TableOp` unified enum for instruction dispatching
//! - `TableOperations` trait defining the interface to table implementations
//! - `TableContext` trait for execution context integration
//!
//! # Features
//!
//! - Support for all WebAssembly table operations
//! - Function and external reference handling
//! - Multi-table support
//! - Element segment operations
//! - Bounds checking and validation
//!
//! # Usage
//!
//! ```no_run
//! use wrt_foundation::values::FuncRef;
//! use wrt_instructions::{
//!     table_ops::{
//!         TableGet,
//!         TableSet,
//!     },
//!     Value,
//! };
//!
//! // Get element from table
//! let get_op = TableGet::new(0); // table index 0
//!                                // Execute with appropriate context
//!
//! // Set element in table
//! let set_op = TableSet::new(0); // table index 0
//!                                // Execute with appropriate context
//! ```

// WebAssembly value semantics require bit reinterpretation casts.
// i32/u32 and i64/u64 are the same bits - just different interpretations.
// These casts implement correct WASM behavior, not bugs.
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_possible_truncation)]

use crate::{
    prelude::{
        Debug,
        Error,
        PartialEq,
        PureInstruction,
        Result,
        Value,
        ValueType,
    },
    validation::{
        Validate,
        ValidationContext,
    },
};

/// Table operations trait defining the interface to table implementations
pub trait TableOperations {
    /// Get a reference from a table
    ///
    /// # Errors
    ///
    /// Returns an error if table or element index is out of bounds
    fn get_table_element(&self, table_index: u32, elem_index: u32) -> Result<Value>;

    /// Set a reference in a table
    ///
    /// # Errors
    ///
    /// Returns an error if table or element index is out of bounds or type mismatch occurs
    fn set_table_element(&mut self, table_index: u32, elem_index: u32, value: Value) -> Result<()>;

    /// Get the size of a table
    ///
    /// # Errors
    ///
    /// Returns an error if table index is invalid
    fn get_table_size(&self, table_index: u32) -> Result<u32>;

    /// Grow a table by a given number of elements, returning previous size or
    /// -1 on failure
    ///
    /// # Errors
    ///
    /// Returns an error if table index is invalid or growth fails
    fn grow_table(&mut self, table_index: u32, delta: u32, init_value: Value) -> Result<i32>;

    /// Fill a table range with a value
    ///
    /// # Errors
    ///
    /// Returns an error if table index or range is invalid
    fn fill_table(&mut self, table_index: u32, dst: u32, val: Value, len: u32) -> Result<()>;

    /// Copy elements from one table to another
    ///
    /// # Errors
    ///
    /// Returns an error if table indices or ranges are invalid
    fn copy_table(
        &mut self,
        dst_table: u32,
        dst_index: u32,
        src_table: u32,
        src_index: u32,
        len: u32,
    ) -> Result<()>;
}

/// Element segment operations trait for table.init and elem.drop
pub trait ElementSegmentOperations {
    /// Get element from segment
    ///
    /// # Errors
    ///
    /// Returns an error if element index is invalid
    #[cfg(feature = "std")]
    fn get_element_segment(&self, elem_index: u32) -> Result<Option<Vec<Value>>>;

    #[cfg(not(feature = "std"))]
    fn get_element_segment(
        &self,
        elem_index: u32,
    ) -> Result<
        Option<wrt_foundation::BoundedVec<Value, 65536, wrt_foundation::NoStdProvider<65536>>>,
    >;

    /// Drop (mark as unavailable) an element segment
    ///
    /// # Errors
    ///
    /// Returns an error if element index is invalid
    fn drop_element_segment(&mut self, elem_index: u32) -> Result<()>;
}

/// Table get operation (table.get)
#[derive(Debug, Clone, PartialEq)]
pub struct TableGet {
    /// Table index to get from
    pub table_index: u32,
}

impl TableGet {
    /// Create a new table.get operation
    #[must_use]
    pub fn new(table_index: u32) -> Self {
        Self { table_index }
    }

    /// Execute table.get operation
    ///
    /// # Arguments
    ///
    /// * `table` - The table to operate on
    /// * `index` - Index to get (i32 value)
    ///
    /// # Returns
    ///
    /// The reference value at the index
    ///
    /// # Errors
    ///
    /// Returns an error if index is invalid or out of bounds
    pub fn execute(&self, table: &(impl TableOperations + ?Sized), index: &Value) -> Result<Value> {
        let idx = match index {
            Value::I32(i) => {
                if *i < 0 {
                    return Err(Error::runtime_error("Table index cannot be negative"));
                }
                // Safety: Validated i >= 0, safe to cast to u32
                #[allow(clippy::cast_sign_loss)]
                {
                    *i as u32
                }
            },
            _ => return Err(Error::type_error("table.get index must be i32")),
        };

        table.get_table_element(self.table_index, idx)
    }
}

/// Table set operation (table.set)
#[derive(Debug, Clone, PartialEq)]
pub struct TableSet {
    /// Table index to set in
    pub table_index: u32,
}

impl TableSet {
    /// Create a new table.set operation
    #[must_use]
    pub fn new(table_index: u32) -> Self {
        Self { table_index }
    }

    /// Execute table.set operation
    ///
    /// # Arguments
    ///
    /// * `table` - The table to operate on
    /// * `index` - Index to set (i32 value)
    /// * `value` - Reference value to set
    ///
    /// # Returns
    ///
    /// Success or an error
    ///
    /// # Errors
    ///
    /// Returns an error if index or value type is invalid or table access fails
    pub fn execute(
        &self,
        table: &mut (impl TableOperations + ?Sized),
        index: &Value,
        value: &Value,
    ) -> Result<()> {
        let idx = match index {
            Value::I32(i) => {
                if *i < 0 {
                    return Err(Error::runtime_error("Table index cannot be negative"));
                }
                // Safety: Validated i >= 0, safe to cast to u32
                #[allow(clippy::cast_sign_loss)]
                {
                    *i as u32
                }
            },
            _ => return Err(Error::type_error("table.set index must be i32")),
        };

        // Validate that value is a reference type
        match value {
            Value::FuncRef(_) | Value::ExternRef(_) => {},
            _ => {
                return Err(Error::type_error(
                    "table.set value must be a reference type",
                ))
            },
        }

        table.set_table_element(self.table_index, idx, value.clone())
    }
}

/// Table size operation (table.size)
#[derive(Debug, Clone, PartialEq)]
pub struct TableSize {
    /// Table index to get size of
    pub table_index: u32,
}

impl TableSize {
    /// Create a new table.size operation
    #[must_use]
    pub fn new(table_index: u32) -> Self {
        Self { table_index }
    }

    /// Execute table.size operation
    ///
    /// # Arguments
    ///
    /// * `table` - The table to query
    ///
    /// # Returns
    ///
    /// The size of the table as an i32 Value
    ///
    /// # Errors
    ///
    /// Returns an error if table index is invalid
    pub fn execute(&self, table: &(impl TableOperations + ?Sized)) -> Result<Value> {
        let size = table.get_table_size(self.table_index)?;
        Ok(Value::I32(size as i32))
    }
}

/// Table grow operation (table.grow)
#[derive(Debug, Clone, PartialEq)]
pub struct TableGrow {
    /// Table index to grow
    pub table_index: u32,
}

impl TableGrow {
    /// Create a new table.grow operation
    #[must_use]
    pub fn new(table_index: u32) -> Self {
        Self { table_index }
    }

    /// Execute table.grow operation
    ///
    /// # Arguments
    ///
    /// * `table` - The table to grow
    /// * `init_value` - Initial value for new elements
    /// * `delta` - Number of elements to grow by (i32 value)
    ///
    /// # Returns
    ///
    /// The previous size, or -1 if the operation failed (as i32 Value)
    ///
    /// # Errors
    ///
    /// Returns an error if table grow operation fails
    pub fn execute(
        &self,
        table: &mut (impl TableOperations + ?Sized),
        init_value: &Value,
        delta: &Value,
    ) -> Result<Value> {
        let delta_elems = match delta {
            Value::I32(d) => {
                if *d < 0 {
                    return Ok(Value::I32(-1)); // Negative delta fails
                }
                *d as u32
            },
            _ => return Err(Error::type_error("table.grow delta must be i32")),
        };

        // Validate that init_value is a reference type
        match init_value {
            Value::FuncRef(_) | Value::ExternRef(_) => {},
            _ => {
                return Err(Error::type_error(
                    "table.grow init value must be a reference type",
                ))
            },
        }

        let prev_size = table.grow_table(self.table_index, delta_elems, init_value.clone())?;
        Ok(Value::I32(prev_size))
    }
}

/// Table fill operation (table.fill)
#[derive(Debug, Clone, PartialEq)]
pub struct TableFill {
    /// Table index to fill
    pub table_index: u32,
}

impl TableFill {
    /// Create a new table.fill operation
    #[must_use]
    pub fn new(table_index: u32) -> Self {
        Self { table_index }
    }

    /// Execute table.fill operation
    ///
    /// # Arguments
    ///
    /// * `table` - The table to fill
    /// * `dest` - Destination index (i32)
    /// * `value` - Fill value (reference)
    /// * `size` - Number of elements to fill (i32)
    ///
    /// # Returns
    ///
    /// Success or an error
    ///
    /// # Errors
    ///
    /// Returns an error if table fill operation fails
    pub fn execute(
        &self,
        table: &mut (impl TableOperations + ?Sized),
        dest: &Value,
        value: &Value,
        size: &Value,
    ) -> Result<()> {
        let dest_idx = match dest {
            Value::I32(d) => {
                if *d < 0 {
                    return Err(Error::runtime_error(
                        "Table destination index cannot be negative",
                    ));
                }
                *d as u32
            },
            _ => return Err(Error::type_error("table.fill dest must be i32")),
        };

        let fill_size = match size {
            Value::I32(s) => {
                if *s < 0 {
                    return Err(Error::runtime_error("Table fill size cannot be negative"));
                }
                *s as u32
            },
            _ => return Err(Error::type_error("table.fill size must be i32")),
        };

        // Validate that value is a reference type
        match value {
            Value::FuncRef(_) | Value::ExternRef(_) => {},
            _ => {
                return Err(Error::type_error(
                    "table.fill value must be a reference type",
                ))
            },
        }

        table.fill_table(self.table_index, dest_idx, value.clone(), fill_size)
    }
}

/// Table copy operation (table.copy)
#[derive(Debug, Clone, PartialEq)]
pub struct TableCopy {
    /// Destination table index
    pub dest_table_index: u32,
    /// Source table index  
    pub src_table_index:  u32,
}

impl TableCopy {
    /// Create a new table.copy operation
    #[must_use]
    pub fn new(dest_table_index: u32, src_table_index: u32) -> Self {
        Self {
            dest_table_index,
            src_table_index,
        }
    }

    /// Execute table.copy operation
    ///
    /// # Arguments
    ///
    /// * `table` - The table operations interface
    /// * `dest` - Destination index (i32)
    /// * `src` - Source index (i32)
    /// * `size` - Number of elements to copy (i32)
    ///
    /// # Returns
    ///
    /// Success or an error
    ///
    /// # Errors
    ///
    /// Returns an error if table copy operation fails
    pub fn execute(
        &self,
        table: &mut (impl TableOperations + ?Sized),
        dest: &Value,
        src: &Value,
        size: &Value,
    ) -> Result<()> {
        let dest_idx = match dest {
            Value::I32(d) => {
                if *d < 0 {
                    return Err(Error::runtime_error(
                        "Table destination index cannot be negative",
                    ));
                }
                *d as u32
            },
            _ => return Err(Error::type_error("table.copy dest must be i32")),
        };

        let src_idx = match src {
            Value::I32(s) => {
                if *s < 0 {
                    return Err(Error::runtime_error(
                        "Table source index cannot be negative",
                    ));
                }
                *s as u32
            },
            _ => return Err(Error::type_error("table.copy src must be i32")),
        };

        let copy_size = match size {
            Value::I32(s) => {
                if *s < 0 {
                    return Err(Error::runtime_error("Table copy size cannot be negative"));
                }
                *s as u32
            },
            _ => return Err(Error::type_error("table.copy size must be i32")),
        };

        table.copy_table(
            self.dest_table_index,
            dest_idx,
            self.src_table_index,
            src_idx,
            copy_size,
        )
    }
}

/// Table init operation (table.init)
#[derive(Debug, Clone, PartialEq)]
pub struct TableInit {
    /// Table index to initialize
    pub table_index: u32,
    /// Element segment index to use
    pub elem_index:  u32,
}

impl TableInit {
    /// Create a new table.init operation
    #[must_use]
    pub fn new(table_index: u32, elem_index: u32) -> Self {
        Self {
            table_index,
            elem_index,
        }
    }

    /// Execute table.init operation
    ///
    /// # Arguments
    ///
    /// * `table` - The table to initialize
    /// * `elem_segments` - Access to element segments
    /// * `dest` - Destination index in table (i32)
    /// * `src` - Source index in element segment (i32)
    /// * `size` - Number of elements to copy (i32)
    ///
    /// # Returns
    ///
    /// Success or an error
    ///
    /// # Errors
    ///
    /// Returns an error if table init operation fails
    pub fn execute(
        &self,
        table: &mut (impl TableOperations + ?Sized),
        elem_segments: &(impl ElementSegmentOperations + ?Sized),
        dest: &Value,
        src: &Value,
        size: &Value,
    ) -> Result<()> {
        let dest_idx = match dest {
            Value::I32(d) => {
                if *d < 0 {
                    return Err(Error::runtime_error(
                        "Table destination index cannot be negative",
                    ));
                }
                *d as u32
            },
            _ => return Err(Error::type_error("table.init dest must be i32")),
        };

        let src_idx = match src {
            Value::I32(s) => {
                if *s < 0 {
                    return Err(Error::runtime_error(
                        "Element segment source index cannot be negative",
                    ));
                }
                *s as u32
            },
            _ => return Err(Error::type_error("table.init src must be i32")),
        };

        let copy_size = match size {
            Value::I32(s) => {
                if *s < 0 {
                    return Err(Error::runtime_error("Table init size cannot be negative"));
                }
                *s as u32
            },
            _ => return Err(Error::type_error("table.init size must be i32")),
        };

        // Get element segment
        let elements = elem_segments
            .get_element_segment(self.elem_index)?
            .ok_or_else(|| Error::runtime_error("Element segment has been dropped"))?;

        // Check bounds in element segment
        let elements_len = elements.len() as u32;
        let src_end = src_idx
            .checked_add(copy_size)
            .ok_or_else(|| Error::runtime_error("table.init src index overflow"))?;

        if src_end > elements_len {
            return Err(Error::runtime_error("table.init src out of bounds"));
        }

        // Check table bounds
        let table_size = table.get_table_size(self.table_index)?;
        let dest_end = dest_idx
            .checked_add(copy_size)
            .ok_or_else(|| Error::runtime_error("table.init dest index overflow"))?;

        if dest_end > table_size {
            return Err(Error::runtime_error("table.init dest out of bounds"));
        }

        // Copy elements from segment to table
        #[cfg(feature = "std")]
        {
            for i in 0..copy_size {
                let elem_value = &elements[(src_idx + i) as usize];
                table.set_table_element(self.table_index, dest_idx + i, elem_value.clone())?;
            }
        }
        #[cfg(not(feature = "std"))]
        {
            for i in 0..copy_size {
                let elem_value = elements
                    .get((src_idx + i) as usize)
                    .map_err(|_| Error::runtime_error("Element segment index out of bounds"))?;
                table.set_table_element(self.table_index, dest_idx + i, elem_value.clone())?;
            }
        }

        Ok(())
    }
}

/// Element drop operation (elem.drop)
#[derive(Debug, Clone, PartialEq)]
pub struct ElemDrop {
    /// Element segment index to drop
    pub elem_index: u32,
}

impl ElemDrop {
    /// Create a new elem.drop operation
    #[must_use]
    pub fn new(elem_index: u32) -> Self {
        Self { elem_index }
    }

    /// Execute elem.drop operation
    ///
    /// # Arguments
    ///
    /// * `elem_segments` - Access to element segments
    ///
    /// # Returns
    ///
    /// Success or an error
    ///
    /// # Errors
    ///
    /// Returns an error if element drop operation fails
    pub fn execute(
        &self,
        elem_segments: &mut (impl ElementSegmentOperations + ?Sized),
    ) -> Result<()> {
        elem_segments.drop_element_segment(self.elem_index)
    }
}

/// Unified table operation enum combining all table instructions
#[derive(Debug, Clone, PartialEq)]
pub enum TableOp {
    /// Get operation (table.get)
    Get(TableGet),
    /// Set operation (table.set)
    Set(TableSet),
    /// Size operation (table.size)
    Size(TableSize),
    /// Grow operation (table.grow)
    Grow(TableGrow),
    /// Fill operation (table.fill)
    Fill(TableFill),
    /// Copy operation (table.copy)
    Copy(TableCopy),
    /// Init operation (table.init)
    Init(TableInit),
    /// Element drop operation (elem.drop)
    ElemDrop(ElemDrop),
}

/// Execution context for unified table operations
pub trait TableContext {
    /// Pop a value from the stack
    ///
    /// # Errors
    ///
    /// Returns an error if stack is empty
    fn pop_value(&mut self) -> Result<Value>;

    /// Push a value to the stack
    ///
    /// # Errors
    ///
    /// Returns an error if stack is full
    fn push_value(&mut self, value: Value) -> Result<()>;

    /// Get table operations interface
    ///
    /// # Errors
    ///
    /// Returns an error if table operations are unavailable
    fn get_tables(&mut self) -> Result<&mut dyn TableOperations>;

    /// Get element segment operations interface
    ///
    /// # Errors
    ///
    /// Returns an error if element operations are unavailable
    fn get_element_segments(&mut self) -> Result<&mut dyn ElementSegmentOperations>;

    /// Execute table.init operation (helper to avoid borrowing issues)
    ///
    /// # Errors
    ///
    /// Returns an error if table init operation fails
    fn execute_table_init(
        &mut self,
        table_index: u32,
        elem_index: u32,
        dest: i32,
        src: i32,
        size: i32,
    ) -> Result<()>;
}

impl TableOp {
    /// Helper to extract 3 i32 arguments from stack
    fn pop_three_i32s(ctx: &mut impl TableContext) -> Result<(i32, i32, i32)> {
        let arg3 = ctx
            .pop_value()?
            .into_i32()
            .map_err(|_| Error::type_error("Expected i32 for table operation"))?;
        let arg2 = ctx
            .pop_value()?
            .into_i32()
            .map_err(|_| Error::type_error("Expected i32 for table operation"))?;
        let arg1 = ctx
            .pop_value()?
            .into_i32()
            .map_err(|_| Error::type_error("Expected i32 for table operation"))?;
        Ok((arg1, arg2, arg3))
    }
}

impl<T: TableContext> PureInstruction<T, Error> for TableOp {
    fn execute(&self, context: &mut T) -> Result<()> {
        match self {
            Self::Get(get) => {
                let index = context.pop_value()?;
                let tables = context.get_tables()?;
                let result = get.execute(tables, &index)?;
                context.push_value(result)
            },
            Self::Set(set) => {
                let value = context.pop_value()?;
                let index = context.pop_value()?;
                let tables = context.get_tables()?;
                set.execute(tables, &index, &value)
            },
            Self::Size(size) => {
                let tables = context.get_tables()?;
                let result = size.execute(tables)?;
                context.push_value(result)
            },
            Self::Grow(grow) => {
                let delta = context.pop_value()?;
                let init_value = context.pop_value()?;
                let tables = context.get_tables()?;
                let result = grow.execute(tables, &init_value, &delta)?;
                context.push_value(result)
            },
            Self::Fill(fill) => {
                let (dest, value, size) = Self::pop_three_i32s(context)?;
                let tables = context.get_tables()?;
                fill.execute(
                    tables,
                    &Value::I32(dest),
                    &Value::I32(value), // This should be a reference, will be validated in execute
                    &Value::I32(size),
                )
            },
            Self::Copy(copy) => {
                let (dest, src, size) = Self::pop_three_i32s(context)?;
                let tables = context.get_tables()?;
                copy.execute(
                    tables,
                    &Value::I32(dest),
                    &Value::I32(src),
                    &Value::I32(size),
                )
            },
            Self::Init(init) => {
                let (dest, src, size) = Self::pop_three_i32s(context)?;
                context.execute_table_init(init.table_index, init.elem_index, dest, src, size)
            },
            Self::ElemDrop(drop) => {
                let elem_segments = context.get_element_segments()?;
                drop.execute(elem_segments)
            },
        }
    }
}

// Validation implementations

impl Validate for TableGet {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // table.get: [i32] -> [ref]
        if !ctx.is_unreachable() {
            ctx.pop_expect(ValueType::I32)?; // index
                                             // Push appropriate reference type based on table type
                                             // For simplicity, assume funcref for now
            ctx.push_type(ValueType::FuncRef)?;
        }
        Ok(())
    }
}

impl Validate for TableSet {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // table.set: [i32 ref] -> []
        if !ctx.is_unreachable() {
            ctx.pop_type()?; // reference value (type depends on table)
            ctx.pop_expect(ValueType::I32)?; // index
        }
        Ok(())
    }
}

impl Validate for TableSize {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // table.size: [] -> [i32]
        if !ctx.is_unreachable() {
            ctx.push_type(ValueType::I32)?;
        }
        Ok(())
    }
}

impl Validate for TableGrow {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // table.grow: [ref i32] -> [i32]
        if !ctx.is_unreachable() {
            ctx.pop_expect(ValueType::I32)?; // delta
            ctx.pop_type()?; // init value (reference type)
            ctx.push_type(ValueType::I32)?; // previous size or -1
        }
        Ok(())
    }
}

impl Validate for TableFill {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // table.fill: [i32 ref i32] -> []
        if !ctx.is_unreachable() {
            ctx.pop_expect(ValueType::I32)?; // size
            ctx.pop_type()?; // value (reference type)
            ctx.pop_expect(ValueType::I32)?; // dest
        }
        Ok(())
    }
}

impl Validate for TableCopy {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // table.copy: [i32 i32 i32] -> []
        if !ctx.is_unreachable() {
            ctx.pop_expect(ValueType::I32)?; // size
            ctx.pop_expect(ValueType::I32)?; // src
            ctx.pop_expect(ValueType::I32)?; // dest
        }
        Ok(())
    }
}

impl Validate for TableInit {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // table.init: [i32 i32 i32] -> []
        if !ctx.is_unreachable() {
            ctx.pop_expect(ValueType::I32)?; // size
            ctx.pop_expect(ValueType::I32)?; // src
            ctx.pop_expect(ValueType::I32)?; // dest
        }
        Ok(())
    }
}

impl Validate for ElemDrop {
    fn validate(&self, _ctx: &mut ValidationContext) -> Result<()> {
        // elem.drop: [] -> []
        // No stack operations required
        Ok(())
    }
}

impl Validate for TableOp {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        match self {
            Self::Get(op) => op.validate(ctx),
            Self::Set(op) => op.validate(ctx),
            Self::Size(op) => op.validate(ctx),
            Self::Grow(op) => op.validate(ctx),
            Self::Fill(op) => op.validate(ctx),
            Self::Copy(op) => op.validate(ctx),
            Self::Init(op) => op.validate(ctx),
            Self::ElemDrop(op) => op.validate(ctx),
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    // Import Vec based on feature flags
    use std::{
        vec,
        vec::Vec,
    };

    use wrt_foundation::values::{
        ExternRef,
        FuncRef,
    };

    use super::*;

    /// Mock table implementation for testing
    struct MockTable {
        elements: Vec<Value>,
        max_size: Option<u32>,
    }

    impl MockTable {
        fn new(initial_size: u32, max_size: Option<u32>) -> Self {
            let mut elements = Vec::with_capacity(initial_size as usize);
            for _ in 0..initial_size {
                elements.push(Value::FuncRef(None)); // Initialize with null
                                                     // references
            }
            Self { elements, max_size }
        }
    }

    /// Mock table operations implementation
    struct MockTableOperations {
        tables: Vec<MockTable>,
    }

    impl MockTableOperations {
        fn new() -> Self {
            let mut tables = Vec::new();
            tables.push(MockTable::new(10, Some(20))); // Table 0: size 10, max 20
            tables.push(MockTable::new(5, None)); // Table 1: size 5, no max
            Self { tables }
        }
    }

    impl TableOperations for MockTableOperations {
        fn get_table_element(&self, table_index: u32, elem_index: u32) -> Result<Value> {
            let table = self
                .tables
                .get(table_index as usize)
                .ok_or_else(|| Error::runtime_error("Invalid table index"))?;

            let element = table
                .elements
                .get(elem_index as usize)
                .ok_or_else(|| Error::runtime_error("Table access out of bounds"))?;

            Ok(element.clone())
        }

        fn set_table_element(
            &mut self,
            table_index: u32,
            elem_index: u32,
            value: Value,
        ) -> Result<()> {
            let table = self
                .tables
                .get_mut(table_index as usize)
                .ok_or_else(|| Error::runtime_error("Invalid table index"))?;

            let element = table
                .elements
                .get_mut(elem_index as usize)
                .ok_or_else(|| Error::runtime_error("Table access out of bounds"))?;

            *element = value;
            Ok(())
        }

        fn get_table_size(&self, table_index: u32) -> Result<u32> {
            let table = self
                .tables
                .get(table_index as usize)
                .ok_or_else(|| Error::runtime_error("Invalid table index"))?;

            Ok(table.elements.len() as u32)
        }

        fn grow_table(&mut self, table_index: u32, delta: u32, init_value: Value) -> Result<i32> {
            let table = self
                .tables
                .get_mut(table_index as usize)
                .ok_or_else(|| Error::runtime_error("Invalid table index"))?;

            let old_size = table.elements.len() as i32;
            let new_size = old_size as u32 + delta;

            // Check max size limit
            if let Some(max) = table.max_size {
                if new_size > max {
                    return Ok(-1); // Growth failed
                }
            }

            // Grow the table
            for _ in 0..delta {
                table.elements.push(init_value.clone());
            }

            Ok(old_size)
        }

        fn fill_table(&mut self, table_index: u32, dst: u32, val: Value, len: u32) -> Result<()> {
            let table = self
                .tables
                .get_mut(table_index as usize)
                .ok_or_else(|| Error::runtime_error("Invalid table index"))?;

            let end_idx = dst as usize + len as usize;
            if end_idx > table.elements.len() {
                return Err(Error::runtime_error("Table fill out of bounds"));
            }

            for i in 0..len {
                table.elements[dst as usize + i as usize] = val.clone();
            }

            Ok(())
        }

        fn copy_table(
            &mut self,
            dst_table: u32,
            dst_index: u32,
            src_table: u32,
            src_index: u32,
            len: u32,
        ) -> Result<()> {
            // For simplicity, handle same-table copy only in this test
            if dst_table != src_table {
                return Err(Error::runtime_error(
                    "Cross-table copy not implemented in test",
                ));
            }

            let table = self
                .tables
                .get_mut(dst_table as usize)
                .ok_or_else(|| Error::runtime_error("Invalid table index"))?;

            let src_end = src_index as usize + len as usize;
            let dst_end = dst_index as usize + len as usize;

            if src_end > table.elements.len() || dst_end > table.elements.len() {
                return Err(Error::runtime_error("Table copy out of bounds"));
            }

            // Copy elements (handle overlapping regions correctly)
            if len > 0 {
                let temp: Vec<Value> = table.elements[src_index as usize..src_end].to_vec();
                for (i, value) in temp.into_iter().enumerate() {
                    table.elements[dst_index as usize + i] = value;
                }
            }

            Ok(())
        }
    }

    /// Mock element segment operations
    struct MockElementSegments {
        segments: Vec<Option<Vec<Value>>>,
    }

    impl MockElementSegments {
        fn new() -> Self {
            let mut segments = Vec::new();

            // Segment 0: [FuncRef(1), FuncRef(2), FuncRef(3)]
            let mut seg0 = Vec::new();
            seg0.push(Value::FuncRef(Some(FuncRef::from_index(1))));
            seg0.push(Value::FuncRef(Some(FuncRef::from_index(2))));
            seg0.push(Value::FuncRef(Some(FuncRef::from_index(3))));
            segments.push(Some(seg0));

            // Segment 1: [ExternRef(4), ExternRef(5)]
            let mut seg1 = Vec::new();
            seg1.push(Value::ExternRef(Some(ExternRef { index: 4 })));
            seg1.push(Value::ExternRef(Some(ExternRef { index: 5 })));
            segments.push(Some(seg1));

            Self { segments }
        }
    }

    impl ElementSegmentOperations for MockElementSegments {
        #[cfg(feature = "std")]
        fn get_element_segment(&self, elem_index: u32) -> Result<Option<Vec<Value>>> {
            if let Some(seg) = self.segments.get(elem_index as usize) {
                Ok(seg.clone())
            } else {
                Err(Error::runtime_error("Invalid element segment index"))
            }
        }

        #[cfg(not(feature = "std"))]
        fn get_element_segment(
            &self,
            elem_index: u32,
        ) -> Result<
            Option<wrt_foundation::BoundedVec<Value, 65536, wrt_foundation::NoStdProvider<65536>>>,
        > {
            if let Some(Some(seg)) = self.segments.get(elem_index as usize) {
                let mut bounded = wrt_foundation::BoundedVec::new();
                for value in seg {
                    bounded
                        .push(value.clone())
                        .map_err(|_| Error::runtime_error("BoundedVec capacity exceeded"))?;
                }
                Ok(Some(bounded))
            } else if self.segments.get(elem_index as usize).is_some() {
                Ok(None) // Dropped segment
            } else {
                Err(Error::runtime_error("Invalid element segment index"))
            }
        }

        fn drop_element_segment(&mut self, elem_index: u32) -> Result<()> {
            if let Some(seg) = self.segments.get_mut(elem_index as usize) {
                *seg = None; // Drop the segment
                Ok(())
            } else {
                Err(Error::runtime_error("Invalid element segment index"))
            }
        }
    }

    /// Mock table context for testing unified operations
    struct MockTableContext {
        stack:    Vec<Value>,
        tables:   MockTableOperations,
        elements: MockElementSegments,
    }

    impl MockTableContext {
        fn new() -> Self {
            Self {
                stack:    Vec::new(),
                tables:   MockTableOperations::new(),
                elements: MockElementSegments::new(),
            }
        }
    }

    impl TableContext for MockTableContext {
        fn pop_value(&mut self) -> Result<Value> {
            self.stack.pop().ok_or_else(|| Error::runtime_error("Stack underflow"))
        }

        fn push_value(&mut self, value: Value) -> Result<()> {
            self.stack.push(value);
            Ok(())
        }

        fn get_tables(&mut self) -> Result<&mut dyn TableOperations> {
            Ok(&mut self.tables)
        }

        fn get_element_segments(&mut self) -> Result<&mut dyn ElementSegmentOperations> {
            Ok(&mut self.elements)
        }

        fn execute_table_init(
            &mut self,
            table_index: u32,
            elem_index: u32,
            dest: i32,
            src: i32,
            size: i32,
        ) -> Result<()> {
            let init_op = TableInit::new(table_index, elem_index);
            init_op.execute(
                &mut self.tables,
                &self.elements,
                &Value::I32(dest),
                &Value::I32(src),
                &Value::I32(size),
            )
        }
    }

    #[test]
    fn test_table_get_set() {
        let mut tables = MockTableOperations::new();

        // Test set operation
        let set_op = TableSet::new(0);
        let func_ref = Value::FuncRef(Some(FuncRef::from_index(42)));
        set_op.execute(&mut tables, &Value::I32(5), &func_ref).unwrap();

        // Test get operation
        let get_op = TableGet::new(0);
        let result = get_op.execute(&tables, &Value::I32(5)).unwrap();
        assert_eq!(result, func_ref);
    }

    #[test]
    fn test_table_size_grow() {
        let mut tables = MockTableOperations::new();

        // Test size operation
        let size_op = TableSize::new(0);
        let size = size_op.execute(&tables).unwrap();
        assert_eq!(size, Value::I32(10));

        // Test grow operation
        let grow_op = TableGrow::new(0);
        let prev_size =
            grow_op.execute(&mut tables, &Value::FuncRef(None), &Value::I32(3)).unwrap();
        assert_eq!(prev_size, Value::I32(10));

        // Check new size
        let new_size = size_op.execute(&tables).unwrap();
        assert_eq!(new_size, Value::I32(13));
    }

    #[test]
    fn test_table_fill() {
        let mut tables = MockTableOperations::new();

        let fill_op = TableFill::new(0);
        let func_ref = Value::FuncRef(Some(FuncRef::from_index(99)));

        // Fill table[0][2..5] with FuncRef(99)
        fill_op
            .execute(
                &mut tables,
                &Value::I32(2), // dest
                &func_ref,      // value
                &Value::I32(3), // size
            )
            .unwrap();

        // Verify fill worked
        let get_op = TableGet::new(0);
        for i in 2..5 {
            let result = get_op.execute(&tables, &Value::I32(i)).unwrap();
            assert_eq!(result, func_ref);
        }
    }

    #[test]
    fn test_table_copy() {
        let mut tables = MockTableOperations::new();

        // Set up source values
        let set_op = TableSet::new(0);
        set_op
            .execute(
                &mut tables,
                &Value::I32(1),
                &Value::FuncRef(Some(FuncRef::from_index(101))),
            )
            .unwrap();
        set_op
            .execute(
                &mut tables,
                &Value::I32(2),
                &Value::FuncRef(Some(FuncRef::from_index(102))),
            )
            .unwrap();
        set_op
            .execute(
                &mut tables,
                &Value::I32(3),
                &Value::FuncRef(Some(FuncRef::from_index(103))),
            )
            .unwrap();

        // Copy table[0][1..4] to table[0][6..9]
        let copy_op = TableCopy::new(0, 0);
        copy_op
            .execute(
                &mut tables,
                &Value::I32(6), // dest
                &Value::I32(1), // src
                &Value::I32(3), // size
            )
            .unwrap();

        // Verify copy worked
        let get_op = TableGet::new(0);
        let expected = [
            Value::FuncRef(Some(FuncRef::from_index(101))),
            Value::FuncRef(Some(FuncRef::from_index(102))),
            Value::FuncRef(Some(FuncRef::from_index(103))),
        ];

        for (i, expected_val) in expected.iter().enumerate() {
            let result = get_op.execute(&tables, &Value::I32(6 + i as i32)).unwrap();
            assert_eq!(result, *expected_val);
        }
    }

    #[test]
    fn test_table_init_elem_drop() {
        let mut tables = MockTableOperations::new();
        let mut elements = MockElementSegments::new();

        // Initialize table[0][4..6] from element segment 0[1..3]
        let init_op = TableInit::new(0, 0);
        init_op
            .execute(
                &mut tables,
                &elements,
                &Value::I32(4), // dest
                &Value::I32(1), // src
                &Value::I32(2), // size
            )
            .unwrap();

        // Verify initialization (should copy FuncRef(2) and FuncRef(3))
        let get_op = TableGet::new(0);
        let result1 = get_op.execute(&tables, &Value::I32(4)).unwrap();
        assert_eq!(result1, Value::FuncRef(Some(FuncRef::from_index(2))));

        let result2 = get_op.execute(&tables, &Value::I32(5)).unwrap();
        assert_eq!(result2, Value::FuncRef(Some(FuncRef::from_index(3))));

        // Drop element segment
        let drop_op = ElemDrop::new(0);
        drop_op.execute(&mut elements).unwrap();

        // Try to init from dropped segment - should fail
        let result = init_op.execute(
            &mut tables,
            &elements,
            &Value::I32(7),
            &Value::I32(0),
            &Value::I32(1),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_unified_table_operations() {
        let mut ctx = MockTableContext::new();

        // Test unified table.size
        let size_op = TableOp::Size(TableSize::new(0));
        size_op.execute(&mut ctx).unwrap();
        assert_eq!(ctx.pop_value().unwrap(), Value::I32(10));

        // Test unified table.set
        ctx.push_value(Value::I32(3)).unwrap(); // index
        ctx.push_value(Value::FuncRef(Some(FuncRef::from_index(77)))).unwrap(); // value
        let set_op = TableOp::Set(TableSet::new(0));
        set_op.execute(&mut ctx).unwrap();

        // Test unified table.get
        ctx.push_value(Value::I32(3)).unwrap(); // index
        let get_op = TableOp::Get(TableGet::new(0));
        get_op.execute(&mut ctx).unwrap();
        assert_eq!(
            ctx.pop_value().unwrap(),
            Value::FuncRef(Some(FuncRef::from_index(77)))
        );
    }

    #[test]
    fn test_error_handling() {
        let mut tables = MockTableOperations::new();

        // Test negative index
        let get_op = TableGet::new(0);
        let result = get_op.execute(&tables, &Value::I32(-1));
        assert!(result.is_err());

        // Test out of bounds
        let result = get_op.execute(&tables, &Value::I32(100));
        assert!(result.is_err());

        // Test invalid table index
        let invalid_get_op = TableGet::new(99);
        let result = invalid_get_op.execute(&tables, &Value::I32(0));
        assert!(result.is_err());

        // Test grow beyond max size
        let grow_op = TableGrow::new(0);
        let result = grow_op
            .execute(
                &mut tables,
                &Value::FuncRef(None),
                &Value::I32(50), // Would exceed max size of 20
            )
            .unwrap();
        assert_eq!(result, Value::I32(-1)); // Growth failed
    }
}
