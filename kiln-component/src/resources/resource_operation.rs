// Re-export ResourceOperation for convenience
pub use wrt_foundation::resource::ResourceOperation;

use crate::prelude::*;

/// Convert from local ResourceOperation enum to format ResourceOperation
pub fn to_format_resource_operation(
    op: ResourceOperation,
    type_idx: u32,
) -> wrt_format::component::FormatResourceOperation {
    use wrt_format::component::FormatResourceOperation as FormatOp;
    use wrt_foundation::resource::{ResourceDrop, ResourceNew, ResourceRep};

    match op {
        ResourceOperation::Read => FormatOp::Rep(ResourceRep { type_idx }),
        ResourceOperation::Write => FormatOp::Rep(ResourceRep { type_idx }), // Map to Rep
        ResourceOperation::Execute => FormatOp::Rep(ResourceRep { type_idx }), // Map to Rep
        ResourceOperation::Create => FormatOp::New(ResourceNew { type_idx }),
        ResourceOperation::Delete => FormatOp::Drop(ResourceDrop { type_idx }),
        ResourceOperation::Reference => FormatOp::Rep(ResourceRep { type_idx }), // Map to Rep
        ResourceOperation::Dereference => FormatOp::Rep(ResourceRep { type_idx }), // Map to Rep
        ResourceOperation::New => FormatOp::New(ResourceNew { type_idx }),
        ResourceOperation::Drop => FormatOp::Drop(ResourceDrop { type_idx }),
        ResourceOperation::Rep => FormatOp::Rep(ResourceRep { type_idx }),
    }
}

/// Convert from format ResourceOperation to local ResourceOperation
pub fn from_format_resource_operation(
    op: &wrt_format::component::FormatResourceOperation,
) -> ResourceOperation {
    use wrt_format::component::FormatResourceOperation as FormatOp;

    match op {
        FormatOp::Rep(_) => ResourceOperation::Rep,
        FormatOp::New(_) => ResourceOperation::New,
        FormatOp::Drop(_) => ResourceOperation::Drop,
    }
}

/// Convert a Core ResourceOperation to a Format ResourceOperation
#[cfg(not(feature = "safe-memory"))]
pub fn core_to_format_resource_operation(
    op: &wrt_foundation::ResourceOperation,
) -> ResourceOperation {
    match op {
        wrt_foundation::ResourceOperation::New => ResourceOperation::New,
        wrt_foundation::ResourceOperation::Drop => ResourceOperation::Drop,
        wrt_foundation::ResourceOperation::Rep => ResourceOperation::Rep,
        wrt_foundation::ResourceOperation::Read => ResourceOperation::Read,
        wrt_foundation::ResourceOperation::Write => ResourceOperation::Write,
        wrt_foundation::ResourceOperation::Execute => ResourceOperation::Execute,
        wrt_foundation::ResourceOperation::Create => ResourceOperation::Create,
        wrt_foundation::ResourceOperation::Delete => ResourceOperation::Delete,
        wrt_foundation::ResourceOperation::Reference => ResourceOperation::Reference,
        wrt_foundation::ResourceOperation::Dereference => ResourceOperation::Dereference,
    }
}

/// Convert a Format ResourceOperation to a Core ResourceOperation
#[cfg(not(feature = "safe-memory"))]
pub fn format_to_core_resource_operation(
    op: &ResourceOperation,
) -> wrt_foundation::ResourceOperation {
    match op {
        ResourceOperation::New => wrt_foundation::ResourceOperation::New,
        ResourceOperation::Drop => wrt_foundation::ResourceOperation::Drop,
        ResourceOperation::Rep => wrt_foundation::ResourceOperation::Rep,
        ResourceOperation::Read => wrt_foundation::ResourceOperation::Read,
        ResourceOperation::Write => wrt_foundation::ResourceOperation::Write,
        ResourceOperation::Execute => wrt_foundation::ResourceOperation::Execute,
        ResourceOperation::Create => wrt_foundation::ResourceOperation::Create,
        ResourceOperation::Delete => wrt_foundation::ResourceOperation::Delete,
        ResourceOperation::Reference => wrt_foundation::ResourceOperation::Reference,
        ResourceOperation::Dereference => wrt_foundation::ResourceOperation::Dereference,
    }
}

#[cfg(feature = "safe-memory")]
mod safe_memory {
    use wrt_foundation::ResourceOperation as FormatOp;

    use crate::prelude::*;

    /// Convert a Core ResourceOperation to a Format ResourceOperation
    pub fn core_to_format_resource_operation(op: &wrt_foundation::ResourceOperation) -> FormatOp {
        match op {
            wrt_foundation::ResourceOperation::New => FormatOp::New,
            wrt_foundation::ResourceOperation::Drop => FormatOp::Drop,
            wrt_foundation::ResourceOperation::Rep => FormatOp::Rep,
            wrt_foundation::ResourceOperation::Read => FormatOp::Read,
            wrt_foundation::ResourceOperation::Write => FormatOp::Write,
            wrt_foundation::ResourceOperation::Execute => FormatOp::Execute,
            wrt_foundation::ResourceOperation::Create => FormatOp::Create,
            wrt_foundation::ResourceOperation::Delete => FormatOp::Delete,
            wrt_foundation::ResourceOperation::Reference => FormatOp::Reference,
            wrt_foundation::ResourceOperation::Dereference => FormatOp::Dereference,
        }
    }

    /// Convert a Format ResourceOperation to a Core ResourceOperation
    pub fn format_to_core_resource_operation(op: &FormatOp) -> wrt_foundation::ResourceOperation {
        match op {
            FormatOp::New => wrt_foundation::ResourceOperation::New,
            FormatOp::Drop => wrt_foundation::ResourceOperation::Drop,
            FormatOp::Rep => wrt_foundation::ResourceOperation::Rep,
            FormatOp::Read => wrt_foundation::ResourceOperation::Read,
            FormatOp::Write => wrt_foundation::ResourceOperation::Write,
            FormatOp::Execute => wrt_foundation::ResourceOperation::Execute,
            FormatOp::Create => wrt_foundation::ResourceOperation::Create,
            FormatOp::Delete => wrt_foundation::ResourceOperation::Delete,
            FormatOp::Reference => wrt_foundation::ResourceOperation::Reference,
            FormatOp::Dereference => wrt_foundation::ResourceOperation::Dereference,
        }
    }
}
