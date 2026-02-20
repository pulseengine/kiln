//! Source-level line tracing support for wrtd
//!
//! This module provides DWARF-based source line tracing during WebAssembly
//! execution. When enabled via `--trace-lines`, it prints file:line information
//! as the program executes.

use wrt_debug::runtime_traits::{
    Breakpoint, DebugAction, RuntimeDebugger, RuntimeState,
};
use wrt_error::{Error, Result};

/// WASM binary format constants
const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];
const WASM_VERSION_CORE: [u8; 4] = [0x01, 0x00, 0x00, 0x00];
const WASM_VERSION_COMPONENT: [u8; 4] = [0x0D, 0x00, 0x01, 0x00];
const CUSTOM_SECTION_ID: u8 = 0x00;
const CORE_MODULE_SECTION_ID: u8 = 0x01;

/// Represents a custom section extracted from a WASM binary
#[derive(Debug)]
pub struct CustomSection {
    /// Section name (e.g., ".debug_line")
    pub name: String,
    /// Section data
    pub data: Vec<u8>,
    /// Offset within the WASM binary where this section's data starts
    pub offset: usize,
}

/// Read an unsigned LEB128 integer from bytes at position
fn read_leb128_u32(bytes: &[u8], pos: usize) -> Result<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut offset = 0;

    loop {
        if pos + offset >= bytes.len() {
            return Err(Error::parse_error("LEB128 exceeds buffer bounds"));
        }

        let byte = bytes[pos + offset];
        result |= ((byte & 0x7F) as u32) << shift;
        offset += 1;

        if byte & 0x80 == 0 {
            break;
        }

        shift += 7;
        if shift >= 35 {
            return Err(Error::parse_error("LEB128 integer too large"));
        }
    }

    Ok((result, pos + offset))
}

/// Parse all custom sections from a WASM binary (core or component)
pub fn parse_custom_sections(wasm_bytes: &[u8]) -> Result<Vec<CustomSection>> {
    // Verify WASM magic and version
    if wasm_bytes.len() < 8 {
        return Err(Error::parse_error("WASM file too small"));
    }
    if &wasm_bytes[0..4] != WASM_MAGIC {
        return Err(Error::parse_error("Invalid WASM magic bytes"));
    }

    // Check for both core WASM and component model versions
    let is_core = &wasm_bytes[4..8] == WASM_VERSION_CORE;
    let is_component = &wasm_bytes[4..8] == WASM_VERSION_COMPONENT;

    if !is_core && !is_component {
        return Err(Error::parse_error("Unsupported WASM version"));
    }

    let mut sections = Vec::new();
    let mut pos = 8; // After magic and version

    while pos < wasm_bytes.len() {
        // Read section ID
        let section_id = wasm_bytes[pos];
        pos += 1;

        // Read section size
        let (section_size, new_pos) = read_leb128_u32(wasm_bytes, pos)?;
        pos = new_pos;

        let section_end = pos + section_size as usize;
        if section_end > wasm_bytes.len() {
            return Err(Error::parse_error("Section size exceeds file bounds"));
        }

        if section_id == CUSTOM_SECTION_ID {
            // Parse custom section name
            let (name_len, name_start) = read_leb128_u32(wasm_bytes, pos)?;

            let name_end = name_start + name_len as usize;
            if name_end > section_end {
                return Err(Error::parse_error(
                    "Custom section name exceeds section bounds",
                ));
            }

            let name = String::from_utf8(wasm_bytes[name_start..name_end].to_vec())
                .map_err(|_| Error::parse_error("Invalid UTF-8 in custom section name"))?;

            let data_start = name_end;
            let data = wasm_bytes[data_start..section_end].to_vec();

            sections.push(CustomSection {
                name,
                data,
                offset: data_start,
            });
        } else if section_id == CORE_MODULE_SECTION_ID && is_component {
            // Component model: parse nested core module for custom sections
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
fn parse_core_module_sections(
    module_bytes: &[u8],
    base_offset: usize,
) -> Result<Vec<CustomSection>> {
    // Core module starts with magic + version (8 bytes)
    if module_bytes.len() < 8 {
        return Err(Error::parse_error("Core module too small"));
    }
    if &module_bytes[0..4] != WASM_MAGIC {
        return Err(Error::parse_error("Invalid core module magic"));
    }
    if &module_bytes[4..8] != WASM_VERSION_CORE {
        return Err(Error::parse_error("Invalid core module version"));
    }

    let mut sections = Vec::new();
    let mut pos = 8;

    while pos < module_bytes.len() {
        let section_id = module_bytes[pos];
        pos += 1;

        let (section_size, new_pos) = read_leb128_u32(module_bytes, pos)?;
        pos = new_pos;

        let section_end = pos + section_size as usize;
        if section_end > module_bytes.len() {
            break; // Section extends beyond module, stop parsing
        }

        if section_id == CUSTOM_SECTION_ID {
            // Parse custom section name
            let (name_len, name_start) = read_leb128_u32(module_bytes, pos)?;

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

/// Source line tracer that wraps DwarfDebugInfo and provides line-by-line output
pub struct SourceLineTracer<'a> {
    /// DWARF debug info parser
    debug_info: wrt_debug::DwarfDebugInfo<'a>,
    /// Last printed file index (to avoid duplicates)
    last_file: u16,
    /// Last printed line (to avoid duplicates)
    last_line: u32,
    /// Whether any debug info is available
    has_debug_info: bool,
}

impl<'a> SourceLineTracer<'a> {
    /// Create a new source line tracer from WASM bytes
    pub fn new(wasm_bytes: &'a [u8]) -> Result<Self> {
        let mut debug_info = wrt_debug::DwarfDebugInfo::new(wasm_bytes)?;

        // Parse custom sections and register DWARF sections
        let sections = parse_custom_sections(wasm_bytes)?;
        let mut dwarf_count = 0;

        for section in &sections {
            if section.name.starts_with(".debug_") {
                debug_info.add_section(
                    &section.name,
                    section.offset as u32,
                    section.data.len() as u32,
                );
                dwarf_count += 1;
            }
        }

        let has_debug_info = dwarf_count > 0;

        if has_debug_info {
            eprintln!("[trace-lines] Loaded {} DWARF sections", dwarf_count);
        } else {
            eprintln!(
                "[trace-lines] Warning: No DWARF debug sections found in binary"
            );
        }

        Ok(Self {
            debug_info,
            last_file: u16::MAX,
            last_line: u32::MAX,
            has_debug_info,
        })
    }

    /// Called for each instruction execution
    /// Returns true if a new source line was printed
    pub fn on_instruction(&mut self, pc: u32) -> bool {
        if !self.has_debug_info {
            return false;
        }

        match self.debug_info.find_line_info(pc) {
            Ok(Some(info)) => {
                // Only print if file or line changed
                if info.file_index != self.last_file || info.line != self.last_line {
                    self.last_file = info.file_index;
                    self.last_line = info.line;

                    // Print location
                    // Note: file_index is an index into the file table; in a full
                    // implementation we'd resolve this to the actual filename
                    if info.is_stmt {
                        eprintln!(
                            "[0x{:04x}] file[{}]:{}:{}",
                            pc, info.file_index, info.line, info.column
                        );
                    }
                    return true;
                }
            }
            Ok(None) => {
                // No line info for this PC - common for generated code
            }
            Err(_) => {
                // Parse error - log once and continue
            }
        }
        false
    }

    /// Check if debug info is available
    pub fn has_debug_info(&self) -> bool {
        self.has_debug_info
    }

    /// Print summary of DWARF sections
    pub fn print_debug_summary(&self, wasm_bytes: &[u8]) {
        if let Ok(sections) = parse_custom_sections(wasm_bytes) {
            let dwarf_sections: Vec<_> = sections
                .iter()
                .filter(|s| s.name.starts_with(".debug_"))
                .collect();

            if !dwarf_sections.is_empty() {
                eprintln!("\n[trace-lines] DWARF Debug Sections:");
                for section in &dwarf_sections {
                    let size_kb = section.data.len() as f64 / 1024.0;
                    eprintln!("  {:20} {:>8} bytes ({:.1} KB)",
                        section.name,
                        section.data.len(),
                        size_kb
                    );
                }
                eprintln!();
            }
        }
    }
}


/// Parsed file entry from DWARF (simplified for storage)
#[derive(Clone, Debug)]
struct FileInfo {
    /// File name
    filename: String,
}

/// Line tracing debugger that implements RuntimeDebugger
///
/// This owns the WASM data and DWARF sections, allowing it to be used
/// as a boxed trait object for the engine's debug callbacks.
/// Profiling mode for the debugger
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DebuggerMode {
    /// Print source locations as execution happens
    Trace,
    /// Collect hit counts and print summary at end
    Profile,
}

pub struct LineTracingDebugger {
    /// Owned WASM bytes
    wasm_bytes: Vec<u8>,
    /// DWARF section offsets and sizes
    dwarf_sections: Vec<(String, u32, u32)>,
    /// File table parsed using wrt_debug
    file_table: Vec<FileInfo>,
    /// Last printed file index
    last_file: std::sync::atomic::AtomicU32,
    /// Last printed line
    last_line: std::sync::atomic::AtomicU32,
    /// Whether debug info is available
    has_debug_info: bool,
    /// Instruction counter for less verbose output
    instruction_count: std::sync::atomic::AtomicU64,
    /// Debugger mode (trace or profile)
    mode: DebuggerMode,
    /// Hit counts per (file_index, line) for profiling mode
    /// Uses a simple approach: file_index << 32 | line as key
    hit_counts: std::collections::HashMap<u64, u64>,
}

impl LineTracingDebugger {
    /// Create a new line tracing debugger from WASM bytes
    pub fn new(wasm_bytes: Vec<u8>) -> Result<Self> {
        let sections = parse_custom_sections(&wasm_bytes)?;

        let mut dwarf_sections = Vec::new();

        for section in &sections {
            if section.name.starts_with(".debug_") {
                dwarf_sections.push((
                    section.name.clone(),
                    section.offset as u32,
                    section.data.len() as u32,
                ));
            }
        }

        let has_debug_info = !dwarf_sections.is_empty();

        // Parse file table using wrt_debug
        let file_table = if has_debug_info {
            let mut debug_info = wrt_debug::DwarfDebugInfo::new(&wasm_bytes)?;
            for (name, offset, size) in &dwarf_sections {
                debug_info.add_section(name, *offset, *size);
            }

            // Parse file table from .debug_line
            if let Err(e) = debug_info.parse_file_table() {
                eprintln!("[trace-lines] Warning: File table parse error: {}", e);
            }

            // Extract filenames (wrt_debug uses 1-based indexing)
            let mut files = Vec::new();
            for i in 1..=debug_info.file_count() {
                if let Some(name) = debug_info.get_filename(i as u16) {
                    files.push(FileInfo { filename: name.to_string() });
                }
            }
            files
        } else {
            Vec::new()
        };

        if has_debug_info {
            eprintln!("[trace-lines] Debugger initialized with {} DWARF sections", dwarf_sections.len());
            if !file_table.is_empty() {
                eprintln!("[trace-lines] File table: {} entries", file_table.len());
                for (i, f) in file_table.iter().take(5).enumerate() {
                    eprintln!("  [{}] {}", i + 1, f.filename);
                }
                if file_table.len() > 5 {
                    eprintln!("  ... and {} more", file_table.len() - 5);
                }
            }
        }

        Ok(Self {
            wasm_bytes,
            dwarf_sections,
            file_table,
            last_file: std::sync::atomic::AtomicU32::new(u32::MAX),
            last_line: std::sync::atomic::AtomicU32::new(u32::MAX),
            has_debug_info,
            instruction_count: std::sync::atomic::AtomicU64::new(0),
            mode: DebuggerMode::Trace,
            hit_counts: std::collections::HashMap::new(),
        })
    }

    /// Create a new debugger in profiling mode
    pub fn new_profiler(wasm_bytes: Vec<u8>) -> Result<Self> {
        let mut debugger = Self::new(wasm_bytes)?;
        debugger.mode = DebuggerMode::Profile;
        Ok(debugger)
    }

    /// Set the debugger mode
    pub fn set_mode(&mut self, mode: DebuggerMode) {
        self.mode = mode;
    }

    /// Record a hit for profiling mode
    fn record_hit(&mut self, file_index: u16, line: u32) {
        let key = ((file_index as u64) << 32) | (line as u64);
        *self.hit_counts.entry(key).or_insert(0) += 1;
    }

    /// Print profiling summary sorted by hit count
    pub fn print_profile_summary(&self) {
        if self.hit_counts.is_empty() {
            eprintln!("=== Execution Profile ===");
            eprintln!("(No source-level hits recorded - DWARF info may be unavailable)");
            return;
        }

        // Collect and sort by hit count (descending)
        let mut hits: Vec<_> = self.hit_counts.iter().collect();
        hits.sort_by(|a, b| b.1.cmp(a.1));

        eprintln!("=== Execution Profile ===");
        eprintln!("{} unique source locations hit", hits.len());
        eprintln!();

        // Print top 20
        for (key, count) in hits.iter().take(20) {
            let file_idx = (*key >> 32) as u16;
            let line = *key as u32;
            let filename = self.get_filename(file_idx).unwrap_or("<unknown>");
            eprintln!("{:>10} hits  {}:{}", count, filename, line);
        }

        if hits.len() > 20 {
            eprintln!("  ... and {} more locations", hits.len() - 20);
        }
    }

    /// Get filename for a file index (1-based as per DWARF spec)
    fn get_filename(&self, file_index: u16) -> Option<&str> {
        if file_index == 0 || file_index as usize > self.file_table.len() {
            return None;
        }
        Some(&self.file_table[file_index as usize - 1].filename)
    }

    /// Check if debug info is available
    pub fn has_debug_info(&self) -> bool {
        self.has_debug_info
    }

    /// Resolve PC to line info and print if changed
    fn trace_instruction(&self, pc: u32) {
        if !self.has_debug_info {
            return;
        }

        // Create a temporary DwarfDebugInfo to resolve line info
        // This is not ideal for performance but works for the debugger use case
        if let Ok(mut debug_info) = wrt_debug::DwarfDebugInfo::new(&self.wasm_bytes) {
            for (name, offset, size) in &self.dwarf_sections {
                debug_info.add_section(name, *offset, *size);
            }

            if let Ok(Some(info)) = debug_info.find_line_info(pc) {
                let last_file = self.last_file.load(std::sync::atomic::Ordering::Relaxed);
                let last_line = self.last_line.load(std::sync::atomic::Ordering::Relaxed);

                // Only print if file or line changed
                if info.file_index as u32 != last_file || info.line != last_line {
                    self.last_file.store(info.file_index as u32, std::sync::atomic::Ordering::Relaxed);
                    self.last_line.store(info.line, std::sync::atomic::Ordering::Relaxed);

                    // Print location with resolved filename
                    if info.is_stmt {
                        let filename = self.get_filename(info.file_index)
                            .unwrap_or("<unknown>");
                        eprintln!(
                            "[0x{:06x}] {}:{}:{}",
                            pc, filename, info.line, info.column
                        );
                    }
                }
            }
        }
    }
}

// Note: LineTracingDebugger is automatically Send + Sync because all its fields
// (Vec<u8>, Vec<(String, u32, u32)>, AtomicU32, AtomicU64, bool) are Send + Sync.

impl RuntimeDebugger for LineTracingDebugger {
    fn on_breakpoint(&mut self, bp: &Breakpoint, state: &dyn RuntimeState) -> DebugAction {
        eprintln!(
            "[BREAKPOINT] ID={:?} at PC=0x{:04x}, func={}",
            bp.id,
            state.pc(),
            state.current_function().unwrap_or(0)
        );
        self.trace_instruction(state.pc());
        DebugAction::Continue
    }

    fn on_instruction(&mut self, pc: u32, _state: &dyn RuntimeState) -> DebugAction {
        let count = self.instruction_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        match self.mode {
            DebuggerMode::Trace => {
                // Always trace, but the trace_instruction method will deduplicate
                self.trace_instruction(pc);

                // Print progress every 10000 instructions
                if count > 0 && count % 10000 == 0 {
                    eprintln!("[trace-lines] {} instructions executed", count);
                }
            }
            DebuggerMode::Profile => {
                // In profile mode, collect hit counts silently
                if self.has_debug_info {
                    if let Ok(mut debug_info) = wrt_debug::DwarfDebugInfo::new(&self.wasm_bytes) {
                        for (name, offset, size) in &self.dwarf_sections {
                            debug_info.add_section(name, *offset, *size);
                        }
                        if let Ok(Some(info)) = debug_info.find_line_info(pc) {
                            self.record_hit(info.file_index, info.line);
                        }
                    }
                }

                // Print progress every 100000 instructions in profile mode (less verbose)
                if count > 0 && count % 100000 == 0 {
                    eprintln!("[profile] {} instructions executed", count);
                }
            }
        }

        DebugAction::Continue
    }

    fn on_function_entry(&mut self, func_idx: u32, state: &dyn RuntimeState) {
        eprintln!("[CALL] Entering function {} at PC=0x{:04x}", func_idx, state.pc());
        self.trace_instruction(state.pc());
    }

    fn on_function_exit(&mut self, func_idx: u32, state: &dyn RuntimeState) {
        eprintln!("[RET] Exiting function {} at PC=0x{:04x}", func_idx, state.pc());
    }

    fn on_trap(&mut self, trap_code: u32, state: &dyn RuntimeState) {
        eprintln!(
            "[TRAP] Code={} at PC=0x{:04x}, func={}",
            trap_code,
            state.pc(),
            state.current_function().unwrap_or(0)
        );
        self.trace_instruction(state.pc());
    }
}

/// Helper to extract DWARF summary without full initialization
pub fn get_dwarf_summary(wasm_bytes: &[u8]) -> Option<DwarfSummary> {
    let sections = parse_custom_sections(wasm_bytes).ok()?;

    let dwarf_sections: Vec<_> = sections
        .iter()
        .filter(|s| s.name.starts_with(".debug_"))
        .collect();

    if dwarf_sections.is_empty() {
        return None;
    }

    let total_size: usize = dwarf_sections.iter().map(|s| s.data.len()).sum();
    let has_line = dwarf_sections.iter().any(|s| s.name == ".debug_line");
    let has_info = dwarf_sections.iter().any(|s| s.name == ".debug_info");

    Some(DwarfSummary {
        section_count: dwarf_sections.len(),
        total_size,
        has_line_info: has_line,
        has_debug_info: has_info,
    })
}

/// Summary of available DWARF debug information
#[derive(Debug)]
pub struct DwarfSummary {
    /// Number of DWARF sections found
    pub section_count: usize,
    /// Total size of all DWARF sections
    pub total_size: usize,
    /// Whether .debug_line section is present
    pub has_line_info: bool,
    /// Whether .debug_info section is present
    pub has_debug_info: bool,
}
