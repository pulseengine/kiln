//! Shared decoded structure for caching parsing results
//!
//! This module provides a caching system to avoid redundant parsing of WASM
//! sections when the same binary is accessed multiple times or by different
//! components of the system.

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap as HashMap, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::HashMap;

use crate::{
    prelude::*,
    unified_loader::{ExportInfo, ImportInfo, WasmFormat},
};

/// Cached section data to avoid re-parsing
#[derive(Debug, Clone)]
pub enum SectionData {
    /// Type section data
    Types(Vec<String>),
    /// Import section data
    Imports(Vec<ImportInfo>),
    /// Function section data (type indices)
    Functions(Vec<u32>),
    /// Table section data
    Tables(Vec<String>),
    /// Memory section data
    Memories(Vec<(u32, Option<u32>)>), // (min, max) pages
    /// Global section data
    Globals(Vec<String>),
    /// Export section data
    Exports(Vec<ExportInfo>),
    /// Start section data
    Start(u32),
    /// Element section data
    Elements(Vec<String>),
    /// Code section data (function bodies)
    Code(Vec<Vec<u8>>),
    /// Data section data
    Data(Vec<Vec<u8>>),
    /// Custom section data
    Custom { name: String, data: Vec<u8> },
}

/// Decoded module cache to store parsed section information
#[derive(Debug, Clone)]
pub struct DecodedCache {
    /// WASM format type
    pub format_type: WasmFormat,
    /// Raw binary size
    pub binary_size: usize,
    /// Parsed section data by section ID
    pub sections: HashMap<u8, SectionData>,
    /// Cached import section for quick builtin scanning
    pub import_cache: Option<Vec<ImportInfo>>,
    /// Cached export section for quick lookups
    pub export_cache: Option<Vec<ExportInfo>>,
    /// Builtin imports extracted from import section
    pub builtin_imports: Option<Vec<String>>,
}

impl DecodedCache {
    /// Create a new empty cache
    pub fn new(format_type: WasmFormat, binary_size: usize) -> Self {
        Self {
            format_type,
            binary_size,
            sections: HashMap::new(),
            import_cache: None,
            export_cache: None,
            builtin_imports: None,
        }
    }

    /// Get cached imports, parsing if not cached
    ///
    /// # Panics
    ///
    /// This function will not panic as `unwrap()` is called on a value that was
    /// just set to `Some`.
    pub fn get_imports(&mut self, binary: &[u8]) -> Result<&Vec<ImportInfo>> {
        if self.import_cache.is_none() {
            let imports = parse_imports_from_binary(binary)?;
            self.import_cache = Some(imports);

            // Also cache as section data
            if let Some(ref imports) = self.import_cache {
                self.sections.insert(2, SectionData::Imports(imports.clone()));
            }
        }

        Ok(self.import_cache.as_ref().unwrap())
    }

    /// Get cached exports, parsing if not cached
    ///
    /// # Panics
    ///
    /// This function will not panic as `unwrap()` is called on a value that was
    /// just set to `Some`.
    pub fn get_exports(&mut self, binary: &[u8]) -> Result<&Vec<ExportInfo>> {
        if self.export_cache.is_none() {
            let exports = parse_exports_from_binary(binary)?;
            self.export_cache = Some(exports);

            // Also cache as section data
            if let Some(ref exports) = self.export_cache {
                self.sections.insert(7, SectionData::Exports(exports.clone()));
            }
        }

        Ok(self.export_cache.as_ref().unwrap())
    }

    /// Get cached builtin imports, parsing if not cached
    ///
    /// # Panics
    ///
    /// This function will not panic as `unwrap()` is called on a value that was
    /// just set to `Some`.
    pub fn get_builtin_imports(&mut self, binary: &[u8]) -> Result<&Vec<String>> {
        if self.builtin_imports.is_none() {
            let imports = self.get_imports(binary)?;
            let builtins: Vec<String> = imports
                .iter()
                .filter(|import| import.module == "wasi_builtin")
                .map(|import| import.name.clone())
                .collect();
            self.builtin_imports = Some(builtins);
        }

        Ok(self.builtin_imports.as_ref().unwrap())
    }

    /// Check if a section is cached
    pub fn has_section(&self, section_id: u8) -> bool {
        self.sections.contains_key(&section_id)
    }

    /// Get cached section data
    pub fn get_section(&self, section_id: u8) -> Option<&SectionData> {
        self.sections.get(&section_id)
    }

    /// Cache section data
    pub fn cache_section(&mut self, section_id: u8, data: SectionData) {
        self.sections.insert(section_id, data);
    }

    /// Get memory requirements from cached data
    pub fn get_memory_info(&self) -> Option<(u32, Option<u32>)> {
        if let Some(SectionData::Memories(memories)) = self.sections.get(&5) {
            memories.first().copied()
        } else {
            None
        }
    }

    /// Get start function from cached data
    pub fn get_start_function(&self) -> Option<u32> {
        if let Some(SectionData::Start(start)) = self.sections.get(&8) {
            Some(*start)
        } else {
            None
        }
    }

    /// Get function count from cached data
    pub fn get_function_count(&self) -> usize {
        if let Some(SectionData::Functions(functions)) = self.sections.get(&3) {
            functions.len()
        } else {
            0
        }
    }

    /// Clear all cached data
    pub fn clear(&mut self) {
        self.sections.clear();
        self.import_cache = None;
        self.export_cache = None;
        self.builtin_imports = None;
    }

    /// Get cache memory usage estimate
    pub fn cache_size_estimate(&self) -> usize {
        let mut size = core::mem::size_of::<Self>();

        // Estimate section data size
        for section in self.sections.values() {
            size += match section {
                SectionData::Types(types) => types.len() * 50, // Rough estimate
                SectionData::Imports(imports) => imports.len() * 100,
                SectionData::Functions(functions) => functions.len() * 4,
                SectionData::Tables(tables) => tables.len() * 50,
                SectionData::Memories(memories) => memories.len() * 8,
                SectionData::Globals(globals) => globals.len() * 50,
                SectionData::Exports(exports) => exports.len() * 100,
                SectionData::Start(_) => 4,
                SectionData::Elements(elements) => elements.len() * 50,
                SectionData::Code(code) => code.iter().map(|c| c.len()).sum(),
                SectionData::Data(data) => data.iter().map(|d| d.len()).sum(),
                SectionData::Custom { name, data } => name.len() + data.len(),
            };
        }

        // Add cache data
        if let Some(ref imports) = self.import_cache {
            size += imports.len() * 100;
        }
        if let Some(ref exports) = self.export_cache {
            size += exports.len() * 100;
        }
        if let Some(ref builtins) = self.builtin_imports {
            size += builtins.len() * 50;
        }

        size
    }
}

/// Cache manager for multiple WASM binaries
#[derive(Debug)]
pub struct CacheManager {
    /// Cache entries by binary hash
    #[cfg(feature = "std")]
    caches: HashMap<u64, DecodedCache>,
    #[cfg(not(feature = "std"))]
    caches: HashMap<u64, DecodedCache>,
    /// Maximum cache size in bytes
    max_cache_size: usize,
    /// Current cache size estimate
    current_cache_size: usize,
}

impl CacheManager {
    /// Create a new cache manager
    pub fn new(max_cache_size: usize) -> Self {
        Self {
            caches: HashMap::new(),
            max_cache_size,
            current_cache_size: 0,
        }
    }

    /// Get or create cache for a binary
    ///
    /// # Panics
    ///
    /// This function will not panic as `unwrap()` is called on a value that was
    /// just inserted into the hashmap.
    pub fn get_cache(&mut self, binary: &[u8]) -> Result<&mut DecodedCache> {
        let hash = calculate_hash(binary);

        if !self.caches.contains_key(&hash) {
            // Detect format
            let format_type = if binary.len() >= 8 && &binary[0..4] == b"\0asm" {
                let version = u32::from_le_bytes([binary[4], binary[5], binary[6], binary[7]]);
                if version == 1 { WasmFormat::CoreModule } else { WasmFormat::Component }
            } else {
                WasmFormat::Unknown
            };

            let cache = DecodedCache::new(format_type, binary.len());
            let cache_size = cache.cache_size_estimate();

            // Check if we need to evict entries
            while self.current_cache_size + cache_size > self.max_cache_size
                && !self.caches.is_empty()
            {
                self.evict_lru_entry();
            }

            self.current_cache_size += cache_size;
            self.caches.insert(hash, cache);
        }

        Ok(self.caches.get_mut(&hash).unwrap())
    }

    /// Clear all caches
    pub fn clear(&mut self) {
        self.caches.clear();
        self.current_cache_size = 0;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            cache_count: self.caches.len(),
            total_size: self.current_cache_size,
            max_size: self.max_cache_size,
        }
    }

    /// Evict least recently used entry (simplified implementation)
    fn evict_lru_entry(&mut self) {
        // Simple implementation: remove first entry
        // In a real implementation, we'd track access times
        if let Some((&key, _)) = self.caches.iter().next() {
            if let Some(cache) = self.caches.remove(&key) {
                self.current_cache_size =
                    self.current_cache_size.saturating_sub(cache.cache_size_estimate());
            }
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of cached entries
    pub cache_count: usize,
    /// Total cache size in bytes
    pub total_size: usize,
    /// Maximum cache size in bytes
    pub max_size: usize,
}

/// Create a default cache manager for use in applications
pub fn create_default_cache() -> CacheManager {
    CacheManager::new(1024 * 1024) // 1MB default
}

/// Create a cache manager with custom size
pub fn create_cache_with_size(max_size: usize) -> CacheManager {
    CacheManager::new(max_size)
}

/// Calculate simple hash for binary data
fn calculate_hash(data: &[u8]) -> u64 {
    // Simple FNV-1a hash implementation
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Parse imports from binary without full module parsing
fn parse_imports_from_binary(binary: &[u8]) -> Result<Vec<ImportInfo>> {
    use crate::unified_loader::parse_import_section_info;

    let mut offset = 8; // Skip header
    let mut imports = Vec::new();

    // Find import section
    while offset < binary.len() {
        if offset + 1 >= binary.len() {
            break;
        }

        let section_id = binary[offset];
        offset += 1;

        let (section_size, bytes_read) = read_leb128_u32(binary, offset)?;
        offset += bytes_read;

        let section_end = offset + section_size as usize;
        if section_end > binary.len() {
            return Err(Error::parse_error("Section extends beyond binary"));
        }

        if section_id == 2 {
            // Import section
            let section_data = &binary[offset..section_end];
            let mut dummy_info = crate::unified_loader::ModuleInfo {
                function_types: Vec::new(),
                imports: Vec::new(),
                exports: Vec::new(),
                memory_pages: None,
                start_function: None,
            };
            parse_import_section_info(section_data, &mut dummy_info)?;
            imports = dummy_info.imports;
            break;
        }

        offset = section_end;
    }

    Ok(imports)
}

/// Parse exports from binary without full module parsing
fn parse_exports_from_binary(binary: &[u8]) -> Result<Vec<ExportInfo>> {
    use crate::unified_loader::parse_export_section_info;

    let mut offset = 8; // Skip header
    let mut exports = Vec::new();

    // Find export section
    while offset < binary.len() {
        if offset + 1 >= binary.len() {
            break;
        }

        let section_id = binary[offset];
        offset += 1;

        let (section_size, bytes_read) = read_leb128_u32(binary, offset)?;
        offset += bytes_read;

        let section_end = offset + section_size as usize;
        if section_end > binary.len() {
            return Err(Error::parse_error("Section extends beyond binary"));
        }

        if section_id == 7 {
            // Export section
            let section_data = &binary[offset..section_end];
            let mut dummy_info = crate::unified_loader::ModuleInfo {
                function_types: Vec::new(),
                imports: Vec::new(),
                exports: Vec::new(),
                memory_pages: None,
                start_function: None,
            };
            parse_export_section_info(section_data, &mut dummy_info)?;
            exports = dummy_info.exports;
            break;
        }

        offset = section_end;
    }

    Ok(exports)
}

/// Helper function to read LEB128 unsigned 32-bit integer
fn read_leb128_u32(data: &[u8], offset: usize) -> Result<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut bytes_read = 0;

    for i in 0..5 {
        // Max 5 bytes for u32
        if offset + i >= data.len() {
            return Err(Error::parse_error(
                "Unexpected end of data while reading LEB128",
            ));
        }

        let byte = data[offset + i];
        bytes_read += 1;

        result |= ((byte & 0x7F) as u32) << shift;

        if byte & 0x80 == 0 {
            break;
        }

        shift += 7;
        if shift >= 32 {
            return Err(Error::parse_error("LEB128 value too large for u32"));
        }
    }

    Ok((result, bytes_read))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = DecodedCache::new(WasmFormat::CoreModule, 1024);
        assert_eq!(cache.format_type, WasmFormat::CoreModule);
        assert_eq!(cache.binary_size, 1024);
        assert!(cache.sections.is_empty());
    }

    #[test]
    fn test_cache_manager() {
        let mut manager = CacheManager::new(1024 * 1024);
        let binary = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

        let cache1 = manager.get_cache(&binary).unwrap();
        assert_eq!(cache1.format_type, WasmFormat::CoreModule);

        // Should return same cache for same binary
        let cache2 = manager.get_cache(&binary).unwrap();
        assert_eq!(cache2.format_type, WasmFormat::CoreModule);
    }

    #[test]
    fn test_hash_calculation() {
        let data1 = [1, 2, 3, 4];
        let data2 = [1, 2, 3, 4];
        let data3 = [4, 3, 2, 1];

        assert_eq!(calculate_hash(&data1), calculate_hash(&data2));
        assert_ne!(calculate_hash(&data1), calculate_hash(&data3));
    }
}
