//! A detailed WebAssembly analyzer using wrt ecosystem
//!
//! This example demonstrates how to use wrt-decoder, wrt-component, and
//! wrt-format to analyze WebAssembly modules and components in depth.

use std::{env, fs, path::Path, str};

use wrt_decoder::{
    Module,
    component::{
        ComponentAnalyzer, ComponentSummary, ExtendedExportInfo, ExtendedImportInfo,
        ModuleExportInfo, ModuleImportInfo, analyze_component, analyze_component_extended,
        extract_embedded_modules, is_valid_module,
    },
};
use wrt_error::Result;
use wrt_format::{binary, wasmparser::SectionId};

/// Displays a hexadecimal dump of a portion of a binary slice.
fn hex_dump(data: &[u8], offset: usize, len: usize) {
    let end = std::cmp::min(offset + len, data.len());
    let chunk = &data[offset..end];

    for (i, bytes) in chunk.chunks(16).enumerate() {
        let addr = offset + i * 16;
        print!("{:08x}:  ", addr);

        // Print hex values
        for (j, byte) in bytes.iter().enumerate() {
            print!("{:02x} ", byte);
            if j == 7 {
                print!(" ");
            }
        }

        // Padding for last row if needed
        for _ in bytes.len()..16 {
            print!("   ");
        }
        if bytes.len() <= 8 {
            print!(" ");
        }

        // Print ASCII representation
        print!(" |");
        for byte in bytes {
            if *byte >= 32 && *byte <= 126 {
                print!("{}", *byte as char);
            } else {
                print!(".");
            }
        }
        for _ in bytes.len()..16 {
            print!(" ");
        }
        println!("|");
    }
}

/// Analyzes a WebAssembly module to extract information.
fn analyze_module(binary: &[u8]) -> Result<()> {
    println!("\n=== Module Analysis ===");

    if !is_valid_module(binary) {
        println!("Not a valid WebAssembly module");
        return Ok();
    }

    let mut offset = 8; // Skip magic and version

    // Read and analyze each section
    while offset < binary.len() {
        let section_id = binary[offset];
        offset += 1;

        let (section_size, bytes_read) = binary::read_leb128_u32(binary, offset)?;
        offset += bytes_read;

        let section_end = offset + section_size as usize;

        // Skip if we've reached the end of the binary
        if section_end > binary.len() {
            break;
        }

        let section_name = match section_id {
            0 => "Custom",
            1 => "Type",
            2 => "Import",
            3 => "Function",
            4 => "Table",
            5 => "Memory",
            6 => "Global",
            7 => "Export",
            8 => "Start",
            9 => "Element",
            10 => "Code",
            11 => "Data",
            12 => "DataCount",
            _ => "Unknown",
        };

        println!(
            "Section {}: {} (size: {})",
            section_id, section_name, section_size
        );

        // Analyze custom section
        if section_id == 0 {
            let (name, bytes_read) = binary::read_string(binary, offset)?;
            println!("  Name: \"{}\"", name);

            if name == "name" {
                // Name section: extract function names, etc.
                let name_section_offset = offset + bytes_read;
                let name_section_size = section_size as usize - bytes_read;

                // Name section will be analyzed separately in
                // parse_name_section
            }
        }

        // For import section, count and list imports
        if section_id == 2 {
            let mut import_offset = offset;
            let (count, bytes_read) = binary::read_leb128_u32(binary, import_offset)?;
            import_offset += bytes_read;

            println!("  Imports: {}", count);

            for i in 0..count {
                if import_offset >= section_end {
                    break;
                }

                let (module, bytes_read) = binary::read_string(binary, import_offset)?;
                import_offset += bytes_read;

                let (name, bytes_read) = binary::read_string(binary, import_offset)?;
                import_offset += bytes_read;

                let kind = binary[import_offset];
                import_offset += 1;

                let kind_name = match kind {
                    0 => "Function",
                    1 => "Table",
                    2 => "Memory",
                    3 => "Global",
                    _ => "Unknown",
                };

                // Skip the kind-specific details
                match kind {
                    0 => {
                        // Function import
                        let (_, bytes_read) = binary::read_leb128_u32(binary, import_offset)?;
                        import_offset += bytes_read;
                    },
                    1 => {
                        // Table import
                        import_offset += 1; // element type
                        if binary[import_offset] & 0x01 == 0 {
                            // No max
                            import_offset += 1 + binary::leb128_size(binary, import_offset + 1)?;
                        } else {
                            // Has max
                            import_offset += 1
                                + binary::leb128_size(binary, import_offset + 1)?
                                + binary::leb128_size(
                                    binary,
                                    import_offset
                                        + 1
                                        + binary::leb128_size(binary, import_offset + 1)?,
                                )?;
                        }
                    },
                    2 => {
                        // Memory import
                        if binary[import_offset] & 0x01 == 0 {
                            // No max
                            import_offset += 1 + binary::leb128_size(binary, import_offset + 1)?;
                        } else {
                            // Has max
                            import_offset += 1
                                + binary::leb128_size(binary, import_offset + 1)?
                                + binary::leb128_size(
                                    binary,
                                    import_offset
                                        + 1
                                        + binary::leb128_size(binary, import_offset + 1)?,
                                )?;
                        }
                    },
                    3 => {
                        // Global import
                        import_offset += 1; // value type
                        import_offset += 1; // mutability
                    },
                    _ => {
                        // Unknown kind, can't parse further
                        break;
                    },
                }

                println!("    Import {}: {}.{} ({})", i, module, name, kind_name);
            }
        }

        // For export section, count and list exports
        if section_id == 7 {
            let mut export_offset = offset;
            let (count, bytes_read) = binary::read_leb128_u32(binary, export_offset)?;
            export_offset += bytes_read;

            println!("  Exports: {}", count);

            for i in 0..count {
                if export_offset >= section_end {
                    break;
                }

                let (name, bytes_read) = binary::read_string(binary, export_offset)?;
                export_offset += bytes_read;

                let kind = binary[export_offset];
                export_offset += 1;

                let (idx, bytes_read) = binary::read_leb128_u32(binary, export_offset)?;
                export_offset += bytes_read;

                let kind_name = match kind {
                    0 => "Function",
                    1 => "Table",
                    2 => "Memory",
                    3 => "Global",
                    _ => "Unknown",
                };

                println!("    Export {}: {} ({} {})", i, name, kind_name, idx);
            }
        }

        offset = section_end;
    }

    Ok(())
}

/// Parses and displays information from the name section of a WebAssembly
/// module.
fn parse_name_section(module: &Module) -> Result<()> {
    println!("\n=== Name Section Analysis ===");

    let mut found_name_section = false;

    // Iterate through custom sections looking for the name section
    for (section_id, section_data) in module.custom_sections() {
        if section_id == "name" {
            found_name_section = true;

            // Parse name subsections
            let data = section_data.as_slice();
            let mut offset = 0;

            while offset < data.len() {
                if offset + 1 > data.len() {
                    break;
                }

                let name_type = data[offset];
                offset += 1;

                if offset >= data.len() {
                    break;
                }

                let (subsection_size, bytes_read) = binary::read_leb128_u32(data, offset)?;
                offset += bytes_read;

                let subsection_end = offset + subsection_size as usize;
                if subsection_end > data.len() {
                    break;
                }

                match name_type {
                    0 => {
                        // Module name
                        let (name, _) = binary::read_string(data, offset)?;
                        println!("Module name: {}", name);
                    },
                    1 => {
                        // Function names
                        let (count, bytes_read) = binary::read_leb128_u32(data, offset)?;
                        let mut name_offset = offset + bytes_read;

                        println!("Function names: {}", count);

                        for _ in 0..count {
                            if name_offset >= subsection_end {
                                break;
                            }

                            let (index, bytes_read) = binary::read_leb128_u32(data, name_offset)?;
                            name_offset += bytes_read;

                            if name_offset >= subsection_end {
                                break;
                            }

                            let (name, bytes_read) = binary::read_string(data, name_offset)?;
                            name_offset += bytes_read;

                            println!("  Function {}: {}", index, name);
                        }
                    },
                    2 => {
                        // Local names
                        let (count, bytes_read) = binary::read_leb128_u32(data, offset)?;
                        let mut func_offset = offset + bytes_read;

                        println!("Local names in {} functions", count);

                        for _ in 0..count {
                            if func_offset >= subsection_end {
                                break;
                            }

                            let (func_index, bytes_read) =
                                binary::read_leb128_u32(data, func_offset)?;
                            func_offset += bytes_read;

                            if func_offset >= subsection_end {
                                break;
                            }

                            let (local_count, bytes_read) =
                                binary::read_leb128_u32(data, func_offset)?;
                            func_offset += bytes_read;

                            println!("  Function {}: {} locals", func_index, local_count);

                            for _ in 0..local_count {
                                if func_offset >= subsection_end {
                                    break;
                                }

                                let (local_index, bytes_read) =
                                    binary::read_leb128_u32(data, func_offset)?;
                                func_offset += bytes_read;

                                if func_offset >= subsection_end {
                                    break;
                                }

                                let (name, bytes_read) = binary::read_string(data, func_offset)?;
                                func_offset += bytes_read;

                                println!("    Local {}: {}", local_index, name);
                            }
                        }
                    },
                    _ => {
                        println!("Unknown name subsection type: {}", name_type);
                    },
                }

                offset = subsection_end;
            }
        }
    }

    if !found_name_section {
        println!("No name section found in the module");
    }

    Ok(())
}

/// Analyzes memory usage in a WebAssembly module.
fn analyze_memory_usage(module: &Module) -> Result<()> {
    println!("\n=== Memory Usage Analysis ===");

    // Check for memory definitions
    let memory_count = module.memories().len();
    println!("Memory definitions: {}", memory_count);

    for (i, memory) in module.memories().iter().enumerate() {
        println!(
            "  Memory {}: min={} pages ({} bytes)",
            i,
            memory.minimum,
            memory.minimum as usize * 65536
        );

        if let Some(max) = memory.maximum {
            println!("    maximum={} pages ({} bytes)", max, max as usize * 65536);
        } else {
            println!("    no maximum specified");
        }
    }

    // Check for data sections
    let data_segments = module.data_sections();
    println!("Data segments: {}", data_segments.len());

    for (i, data) in data_segments.iter().enumerate() {
        println!(
            "  Data {}: memory={}, size={} bytes",
            i,
            data.memory_index,
            data.data.len()
        );

        match &data.offset {
            wrt_decoder::DataSegmentOffset::Active(expr) => {
                println!("    mode: active, offset expression: {:?}", expr);
            },
            wrt_decoder::DataSegmentOffset::Passive => {
                println!("    mode: passive");
            },
        }

        // Print a short preview of the data content
        if !data.data.is_empty() {
            let preview_len = std::cmp::min(16, data.data.len());
            let preview = &data.data[0..preview_len];

            print!("    data (first {} bytes): ", preview_len);
            for byte in preview {
                print!("{:02x} ", byte);
            }
            println!();

            // Try to interpret as ASCII if it looks like text
            if preview.iter().all(|&b| b >= 32 && b <= 126) {
                if let Ok(text) = str::from_utf8(preview) {
                    println!("    as text: \"{}\"", text);
                }
            }
        }
    }

    Ok(())
}

/// Analyzes a WebAssembly Component.
fn analyze_component(binary: &[u8]) -> Result<()> {
    println!("\n=== Component Analysis ===");

    // Use the built-in component analyzer
    let summary = analyze_component(binary)?;
    println!("Core modules: {}", summary.core_modules_count);
    println!("Core instances: {}", summary.core_instances_count);
    println!("Imports: {}", summary.imports_count);
    println!("Exports: {}", summary.exports_count);
    println!("Aliases: {}", summary.aliases_count);

    // Get extended information
    let (_, imports, exports, module_imports, module_exports) = analyze_component_extended(binary)?;

    // Display component imports
    if !imports.is_empty() {
        println!("\nComponent Imports:");
        for import in &imports {
            println!("  {}.{}: {}", import.namespace, import.name, import.kind);
        }
    }

    // Display component exports
    if !exports.is_empty() {
        println!("\nComponent Exports:");
        for export in &exports {
            println!(
                "  {}: {} (index: {})",
                export.name, export.kind, export.index
            );
        }
    }

    // Display module imports
    if !module_imports.is_empty() {
        println!("\nModule Imports:");
        for import in &module_imports {
            println!(
                "  Module {}: {}.{} ({}, index: {})",
                import.module_idx, import.module, import.name, import.kind, import.index
            );
        }
    }

    // Display module exports
    if !module_exports.is_empty() {
        println!("\nModule Exports:");
        for export in &module_exports {
            println!(
                "  Module {}: {} ({}, index: {})",
                export.module_idx, export.name, export.kind, export.index
            );
        }
    }

    Ok(())
}

/// Analyzes the binary format of a WebAssembly file.
fn analyze_binary_format(binary: &[u8]) -> Result<()> {
    println!("\n=== Binary Format Analysis ===");

    // Check if file is long enough for header
    if binary.len() < 8 {
        println!("File too short to be a valid WebAssembly binary");
        return Ok();
    }

    // Check magic bytes
    let magic = &binary[0..4];
    let version = u32::from_le_bytes([binary[4], binary[5], binary[6], binary[7]]);

    println!(
        "Magic bytes: {:02x} {:02x} {:02x} {:02x}",
        magic[0], magic[1], magic[2], magic[3]
    );
    println!("Version: {}", version);

    // Identify binary type
    if magic == b"\0asm" {
        println!("File type: WebAssembly Module");
    } else if magic == b"\0age" {
        println!("File type: WebAssembly Component");
    } else {
        println!("File type: Unknown (not a valid WebAssembly binary)");
        return Ok();
    }

    // Count and identify sections
    let mut offset = 8; // Skip magic and version
    let mut section_counts = std::collections::HashMap::new();

    // Module section names
    let module_section_names = [
        "Custom",
        "Type",
        "Import",
        "Function",
        "Table",
        "Memory",
        "Global",
        "Export",
        "Start",
        "Element",
        "Code",
        "Data",
        "DataCount",
    ];

    // Component section names
    let component_section_names = [
        "Custom",
        "Component Type",
        "Import",
        "Core Module",
        "Instance",
        "Canonical Function",
        "Component",
        "Instance Export",
        "Alias",
        "Type",
        "Core Type",
        "Component Import",
        "Outer Alias",
        "Core Instance",
        "Export",
        "Start",
    ];

    while offset < binary.len() {
        let section_id = binary[offset];
        offset += 1;

        let (section_size, bytes_read) = binary::read_leb128_u32(binary, offset)?;
        offset += bytes_read;

        let section_end = offset + section_size as usize;

        // Skip if we've reached the end of the binary
        if section_end > binary.len() {
            break;
        }

        // Count this section type
        *section_counts.entry(section_id).or_insert(0) += 1;

        // Move to the next section
        offset = section_end;
    }

    // Print section counts
    println!("\nSection counts:");
    let section_names =
        if magic == b"\0asm" { &module_section_names } else { &component_section_names };

    for (id, count) in section_counts.iter() {
        let name = if *id < section_names.len() as u8 {
            section_names[*id as usize]
        } else {
            "Unknown"
        };

        println!("  Section {}: {} (count: {})", id, name, count);
    }

    Ok(())
}

fn main() -> Result<()> {
    // Get the file path from command-line argument
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <path-to-wasm-file>", args[0]);
        return Ok();
    }

    let path = &args[1];
    println!("Loading file: {}", path);

    // Read the file
    let binary = fs::read(Path::new(path))?;

    // Print hexdump of the file header
    println!("\n=== File Header Hexdump ===");
    hex_dump(&binary, 0, 64);

    // Analyze binary format using wrt-format
    analyze_binary_format(&binary)?;

    // Check if it's a component
    let is_component = wrt_decoder::component::utils::is_component(&binary).unwrap_or(false);

    if is_component {
        // Analyze as a component
        analyze_component(&binary)?;

        // Extract and analyze the first embedded module
        if let Ok(modules) = wrt_decoder::component::extract_embedded_modules(&binary) {
            if !modules.is_empty() {
                let module_binary = &modules[0];

                // Analyze the module
                if let Ok(module) = wrt_decoder::decode(module_binary) {
                    analyze_module(module_binary)?;
                    parse_name_section(&module)?;
                    analyze_memory_usage(&module)?;
                }
            }
        }
    } else {
        // Try to analyze as a module
        match wrt_decoder::decode(&binary) {
            Ok(module) => {
                analyze_module(&binary)?;
                parse_name_section(&module)?;
                analyze_memory_usage(&module)?;
            },
            Err(e) => {
                println!("\nError: Failed to decode as WebAssembly module: {}", e);
            },
        }
    }

    Ok(())
}
