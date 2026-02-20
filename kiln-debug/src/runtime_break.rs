#![cfg(feature = "runtime-breakpoints")]

use wrt_error::Error;
use wrt_foundation::{
    bounded::{BoundedVec, MAX_DWARF_FILE_TABLE},
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

use crate::bounded_debug_infra::{DEBUG_PROVIDER_SIZE, DebugProvider};
/// Runtime breakpoint management implementation
/// Provides breakpoint setting, hit detection, and condition evaluation
use crate::{
    FileTable, LineInfo,
    runtime_api::{
        Breakpoint, BreakpointCondition, BreakpointId, DebugAction, DebugError, RuntimeDebugger,
        RuntimeState,
    },
};

/// Breakpoint manager for runtime debugging
pub struct BreakpointManager {
    /// Active breakpoints
    breakpoints:
        BoundedVec<Breakpoint, MAX_DWARF_FILE_TABLE, crate::bounded_debug_infra::DebugProvider>,
    /// Next breakpoint ID
    next_id: u32,
    /// Global enable/disable
    enabled: bool,
}

impl BreakpointManager {
    /// Create a new breakpoint manager
    pub fn new() -> Result<Self, Error> {
        let provider = safe_managed_alloc!(DEBUG_PROVIDER_SIZE, CrateId::Debug)?;
        Ok(Self {
            breakpoints: BoundedVec::new(provider)
                .map_err(|_| Error::memory_error("Failed to create breakpoints vector"))?,
            next_id: 1,
            enabled: true,
        })
    }

    /// Enable or disable all breakpoints
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Add a breakpoint at an address
    pub fn add_breakpoint(&mut self, addr: u32) -> Result<BreakpointId, DebugError> {
        // Check for duplicate
        if self.find_by_address(addr).is_some() {
            return Err(DebugError::DuplicateBreakpoint);
        }

        let id = BreakpointId(self.next_id);
        self.next_id += 1;

        let bp = Breakpoint {
            id,
            address: addr,
            file_index: None,
            line: None,
            condition: None,
            hit_count: 0,
            enabled: true,
        };

        self.breakpoints.push(bp).map_err(|_| DebugError::InvalidAddress)?;

        Ok(id)
    }

    /// Add a breakpoint at file:line
    pub fn add_line_breakpoint(
        &mut self,
        file_index: u16,
        line: u32,
        addr: u32,
    ) -> Result<BreakpointId, DebugError> {
        // Check for duplicate
        if self.find_by_location(file_index, line).is_some() {
            return Err(DebugError::DuplicateBreakpoint);
        }

        let id = BreakpointId(self.next_id);
        self.next_id += 1;

        let bp = Breakpoint {
            id,
            address: addr,
            file_index: Some(file_index),
            line: Some(line),
            condition: None,
            hit_count: 0,
            enabled: true,
        };

        self.breakpoints.push(bp).map_err(|_| DebugError::InvalidAddress)?;

        Ok(id)
    }

    /// Remove a breakpoint
    pub fn remove_breakpoint(&mut self, id: BreakpointId) -> Result<(), DebugError> {
        let pos = self
            .breakpoints
            .iter()
            .position(|bp| bp.id == id)
            .ok_or(DebugError::BreakpointNotFound)?;

        self.breakpoints.remove(pos);
        Ok(())
    }

    /// Enable/disable a specific breakpoint
    pub fn set_breakpoint_enabled(
        &mut self,
        id: BreakpointId,
        enabled: bool,
    ) -> Result<(), DebugError> {
        let bp = self
            .breakpoints
            .iter_mut()
            .find(|bp| bp.id == id)
            .ok_or(DebugError::BreakpointNotFound)?;

        bp.enabled = enabled;
        Ok(())
    }

    /// Set a condition on a breakpoint
    pub fn set_condition(
        &mut self,
        id: BreakpointId,
        condition: BreakpointCondition,
    ) -> Result<(), DebugError> {
        let bp = self
            .breakpoints
            .iter_mut()
            .find(|bp| bp.id == id)
            .ok_or(DebugError::BreakpointNotFound)?;

        bp.condition = Some(condition);
        Ok(())
    }

    /// Find breakpoint by address
    pub fn find_by_address(&self, addr: u32) -> Option<&Breakpoint> {
        self.breakpoints.iter().find(|bp| bp.address == addr && bp.enabled)
    }

    /// Find breakpoint by file:line
    pub fn find_by_location(&self, file_index: u16, line: u32) -> Option<&Breakpoint> {
        self.breakpoints
            .iter()
            .find(|bp| bp.enabled && bp.file_index == Some(file_index) && bp.line == Some(line))
    }

    /// Check if we should break at this PC
    pub fn should_break(&mut self, pc: u32, state: &dyn RuntimeState) -> Option<&mut Breakpoint> {
        if !self.enabled {
            return None;
        }

        // Find matching breakpoint
        let bp_idx = self.breakpoints.iter().position(|bp| bp.enabled && bp.address == pc)?;

        let bp = &mut self.breakpoints[bp_idx];
        bp.hit_count += 1;

        // Check condition
        match &bp.condition {
            None => Some(bp),
            Some(BreakpointCondition::Always) => Some(bp),
            Some(BreakpointCondition::HitCount(count)) => {
                if bp.hit_count >= *count {
                    Some(bp)
                } else {
                    None
                }
            },
            Some(BreakpointCondition::LocalEquals { index, value }) => {
                if let Some(local_val) = state.read_local(*index) {
                    if local_val == *value { Some(bp) } else { None }
                } else {
                    None
                }
            },
        }
    }

    /// Get all breakpoints
    pub fn list_breakpoints(&self) -> &[Breakpoint] {
        self.breakpoints.as_slice()
    }

    /// Clear all breakpoints
    pub fn clear_all(&mut self) {
        self.breakpoints.clear();
    }

    /// Get breakpoint by ID
    pub fn get_breakpoint(&self, id: BreakpointId) -> Option<&Breakpoint> {
        self.breakpoints.iter().find(|bp| bp.id == id)
    }
}

/// Default debugger implementation
pub struct DefaultDebugger {
    /// Breakpoint manager
    breakpoints: BreakpointManager,
    /// Current action
    action: DebugAction,
    /// Single step mode
    single_step: bool,
    /// Step over depth
    step_over_depth: Option<u32>,
    /// File table for display
    file_table: Option<FileTable<'static>>,
}

impl DefaultDebugger {
    /// Create a new debugger
    pub fn new() -> Self {
        Self {
            breakpoints: BreakpointManager::new(),
            action: DebugAction::Continue,
            single_step: false,
            step_over_depth: None,
            file_table: None,
        }
    }

    /// Get breakpoint manager
    pub fn breakpoints(&mut self) -> &mut BreakpointManager {
        &mut self.breakpoints
    }

    /// Set file table for display
    pub fn set_file_table(&mut self, file_table: FileTable<'static>) {
        self.file_table = Some(file_table);
    }

    /// Set next action
    pub fn set_action(&mut self, action: DebugAction) {
        self.action = action;

        match action {
            DebugAction::StepInstruction | DebugAction::StepLine => {
                self.single_step = true;
            },
            DebugAction::Continue => {
                self.single_step = false;
            },
            _ => {},
        }
    }

    /// Format breakpoint location
    pub fn format_breakpoint<F>(
        &self,
        bp: &Breakpoint,
        mut writer: F,
    ) -> Result<(), core::fmt::Error>
    where
        F: FnMut(&str) -> Result<(), core::fmt::Error>,
    {
        writer("Breakpoint ")?;
        let mut buf = [0u8; 10];
        writer(format_u32(bp.id.0, &mut buf))?;
        writer(" at 0x")?;
        let mut hex_buf = [0u8; 8];
        writer(format_hex_u32(bp.address, &mut hex_buf))?;

        if let (Some(file_idx), Some(line)) = (bp.file_index, bp.line) {
            writer(" (")?;
            if let Some(ref file_table) = self.file_table {
                if let Some(path) = file_table.get_full_path(file_idx) {
                    path.display(&mut writer)?;
                    writer(":")?;
                    writer(format_u32(line, &mut buf))?;
                } else {
                    writer("file ")?;
                    writer(format_u32(file_idx as u32, &mut buf))?;
                    writer(":")?;
                    writer(format_u32(line, &mut buf))?;
                }
            } else {
                writer("file ")?;
                writer(format_u32(file_idx as u32, &mut buf))?;
                writer(":")?;
                writer(format_u32(line, &mut buf))?;
            }
            writer(")")?;
        }

        if !bp.enabled {
            writer(" [disabled]")?;
        }

        if bp.hit_count > 0 {
            writer(" hit ")?;
            writer(format_u32(bp.hit_count, &mut buf))?;
            writer(" times")?;
        }

        Ok(())
    }
}

impl RuntimeDebugger for DefaultDebugger {
    fn on_breakpoint(&mut self, bp: &Breakpoint, _state: &dyn RuntimeState) -> DebugAction {
        // In a real implementation, this would interact with the user
        // For now, just return the preset action
        self.action
    }

    fn on_instruction(&mut self, _pc: u32, _state: &dyn RuntimeState) -> DebugAction {
        if self.single_step {
            self.single_step = false;
            DebugAction::Break
        } else {
            DebugAction::Continue
        }
    }

    fn on_function_entry(&mut self, _func_idx: u32, _state: &dyn RuntimeState) {
        // Track for step-over
        if let Some(ref mut depth) = self.step_over_depth {
            *depth += 1;
        }
    }

    fn on_function_exit(&mut self, _func_idx: u32, _state: &dyn RuntimeState) {
        // Track for step-over
        if let Some(ref mut depth) = self.step_over_depth {
            *depth = depth.saturating_sub(1);
            if *depth == 0 {
                self.step_over_depth = None;
                self.single_step = true;
            }
        }
    }

    fn on_trap(&mut self, _trap_code: u32, _state: &dyn RuntimeState) {
        // Always break on trap
        self.action = DebugAction::Break;
    }
}

// Formatting helpers
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

fn format_hex_u32(mut n: u32, buf: &mut [u8; 8]) -> &str {
    for i in (0..8).rev() {
        let digit = (n & 0xF) as u8;
        buf[i] = if digit < 10 { b'0' + digit } else { b'a' + digit - 10 };
        n >>= 4;
    }
    core::str::from_utf8(buf).unwrap_or("????????")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breakpoint_management() {
        let mut manager = BreakpointManager::new();

        // Add breakpoint
        let id1 = manager.add_breakpoint(0x1000).unwrap();
        assert!(manager.find_by_address(0x1000).is_some());

        // No duplicate
        assert!(manager.add_breakpoint(0x1000).is_err());

        // Add line breakpoint
        let id2 = manager.add_line_breakpoint(1, 42, 0x2000).unwrap();
        assert!(manager.find_by_location(1, 42).is_some());

        // Remove breakpoint
        manager.remove_breakpoint(id1).unwrap();
        assert!(manager.find_by_address(0x1000).is_none());

        // List breakpoints
        assert_eq!(manager.list_breakpoints().len(), 1);
    }

    #[test]
    fn test_breakpoint_conditions() {
        let mut manager = BreakpointManager::new();

        let id = manager.add_breakpoint(0x1000).unwrap();

        // Set hit count condition
        manager.set_condition(id, BreakpointCondition::HitCount(3)).unwrap();

        // Mock state
        struct MockState;
        impl RuntimeState for MockState {
            fn pc(&self) -> u32 {
                0x1000
            }

            fn sp(&self) -> u32 {
                0
            }

            fn fp(&self) -> Option<u32> {
                None
            }

            fn read_local(&self, _: u32) -> Option<u64> {
                None
            }

            fn read_stack(&self, _: u32) -> Option<u64> {
                None
            }

            fn current_function(&self) -> Option<u32> {
                None
            }
        }

        let state = MockState;

        // Should not break on first two hits
        assert!(manager.should_break(0x1000, &state).is_none());
        assert!(manager.should_break(0x1000, &state).is_none());

        // Should break on third hit
        assert!(manager.should_break(0x1000, &state).is_some());
    }
}
