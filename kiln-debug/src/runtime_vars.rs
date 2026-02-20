#![cfg(feature = "runtime-variables")]

use wrt_error::Error;
use wrt_foundation::{
    bounded::{BoundedVec, MAX_DWARF_FILE_TABLE},
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

use crate::bounded_debug_infra::{DEBUG_PROVIDER_SIZE, DebugProvider};
/// Runtime variable inspection implementation
/// Provides the ability to read variable values from runtime state
use crate::{
    parameter::{BasicType, Parameter},
    runtime_api::{DebugMemory, DwarfLocation, LiveVariable, RuntimeState, VariableValue},
    strings::DebugString,
};

/// Variable scope information
#[derive(Debug, Clone)]
pub struct VariableScope {
    /// PC range start (inclusive)
    pub start_pc: u32,
    /// PC range end (exclusive)
    pub end_pc: u32,
    /// Lexical scope depth
    pub depth: u16,
}

/// Variable definition from DWARF
#[derive(Debug)]
pub struct VariableDefinition<'a> {
    /// Variable name
    pub name: Option<DebugString<'a>>,
    /// Variable type
    pub var_type: BasicType,
    /// DWARF location description
    pub location: DwarfLocation,
    /// Scope information
    pub scope: VariableScope,
    /// Source file
    pub file_index: u16,
    /// Source line
    pub line: u32,
}

/// Runtime variable inspector
pub struct VariableInspector<'a> {
    /// Variable definitions from DWARF
    variables: BoundedVec<
        VariableDefinition<'a>,
        MAX_DWARF_FILE_TABLE,
        crate::bounded_debug_infra::DebugProvider,
    >,
}

impl<'a> VariableInspector<'a> {
    /// Create a new variable inspector
    pub fn new() -> Result<Self, Error> {
        let provider = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
        Ok(Self {
            variables: BoundedVec::new(provider)
                .map_err(|_| Error::memory_error("Failed to create variables vector"))?,
        })
    }

    /// Add a variable definition from DWARF
    pub fn add_variable(&mut self, var: VariableDefinition<'a>) -> Result<(), ()> {
        self.variables.push(var).map_err(|_| ())
    }

    /// Find all variables in scope at the given PC
    pub fn find_variables_at_pc(&self, pc: u32) -> impl Iterator<Item = &VariableDefinition<'a>> {
        self.variables
            .iter()
            .filter(move |var| pc >= var.scope.start_pc && pc < var.scope.end_pc)
    }

    /// Read a variable's value from runtime state
    pub fn read_variable(
        &self,
        var: &VariableDefinition<'a>,
        state: &dyn RuntimeState,
        memory: &dyn DebugMemory,
    ) -> Option<VariableValue> {
        match &var.location {
            DwarfLocation::Register(index) => {
                // Read from local variable slot
                state.read_local(*index).map(|value| {
                    let mut bytes = [0u8; 8];
                    bytes[0..8].copy_from_slice(&value.to_le_bytes);
                    VariableValue {
                        bytes,
                        size: size_for_type(&var.var_type),
                        var_type: var.var_type.clone(),
                        address: None,
                    }
                })
            },
            DwarfLocation::Memory(addr) => {
                // Read from memory address
                let size = size_for_type(&var.var_type) as usize;
                memory.read_bytes(*addr, size).map(|data| {
                    let mut bytes = [0u8; 8];
                    let copy_size = size.min(8);
                    bytes[0..copy_size].copy_from_slice(&data[0..copy_size]);
                    VariableValue {
                        bytes,
                        size: size as u8,
                        var_type: var.var_type.clone(),
                        address: Some(*addr),
                    }
                })
            },
            DwarfLocation::FrameOffset(offset) => {
                // Calculate address from frame pointer
                if let Some(fp) = state.fp() {
                    let addr = (fp as i32 + offset) as u32;
                    let size = size_for_type(&var.var_type) as usize;
                    memory.read_bytes(addr, size).map(|data| {
                        let mut bytes = [0u8; 8];
                        let copy_size = size.min(8);
                        bytes[0..copy_size].copy_from_slice(&data[0..copy_size]);
                        VariableValue {
                            bytes,
                            size: size as u8,
                            var_type: var.var_type.clone(),
                            address: Some(addr),
                        }
                    })
                } else {
                    None
                }
            },
            DwarfLocation::Expression(_) => {
                // Complex DWARF expressions not yet supported
                None
            },
        }
    }

    /// Get all live variables at PC with their current values
    pub fn get_live_variables(
        &self,
        pc: u32,
        state: &dyn RuntimeState,
        memory: &dyn DebugMemory,
    ) -> BoundedVec<LiveVariable<'a>, MAX_DWARF_FILE_TABLE, crate::bounded_debug_infra::DebugProvider>
    {
        let mut live_vars = crate::bounded_debug_infra::create_debug_vec();

        for var_def in self.find_variables_at_pc(pc) {
            let value = self.read_variable(var_def, state, memory);

            let live_var = LiveVariable {
                name: var_def.name.clone(),
                var_type: var_def.var_type.clone(),
                location: var_def.location.clone(),
                value,
                scope_start: var_def.scope.start_pc,
                scope_end: var_def.scope.end_pc,
            };

            live_vars.push(live_var).ok(); // Ignore capacity errors
        }

        live_vars
    }

    /// Format a variable value for display
    pub fn format_value(value: &VariableValue) -> ValueDisplay {
        ValueDisplay { value }
    }
}

/// Helper to get size in bytes for a basic type
fn size_for_type(ty: &BasicType) -> u8 {
    match ty {
        BasicType::Void => 0,
        BasicType::Bool => 1,
        BasicType::SignedInt(size) | BasicType::UnsignedInt(size) => *size,
        BasicType::Float(size) => *size,
        BasicType::Pointer | BasicType::Reference => 4, // 32-bit pointers in WASM
        BasicType::Array | BasicType::Struct | BasicType::Unknown => 4, // Default
    }
}

/// Display helper for variable values
pub struct ValueDisplay<'a> {
    value: &'a VariableValue,
}

impl<'a> ValueDisplay<'a> {
    /// Write the value in appropriate format
    pub fn display<F>(&self, mut writer: F) -> Result<(), core::fmt::Error>
    where
        F: FnMut(&str) -> Result<(), core::fmt::Error>,
    {
        match &self.value.var_type {
            BasicType::Bool => {
                writer(if self.value.bytes[0] != 0 { "true" } else { "false" })?;
            },
            BasicType::SignedInt(4) => {
                if let Some(val) = self.value.as_i32() {
                    let mut buf = [0u8; 11]; // -2147483648
                    writer(format_i32(val, &mut buf))?;
                }
            },
            BasicType::UnsignedInt(4) => {
                if let Some(val) = self.value.as_u32() {
                    let mut buf = [0u8; 10]; // 4294967295
                    writer(format_u32(val, &mut buf))?;
                }
            },
            BasicType::Float(4) => {
                if let Some(val) = self.value.as_f32() {
                    // Simplified float display
                    writer("<f32:")?;
                    let mut buf = [0u8; 10];
                    writer(format_u32(val.to_bits(), &mut buf))?;
                    writer(">")?;
                }
            },
            BasicType::Float(8) => {
                if let Some(val) = self.value.as_f64() {
                    // Simplified float display
                    writer("<f64:")?;
                    let bits = val.to_bits();
                    let mut buf = [0u8; 16];
                    writer(format_hex_u64(bits, &mut buf))?;
                    writer(">")?;
                }
            },
            BasicType::Pointer => {
                writer("0x")?;
                if let Some(addr) = self.value.address {
                    let mut buf = [0u8; 8];
                    writer(format_hex_u32(addr, &mut buf))?;
                } else if let Some(val) = self.value.as_u32() {
                    let mut buf = [0u8; 8];
                    writer(format_hex_u32(val, &mut buf))?;
                }
            },
            _ => {
                // Unknown type - show hex bytes
                writer("<")?;
                for i in 0..self.value.size {
                    if i > 0 {
                        writer(" ")?;
                    }
                    let mut buf = [0u8; 2];
                    writer(format_hex_u8(self.value.bytes[i as usize], &mut buf))?;
                }
                writer(">")?;
            },
        }

        Ok(())
    }
}

// Binary std/no_std choice
fn format_i32(mut n: i32, buf: &mut [u8]) -> &str {
    let mut i = buf.len();
    let negative = n < 0;

    if negative {
        n = -n;
    }

    if n == 0 {
        return "0";
    }

    while n > 0 && i > 1 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    if negative && i > 0 {
        i -= 1;
        buf[i] = b'-';
    }

    core::str::from_utf8(&buf[i..]).unwrap_or("?")
}

fn format_u32(mut n: u32, buf: &mut [u8]) -> &str {
    if n == 0 {
        return "0";
    }

    let mut i = buf.len();
    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    core::str::from_utf8(&buf[i..]).unwrap_or("?")
}

fn format_hex_u8(n: u8, buf: &mut [u8; 2]) -> &str {
    let high = (n >> 4) & 0xF;
    let low = n & 0xF;

    buf[0] = if high < 10 { b'0' + high } else { b'a' + high - 10 };
    buf[1] = if low < 10 { b'0' + low } else { b'a' + low - 10 };

    core::str::from_utf8(buf).unwrap_or("??")
}

fn format_hex_u32(mut n: u32, buf: &mut [u8]) -> &str {
    for i in (0..8).rev() {
        let digit = (n & 0xF) as u8;
        buf[i] = if digit < 10 { b'0' + digit } else { b'a' + digit - 10 };
        n >>= 4;
    }
    core::str::from_utf8(buf).unwrap_or("????????")
}

fn format_hex_u64(mut n: u64, buf: &mut [u8]) -> &str {
    for i in (0..16).rev() {
        let digit = (n & 0xF) as u8;
        buf[i] = if digit < 10 { b'0' + digit } else { b'a' + digit - 10 };
        n >>= 4;
    }
    core::str::from_utf8(buf).unwrap_or("????????????????")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_formatting() {
        // Test integer formatting
        let mut value = VariableValue {
            bytes: [42, 0, 0, 0, 0, 0, 0, 0],
            size: 4,
            var_type: BasicType::SignedInt(4),
            address: None,
        };

        let mut output = String::new();
        ValueDisplay { value: &value }
            .display(|s| {
                output.push_str(s);
                Ok(())
            })
            .unwrap();

        assert_eq!(output, "42");

        // Test boolean formatting
        value.var_type = BasicType::Bool;
        value.size = 1;
        value.bytes[0] = 1;

        output.clear();
        ValueDisplay { value: &value }
            .display(|s| {
                output.push_str(s);
                Ok(())
            })
            .unwrap();

        assert_eq!(output, "true");
    }

    #[test]
    fn test_variable_scope() {
        let mut inspector = VariableInspector::new();

        let var = VariableDefinition {
            name: None,
            var_type: BasicType::SignedInt(4),
            location: DwarfLocation::Register(0),
            scope: VariableScope {
                start_pc: 0x1000,
                end_pc: 0x2000,
                depth: 0,
            },
            file_index: 0,
            line: 0,
        };

        inspector.add_variable(var).unwrap();

        // Variable should be in scope at 0x1500
        let vars: Vec<_> = inspector.find_variables_at_pc(0x1500).collect();
        assert_eq!(vars.len(), 1);

        // Variable should not be in scope at 0x2500
        let vars: Vec<_> = inspector.find_variables_at_pc(0x2500).collect();
        assert_eq!(vars.len(), 0);
    }
}
