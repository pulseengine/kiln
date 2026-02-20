//! Export mapping for the WebAssembly Component Model
//!
//! This module provides functionality to map between different export
//! representations.


#[cfg(not(feature = "std"))]
extern crate alloc;

use crate::{export::Export, prelude::*};
use wrt_foundation::{
    collections::StaticMap as BoundedMap,
    bounded::BoundedString,
};
use wrt_foundation::budget_aware_provider::CrateId;
use wrt_foundation::capabilities::{CapabilityAwareProvider, MemoryCapabilityContext};
use wrt_error::{Error, ErrorCategory, codes};

/// Maximum number of exports in a component
const MAX_EXPORTS: usize = 512;

/// Maximum length for export names
const MAX_EXPORT_NAME_LEN: usize = 128;

/// Helper function to create component provider using capability-driven design
fn create_component_provider() -> Result<impl wrt_foundation::MemoryProvider> {
    use wrt_foundation::memory_init::get_global_capability_context;
    
    let context = get_global_capability_context()
        .map_err(|_| Error::initialization_error("Failed to get global capability context for component provider"))?
    
    context.create_provider(CrateId::Component, 4096)
        .map_err(|_| Error::memory_out_of_bounds("Failed to create component provider with 4096 bytes"))?
}

/// Map of export names to exports using bounded collections
#[derive(Debug)]
pub struct ExportMap<P: MemoryProvider + Default + Clone> {
    /// Name-to-export mapping with bounded capacity
    exports: BoundedMap<
        BoundedString<MAX_EXPORT_NAME_LEN, P>, 
        Arc<Export>, 
        MAX_EXPORTS, 
        P
    >,
}

impl<P: MemoryProvider + Default + Clone> Default for ExportMap<P> {
    fn default() -> Self {
        Self::new().expect("Failed to create default ExportMap")
    }
}

/// Map of export names to exports using SafeMemory
#[cfg(feature = "safe-memory")]
#[derive(Debug)]
pub struct SafeExportMap {
    /// Name-to-export mapping with safe memory guarantees
    exports: wrt_foundation::safe_memory::SafeStack<(String, Arc<Export>)>,
}

impl<P: MemoryProvider + Default + Clone> ExportMap<P> {
    /// Create a new empty export map
    pub fn new() -> Result<Self> {
        let provider = P::default());
        let exports = BoundedMap::new();
        Ok(Self { exports })
    }

    /// Add an export to the map
    pub fn add(&mut self, name: &str, export: Arc<Export>) -> Result<()> {
        let bounded_name = BoundedString::try_from_str(name, self.exports.provider().clone())?;
        self.exports.insert(bounded_name, export)?;
        Ok(())
    }

    /// Get an export by name
    pub fn get(&self, name: &str) -> Result<Option<Arc<Export>>> {
        let bounded_name = BoundedString::try_from_str(name, self.exports.provider().clone())?;
        Ok(self.exports.get(&bounded_name).cloned())
    }

    /// Remove an export by name
    pub fn remove(&mut self, name: &str) -> Result<Option<Arc<Export>>> {
        let bounded_name = BoundedString::try_from_str(name, self.exports.provider().clone())?;
        Ok(self.exports.remove(&bounded_name))
    }

    /// Check if an export exists by name
    pub fn contains(&self, name: &str) -> bool {
        if let Ok(bounded_name) = BoundedString::try_from_str(name, self.exports.provider().clone()) {
            self.exports.contains_key(&bounded_name)
        } else {
            false
        }
    }

    /// Get all export names as bounded strings
    pub fn names(&self) -> Result<BoundedVec<BoundedString<MAX_EXPORT_NAME_LEN, P>, MAX_EXPORTS, P>> {
        let mut names = BoundedVec::new(self.exports.provider().clone())?;
        
        for (name, _) in self.exports.iter() {
            if names.push(name.clone()).is_err() {
                break; // Stop if we can't add more names
            }
        }
        Ok(names)
    }

    /// Get all exports as bounded collection of (name, export) pairs
    pub fn get_all(&self) -> Result<BoundedVec<(BoundedString<MAX_EXPORT_NAME_LEN, P>, Arc<Export>), MAX_EXPORTS, P>> {
        let mut pairs = BoundedVec::new(self.exports.provider().clone())?;
        
        for (name, export) in self.exports.iter() {
            if pairs.push((name.clone(), export.clone())).is_err() {
                break; // Stop if we can't add more pairs
            }
        }
        Ok(pairs)
    }

    /// Convert this export map to one using SafeMemory containers
    #[cfg(feature = "safe-memory")]
    pub fn to_safe_memory(&self) -> Result<SafeExportMap> {
        let mut result = SafeExportMap::new();
        for (name, export) in self.exports.iter() {
            let name_str = name.as_str()?;
            result.add(name_str, export.clone())?;
        }
        Ok(result)
    }
}

#[cfg(feature = "safe-memory")]
impl SafeExportMap {
    /// Create a new empty export map
    pub fn new() -> Self {
        Self { exports: wrt_foundation::safe_memory::SafeStack::new() }
    }

    /// Add an export to the map
    pub fn add(&mut self, name: &str, export: Arc<Export>) -> Result<()> {
        // Check if the name already exists
        for i in 0..self.exports.len() {
            let (existing_name, _) = self.exports.get(i)?;
            if existing_name == name {
                // Replace the existing export
                self.exports.set(i, (name.to_owned(), export))?;
                return Ok();
            }
        }

        // Name doesn't exist, add a new entry
        self.exports.push((name.to_owned(), export))?;
        Ok(()
    }

    /// Get an export by name
    pub fn get(&self, name: &str) -> Option<Arc<Export>> {
        // Search for the name
        for i in 0..self.exports.len() {
            if let Ok((existing_name, export)) = self.exports.get(i) {
                if existing_name == name {
                    return Some(export;
                }
            }
        }
        None
    }

    /// Remove an export by name
    pub fn remove(&mut self, name: &str) -> Option<Arc<Export>> {
        // Search for the name
        for i in 0..self.exports.len() {
            if let Ok((existing_name, _)) = self.exports.get(i) {
                if existing_name == name {
                    // Found the name, remove it
                    if let Ok(items) = self.exports.to_vec() {
                        let export = items[i].1.clone();
                        // Clear and rebuild the stack without the removed item
                        self.exports.clear);
                        for (j, item) in items.into_iter().enumerate() {
                            if j != i {
                                let _ = self.exports.push(item);
                            }
                        }

                        return Some(export;
                    }
                }
            }
        }
        None
    }

    /// Check if an export exists by name
    pub fn contains(&self, name: &str) -> bool {
        // Search for the name
        for i in 0..self.exports.len() {
            if let Ok((existing_name, _)) = self.exports.get(i) {
                if existing_name == name {
                    return true;
                }
            }
        }
        false
    }

    /// Get all export names
    pub fn names(&self) -> Result<BoundedVec<String, MAX_EXPORTS>> {
        let provider = create_component_provider()?;
        let mut names = BoundedVec::new().unwrap();
        for i in 0..self.exports.len() {
            if let Ok((name, _)) = self.exports.get(i) {
                names.push(name)?;
            }
        }
        Ok(names)
    }

    /// Get all exports as bounded collection of (name, export) pairs
    pub fn get_all(&self) -> Result<BoundedVec<(String, Arc<Export>), MAX_EXPORTS>> {
        let provider = create_component_provider()?;
        let mut pairs = BoundedVec::new().unwrap();
        let items = self.exports.to_vec()?;
        for item in items {
            pairs.push(item)?;
        }
        Ok(pairs)
    }

    /// Convert to standard ExportMap
    pub fn to_standard<P: MemoryProvider + Default + Clone>(&self) -> Result<ExportMap<P>> {
        let mut result = ExportMap::new()?;
        let items = self.exports.to_vec()?;
        for (name, export) in items {
            result.add(&name, export)?;
        }
        Ok(result)
    }

    /// Get the number of exports
    pub fn len(&self) -> usize {
        self.exports.len()
    }

    /// Check if the export map is empty
    pub fn is_empty(&self) -> bool {
        self.exports.is_empty()
    }

    /// Set the verification level for memory operations
    pub fn set_verification_level(
        &mut self,
        level: wrt_foundation::verification::VerificationLevel,
    ) {
        self.exports.set_verification_level(level;
