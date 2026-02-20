//! Import mapping for the WebAssembly Component Model
//!
//! This module provides functionality to map between different import
//! representations.

use crate::{import::Import, prelude::*};

/// Map of import names to imports
#[derive(Debug, Default)]
pub struct ImportMap {
    /// Name-to-import mapping
    imports: HashMap<String, Arc<Import>>,
}

/// Map of import names to imports using SafeMemory
#[cfg(feature = "safe-memory")]
#[derive(Debug)]
pub struct SafeImportMap {
    /// Name-to-import mapping with safe memory guarantees
    imports: wrt_foundation::safe_memory::SafeStack<(String, Arc<Import>)>,
}

impl ImportMap {
    /// Create a new empty import map
    pub fn new() -> Self {
        Self { imports: HashMap::new() }
    }

    /// Add an import to the map
    pub fn add(&mut self, name: &str, import: Arc<Import>) -> Result<()> {
        self.imports.insert(name.to_owned(), import);
        Ok(())
    }

    /// Get an import by name
    pub fn get(&self, name: &str) -> Option<Arc<Import>> {
        self.imports.get(name).cloned()
    }

    /// Remove an import by name
    pub fn remove(&mut self, name: &str) -> Option<Arc<Import>> {
        self.imports.remove(name)
    }

    /// Check if an import exists by name
    pub fn contains(&self, name: &str) -> bool {
        self.imports.contains_key(name)
    }

    /// Get all import names
    pub fn names(&self) -> Vec<String> {
        self.imports.keys().cloned().collect()
    }

    /// Get all imports as a Vec of (name, import) pairs
    pub fn get_all(&self) -> Vec<(String, Arc<Import>)> {
        self.imports.iter().map(|(name, import)| (name.clone(), import.clone())).collect()
    }

    /// Convert this import map to one using SafeMemory containers
    #[cfg(feature = "safe-memory")]
    pub fn to_safe_memory(&self) -> SafeImportMap {
        let mut result = SafeImportMap::new();
        for (name, import) in &self.imports {
            result.add(name, import.clone()).unwrap();
        }
        result
    }
}

#[cfg(feature = "safe-memory")]
impl SafeImportMap {
    /// Create a new empty import map
    pub fn new() -> Self {
        Self { imports: wrt_foundation::safe_memory::SafeStack::new() }
    }

    /// Add an import to the map
    pub fn add(&mut self, name: &str, import: Arc<Import>) -> Result<()> {
        // Check if the name already exists
        for i in 0..self.imports.len() {
            let (existing_name, _) = self.imports.get(i)?;
            if existing_name == name {
                // Replace the existing import
                self.imports.set(i, (name.to_owned(), import))?;
                return Ok();
            }
        }

        // Name doesn't exist, add a new entry
        self.imports.push((name.to_owned(), import))?;
        Ok(())
    }

    /// Get an import by name
    pub fn get(&self, name: &str) -> Option<Arc<Import>> {
        // Search for the name
        for i in 0..self.imports.len() {
            if let Ok((existing_name, import)) = self.imports.get(i) {
                if existing_name == name {
                    return Some(import;
                }
            }
        }
        None
    }

    /// Remove an import by name
    pub fn remove(&mut self, name: &str) -> Option<Arc<Import>> {
        // Search for the name
        for i in 0..self.imports.len() {
            if let Ok((existing_name, _)) = self.imports.get(i) {
                if existing_name == name {
                    // Found the name, remove it
                    if let Ok(items) = self.imports.to_vec() {
                        let import = items[i].1.clone();
                        // Create a new vector without the removed item
                        let mut new_items = Vec::with_capacity(items.len() - 1;
                        for (j, item) in items.into_iter().enumerate() {
                            if j != i {
                                new_items.push(item);
                            }
                        }

                        // Clear and rebuild the stack
                        self.imports.clear);
                        for item in new_items {
                            let _ = self.imports.push(item);
                        }

                        return Some(import;
                    }
                }
            }
        }
        None
    }

    /// Check if an import exists by name
    pub fn contains(&self, name: &str) -> bool {
        // Search for the name
        for i in 0..self.imports.len() {
            if let Ok((existing_name, _)) = self.imports.get(i) {
                if existing_name == name {
                    return true;
                }
            }
        }
        false
    }

    /// Get all import names
    pub fn names(&self) -> Result<Vec<String>> {
        let mut names = Vec::with_capacity(self.imports.len();
        for i in 0..self.imports.len() {
            if let Ok((name, _)) = self.imports.get(i) {
                names.push(name);
            }
        }
        Ok(names)
    }

    /// Get all imports as a Vec of (name, import) pairs
    pub fn get_all(&self) -> core::result::Result<Vec<(String, Arc<Import>)>> {
        self.imports.to_vec()
    }

    /// Convert to standard ImportMap
    pub fn to_standard(&self) -> Result<ImportMap> {
        let mut result = ImportMap::new();
        let items = self.imports.to_vec()?;
        for (name, import) in items {
            result.add(&name, import)?;
        }
        Ok(result)
    }

    /// Get the number of imports
    pub fn len(&self) -> usize {
        self.imports.len()
    }

    /// Check if the import map is empty
    pub fn is_empty(&self) -> bool {
        self.imports.is_empty()
    }

    /// Set the verification level for memory operations
    pub fn set_verification_level(
        &mut self,
        level: wrt_foundation::verification::VerificationLevel,
    ) {
        self.imports.set_verification_level(level;

}
