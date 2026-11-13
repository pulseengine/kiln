//! Per-import ownership of a fused module's host imports (SR-81).
//!
//! meld's `--emit-import-map` names, for every function import of a fused
//! module, the memory and `cabi_realloc` of the component that owns it. Under
//! `--memory multi` that is the only way to know which of the module's
//! memories a host import reads and writes. This module is the runtime's view
//! of that map; the host parses the JSON and hands the entries over.
//!
//! Nothing here guesses: an import the map does not cover, or covers under a
//! different name, is an error, and an owner meld could not determine stays
//! unknown and refuses any call that needs it.

use std::{collections::HashMap, string::String, vec::Vec};

use kiln_error::{Error, Result};
use kiln_foundation::traits::MemoryAccessor;

/// The memory a host import operates on, as meld's map states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportMemory {
    /// The call takes no memory (`"memory": null`).
    NotNeeded,
    /// The fused module's memory index.
    Index(u32),
    /// meld could not determine the owner; the string is meld's reason.
    Unknown(String),
}

/// The `cabi_realloc` an import's pre-allocated guest buffers come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRealloc {
    /// The call allocates nothing in the guest (`"realloc": null`).
    NotNeeded,
    /// The fused module's function index of the allocator.
    Func(u32),
    /// meld could not determine the allocator; the string is meld's reason.
    Unknown(String),
}

/// One function import of the fused module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportMapEntry {
    /// Function-import index in the fused module.
    pub index: u32,
    /// Import module name, exactly as in the module.
    pub module: String,
    /// Import field name, exactly as in the module (suffix included).
    pub name: String,
    /// Memory the import reads and writes.
    pub memory: ImportMemory,
    /// Allocator the host calls to place guest-owned results.
    pub realloc: ImportRealloc,
    /// The fused component that owns the import (meld's `component`), when
    /// the map names one. P3 thread handles are per owner (SR-82).
    pub component: Option<u32>,
    /// `[thread-new-indirect-v0]` only: the funcref table its `fn_idx`
    /// indexes, as meld records it (SR-82).
    pub thread_table: Option<ThreadTable>,
}

/// The table meld exports for `[thread-new-indirect-v0]` (meld's
/// `thread_table`: `{"export", "index"}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadTable {
    /// Export name (meld exports `__meld_thread_table`).
    pub export: String,
    /// The fused module's table index.
    pub index: u32,
}

/// meld's import map for one fused module.
#[derive(Debug, Clone, Default)]
pub struct HostImportMap {
    entries: HashMap<u32, ImportMapEntry>,
}

impl HostImportMap {
    /// Build a map from entries; two entries for one index are refused.
    ///
    /// # Errors
    /// Fails when an index appears twice.
    pub fn new(entries: Vec<ImportMapEntry>) -> Result<Self> {
        let mut by_index = HashMap::with_capacity(entries.len());
        for entry in entries {
            if by_index.insert(entry.index, entry).is_some() {
                return Err(Error::validation_invalid_input(
                    "import map lists one import index twice",
                ));
            }
        }
        Ok(Self { entries: by_index })
    }

    /// The entry for function import `index`, checked against the import's
    /// own module and field name.
    ///
    /// # Errors
    /// Fails when the map has no entry for `index`, or its entry names a
    /// different import (the map belongs to another module).
    pub fn entry(&self, index: u32, module: &str, name: &str) -> Result<&ImportMapEntry> {
        let entry = self.entries.get(&index).ok_or_else(|| {
            Error::runtime_import_not_found("import map has no entry for a function import")
        })?;
        if entry.module != module || entry.name != name {
            return Err(Error::validation_invalid_input(
                "import map entry names a different import than the module's import at that index",
            ));
        }
        Ok(entry)
    }
}

/// Memory handed to a host call that must not use memory: every access fails.
///
/// Two reasons exist so the error says which: the map says the call takes no
/// memory, or meld could not determine the owning memory.
#[derive(Debug, Clone, Copy)]
pub struct RefusedMemory {
    reason: &'static str,
}

impl RefusedMemory {
    /// The import map says this call takes no memory.
    pub const NOT_NEEDED: Self = Self {
        reason: "Host import accessed memory, but the import map says it takes none",
    };
    /// The import map could not determine the owning memory.
    pub const UNKNOWN: Self = Self {
        reason: "Host import needs memory, but the import map could not determine the owning \
                 instance's memory",
    };
}

impl RefusedMemory {
    /// Why a `$N`-suffixed import in a multi-memory module is refused when no
    /// import map says which memory it owns.
    pub const OWNER_UNDETERMINED_REASON: &'static str = "Host import carries a meld $N instance suffix in a multi-memory module; \
         the owning instance cannot be determined without an import map";
}

impl MemoryAccessor for RefusedMemory {
    fn read_bytes(&self, _offset: u32, _buffer: &mut [u8]) -> Result<()> {
        Err(Error::runtime_error(self.reason))
    }

    fn write_bytes(&self, _offset: u32, _data: &[u8]) -> Result<()> {
        Err(Error::runtime_error(self.reason))
    }

    fn size(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(index: u32, module: &str, name: &str, memory: ImportMemory) -> ImportMapEntry {
        ImportMapEntry {
            index,
            module: module.into(),
            name: name.into(),
            memory,
            realloc: ImportRealloc::NotNeeded,
            component: None,
            thread_table: None,
        }
    }

    // rivet: verifies SR-81
    #[test]
    fn entry_is_found_by_index_and_checked_against_the_import() -> Result<()> {
        let map = HostImportMap::new(vec![entry(
            3,
            "wasi:cli/stderr@0.2.6",
            "get-stderr$5",
            ImportMemory::Index(2),
        )])?;
        let found = map.entry(3, "wasi:cli/stderr@0.2.6", "get-stderr$5")?;
        assert_eq!(found.memory, ImportMemory::Index(2));
        Ok(())
    }

    // rivet: verifies SR-81
    #[test]
    fn a_mismatched_name_missing_index_or_duplicate_is_an_error() -> Result<()> {
        let map = HostImportMap::new(vec![entry(
            3,
            "wasi:cli/stderr@0.2.6",
            "get-stderr$5",
            ImportMemory::NotNeeded,
        )])?;
        assert!(map.entry(3, "wasi:cli/stderr@0.2.6", "get-stderr$2").is_err());
        assert!(map.entry(3, "wasi:cli/stdout@0.2.6", "get-stderr$5").is_err());
        assert!(map.entry(4, "wasi:cli/stderr@0.2.6", "get-stderr$5").is_err());
        assert!(
            HostImportMap::new(vec![
                entry(1, "a", "b", ImportMemory::NotNeeded),
                entry(1, "a", "c", ImportMemory::NotNeeded),
            ])
            .is_err()
        );
        Ok(())
    }

    // rivet: verifies SR-81
    #[test]
    fn refused_memory_fails_every_access() {
        for refused in [RefusedMemory::NOT_NEEDED, RefusedMemory::UNKNOWN] {
            let mut buf = [0u8; 1];
            assert!(refused.read_bytes(0, &mut buf).is_err());
            assert!(refused.write_bytes(0, &[1]).is_err());
            assert_eq!(refused.size(), 0);
        }
    }
}
