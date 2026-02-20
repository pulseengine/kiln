//! Neural network graph/model management
//!
//! This module handles the loading, storage, and management of neural network
//! models (graphs) with capability-based access control.

use core::fmt;
use std::sync::{
    Mutex,
    OnceLock,
};

use wrt_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    BoundedVec,
};

use super::{
    ModelCapability,
    ModelFormat,
    NeuralNetworkCapability,
    VerificationLevel,
};
use crate::prelude::*;

/// Maximum number of graphs that can be loaded
const MAX_GRAPHS: usize = 16;

/// Graph encoding formats (matches WIT definition)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GraphEncoding {
    /// ONNX format
    ONNX,
    /// TensorFlow SavedModel/frozen graph
    TensorFlow,
    /// PyTorch TorchScript
    PyTorch,
    /// OpenVINO IR format
    OpenVINO,
    /// Tract native format
    TractNative,
}

impl GraphEncoding {
    /// Convert to internal model format
    pub fn to_model_format(self) -> ModelFormat {
        match self {
            GraphEncoding::ONNX => ModelFormat::ONNX,
            GraphEncoding::TensorFlow => ModelFormat::TensorFlow,
            GraphEncoding::PyTorch => ModelFormat::PyTorch,
            GraphEncoding::OpenVINO => ModelFormat::OpenVINO,
            GraphEncoding::TractNative => ModelFormat::TractNative,
        }
    }
}

/// Execution target for optimization hints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExecutionTarget {
    /// CPU execution
    CPU,
    /// GPU execution
    GPU,
    /// Tensor Processing Unit
    TPU,
    /// Neural Processing Unit
    NPU,
}

impl Default for ExecutionTarget {
    fn default() -> Self {
        ExecutionTarget::CPU
    }
}

/// A loaded neural network graph
pub struct Graph {
    /// Unique identifier
    id:               u32,
    /// Graph encoding format
    encoding:         GraphEncoding,
    /// Execution target
    target:           ExecutionTarget,
    /// Size of the model in bytes
    size:             usize,
    /// SHA-256 hash of the model data
    hash:             [u8; 32],
    /// Backend-specific model handle
    backend_model:    Box<dyn ModelCapability>,
    /// Capability level used to load this graph
    capability_level: VerificationLevel,
}

impl Graph {
    /// Create a new graph
    pub fn new(
        id: u32,
        encoding: GraphEncoding,
        target: ExecutionTarget,
        data: &[u8],
        backend_model: Box<dyn ModelCapability>,
        capability: &dyn NeuralNetworkCapability,
    ) -> Result<Self> {
        // Calculate hash for verification
        let hash = calculate_sha256(data);

        // For higher safety levels, verify the model is approved
        let verification_level = capability.verification_level();
        if matches!(
            verification_level,
            VerificationLevel::Continuous
                | VerificationLevel::Redundant
                | VerificationLevel::Formal
        ) {
            if !capability.is_model_approved(&hash) {
                return Err(Error::wasi_verification_failed(
                    "Model hash not in approved list",
                ));
            }
        }

        Ok(Self {
            id,
            encoding,
            target,
            size: data.len(),
            hash,
            backend_model,
            capability_level: verification_level,
        })
    }

    /// Get the graph ID
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get the encoding format
    pub fn encoding(&self) -> GraphEncoding {
        self.encoding
    }

    /// Get the execution target
    pub fn target(&self) -> ExecutionTarget {
        self.target
    }

    /// Get the model size
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get the model hash
    pub fn hash(&self) -> &[u8; 32] {
        &self.hash
    }

    /// Get the backend model
    pub fn backend_model(&self) -> &dyn ModelCapability {
        self.backend_model.as_ref()
    }

    /// Get the capability level
    pub fn capability_level(&self) -> VerificationLevel {
        self.capability_level
    }
}

impl fmt::Debug for Graph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Graph")
            .field("id", &self.id)
            .field("encoding", &self.encoding)
            .field("target", &self.target)
            .field("size", &self.size)
            .field("capability_level", &self.capability_level)
            .finish()
    }
}

/// Graph store for managing loaded models
pub struct GraphStore {
    graphs:  Vec<Graph>,
    next_id: u32,
}

impl GraphStore {
    /// Create a new graph store
    pub fn new() -> Result<Self> {
        Ok(Self {
            graphs:  Vec::new(),
            next_id: 1,
        })
    }

    /// Add a graph to the store
    pub fn add(&mut self, graph: Graph) -> Result<u32> {
        let id = graph.id;
        if self.graphs.len() >= MAX_GRAPHS {
            return Err(Error::wasi_resource_exhausted(
                "Maximum number of graphs reached",
            ));
        }
        self.graphs.push(graph);
        Ok(id)
    }

    /// Get a graph by ID
    pub fn get(&self, id: u32) -> Result<&Graph> {
        self.graphs
            .iter()
            .find(|g| g.id() == id)
            .ok_or_else(|| Error::wasi_invalid_argument("Graph not found"))
    }

    /// Get a mutable graph by ID
    pub fn get_mut(&mut self, id: u32) -> Result<&mut Graph> {
        self.graphs
            .iter_mut()
            .find(|g| g.id() == id)
            .ok_or_else(|| Error::wasi_invalid_argument("Graph not found"))
    }

    /// Remove a graph by ID
    pub fn remove(&mut self, id: u32) -> Result<Graph> {
        let pos = self
            .graphs
            .iter()
            .position(|g| g.id() == id)
            .ok_or_else(|| Error::wasi_invalid_argument("Graph not found"))?;

        Ok(self.graphs.remove(pos))
    }

    /// Get the next available ID
    pub fn next_id(&mut self) -> Result<u32> {
        // Check if we're approaching wraparound danger zone
        if self.next_id > u32::MAX - 1000 {
            return Err(Error::wasi_resource_exhausted("Graph ID space exhausted"));
        }

        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| Error::wasi_resource_exhausted("Graph ID overflow"))?;
        Ok(id)
    }

    /// Get the number of loaded graphs
    pub fn count(&self) -> usize {
        self.graphs.len()
    }

    /// Check if the store is at capacity
    pub fn is_full(&self) -> bool {
        self.graphs.len() >= MAX_GRAPHS
    }
}

/// Calculate SHA-256 hash of data
fn calculate_sha256(data: &[u8]) -> [u8; 32] {
    super::sha256::sha256(data)
}

/// Global graph store instance
static GRAPH_STORE: OnceLock<Mutex<GraphStore>> = OnceLock::new();

/// Initialize the graph store
pub fn initialize_graph_store() -> Result<()> {
    let store = GraphStore::new()?;
    let mutex = Mutex::new(store);

    GRAPH_STORE
        .set(mutex)
        .map_err(|_| Error::wasi_capability_unavailable("Graph store already initialized"))
}

/// Get the global graph store
///
/// Returns a guard that can be used to access the store safely.
pub fn get_graph_store() -> Result<std::sync::MutexGuard<'static, GraphStore>> {
    let mutex = GRAPH_STORE
        .get()
        .ok_or_else(|| Error::wasi_capability_unavailable("Graph store not initialized"))?;

    mutex
        .lock()
        .map_err(|_| Error::wasi_runtime_error("Graph store mutex poisoned"))
}

/// Get the global graph store for mutable operations
///
/// Returns a guard that can be used to mutate the store safely.
pub fn get_graph_store_mut() -> Result<std::sync::MutexGuard<'static, GraphStore>> {
    get_graph_store()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_encoding_conversion() {
        assert_eq!(GraphEncoding::ONNX.to_model_format(), ModelFormat::ONNX);
        assert_eq!(
            GraphEncoding::TractNative.to_model_format(),
            ModelFormat::TractNative
        );
    }

    #[test]
    fn test_graph_store() {
        let mut store = GraphStore::new().unwrap();
        assert_eq!(store.count(), 0);
        assert!(!store.is_full());

        let id = store.next_id().unwrap();
        assert_eq!(id, 1);
        assert_eq!(store.next_id().unwrap(), 2);
    }
}
