// Resource built-ins implementation for the WebAssembly Component Model
//
// This module implements the resource-related built-in functions:
// - resource.create: Create a new resource
// - resource.drop: Drop a resource
// - resource.rep: Get the representation of a resource
// - resource.get: Get a resource handle

#[cfg(feature = "std")]
use std::{
    boxed::Box,
    sync::{Arc, Mutex},
    vec::Vec,
};

#[cfg(not(feature = "std"))]
use wrt_foundation::{
    budget_aware_provider::CrateId, collections::StaticVec as BoundedVec, safe_managed_alloc,
    safe_memory::NoStdProvider,
};

// Enable vec! macro for no_std
#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, sync::Arc, vec, vec::Vec};

use wrt_error::{Error, Result};
#[cfg(feature = "std")]
use wrt_foundation::{builtin::BuiltinType, component_value::ComponentValue};
#[cfg(not(feature = "std"))]
use wrt_sync::Mutex;

#[cfg(not(feature = "std"))]
use wrt_foundation::{builtin::BuiltinType, component_value::ComponentValue};

#[cfg(not(feature = "std"))]
type ComponentProvider = NoStdProvider<4096>;

#[cfg(feature = "std")]
use crate::builtins::ComponentProvider;

use crate::{
    builtins::BuiltinHandler,
    resources::{ResourceId, ResourceManager},
};

/// Handler for the resource.create built-in function
pub struct ResourceCreateHandler {
    resource_manager: Arc<Mutex<ResourceManager>>,
}

impl ResourceCreateHandler {
    /// Create a new resource.create handler
    pub fn new(resource_manager: Arc<Mutex<ResourceManager>>) -> Self {
        Self { resource_manager }
    }
}

impl BuiltinHandler for ResourceCreateHandler {
    fn builtin_type(&self) -> crate::builtins::BuiltinType {
        crate::builtins::BuiltinType::ResourceCreate
    }

    #[cfg(feature = "std")]
    fn execute(
        &self,
        args: &[ComponentValue<ComponentProvider>],
    ) -> Result<Vec<ComponentValue<ComponentProvider>>> {
        // Validate args
        if args.len() != 1 {
            return Err(Error::runtime_execution_error(
                "resource.create requires exactly one argument",
            ));
        }

        // Extract the resource representation from args
        let rep = match &args[0] {
            ComponentValue::U32(value) => *value,
            ComponentValue::U64(value) => *value as u32,
            _ => {
                return Err(Error::new(
                    wrt_error::ErrorCategory::Parameter,
                    wrt_error::codes::TYPE_MISMATCH,
                    "Expected U32 or U64 for resource representation",
                ));
            },
        };

        // Create a new resource based on the representation
        let mut manager = self.resource_manager.lock().unwrap();
        let id = manager.add_host_resource(rep)?;

        // Return the resource ID
        Ok(vec![ComponentValue::U32(id.0)])
    }

    #[cfg(not(feature = "std"))]
    fn execute(
        &self,
        args: &[ComponentValue<ComponentProvider>],
    ) -> Result<BoundedVec<ComponentValue<ComponentProvider>, 16>> {
        // Validate args
        if args.len() != 1 {
            return Err(Error::runtime_execution_error(
                "resource.create requires exactly one argument",
            ));
        }

        // Extract the resource representation from args
        let rep = match &args[0] {
            ComponentValue::U32(value) => *value,
            ComponentValue::U64(value) => *value as u32,
            _ => {
                return Err(Error::new(
                    wrt_error::ErrorCategory::Parameter,
                    wrt_error::codes::TYPE_MISMATCH,
                    "Expected U32 or U64 for resource representation",
                ));
            },
        };

        // Create a new resource based on the representation
        // Call on the Arc<Mutex<>> directly since add_host_resource takes &self and locks internally
        let id = {
            let manager = self.resource_manager.lock();
            drop(manager); // Release lock before calling
            self.resource_manager.lock().add_host_resource(rep)?
        };
        let handle = id.0;

        // Return the resource ID
        let mut result = BoundedVec::new();
        result.push(ComponentValue::U32(handle)).map_err(|_| {
            Error::foundation_bounded_capacity_exceeded("Failed to add result value")
        })?;
        Ok(result)
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(Self {
            resource_manager: self.resource_manager.clone(),
        })
    }
}

/// Handler for the resource.drop built-in function
pub struct ResourceDropHandler {
    resource_manager: Arc<Mutex<ResourceManager>>,
}

impl ResourceDropHandler {
    /// Create a new resource.drop handler
    pub fn new(resource_manager: Arc<Mutex<ResourceManager>>) -> Self {
        Self { resource_manager }
    }
}

impl BuiltinHandler for ResourceDropHandler {
    fn builtin_type(&self) -> crate::builtins::BuiltinType {
        crate::builtins::BuiltinType::ResourceDrop
    }

    #[cfg(feature = "std")]
    fn execute(
        &self,
        args: &[ComponentValue<ComponentProvider>],
    ) -> Result<Vec<ComponentValue<ComponentProvider>>> {
        // Validate args
        if args.len() != 1 {
            return Err(Error::new(
                wrt_error::ErrorCategory::Parameter,
                wrt_error::codes::EXECUTION_ERROR,
                "resource.drop requires exactly one argument",
            ));
        }

        // Extract the resource ID from args
        let id = match &args[0] {
            ComponentValue::U32(value) => ResourceId(*value),
            _ => {
                return Err(Error::runtime_execution_error(
                    "Expected U32 for resource ID",
                ));
            },
        };

        // Drop the resource
        let mut manager = self.resource_manager.lock().unwrap();
        if !manager.has_resource(id)? {
            return Err(Error::new(
                wrt_error::ErrorCategory::Resource,
                wrt_error::codes::RESOURCE_NOT_FOUND,
                "Resource not found",
            ));
        }

        manager.delete_resource(id)?;

        // Return empty result
        Ok(vec![])
    }

    #[cfg(not(feature = "std"))]
    fn execute(
        &self,
        args: &[ComponentValue<ComponentProvider>],
    ) -> Result<BoundedVec<ComponentValue<ComponentProvider>, 16>> {
        // Validate args
        if args.len() != 1 {
            return Err(Error::new(
                wrt_error::ErrorCategory::Parameter,
                wrt_error::codes::EXECUTION_ERROR,
                "resource.drop requires exactly one argument",
            ));
        }

        // Extract the resource ID from args
        let id = match &args[0] {
            ComponentValue::U32(value) => ResourceId(*value),
            _ => {
                return Err(Error::runtime_execution_error(
                    "Expected U32 for resource ID",
                ));
            },
        };

        // Drop the resource
        let mut manager = self.resource_manager.lock();
        if !manager.has_resource(id)? {
            return Err(Error::new(
                wrt_error::ErrorCategory::Resource,
                wrt_error::codes::RESOURCE_NOT_FOUND,
                "Resource not found",
            ));
        }

        manager.drop_resource(id.0)?;

        // Return empty result
        Ok(BoundedVec::new())
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(Self {
            resource_manager: self.resource_manager.clone(),
        })
    }
}

/// Handler for the resource.rep built-in function
pub struct ResourceRepHandler {
    resource_manager: Arc<Mutex<ResourceManager>>,
}

impl ResourceRepHandler {
    /// Create a new resource.rep handler
    pub fn new(resource_manager: Arc<Mutex<ResourceManager>>) -> Self {
        Self { resource_manager }
    }
}

impl BuiltinHandler for ResourceRepHandler {
    fn builtin_type(&self) -> crate::builtins::BuiltinType {
        crate::builtins::BuiltinType::ResourceRep
    }

    #[cfg(feature = "std")]
    fn execute(
        &self,
        args: &[ComponentValue<ComponentProvider>],
    ) -> Result<Vec<ComponentValue<ComponentProvider>>> {
        // Validate args
        if args.len() != 1 {
            return Err(Error::runtime_execution_error(
                "resource.rep requires exactly one argument",
            ));
        }

        // Extract the resource ID from args
        let id = match &args[0] {
            ComponentValue::U32(value) => ResourceId(*value),
            _ => {
                return Err(Error::new(
                    wrt_error::ErrorCategory::Parameter,
                    wrt_error::codes::TYPE_MISMATCH,
                    "Expected U32 or U64 for resource representation",
                ));
            },
        };

        // Get the resource representation
        let manager = self.resource_manager.lock().unwrap();
        if !manager.has_resource(id)? {
            return Err(Error::runtime_execution_error("Resource not found"));
        }

        // Get the resource as u32
        let resource = manager.get_host_resource::<u32>(id)?;
        let rep = *resource.lock().unwrap();

        // Return the representation
        Ok(vec![ComponentValue::U32(rep)])
    }

    #[cfg(not(feature = "std"))]
    fn execute(
        &self,
        args: &[ComponentValue<ComponentProvider>],
    ) -> Result<BoundedVec<ComponentValue<ComponentProvider>, 16>> {
        // Validate args
        if args.len() != 1 {
            return Err(Error::runtime_execution_error(
                "resource.rep requires exactly one argument",
            ));
        }

        // Extract the resource ID from args
        let id = match &args[0] {
            ComponentValue::U32(value) => ResourceId(*value),
            _ => {
                return Err(Error::new(
                    wrt_error::ErrorCategory::Parameter,
                    wrt_error::codes::TYPE_MISMATCH,
                    "Expected U32 or U64 for resource representation",
                ));
            },
        };

        // Get the resource representation
        let manager = self.resource_manager.lock();
        if !manager.has_resource(id)? {
            return Err(Error::runtime_execution_error("Resource not found"));
        }

        // Get the resource ID and then retrieve the actual resource representation
        let resource_id = manager.get_resource(id.0)?;
        let rep = manager.get_resource_representation(resource_id)?;

        // Return the representation
        let mut result = BoundedVec::new();
        result.push(ComponentValue::U32(rep)).map_err(|_| {
            Error::foundation_bounded_capacity_exceeded("Failed to add result value")
        })?;
        Ok(result)
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(Self {
            resource_manager: self.resource_manager.clone(),
        })
    }
}

/// Handler for the resource.get built-in function
pub struct ResourceGetHandler {
    resource_manager: Arc<Mutex<ResourceManager>>,
}

impl ResourceGetHandler {
    /// Create a new resource.get handler
    pub fn new(resource_manager: Arc<Mutex<ResourceManager>>) -> Self {
        Self { resource_manager }
    }
}

impl BuiltinHandler for ResourceGetHandler {
    fn builtin_type(&self) -> crate::builtins::BuiltinType {
        crate::builtins::BuiltinType::ResourceGet
    }

    #[cfg(feature = "std")]
    fn execute(
        &self,
        args: &[ComponentValue<ComponentProvider>],
    ) -> Result<Vec<ComponentValue<ComponentProvider>>> {
        // Validate args
        if args.len() != 1 {
            return Err(Error::new(
                wrt_error::ErrorCategory::Parameter,
                wrt_error::codes::EXECUTION_ERROR,
                "resource.get requires exactly one argument",
            ));
        }

        // Extract the resource representation from args
        let rep = match &args[0] {
            ComponentValue::U32(value) => *value,
            ComponentValue::U64(value) => *value as u32,
            _ => {
                return Err(Error::runtime_execution_error(
                    "Expected U32 for resource ID",
                ));
            },
        };

        // Find or create resource with this representation
        let mut manager = self.resource_manager.lock().unwrap();

        // For now, always create a new resource
        // TODO: Implement resource lookup once get_resources_iter is available
        let id = manager.add_host_resource(rep)?;
        Ok(vec![ComponentValue::U32(id.0)])
    }

    #[cfg(not(feature = "std"))]
    fn execute(
        &self,
        args: &[ComponentValue<ComponentProvider>],
    ) -> Result<BoundedVec<ComponentValue<ComponentProvider>, 16>> {
        // Validate args
        if args.len() != 1 {
            return Err(Error::new(
                wrt_error::ErrorCategory::Parameter,
                wrt_error::codes::EXECUTION_ERROR,
                "resource.get requires exactly one argument",
            ));
        }

        // Extract the resource representation from args
        let rep = match &args[0] {
            ComponentValue::U32(value) => *value,
            ComponentValue::U64(value) => *value as u32,
            _ => {
                return Err(Error::runtime_execution_error(
                    "Expected U32 for resource ID",
                ));
            },
        };

        // For now, always create a new resource in no_std mode
        // TODO: Implement resource lookup when resource iteration is available
        let id = {
            let manager = self.resource_manager.lock();
            drop(manager); // Release lock before calling
            self.resource_manager.lock().add_host_resource(rep)?
        };
        let handle = id.0;

        let mut result = BoundedVec::new();
        result.push(ComponentValue::U32(handle)).map_err(|_| {
            Error::foundation_bounded_capacity_exceeded("Failed to add result value")
        })?;
        Ok(result)
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(Self {
            resource_manager: self.resource_manager.clone(),
        })
    }
}

/// Create all resource-related built-in handlers
pub fn create_resource_handlers(
    resource_manager: Arc<Mutex<ResourceManager>>,
) -> Vec<Box<dyn BuiltinHandler>> {
    vec![
        Box::new(ResourceCreateHandler::new(resource_manager.clone())),
        Box::new(ResourceDropHandler::new(resource_manager.clone())),
        Box::new(ResourceRepHandler::new(resource_manager.clone())),
        Box::new(ResourceGetHandler::new(resource_manager.clone())),
    ]
}
