//! WebAssembly branch hinting operations implementation.
//!
//! This module implements WebAssembly branch hinting instructions including:
//! - `br_on_null`: Branch if reference is null
//! - `br_on_non_null`: Branch if reference is not null
//!
//! These operations support the WebAssembly branch hinting proposal
//! and work across std, `no_std+alloc`, and pure `no_std` environments.

use wrt_error::{
    Error,
    Result,
};
use wrt_foundation::{
    types::{
        LabelIdx,
        ValueType,
    },
    values::Value,
};

use crate::{
    control_ops::ControlContext,
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

/// Branch on null operation - branches if reference is null
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrOnNull {
    /// Label to branch to if reference is null
    pub label: LabelIdx,
}

impl BrOnNull {
    /// Create a new `br_on_null` instruction
    #[must_use]
    pub fn new(label: LabelIdx) -> Self {
        Self { label }
    }

    /// Execute the `br_on_null` instruction
    /// Returns Ok(true) if branch taken, Ok(false) if not taken
    ///
    /// # Errors
    ///
    /// Returns an error if reference is not a reference type
    pub fn execute(&self, reference: &Value) -> Result<bool> {
        match reference {
            Value::FuncRef(None) | Value::ExternRef(None) => {
                // Branch is taken - reference is null
                Ok(true)
            },
            Value::FuncRef(Some(_)) | Value::ExternRef(Some(_)) => {
                // Branch not taken - reference is non-null
                Ok(false)
            },
            _ => Err(Error::type_error("br_on_null requires a reference type")),
        }
    }

    /// Get the target label for branching
    #[must_use]
    pub fn target_label(&self) -> LabelIdx {
        self.label
    }
}

/// Branch on non-null operation - branches if reference is not null
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrOnNonNull {
    /// Label to branch to if reference is not null
    pub label: LabelIdx,
}

impl BrOnNonNull {
    /// Create a new `br_on_non_null` instruction
    #[must_use]
    pub fn new(label: LabelIdx) -> Self {
        Self { label }
    }

    /// Execute the `br_on_non_null` instruction
    /// Returns Ok(true) if branch taken, Ok(false) if not taken
    /// Also returns the reference value for stack manipulation
    ///
    /// # Errors
    ///
    /// Returns an error if reference is not a reference type
    pub fn execute(&self, reference: &Value) -> Result<(bool, Option<Value>)> {
        match reference {
            Value::FuncRef(None) | Value::ExternRef(None) => {
                // Branch not taken - reference is null
                Ok((false, None))
            },
            Value::FuncRef(Some(_)) | Value::ExternRef(Some(_)) => {
                // Branch is taken - reference is non-null
                // The reference remains on the stack after branching
                Ok((true, Some(reference.clone())))
            },
            _ => Err(Error::type_error(
                "br_on_non_null requires a reference type",
            )),
        }
    }

    /// Get the target label for branching
    #[must_use]
    pub fn target_label(&self) -> LabelIdx {
        self.label
    }
}

/// Branch hinting operation enum for unified handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchHintOp {
    /// `br_on_null` operation
    BrOnNull(BrOnNull),
    /// `br_on_non_null` operation  
    BrOnNonNull(BrOnNonNull),
}

impl BranchHintOp {
    /// Execute the branch hinting operation
    /// Returns (`branch_taken`, `label_to_branch_to`, `value_to_keep_on_stack`)
    ///
    /// # Errors
    ///
    /// Returns an error if branch operation fails
    pub fn execute(&self, operand: &Value) -> Result<(bool, Option<LabelIdx>, Option<Value>)> {
        match self {
            BranchHintOp::BrOnNull(op) => {
                let branch_taken = op.execute(operand)?;
                if branch_taken {
                    Ok((true, Some(op.target_label()), None))
                } else {
                    // If branch not taken, reference stays on stack
                    Ok((false, None, Some(operand.clone())))
                }
            },
            BranchHintOp::BrOnNonNull(op) => {
                let (branch_taken, ref_value) = op.execute(operand)?;
                if branch_taken {
                    Ok((true, Some(op.target_label()), ref_value))
                } else {
                    Ok((false, None, None))
                }
            },
        }
    }
}

/// Trait for contexts that support branch hinting operations
pub trait BranchHintingContext: ControlContext {
    /// Execute a branch on null operation
    ///
    /// # Errors
    ///
    /// Returns an error if branch execution fails
    fn execute_br_on_null(&mut self, label: LabelIdx) -> Result<()>;

    /// Execute a branch on non-null operation
    ///
    /// # Errors
    ///
    /// Returns an error if branch execution fails
    fn execute_br_on_non_null(&mut self, label: LabelIdx) -> Result<()>;
}

// Validation implementations

impl Validate for BrOnNull {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // br_on_null: [ref] -> [ref] (if not taken) or [] (if taken)
        if !ctx.is_unreachable() {
            // Check that we have a reference type on the stack
            let ref_type = ctx.pop_type()?;
            match ref_type {
                ValueType::FuncRef | ValueType::ExternRef => {
                    // Validate the branch target
                    ctx.validate_branch_target(self.label)?;

                    // If branch not taken, reference stays on stack
                    ctx.push_type(ref_type)?;
                },
                _ => return Err(Error::type_error("br_on_null expects reference type")),
            }
        }
        Ok(())
    }
}

impl Validate for BrOnNonNull {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        // br_on_non_null: [ref] -> [] (if not taken) or [ref] (if taken and branched)
        if !ctx.is_unreachable() {
            // Check that we have a reference type on the stack
            let ref_type = ctx.pop_type()?;
            match ref_type {
                ValueType::FuncRef | ValueType::ExternRef => {
                    // Validate the branch target
                    ctx.validate_branch_target(self.label)?;

                    // Note: The typing is complex here because:
                    // - If branch is taken, the reference is on the stack at
                    //   the branch target
                    // - If branch is not taken, the reference is consumed
                    // For now, we don't push the type back as the actual
                    // behavior depends on runtime execution
                },
                _ => return Err(Error::type_error("br_on_non_null expects reference type")),
            }
        }
        Ok(())
    }
}

impl Validate for BranchHintOp {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<()> {
        match self {
            BranchHintOp::BrOnNull(op) => op.validate(ctx),
            BranchHintOp::BrOnNonNull(op) => op.validate(ctx),
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use wrt_foundation::values::{
        ExternRef,
        FuncRef,
    };

    use super::*;

    #[test]
    fn test_br_on_null_with_null_funcref() {
        let op = BrOnNull::new(0);
        let result = op.execute(&Value::FuncRef(None)).unwrap();
        assert!(result); // Branch should be taken
    }

    #[test]
    fn test_br_on_null_with_non_null_funcref() {
        let op = BrOnNull::new(0);
        let result = op.execute(&Value::FuncRef(Some(FuncRef { index: 42 }))).unwrap();
        assert!(!result); // Branch should not be taken
    }

    #[test]
    fn test_br_on_null_with_null_externref() {
        let op = BrOnNull::new(1);
        let result = op.execute(&Value::ExternRef(None)).unwrap();
        assert!(result); // Branch should be taken
    }

    #[test]
    fn test_br_on_null_with_non_null_externref() {
        let op = BrOnNull::new(1);
        let result = op.execute(&Value::ExternRef(Some(ExternRef { index: 123 }))).unwrap();
        assert!(!result); // Branch should not be taken
    }

    #[test]
    fn test_br_on_null_with_non_reference() {
        let op = BrOnNull::new(0);
        let result = op.execute(&Value::I32(42));
        assert!(result.is_err());
    }

    #[test]
    fn test_br_on_non_null_with_null_funcref() {
        let op = BrOnNonNull::new(0);
        let (branch_taken, value) = op.execute(&Value::FuncRef(None)).unwrap();
        assert!(!branch_taken); // Branch should not be taken
        assert!(value.is_none()); // No value kept on stack
    }

    #[test]
    fn test_br_on_non_null_with_non_null_funcref() {
        let op = BrOnNonNull::new(0);
        let ref_value = Value::FuncRef(Some(FuncRef { index: 42 }));
        let (branch_taken, value) = op.execute(&ref_value).unwrap();
        assert!(branch_taken); // Branch should be taken
        assert_eq!(value, Some(ref_value.clone())); // Reference stays on stack
    }

    #[test]
    fn test_br_on_non_null_with_null_externref() {
        let op = BrOnNonNull::new(1);
        let (branch_taken, value) = op.execute(&Value::ExternRef(None)).unwrap();
        assert!(!branch_taken); // Branch should not be taken
        assert!(value.is_none());
    }

    #[test]
    fn test_br_on_non_null_with_non_null_externref() {
        let op = BrOnNonNull::new(1);
        let ref_value = Value::ExternRef(Some(ExternRef { index: 123 }));
        let (branch_taken, value) = op.execute(&ref_value).unwrap();
        assert!(branch_taken); // Branch should be taken
        assert_eq!(value, Some(ref_value.clone())); // Reference stays on stack
    }

    #[test]
    fn test_br_on_non_null_with_non_reference() {
        let op = BrOnNonNull::new(0);
        let result = op.execute(&Value::I32(42));
        assert!(result.is_err());
    }

    #[test]
    fn test_branch_hint_op_enum() {
        // Test BrOnNull
        let br_on_null = BranchHintOp::BrOnNull(BrOnNull::new(2));
        let (taken, label, value) = br_on_null.execute(&Value::FuncRef(None)).unwrap();
        assert!(taken);
        assert_eq!(label, Some(2));
        assert!(value.is_none());

        // Test BrOnNonNull with non-null ref
        let br_on_non_null = BranchHintOp::BrOnNonNull(BrOnNonNull::new(3));
        let ref_value = Value::FuncRef(Some(FuncRef { index: 10 }));
        let (taken, label, value) = br_on_non_null.execute(&ref_value).unwrap();
        assert!(taken);
        assert_eq!(label, Some(3));
        assert_eq!(value, Some(ref_value));
    }

    #[test]
    fn test_target_label() {
        let op1 = BrOnNull::new(5);
        assert_eq!(op1.target_label(), 5);

        let op2 = BrOnNonNull::new(10);
        assert_eq!(op2.target_label(), 10);
    }
}
