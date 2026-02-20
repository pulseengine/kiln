#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap;

use wrt_foundation::{
    BoundedVec,
    bounded_collections::{BoundedString, MAX_GENERATIVE_TYPES},
    prelude::*,
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

use crate::{
    async_types::{Future, FutureHandle, Stream, StreamHandle},
    generative_types::{BoundKind, GenerativeResourceType, GenerativeTypeRegistry, TypeBound},
    types::{ComponentError, ComponentInstanceId, TypeId, ValType},
};

use wrt_format::wit_parser::{
    WitFunction, WitInterface, WitParseError, WitParser, WitType, WitWorld,
};

// Type aliases for WIT integration - removed legacy NoStdProvider usage

#[derive(Debug, Clone)]
pub struct WitComponentBuilder {
    parser: WitParser,
    type_registry: GenerativeTypeRegistry,
    wit_type_mappings: BTreeMap<BoundedString<64>, TypeId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComponentInterface {
    pub name: BoundedString<64>,
    pub imports: BoundedVec<InterfaceFunction, MAX_GENERATIVE_TYPES>,
    pub exports: BoundedVec<InterfaceFunction, MAX_GENERATIVE_TYPES>,
    pub async_imports: BoundedVec<AsyncInterfaceFunction, MAX_GENERATIVE_TYPES>,
    pub async_exports: BoundedVec<AsyncInterfaceFunction, MAX_GENERATIVE_TYPES>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceFunction {
    pub name: BoundedString<64>,
    pub params: BoundedVec<TypedParam, 32>,
    pub results: BoundedVec<TypedResult, 16>,
    pub component_type_id: Option<TypeId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncInterfaceFunction {
    pub name: BoundedString<64>,
    pub params: BoundedVec<TypedParam, 32>,
    pub results: BoundedVec<AsyncTypedResult, 16>,
    pub component_type_id: Option<TypeId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedParam {
    pub name: BoundedString<32>,
    pub val_type: ValType,
    pub wit_type: WitType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedResult {
    pub name: Option<BoundedString<32>>,
    pub val_type: ValType,
    pub wit_type: WitType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncTypedResult {
    pub name: Option<BoundedString<32>>,
    pub val_type: ValType,
    pub wit_type: WitType,
    pub is_stream: bool,
    pub is_future: bool,
}

impl WitComponentBuilder {
    pub fn new() -> Self {
        Self {
            parser: WitParser::new(),
            type_registry: GenerativeTypeRegistry::new(),
            wit_type_mappings: BTreeMap::new(),
        }
    }

    pub fn parse_world_from_source(
        &mut self,
        source: &str,
        instance_id: ComponentInstanceId,
    ) -> core::result::Result<ComponentInterface, ComponentError> {
        let wit_world = self.parser.parse_world(source).map_err(|e| self.convert_parse_error(e))?;

        self.convert_world_to_interface(wit_world, instance_id)
    }

    pub fn parse_interface_from_source(
        &mut self,
        source: &str,
        instance_id: ComponentInstanceId,
    ) -> core::result::Result<ComponentInterface, ComponentError> {
        let wit_interface =
            self.parser.parse_interface(source).map_err(|e| self.convert_parse_error(e))?;

        self.convert_interface_to_component(wit_interface, instance_id)
    }

    pub fn register_wit_type(
        &mut self,
        wit_type_name: &str,
        component_type_id: TypeId,
    ) -> core::result::Result<(), ComponentError> {
        let provider = safe_managed_alloc!(512, CrateId::Component)
            .map_err(|_| ComponentError::TypeMismatch)?;
        let name =
            BoundedString::try_from_str(wit_type_name).map_err(|_| ComponentError::TypeMismatch)?;

        self.wit_type_mappings.insert(name, component_type_id;
        Ok(())
    }

    pub fn create_generative_type_from_wit(
        &mut self,
        wit_type: &WitType,
        instance_id: ComponentInstanceId,
    ) -> core::result::Result<GenerativeResourceType, ComponentError> {
        let val_type = self.parser.convert_to_valtype(wit_type)?;

        let base_resource_type = wrt_foundation::resource::ResourceType::Handle(
            ResourceHandle(0),
        ;

        self.type_registry.create_generative_type(base_resource_type, instance_id)
    }

    pub fn add_type_constraint(
        &mut self,
        type1: TypeId,
        type2: TypeId,
        constraint: BoundKind,
    ) -> core::result::Result<(), ComponentError> {
        let bound = TypeBound { type_id: type1, bound_kind: constraint, target_type: type2 };

        self.type_registry.add_type_bound(type1, bound)
    }

    fn convert_world_to_interface(
        &mut self,
        world: WitWorld,
        instance_id: ComponentInstanceId,
    ) -> core::result::Result<ComponentInterface, ComponentError> {
        let imports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let exports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let async_imports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let async_exports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        
        let mut interface = ComponentInterface {
            name: world.name,
            imports: BoundedVec::new(imports_provider)?,
            exports: BoundedVec::new(exports_provider)?,
            async_imports: BoundedVec::new(async_imports_provider)?,
            async_exports: BoundedVec::new(async_exports_provider)?,
        };

        for import in world.imports.iter() {
            match &import.item {
                crate::wit_parser::WitItem::Function(func) => {
                    if func.is_async {
                        let async_func =
                            self.convert_to_async_interface_function(func, instance_id)?;
                        interface
                            .async_imports
                            .push(async_func)
                            .map_err(|_| ComponentError::TooManyGenerativeTypes)?;
                    } else {
                        let interface_func =
                            self.convert_to_interface_function(func, instance_id)?;
                        interface
                            .imports
                            .push(interface_func)
                            .map_err(|_| ComponentError::TooManyGenerativeTypes)?;
                    }
                }
                _ => {} // Handle other item types as needed
            }
        }

        for export in world.exports.iter() {
            match &export.item {
                crate::wit_parser::WitItem::Function(func) => {
                    if func.is_async {
                        let async_func =
                            self.convert_to_async_interface_function(func, instance_id)?;
                        interface
                            .async_exports
                            .push(async_func)
                            .map_err(|_| ComponentError::TooManyGenerativeTypes)?;
                    } else {
                        let interface_func =
                            self.convert_to_interface_function(func, instance_id)?;
                        interface
                            .exports
                            .push(interface_func)
                            .map_err(|_| ComponentError::TooManyGenerativeTypes)?;
                    }
                }
                _ => {} // Handle other item types as needed
            }
        }

        Ok(interface)
    }

    fn convert_interface_to_component(
        &mut self,
        wit_interface: WitInterface,
        instance_id: ComponentInstanceId,
    ) -> core::result::Result<ComponentInterface, ComponentError> {
        let imports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let exports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let async_imports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let async_exports_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        
        let mut interface = ComponentInterface {
            name: wit_interface.name,
            imports: BoundedVec::new(imports_provider)?,
            exports: BoundedVec::new(exports_provider)?,
            async_imports: BoundedVec::new(async_imports_provider)?,
            async_exports: BoundedVec::new(async_exports_provider)?,
        };

        for func in wit_interface.functions.iter() {
            if func.is_async {
                let async_func = self.convert_to_async_interface_function(func, instance_id)?;
                interface
                    .async_exports
                    .push(async_func)
                    .map_err(|_| ComponentError::TooManyGenerativeTypes)?;
            } else {
                let interface_func = self.convert_to_interface_function(func, instance_id)?;
                interface
                    .exports
                    .push(interface_func)
                    .map_err(|_| ComponentError::TooManyGenerativeTypes)?;
            }
        }

        Ok(interface)
    }

    fn convert_to_interface_function(
        &mut self,
        wit_func: &WitFunction,
        instance_id: ComponentInstanceId,
    ) -> core::result::Result<InterfaceFunction, ComponentError> {
        let params_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let results_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        
        let mut interface_func = InterfaceFunction {
            name: wit_func.name.clone(),
            params: BoundedVec::new(params_provider)?,
            results: BoundedVec::new(results_provider)?,
            component_type_id: None,
        };

        for param in wit_func.params.iter() {
            let val_type = self.parser.convert_to_valtype(&param.ty)?;
            let typed_param =
                TypedParam { name: param.name.clone(), val_type, wit_type: param.ty.clone() };
            interface_func
                .params
                .push(typed_param)
                .map_err(|_| ComponentError::TooManyGenerativeTypes)?;
        }

        for result in wit_func.results.iter() {
            let val_type = self.parser.convert_to_valtype(&result.ty)?;
            let typed_result =
                TypedResult { name: result.name.clone(), val_type, wit_type: result.ty.clone() };
            interface_func
                .results
                .push(typed_result)
                .map_err(|_| ComponentError::TooManyGenerativeTypes)?;
        }

        Ok(interface_func)
    }

    fn convert_to_async_interface_function(
        &mut self,
        wit_func: &WitFunction,
        instance_id: ComponentInstanceId,
    ) -> core::result::Result<AsyncInterfaceFunction, ComponentError> {
        let params_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let results_provider = safe_managed_alloc!(65536, CrateId::Component)?;
        
        let mut async_func = AsyncInterfaceFunction {
            name: wit_func.name.clone(),
            params: BoundedVec::new(params_provider)?,
            results: BoundedVec::new(results_provider)?,
            component_type_id: None,
        };

        for param in wit_func.params.iter() {
            let val_type = self.parser.convert_to_valtype(&param.ty)?;
            let typed_param =
                TypedParam { name: param.name.clone(), val_type, wit_type: param.ty.clone() };
            async_func
                .params
                .push(typed_param)
                .map_err(|_| ComponentError::TooManyGenerativeTypes)?;
        }

        for result in wit_func.results.iter() {
            let val_type = self.parser.convert_to_valtype(&result.ty)?;
            let is_stream = matches!(result.ty, WitType::Stream(_;
            let is_future = matches!(result.ty, WitType::Future(_;

            let async_result = AsyncTypedResult {
                name: result.name.clone(),
                val_type,
                wit_type: result.ty.clone(),
                is_stream,
                is_future,
            };
            async_func
                .results
                .push(async_result)
                .map_err(|_| ComponentError::TooManyGenerativeTypes)?;
        }

        Ok(async_func)
    }

    fn convert_parse_error(&self, error: WitParseError) -> ComponentError {
        match error {
            WitParseError::UnknownType(_) => ComponentError::TypeMismatch,
            WitParseError::TooManyItems => ComponentError::TooManyGenerativeTypes,
            WitParseError::InvalidIdentifier(_) => ComponentError::TypeMismatch,
            WitParseError::DuplicateDefinition(_) => ComponentError::TooManyGenerativeTypes,
            _ => ComponentError::InstantiationFailed,
        }
    }

    pub fn get_type_registry(&self) -> &GenerativeTypeRegistry {
        &self.type_registry
    }

    pub fn get_type_registry_mut(&mut self) -> &mut GenerativeTypeRegistry {
        &mut self.type_registry
    }
}

impl Default for WitComponentBuilder {
    fn default() -> Self {
        Self::new()
    }

}
