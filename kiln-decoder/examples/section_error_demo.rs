// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

// Example demonstrating the enhanced error handling in section_error.rs

use wrt_decoder::section_error::{
    invalid_magic, invalid_section, invalid_value, malformed_content, missing_section,
    section_size_exceeds_module, unexpected_end, unsupported_version,
};
use wrt_format::binary;

fn main() {
    // Demonstrate different error types
    println!("Enhanced Section Error Handling Demo");
    println!("====================================\n");

    // MissingSection error
    let error = missing_section(1, "Import section required for WASI modules");
    println!("Missing Section Error:\n{}\n", error);

    // InvalidSection error
    let error = invalid_section(2, 0x20, "Invalid count value in import section");
    println!("Invalid Section Error:\n{}\n", error);

    // UnexpectedEnd error
    let error = unexpected_end(0x30, 10, 5);
    println!("Unexpected End Error:\n{}\n", error);

    // MalformedContent error
    let error = malformed_content(0x40, 3, "Invalid function type in function section");
    println!("Malformed Content Error:\n{}\n", error);

    // SectionSizeExceedsModule error
    let error = section_size_exceeds_module(4, 100, 50, 0x50);
    println!("Section Size Exceeds Module Error:\n{}\n", error);

    // InvalidMagic error
    let error = invalid_magic(0, binary::WASM_MAGIC, [0x01, 0x02, 0x03, 0x04]);
    println!("Invalid Magic Error:\n{}\n", error);

    // UnsupportedVersion error
    let error = unsupported_version(4, binary::WASM_VERSION, [0x02, 0x00, 0x00, 0x00]);
    println!("Unsupported Version Error:\n{}\n", error);

    // InvalidValue error
    let error = invalid_value(0x60, 5, "Invalid limit type in memory section");
    println!("Invalid Value Error:\n{}\n", error);

    println!("All error types demonstrated successfully!");
}
