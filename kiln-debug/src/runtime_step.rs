#![cfg(feature = "runtime-stepping")]

use wrt_foundation::{
    CrateId, NoStdProvider,
    bounded::{BoundedStack, BoundedVec, MAX_DWARF_FILE_TABLE},
    managed_alloc, safe_managed_alloc,
};

use crate::bounded_debug_infra;
/// Runtime stepping logic implementation
/// Provides single-step, step-over, step-into, and step-out functionality
use crate::{
    LineInfo,
    runtime_api::{DebugAction, RuntimeState},
};

/// Stepping mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    /// Not stepping
    None,
    /// Step one instruction
    Instruction,
    /// Step to next line
    Line,
    /// Step over function calls
    Over,
    /// Step into function calls
    Into,
    /// Step out of current function
    Out,
}

/// Call stack frame for stepping
#[derive(Debug, Clone)]
struct StepFrame {
    /// Function index
    function_idx: u32,
    /// Return address
    return_pc: u32,
    /// Source line at call
    call_line: Option<u32>,
}

/// Step controller for debugging
pub struct StepController {
    /// Current stepping mode
    mode: StepMode,
    /// Target line for line stepping
    target_line: Option<u32>,
    /// Target file for line stepping
    target_file: Option<u16>,
    /// Call stack for step-over/out
    call_stack: BoundedStack<StepFrame, 64, crate::bounded_debug_infra::DebugProvider>,
    /// Depth for step-over
    step_over_depth: u32,
    /// Previous PC for detecting loops
    previous_pc: u32,
    /// Previous line for line stepping
    previous_line: Option<u32>,
}

impl StepController {
    /// Create a new step controller
    pub fn new() -> Self {
        Self {
            mode: StepMode::None,
            target_line: None,
            target_file: None,
            call_stack: BoundedStack::new(
                safe_managed_alloc!(32768, CrateId::Debug).unwrap().provider().clone(),
            )
            .unwrap(),
            step_over_depth: 0,
            previous_pc: 0,
            previous_line: None,
        }
    }

    /// Start stepping with given mode
    pub fn start_step(&mut self, mode: StepMode, current_line: Option<&LineInfo>) {
        self.mode = mode;

        match mode {
            StepMode::Line | StepMode::Over | StepMode::Into => {
                // Remember current line
                if let Some(line) = current_line {
                    self.target_file = Some(line.file_index);
                    self.target_line = Some(line.line);
                    self.previous_line = Some(line.line);
                }
            },
            StepMode::Out => {
                // Step out needs at least one frame
                self.step_over_depth = 1;
            },
            _ => {},
        }
    }

    /// Check if we should break at this instruction
    pub fn should_break(
        &mut self,
        pc: u32,
        state: &dyn RuntimeState,
        current_line: Option<&LineInfo>,
    ) -> DebugAction {
        // Detect infinite loops
        if pc == self.previous_pc {
            return DebugAction::Continue;
        }
        self.previous_pc = pc;

        match self.mode {
            StepMode::None => DebugAction::Continue,

            StepMode::Instruction => {
                // Always break on next instruction
                self.mode = StepMode::None;
                DebugAction::Break
            },

            StepMode::Line => self.check_line_step(current_line),

            StepMode::Over => self.check_step_over(current_line),

            StepMode::Into => self.check_step_into(current_line),

            StepMode::Out => self.check_step_out(),
        }
    }

    /// Handle function entry
    pub fn on_function_entry(&mut self, func_idx: u32, return_pc: u32) {
        match self.mode {
            StepMode::Over => {
                // Track call depth
                self.step_over_depth += 1;
            },
            StepMode::Out => {
                // Track call depth
                self.step_over_depth += 1;
            },
            _ => {},
        }

        // Push frame
        let frame = StepFrame {
            function_idx: func_idx,
            return_pc,
            call_line: self.previous_line,
        };
        self.call_stack.push(frame).ok();
    }

    /// Handle function exit
    pub fn on_function_exit(&mut self) {
        // Pop frame
        self.call_stack.pop();

        match self.mode {
            StepMode::Over => {
                self.step_over_depth = self.step_over_depth.saturating_sub(1);
            },
            StepMode::Out => {
                self.step_over_depth = self.step_over_depth.saturating_sub(1);
                if self.step_over_depth == 0 {
                    // We've stepped out
                    self.mode = StepMode::None;
                }
            },
            _ => {},
        }
    }

    /// Reset stepping state
    pub fn reset(&mut self) {
        self.mode = StepMode::None;
        self.target_line = None;
        self.target_file = None;
        self.step_over_depth = 0;
        self.call_stack.clear();
    }

    /// Get current stepping mode
    pub fn mode(&self) -> StepMode {
        self.mode
    }

    /// Check for line step completion
    fn check_line_step(&mut self, current_line: Option<&LineInfo>) -> DebugAction {
        if let Some(line) = current_line {
            // Check if we've moved to a different line
            if let Some(target_line) = self.target_line {
                if line.line != target_line || line.file_index != self.target_file.unwrap_or(0) {
                    // Different line - stop
                    self.mode = StepMode::None;
                    self.previous_line = Some(line.line);
                    return DebugAction::Break;
                }
            }
        }

        DebugAction::Continue
    }

    /// Check for step-over completion
    fn check_step_over(&mut self, current_line: Option<&LineInfo>) -> DebugAction {
        // Only break if we're at the original call depth
        if self.step_over_depth > 0 {
            return DebugAction::Continue;
        }

        // Check if we've moved to a different line
        if let Some(line) = current_line {
            if let Some(target_line) = self.target_line {
                if line.line != target_line || line.file_index != self.target_file.unwrap_or(0) {
                    // Different line at original depth - stop
                    self.mode = StepMode::None;
                    self.previous_line = Some(line.line);
                    return DebugAction::Break;
                }
            }
        }

        DebugAction::Continue
    }

    /// Check for step-into completion
    fn check_step_into(&mut self, current_line: Option<&LineInfo>) -> DebugAction {
        // Step into breaks on any new line
        if let Some(line) = current_line {
            if let Some(prev_line) = self.previous_line {
                if line.line != prev_line {
                    self.mode = StepMode::None;
                    self.previous_line = Some(line.line);
                    return DebugAction::Break;
                }
            } else {
                // First line
                self.mode = StepMode::None;
                self.previous_line = Some(line.line);
                return DebugAction::Break;
            }
        }

        DebugAction::Continue
    }

    /// Check for step-out completion
    fn check_step_out(&self) -> DebugAction {
        // Step-out is handled in on_function_exit
        if self.step_over_depth == 0 {
            DebugAction::Break
        } else {
            DebugAction::Continue
        }
    }
}

/// Combined stepping and line tracking
pub struct SteppingDebugger {
    /// Step controller
    controller: StepController,
    /// Line number cache
    line_cache:
        BoundedVec<LineCacheEntry, MAX_DWARF_FILE_TABLE, crate::bounded_debug_infra::DebugProvider>,
}

#[derive(Debug, Clone)]
struct LineCacheEntry {
    pc_start: u32,
    pc_end: u32,
    line_info: LineInfo,
}

impl SteppingDebugger {
    /// Create a new stepping debugger
    pub fn new() -> Self {
        Self {
            controller: StepController::new(),
            line_cache: BoundedVec::new(
                safe_managed_alloc!(32768, CrateId::Debug).unwrap().provider().clone(),
            )
            .unwrap(),
        }
    }

    /// Add line mapping to cache
    pub fn add_line_mapping(
        &mut self,
        pc_start: u32,
        pc_end: u32,
        line_info: LineInfo,
    ) -> Result<(), ()> {
        let entry = LineCacheEntry {
            pc_start,
            pc_end,
            line_info,
        };
        self.line_cache.push(entry).map_err(|_| ())
    }

    /// Find line info for PC
    pub fn find_line(&self, pc: u32) -> Option<&LineInfo> {
        self.line_cache
            .iter()
            .find(|entry| pc >= entry.pc_start && pc < entry.pc_end)
            .map(|entry| &entry.line_info)
    }

    /// Start stepping
    pub fn step(&mut self, mode: StepMode, pc: u32) {
        let current_line = self.find_line(pc).copied();
        self.controller.start_step(mode, current_line);
    }

    /// Check if we should break
    pub fn should_break(&mut self, pc: u32, state: &(dyn RuntimeState + 'static)) -> DebugAction {
        let current_line = self.find_line(pc).copied();
        self.controller.should_break(pc, state, current_line)
    }

    /// Handle function entry
    pub fn on_function_entry(&mut self, func_idx: u32, return_pc: u32) {
        self.controller.on_function_entry(func_idx, return_pc);
    }

    /// Handle function exit  
    pub fn on_function_exit(&mut self) {
        self.controller.on_function_exit();
    }

    /// Get controller for direct access
    pub fn controller(&mut self) -> &mut StepController {
        &mut self.controller
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockState {
        pc: u32,
    }

    impl RuntimeState for MockState {
        fn pc(&self) -> u32 {
            self.pc
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

    #[test]
    fn test_instruction_stepping() {
        let mut controller = StepController::new();
        let state = MockState { pc: 0x1000 };

        // Start instruction step
        controller.start_step(StepMode::Instruction, None);

        // Should break on next instruction
        let action = controller.should_break(0x1001, &state, None);
        assert_eq!(action, DebugAction::Break);

        // Mode should be reset
        assert_eq!(controller.mode(), StepMode::None);
    }

    #[test]
    fn test_line_stepping() {
        let mut controller = StepController::new();
        let state = MockState { pc: 0x1000 };

        let line1 = LineInfo {
            file_index: 1,
            line: 10,
            column: 0,
            is_stmt: true,
            end_sequence: false,
        };

        let line2 = LineInfo {
            file_index: 1,
            line: 11,
            column: 0,
            is_stmt: true,
            end_sequence: false,
        };

        // Start line step
        controller.start_step(StepMode::Line, Some(&line1));

        // Same line - continue
        let action = controller.should_break(0x1001, &state, Some(&line1));
        assert_eq!(action, DebugAction::Continue);

        // Different line - break
        let action = controller.should_break(0x1002, &state, Some(&line2));
        assert_eq!(action, DebugAction::Break);
    }

    #[test]
    fn test_step_over() {
        let mut controller = StepController::new();
        let state = MockState { pc: 0x1000 };

        let line1 = LineInfo {
            file_index: 1,
            line: 10,
            column: 0,
            is_stmt: true,
            end_sequence: false,
        };

        // Start step over
        controller.start_step(StepMode::Over, Some(&line1));

        // Enter function - increases depth
        controller.on_function_entry(1, 0x1100);

        // Inside function - continue
        let action = controller.should_break(0x2000, &state, Some(&line1));
        assert_eq!(action, DebugAction::Continue);

        // Exit function
        controller.on_function_exit();

        // Back at original depth with new line - break
        let line2 = LineInfo {
            file_index: 1,
            line: 11,
            column: 0,
            is_stmt: true,
            end_sequence: false,
        };
        let action = controller.should_break(0x1100, &state, Some(&line2));
        assert_eq!(action, DebugAction::Break);
    }
}
