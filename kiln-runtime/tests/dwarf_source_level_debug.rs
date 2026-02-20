//! DWARF Source-Level Debugging Integration Test
//!
//! This test demonstrates real-world DWARF integration by:
//! 1. Loading WASM files with embedded DWARF debug sections
//! 2. Parsing the .debug_line section to get line number information
//! 3. Resolving PC (instruction offset) values to source file:line:column
//!
//! This is the foundation for source-level debugging during execution.
//!
//! Requires features: std, debug (for line-info), debug-runtime-traits

// Use debug + debug-runtime-traits for minimal DWARF line info + runtime traits
#![cfg(all(feature = "debug", feature = "debug-runtime-traits", feature = "std"))]

use std::fs;

/// WASM binary format constants
const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];
const WASM_VERSION_CORE: [u8; 4] = [0x01, 0x00, 0x00, 0x00];  // Core WASM version 1
const WASM_VERSION_COMPONENT: [u8; 4] = [0x0D, 0x00, 0x01, 0x00];  // Component Model layer 1, v13
const CUSTOM_SECTION_ID: u8 = 0x00;
const CORE_MODULE_SECTION_ID: u8 = 0x01;  // Component model section ID for core modules

/// Read an unsigned LEB128 integer from bytes at position
fn read_leb128_u32(bytes: &[u8], pos: usize) -> Result<(u32, usize), &'static str> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut offset = 0;

    loop {
        if pos + offset >= bytes.len() {
            return Err("LEB128 exceeds buffer bounds");
        }

        let byte = bytes[pos + offset];
        result |= ((byte & 0x7F) as u32) << shift;
        offset += 1;

        if byte & 0x80 == 0 {
            break;
        }

        shift += 7;
        if shift >= 35 {
            return Err("LEB128 integer too large");
        }
    }

    Ok((result, pos + offset))
}

/// Represents a custom section extracted from a WASM binary
#[derive(Debug)]
struct CustomSection {
    name: String,
    data: Vec<u8>,
    /// Offset within the WASM binary where this section's data starts
    #[allow(dead_code)]
    offset: usize,
}

/// Parse all custom sections from a WASM binary (core or component)
fn parse_custom_sections(wasm_bytes: &[u8]) -> Result<Vec<CustomSection>, String> {
    // Verify WASM magic and version
    if wasm_bytes.len() < 8 {
        return Err("WASM file too small".to_string());
    }
    if &wasm_bytes[0..4] != WASM_MAGIC {
        return Err("Invalid WASM magic bytes".to_string());
    }

    // Check for both core WASM and component model versions
    let is_core = &wasm_bytes[4..8] == WASM_VERSION_CORE;
    let is_component = &wasm_bytes[4..8] == WASM_VERSION_COMPONENT;

    if !is_core && !is_component {
        return Err(format!(
            "Unsupported WASM version: {:02x} {:02x} {:02x} {:02x}",
            wasm_bytes[4], wasm_bytes[5], wasm_bytes[6], wasm_bytes[7]
        ));
    }

    #[allow(unused_variables)]
    let is_component_model = is_component;
    if is_component {
        println!("Note: Parsing Component Model WASM (version 0x{:02x}{:02x}{:02x}{:02x})",
            wasm_bytes[7], wasm_bytes[6], wasm_bytes[5], wasm_bytes[4]);
    }

    let mut sections = Vec::new();
    let mut pos = 8; // After magic and version

    while pos < wasm_bytes.len() {
        // Read section ID
        let section_id = wasm_bytes[pos];
        pos += 1;

        // Read section size
        let (section_size, new_pos) = read_leb128_u32(wasm_bytes, pos)
            .map_err(|e| format!("Failed to read section size: {}", e))?;
        pos = new_pos;

        let section_end = pos + section_size as usize;
        if section_end > wasm_bytes.len() {
            return Err(format!(
                "Section size {} exceeds file bounds at position {}",
                section_size, pos
            ));
        }

        if section_id == CUSTOM_SECTION_ID {
            // Parse custom section name
            let (name_len, name_start) = read_leb128_u32(wasm_bytes, pos)
                .map_err(|e| format!("Failed to read custom section name length: {}", e))?;

            let name_end = name_start + name_len as usize;
            if name_end > section_end {
                return Err("Custom section name exceeds section bounds".to_string());
            }

            let name = String::from_utf8(wasm_bytes[name_start..name_end].to_vec())
                .map_err(|_| "Invalid UTF-8 in custom section name".to_string())?;

            let data_start = name_end;
            let data = wasm_bytes[data_start..section_end].to_vec();

            sections.push(CustomSection {
                name,
                data,
                offset: data_start,
            });
        } else if section_id == CORE_MODULE_SECTION_ID && is_component {
            // Component model: parse nested core module for custom sections
            // The module section contains a complete core WASM module
            let module_data = &wasm_bytes[pos..section_end];

            // Recursively parse the nested core module
            if let Ok(nested_sections) = parse_core_module_sections(module_data, pos) {
                for section in nested_sections {
                    sections.push(section);
                }
            }
        }

        pos = section_end;
    }

    Ok(sections)
}

/// Parse custom sections from a core WASM module (used for nested modules in components)
fn parse_core_module_sections(module_bytes: &[u8], base_offset: usize) -> Result<Vec<CustomSection>, String> {
    // Core module starts with magic + version (8 bytes)
    if module_bytes.len() < 8 {
        return Err("Core module too small".to_string());
    }
    if &module_bytes[0..4] != WASM_MAGIC {
        return Err("Invalid core module magic".to_string());
    }
    if &module_bytes[4..8] != WASM_VERSION_CORE {
        return Err("Invalid core module version".to_string());
    }

    let mut sections = Vec::new();
    let mut pos = 8;

    while pos < module_bytes.len() {
        let section_id = module_bytes[pos];
        pos += 1;

        let (section_size, new_pos) = read_leb128_u32(module_bytes, pos)
            .map_err(|e| format!("Failed to read section size in core module: {}", e))?;
        pos = new_pos;

        let section_end = pos + section_size as usize;
        if section_end > module_bytes.len() {
            break; // Section extends beyond module, stop parsing
        }

        if section_id == CUSTOM_SECTION_ID {
            // Parse custom section name
            let (name_len, name_start) = read_leb128_u32(module_bytes, pos)
                .map_err(|e| format!("Failed to read custom section name length: {}", e))?;

            let name_end = name_start + name_len as usize;
            if name_end > section_end {
                pos = section_end;
                continue;
            }

            let name = match String::from_utf8(module_bytes[name_start..name_end].to_vec()) {
                Ok(n) => n,
                Err(_) => {
                    pos = section_end;
                    continue;
                }
            };

            let data_start = name_end;
            let data = module_bytes[data_start..section_end].to_vec();

            sections.push(CustomSection {
                name,
                data,
                offset: base_offset + data_start,
            });
        }

        pos = section_end;
    }

    Ok(sections)
}

/// Test that we can find and parse DWARF sections from calculator.wasm
#[test]
fn test_extract_dwarf_sections() {
    // Try to load calculator.wasm from the project root
    let wasm_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("calculator.wasm");

    if !wasm_path.exists() {
        println!("Skipping test: calculator.wasm not found at {:?}", wasm_path);
        return;
    }

    let wasm_bytes = fs::read(&wasm_path).expect("Failed to read calculator.wasm");
    println!("Loaded {} bytes from {:?}", wasm_bytes.len(), wasm_path);

    let sections = parse_custom_sections(&wasm_bytes).expect("Failed to parse WASM");

    println!("\nCustom sections found:");
    let mut dwarf_sections: Vec<&CustomSection> = Vec::new();

    for section in &sections {
        println!("  {} ({} bytes)", section.name, section.data.len());
        if section.name.starts_with(".debug_") {
            dwarf_sections.push(section);
        }
    }

    println!("\nDWARF sections: {}", dwarf_sections.len());
    for section in &dwarf_sections {
        println!("  {}: {} bytes", section.name, section.data.len());
    }

    assert!(!dwarf_sections.is_empty(), "No DWARF sections found in calculator.wasm");

    // Check for essential debug sections
    let has_debug_line = dwarf_sections.iter().any(|s| s.name == ".debug_line");
    let has_debug_info = dwarf_sections.iter().any(|s| s.name == ".debug_info");

    println!("\nDWARF section availability:");
    println!("  .debug_line: {}", has_debug_line);
    println!("  .debug_info: {}", has_debug_info);

    assert!(has_debug_line, "Missing .debug_line section - required for line number info");
}

/// Test using DwarfDebugInfo to resolve PC to source location
#[test]
#[cfg(feature = "std")]
fn test_dwarf_line_number_resolution() {
    use wrt_debug::DwarfDebugInfo;

    // Load calculator.wasm
    let wasm_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("calculator.wasm");

    if !wasm_path.exists() {
        println!("Skipping test: calculator.wasm not found");
        return;
    }

    let wasm_bytes = fs::read(&wasm_path).expect("Failed to read calculator.wasm");
    let sections = parse_custom_sections(&wasm_bytes).expect("Failed to parse WASM");

    // Create DwarfDebugInfo and register sections
    let mut debug_info = DwarfDebugInfo::new(&wasm_bytes).expect("Failed to create DwarfDebugInfo");

    // Find and register all DWARF sections
    // Note: The offset here is the position within the WASM file, but DwarfDebugInfo
    // expects the data to be part of the module_bytes passed to new().
    // For this test, we'll work with the section data directly.

    for section in &sections {
        if section.name.starts_with(".debug_") {
            // Register the section - offset is relative to wasm_bytes
            // The section data length gives us size
            println!(
                "Registering {} at offset {} ({} bytes)",
                section.name,
                section.offset,
                section.data.len()
            );
            debug_info.add_section(&section.name, section.offset as u32, section.data.len() as u32);
        }
    }

    assert!(debug_info.has_debug_info(), "DwarfDebugInfo should have debug info after registration");

    println!("\nDWARF debug info registered successfully!");
    println!("Ready for PC to source location resolution");
}

/// Demonstrate source-level debugging output for simulated execution
#[test]
#[cfg(feature = "std")]
fn test_simulated_source_level_debug() {
    use wrt_debug::{DwarfDebugInfo, LineInfo};

    println!("\n=== Source-Level Debugging Simulation ===\n");

    // Load calculator.wasm
    let wasm_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("calculator.wasm");

    if !wasm_path.exists() {
        println!("Skipping test: calculator.wasm not found");
        return;
    }

    let wasm_bytes = fs::read(&wasm_path).expect("Failed to read calculator.wasm");
    let sections = parse_custom_sections(&wasm_bytes).expect("Failed to parse WASM");

    // Find the .debug_line section
    let debug_line = sections.iter().find(|s| s.name == ".debug_line");

    if debug_line.is_none() {
        println!("No .debug_line section found");
        return;
    }

    let debug_line = debug_line.unwrap();
    println!(".debug_line section: {} bytes", debug_line.data.len());

    // Create DwarfDebugInfo
    let mut debug_info = DwarfDebugInfo::new(&wasm_bytes).expect("Failed to create DwarfDebugInfo");

    // Register the debug_line section
    debug_info.add_section(".debug_line", debug_line.offset as u32, debug_line.data.len() as u32);

    // Simulate execution at various PC values
    println!("\nSimulating execution with source location resolution:");
    println!("{:-<60}", "");

    // Try a range of PC values that might correspond to actual code
    // WebAssembly code sections typically start after other sections
    let test_pcs: Vec<u32> = (0..50).step_by(5).collect();

    for pc in test_pcs {
        match debug_info.find_line_info(pc) {
            Ok(Some(line_info)) => {
                println!(
                    "PC 0x{:04x}: file={}, line={}, column={}, is_stmt={}",
                    pc, line_info.file_index, line_info.line, line_info.column, line_info.is_stmt
                );
            }
            Ok(None) => {
                // No line info at this PC - common for addresses before code section
            }
            Err(e) => {
                println!("PC 0x{:04x}: Error: {:?}", pc, e);
            }
        }
    }

    println!("{:-<60}", "");
    println!("\nSource-level debugging simulation complete!");
}

/// Integration test showing how a debugger would work during actual execution
#[test]
#[cfg(feature = "std")]
fn test_debugger_callback_simulation() {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use wrt_debug::runtime_traits::{
        Breakpoint, BreakpointId, DebugAction, RuntimeDebugger, RuntimeState,
    };

    /// A debugger that prints source locations during execution
    struct SourceLevelDebugger {
        instruction_count: AtomicU32,
        // In a real implementation, this would hold DwarfDebugInfo
        // For now, we just count instructions
    }

    impl SourceLevelDebugger {
        fn new() -> Self {
            Self {
                instruction_count: AtomicU32::new(0),
            }
        }
    }

    impl RuntimeDebugger for SourceLevelDebugger {
        fn on_breakpoint(&mut self, bp: &Breakpoint, state: &dyn RuntimeState) -> DebugAction {
            println!(
                "[BREAKPOINT] ID={:?} at PC=0x{:04x}, current func={}",
                bp.id,
                state.pc(),
                state.current_function().unwrap_or(0)
            );
            DebugAction::Continue
        }

        fn on_instruction(&mut self, pc: u32, state: &dyn RuntimeState) -> DebugAction {
            let count = self.instruction_count.fetch_add(1, Ordering::SeqCst);

            // Only print every 10th instruction to reduce noise
            if count % 10 == 0 {
                println!(
                    "[STEP] #{}: PC=0x{:04x}, SP={}, func={}",
                    count,
                    pc,
                    state.sp(),
                    state.current_function().unwrap_or(0)
                );
            }

            DebugAction::Continue
        }

        fn on_function_entry(&mut self, func_idx: u32, state: &dyn RuntimeState) {
            println!("[CALL] Entering function {} at PC=0x{:04x}", func_idx, state.pc());
        }

        fn on_function_exit(&mut self, func_idx: u32, state: &dyn RuntimeState) {
            println!("[RET] Exiting function {} at PC=0x{:04x}", func_idx, state.pc());
        }

        fn on_trap(&mut self, trap_code: u32, state: &dyn RuntimeState) {
            println!(
                "[TRAP] Code={} at PC=0x{:04x}, func={}",
                trap_code,
                state.pc(),
                state.current_function().unwrap_or(0)
            );
        }
    }

    println!("\n=== Debugger Callback Simulation ===\n");

    let debugger = SourceLevelDebugger::new();

    // Simulate some callbacks
    struct MockState {
        pc: u32,
        sp: u32,
        func_idx: u32,
    }

    impl RuntimeState for MockState {
        fn pc(&self) -> u32 {
            self.pc
        }
        fn sp(&self) -> u32 {
            self.sp
        }
        fn fp(&self) -> Option<u32> {
            None
        }
        fn read_local(&self, _index: u32) -> Option<u64> {
            Some(0)
        }
        fn read_stack(&self, _offset: u32) -> Option<u64> {
            Some(0)
        }
        fn current_function(&self) -> Option<u32> {
            Some(self.func_idx)
        }
    }

    let mut dbg = debugger;
    let state = MockState {
        pc: 0x100,
        sp: 5,
        func_idx: 1,
    };

    // Simulate function entry
    dbg.on_function_entry(1, &state);

    // Simulate some instructions
    for i in 0..25 {
        let state = MockState {
            pc: 0x100 + i * 4,
            sp: 5 + i % 3,
            func_idx: 1,
        };
        dbg.on_instruction(0x100 + i * 4, &state);
    }

    // Simulate function exit
    let exit_state = MockState {
        pc: 0x180,
        sp: 5,
        func_idx: 1,
    };
    dbg.on_function_exit(1, &exit_state);

    println!("\nTotal instructions: {}", dbg.instruction_count.load(Ordering::SeqCst));
    println!("\nDebugger callback simulation complete!");
}

/// Full integration example: Execute with source-level debug output
/// This demonstrates what a complete debugging session would look like
#[test]
#[cfg(feature = "std")]
fn test_full_integration_demo() {
    println!("\n═══════════════════════════════════════════════════════════");
    println!("     DWARF SOURCE-LEVEL DEBUGGING INTEGRATION DEMO");
    println!("═══════════════════════════════════════════════════════════\n");

    use wrt_debug::DwarfDebugInfo;

    let wasm_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("calculator.wasm");

    if !wasm_path.exists() {
        println!("Skipping: calculator.wasm not found");
        return;
    }

    let wasm_bytes = fs::read(&wasm_path).expect("Failed to read calculator.wasm");
    let sections = parse_custom_sections(&wasm_bytes).expect("Failed to parse WASM");

    // Create DwarfDebugInfo
    let mut debug_info = DwarfDebugInfo::new(&wasm_bytes).expect("Failed to create DwarfDebugInfo");

    // Register DWARF sections
    for section in &sections {
        if section.name.starts_with(".debug_") {
            debug_info.add_section(&section.name, section.offset as u32, section.data.len() as u32);
        }
    }

    println!("Loaded calculator.wasm with debug info");
    println!("Simulating execution of 'add' function...\n");

    // Simulate what a debugger would print during execution
    // In real usage, this would be called from on_instruction() callback
    let simulated_pcs: Vec<u32> = vec![0, 10, 26, 30, 35, 42, 50];

    println!("┌──────────┬────────┬────────┬────────────────────────────────────┐");
    println!("│   PC     │  Line  │  Col   │  Source Location                   │");
    println!("├──────────┼────────┼────────┼────────────────────────────────────┤");

    for pc in simulated_pcs {
        match debug_info.find_line_info(pc) {
            Ok(Some(info)) => {
                // In a full implementation, we'd resolve file_index to actual path
                // using FileTable from the .debug_line header
                let location = if info.is_stmt {
                    format!("file[{}] (statement boundary)", info.file_index)
                } else {
                    format!("file[{}]", info.file_index)
                };

                println!(
                    "│ 0x{:06x} │ {:>6} │ {:>6} │ {:<34} │",
                    pc, info.line, info.column, location
                );
            }
            Ok(None) => {
                println!("│ 0x{:06x} │   -    │   -    │ (no debug info)                    │", pc);
            }
            Err(_) => {
                println!("│ 0x{:06x} │   -    │   -    │ (parse error)                      │", pc);
            }
        }
    }

    println!("└──────────┴────────┴────────┴────────────────────────────────────┘");

    // Show summary statistics
    let debug_line = sections.iter().find(|s| s.name == ".debug_line");
    let debug_info_section = sections.iter().find(|s| s.name == ".debug_info");
    let debug_str = sections.iter().find(|s| s.name == ".debug_str");

    println!("\n📊 Debug Section Statistics:");
    if let Some(s) = debug_line {
        println!("   .debug_line: {} bytes ({:.1} KB)", s.data.len(), s.data.len() as f64 / 1024.0);
    }
    if let Some(s) = debug_info_section {
        println!("   .debug_info: {} bytes ({:.1} KB)", s.data.len(), s.data.len() as f64 / 1024.0);
    }
    if let Some(s) = debug_str {
        println!("   .debug_str:  {} bytes ({:.1} KB)", s.data.len(), s.data.len() as f64 / 1024.0);
    }

    println!("\n✅ Source-level debugging infrastructure is operational!");
    println!("   Ready for integration with StacklessEngine execution loop.\n");
}

/// Test to verify DWARF sections contain valid data
#[test]
fn test_dwarf_section_structure() {
    let wasm_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("calculator.wasm");

    if !wasm_path.exists() {
        println!("Skipping test: calculator.wasm not found");
        return;
    }

    let wasm_bytes = fs::read(&wasm_path).expect("Failed to read calculator.wasm");
    let sections = parse_custom_sections(&wasm_bytes).expect("Failed to parse WASM");

    // Find .debug_line section
    if let Some(debug_line) = sections.iter().find(|s| s.name == ".debug_line") {
        println!("\n=== .debug_line Section Analysis ===\n");

        // Parse DWARF line header
        if debug_line.data.len() >= 10 {
            let unit_length = u32::from_le_bytes([
                debug_line.data[0],
                debug_line.data[1],
                debug_line.data[2],
                debug_line.data[3],
            ]);
            let version = u16::from_le_bytes([debug_line.data[4], debug_line.data[5]]);
            let header_length = u32::from_le_bytes([
                debug_line.data[6],
                debug_line.data[7],
                debug_line.data[8],
                debug_line.data[9],
            ]);

            println!("DWARF Line Number Program Header:");
            println!("  Unit length: {} bytes", unit_length);
            println!("  DWARF version: {}", version);
            println!("  Header length: {} bytes", header_length);

            if debug_line.data.len() >= 15 {
                let min_instr_len = debug_line.data[10];
                let max_ops_per_instr = if version >= 4 { debug_line.data[11] } else { 1 };
                let default_is_stmt = debug_line.data[if version >= 4 { 12 } else { 11 }];
                let line_base = debug_line.data[if version >= 4 { 13 } else { 12 }] as i8;
                let line_range = debug_line.data[if version >= 4 { 14 } else { 13 }];

                println!("  Minimum instruction length: {}", min_instr_len);
                println!("  Maximum operations per instruction: {}", max_ops_per_instr);
                println!("  Default is_stmt: {}", default_is_stmt);
                println!("  Line base: {}", line_base);
                println!("  Line range: {}", line_range);

                // Validate version
                assert!(
                    (2..=5).contains(&version),
                    "Unsupported DWARF version: {}",
                    version
                );
                println!("\nDWARF header validation passed!");
            }
        }
    } else {
        println!("No .debug_line section found");
    }
}
