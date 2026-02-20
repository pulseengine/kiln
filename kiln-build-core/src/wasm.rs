//! WebAssembly module verification and analysis
//!
//! This module provides functionality for validating and analyzing WebAssembly
//! binary modules, including format verification, import/export analysis, and
//! performance benchmarking.

use std::{path::Path, time::Instant};

use colored::Colorize;

use crate::{
    diagnostics::{Diagnostic, DiagnosticCollection, Position, Range, Severity},
    error::{BuildError, BuildResult},
};

/// WebAssembly module verification results
#[derive(Debug, serde::Serialize)]
pub struct WasmVerificationResult {
    /// Whether the module is valid
    pub valid: bool,
    /// Module format version
    pub version: u32,
    /// Number of sections
    pub section_count: usize,
    /// Module imports
    pub imports: Vec<WasmImport>,
    /// Module exports
    pub exports: Vec<WasmExport>,
    /// Builtin imports (wasi_builtin)
    pub builtin_imports: Vec<String>,
    /// Verification errors
    pub errors: Vec<String>,
    /// Performance metrics
    pub performance: Option<PerformanceMetrics>,
}

/// WebAssembly import information
#[derive(Debug, Clone, serde::Serialize)]
pub struct WasmImport {
    /// Module name
    pub module: String,
    /// Import name
    pub name: String,
    /// Import kind (function, table, memory, global)
    pub kind: String,
}

/// WebAssembly export information
#[derive(Debug, Clone, serde::Serialize)]
pub struct WasmExport {
    /// Export name
    pub name: String,
    /// Export kind (function, table, memory, global)
    pub kind: String,
}

/// Performance metrics for WebAssembly module parsing
#[derive(Debug, serde::Serialize)]
pub struct PerformanceMetrics {
    /// Time to parse the module (milliseconds)
    pub parse_time_ms: u128,
    /// Module size in bytes
    pub module_size: usize,
    /// Parsing throughput (MB/s)
    pub throughput_mbps: f64,
}

/// WebAssembly module verifier
pub struct WasmVerifier {
    /// Path to the WebAssembly module
    module_path: std::path::PathBuf,
}

impl WasmVerifier {
    /// Create a new WebAssembly verifier
    pub fn new(module_path: impl AsRef<Path>) -> Self {
        Self {
            module_path: module_path.as_ref().to_path_buf(),
        }
    }

    /// Verify the WebAssembly module
    pub fn verify(&self) -> BuildResult<WasmVerificationResult> {
        // Read the module
        let module_bytes = std::fs::read(&self.module_path).map_err(|e| {
            BuildError::Verification(format!(
                "Failed to read WebAssembly module {}: {}",
                self.module_path.display(),
                e
            ))
        })?;

        let start_time = Instant::now();

        // Parse the module using wrt-decoder unified loader
        let mut imports = Vec::new();
        let mut exports = Vec::new();
        let mut builtin_imports = Vec::new();
        let mut errors = Vec::new();
        let section_count = 0; // Will be updated when we have section counting
        let version = 1; // WebAssembly version 1

        match wrt_decoder::load_wasm_unified(&module_bytes) {
            Ok(wasm_info) => {
                match &wasm_info.module_info {
                    Some(module_info) => {
                        // Convert imports
                        for import in &module_info.imports {
                            let wasm_import = WasmImport {
                                module: import.module.clone(),
                                name: import.name.clone(),
                                kind: format!("{:?}", import.import_type),
                            };

                            if import.module == "wasi_builtin" {
                                builtin_imports.push(import.name.clone());
                            }

                            imports.push(wasm_import);
                        }

                        // Convert exports
                        for export in &module_info.exports {
                            exports.push(WasmExport {
                                name: export.name.clone(),
                                kind: format!("{:?}", export.export_type),
                            });
                        }
                    },
                    None => {
                        errors.push("Failed to parse module information".to_string());
                    },
                }
            },
            Err(e) => {
                errors.push(format!("Failed to parse WebAssembly module: {}", e));
            },
        }

        let parse_time = start_time.elapsed();
        let parse_time_ms = parse_time.as_millis();
        let module_size = module_bytes.len();
        let throughput_mbps = (module_size as f64 / 1_048_576.0) / parse_time.as_secs_f64();

        Ok(WasmVerificationResult {
            valid: errors.is_empty(),
            version,
            section_count,
            imports,
            exports,
            builtin_imports,
            errors,
            performance: Some(PerformanceMetrics {
                parse_time_ms,
                module_size,
                throughput_mbps,
            }),
        })
    }

    /// Convert verification results to diagnostics
    pub fn to_diagnostics(&self, result: &WasmVerificationResult) -> DiagnosticCollection {
        let mut diagnostics = DiagnosticCollection::new(
            self.module_path.parent().unwrap_or(&self.module_path).to_path_buf(),
            "wasm-verify".to_string(),
        );

        for (i, error) in result.errors.iter().enumerate() {
            let diagnostic = Diagnostic::new(
                self.module_path.to_string_lossy().to_string(),
                Range::entire_line(i as u32),
                Severity::Error,
                error.clone(),
                "wasm-verifier".to_string(),
            );
            diagnostics.diagnostics.push(diagnostic);
        }

        diagnostics
    }

    /// Print human-readable verification results
    pub fn print_results(&self, result: &WasmVerificationResult) {
        if result.valid {
            println!("{} WebAssembly module is valid", "✅".bright_green());
        } else {
            println!("{} WebAssembly module validation failed", "❌".bright_red());
        }

        println!("\n📊 Module Information:");
        println!("  Version: {}", result.version);
        println!("  Sections: {}", result.section_count);

        if !result.imports.is_empty() {
            println!("\n📥 Imports ({}):", result.imports.len());
            for import in &result.imports {
                println!("  - {}::{} ({})", import.module, import.name, import.kind);
            }
        }

        if !result.exports.is_empty() {
            println!("\n📤 Exports ({}):", result.exports.len());
            for export in &result.exports {
                println!("  - {} ({})", export.name, export.kind);
            }
        }

        if !result.builtin_imports.is_empty() {
            println!("\n🔧 Builtin Imports:");
            for builtin in &result.builtin_imports {
                println!("  - wasi_builtin::{}", builtin);
            }
        }

        if let Some(perf) = &result.performance {
            println!("\n⚡ Performance:");
            println!("  Parse time: {}ms", perf.parse_time_ms);
            println!("  Module size: {} bytes", perf.module_size);
            println!("  Throughput: {:.2} MB/s", perf.throughput_mbps);
        }

        if !result.errors.is_empty() {
            println!("\n❌ Errors:");
            for error in &result.errors {
                println!("  - {}", error.bright_red());
            }
        }
    }
}

/// Scan a WebAssembly module for builtin imports
pub fn scan_for_builtins(module_path: impl AsRef<Path>) -> BuildResult<Vec<String>> {
    let verifier = WasmVerifier::new(module_path);
    let result = verifier.verify()?;
    Ok(result.builtin_imports)
}

/// Verify multiple WebAssembly modules
pub fn verify_modules(
    module_paths: &[impl AsRef<Path>],
) -> BuildResult<Vec<(String, WasmVerificationResult)>> {
    let mut results = Vec::new();

    for path in module_paths {
        let path_ref = path.as_ref();
        let verifier = WasmVerifier::new(path_ref);
        let result = verifier.verify()?;
        results.push((path_ref.to_string_lossy().to_string(), result));
    }

    Ok(results)
}

/// Create a minimal test WebAssembly module
pub fn create_minimal_module() -> Vec<u8> {
    // WebAssembly module header
    let mut module = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

    // Type section (empty)
    module.extend_from_slice(&[0x01, 0x04, 0x01, 0x60, 0x00, 0x00]);

    // Import section with wasi_builtin.random
    module.extend_from_slice(&[
        0x02, 0x17, // Import section ID and size (23 bytes)
        0x01, // Number of imports
        0x0C, // Module name length
        // "wasi_builtin"
        0x77, 0x61, 0x73, 0x69, 0x5F, 0x62, 0x75, 0x69, 0x6C, 0x74, 0x69, 0x6E,
        0x06, // Field name length
        // "random"
        0x72, 0x61, 0x6E, 0x64, 0x6F, 0x6D, 0x00, // Import kind (function)
        0x00, // Type index
    ]);

    module
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn test_minimal_module_verification() {
        let module = create_minimal_module();

        // Write to temporary file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&module).unwrap();

        let verifier = WasmVerifier::new(temp_file.path());
        let result = verifier.verify().unwrap();

        assert!(result.valid);
        assert_eq!(result.version, 1);
        assert_eq!(result.builtin_imports.len(), 1);
        assert_eq!(result.builtin_imports[0], "random");
    }

    #[test]
    fn test_invalid_module() {
        let invalid_module = vec![0xFF, 0xFF, 0xFF, 0xFF];

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&invalid_module).unwrap();

        let verifier = WasmVerifier::new(temp_file.path());
        let result = verifier.verify().unwrap();

        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }
}
