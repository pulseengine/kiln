/// Define our own enum for memory access mode since wrt_intercept doesn't have
/// one
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryAccessMode {
    /// Read access to memory
    Read,
    /// Write access to memory
    Write,
    /// Execute access to memory
    Execute,
}
