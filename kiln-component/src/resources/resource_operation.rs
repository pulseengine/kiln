// Re-export ResourceOperation for convenience
pub use kiln_foundation::resource::ResourceOperation;

use crate::prelude::*;

/// Convert from local ResourceOperation enum to format ResourceOperation
pub fn to_format_resource_operation(
    op: ResourceOperation,
    type_idx: u32,
) -> kiln_format::component::FormatResourceOperation {
    use kiln_format::component::FormatResourceOperation as FormatOp;
    use kiln_foundation::resource::{ResourceDrop, ResourceNew, ResourceRep};

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
    op: &kiln_format::component::FormatResourceOperation,
) -> ResourceOperation {
    use kiln_format::component::FormatResourceOperation as FormatOp;

    match op {
        FormatOp::Rep(_) => ResourceOperation::Rep,
        FormatOp::New(_) => ResourceOperation::New,
        FormatOp::Drop(_) => ResourceOperation::Drop,
    }
}

/// Convert a Core ResourceOperation to a Format ResourceOperation
#[cfg(not(feature = "safe-memory"))]
pub fn core_to_format_resource_operation(
    op: &kiln_foundation::ResourceOperation,
) -> ResourceOperation {
    match op {
        kiln_foundation::ResourceOperation::New => ResourceOperation::New,
        kiln_foundation::ResourceOperation::Drop => ResourceOperation::Drop,
        kiln_foundation::ResourceOperation::Rep => ResourceOperation::Rep,
        kiln_foundation::ResourceOperation::Read => ResourceOperation::Read,
        kiln_foundation::ResourceOperation::Write => ResourceOperation::Write,
        kiln_foundation::ResourceOperation::Execute => ResourceOperation::Execute,
        kiln_foundation::ResourceOperation::Create => ResourceOperation::Create,
        kiln_foundation::ResourceOperation::Delete => ResourceOperation::Delete,
        kiln_foundation::ResourceOperation::Reference => ResourceOperation::Reference,
        kiln_foundation::ResourceOperation::Dereference => ResourceOperation::Dereference,
    }
}

/// Convert a Format ResourceOperation to a Core ResourceOperation
#[cfg(not(feature = "safe-memory"))]
pub fn format_to_core_resource_operation(
    op: &ResourceOperation,
) -> kiln_foundation::ResourceOperation {
    match op {
        ResourceOperation::New => kiln_foundation::ResourceOperation::New,
        ResourceOperation::Drop => kiln_foundation::ResourceOperation::Drop,
        ResourceOperation::Rep => kiln_foundation::ResourceOperation::Rep,
        ResourceOperation::Read => kiln_foundation::ResourceOperation::Read,
        ResourceOperation::Write => kiln_foundation::ResourceOperation::Write,
        ResourceOperation::Execute => kiln_foundation::ResourceOperation::Execute,
        ResourceOperation::Create => kiln_foundation::ResourceOperation::Create,
        ResourceOperation::Delete => kiln_foundation::ResourceOperation::Delete,
        ResourceOperation::Reference => kiln_foundation::ResourceOperation::Reference,
        ResourceOperation::Dereference => kiln_foundation::ResourceOperation::Dereference,
    }
}

#[cfg(feature = "safe-memory")]
mod safe_memory {
    use kiln_foundation::ResourceOperation as FormatOp;

    use crate::prelude::*;

    /// Convert a Core ResourceOperation to a Format ResourceOperation
    pub fn core_to_format_resource_operation(op: &kiln_foundation::ResourceOperation) -> FormatOp {
        match op {
            kiln_foundation::ResourceOperation::New => FormatOp::New,
            kiln_foundation::ResourceOperation::Drop => FormatOp::Drop,
            kiln_foundation::ResourceOperation::Rep => FormatOp::Rep,
            kiln_foundation::ResourceOperation::Read => FormatOp::Read,
            kiln_foundation::ResourceOperation::Write => FormatOp::Write,
            kiln_foundation::ResourceOperation::Execute => FormatOp::Execute,
            kiln_foundation::ResourceOperation::Create => FormatOp::Create,
            kiln_foundation::ResourceOperation::Delete => FormatOp::Delete,
            kiln_foundation::ResourceOperation::Reference => FormatOp::Reference,
            kiln_foundation::ResourceOperation::Dereference => FormatOp::Dereference,
        }
    }

    /// Convert a Format ResourceOperation to a Core ResourceOperation
    pub fn format_to_core_resource_operation(op: &FormatOp) -> kiln_foundation::ResourceOperation {
        match op {
            FormatOp::New => kiln_foundation::ResourceOperation::New,
            FormatOp::Drop => kiln_foundation::ResourceOperation::Drop,
            FormatOp::Rep => kiln_foundation::ResourceOperation::Rep,
            FormatOp::Read => kiln_foundation::ResourceOperation::Read,
            FormatOp::Write => kiln_foundation::ResourceOperation::Write,
            FormatOp::Execute => kiln_foundation::ResourceOperation::Execute,
            FormatOp::Create => kiln_foundation::ResourceOperation::Create,
            FormatOp::Delete => kiln_foundation::ResourceOperation::Delete,
            FormatOp::Reference => kiln_foundation::ResourceOperation::Reference,
            FormatOp::Dereference => kiln_foundation::ResourceOperation::Dereference,
        }
    }
}
