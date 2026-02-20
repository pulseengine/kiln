#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, vec::Vec};
use core::sync::atomic::{AtomicU32, Ordering};
#[cfg(feature = "std")]
use std::{collections::BTreeMap, vec::Vec};

use wrt_foundation::{
    bounded::{BoundedVec, MAX_GENERATIVE_TYPES},
    budget_aware_provider::CrateId,
    component_value::ComponentValue,
    resource::ResourceType,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
};

use crate::{
    bounded_component_infra::ComponentProvider,
    resource_management::ResourceHandle,
    type_bounds::{RelationResult, TypeBoundsChecker},
    types::{ComponentError, ComponentInstanceId, ResourceId, TypeId},
};

#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub struct GenerativeResourceType {
    pub base_type: ResourceType<ComponentProvider>,
    pub instance_id: ComponentInstanceId,
    pub unique_type_id: TypeId,
    pub generation: u32,
}

impl wrt_foundation::traits::Checksummable for GenerativeResourceType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.base_type.update_checksum(checksum);
        self.instance_id.0.update_checksum(checksum);
        self.unique_type_id.0.update_checksum(checksum);
        self.generation.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for GenerativeResourceType {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.base_type.to_bytes_with_provider(writer, provider)?;
        self.instance_id.0.to_bytes_with_provider(writer, provider)?;
        self.unique_type_id.0.to_bytes_with_provider(writer, provider)?;
        self.generation.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for GenerativeResourceType {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self {
            base_type: ResourceType::from_bytes_with_provider(reader, provider)?,
            instance_id: ComponentInstanceId(u32::from_bytes_with_provider(reader, provider)?),
            unique_type_id: TypeId(u32::from_bytes_with_provider(reader, provider)?),
            generation: u32::from_bytes_with_provider(reader, provider)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub struct TypeBound {
    pub type_id: TypeId,
    pub bound_kind: BoundKind,
    pub target_type: TypeId,
}

impl wrt_foundation::traits::Checksummable for TypeBound {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.type_id.0.update_checksum(checksum);
        match self.bound_kind {
            BoundKind::Eq => 0u8.update_checksum(checksum),
            BoundKind::Sub => 1u8.update_checksum(checksum),
        }
        self.target_type.0.update_checksum(checksum);
    }
}

impl wrt_foundation::traits::ToBytes for TypeBound {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.type_id.0.to_bytes_with_provider(writer, provider)?;
        match self.bound_kind {
            BoundKind::Eq => 0u8.to_bytes_with_provider(writer, provider)?,
            BoundKind::Sub => 1u8.to_bytes_with_provider(writer, provider)?,
        }
        self.target_type.0.to_bytes_with_provider(writer, provider)
    }
}

impl wrt_foundation::traits::FromBytes for TypeBound {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let type_id = TypeId(u32::from_bytes_with_provider(reader, provider)?);
        let bound_kind_byte = u8::from_bytes_with_provider(reader, provider)?;
        let bound_kind = match bound_kind_byte {
            0 => BoundKind::Eq,
            1 => BoundKind::Sub,
            _ => BoundKind::Eq, // Default fallback
        };
        let target_type = TypeId(u32::from_bytes_with_provider(reader, provider)?);
        Ok(Self {
            type_id,
            bound_kind,
            target_type,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum BoundKind {
    #[default]
    Eq,
    Sub,
}

#[derive(Debug)]
pub struct GenerativeTypeRegistry {
    next_type_id: AtomicU32,
    instance_types: BTreeMap<
        ComponentInstanceId,
        BoundedVec<GenerativeResourceType, MAX_GENERATIVE_TYPES, NoStdProvider<65536>>,
    >,
    type_bounds:
        BTreeMap<TypeId, BoundedVec<TypeBound, MAX_GENERATIVE_TYPES, NoStdProvider<65536>>>,
    resource_mappings: BTreeMap<ResourceHandle, GenerativeResourceType>,
    bounds_checker: TypeBoundsChecker,
}

impl GenerativeTypeRegistry {
    pub fn new() -> Self {
        Self {
            next_type_id: AtomicU32::new(1),
            instance_types: BTreeMap::new(),
            type_bounds: BTreeMap::new(),
            resource_mappings: BTreeMap::new(),
            bounds_checker: TypeBoundsChecker::new().expect("Failed to create TypeBoundsChecker"),
        }
    }

    pub fn create_generative_type(
        &mut self,
        base_type: ResourceType<ComponentProvider>,
        instance_id: ComponentInstanceId,
    ) -> core::result::Result<GenerativeResourceType, ComponentError> {
        let unique_type_id = TypeId(self.next_type_id.fetch_add(1, Ordering::SeqCst));

        let generative_type = GenerativeResourceType {
            base_type,
            instance_id,
            unique_type_id,
            generation: 0,
        };

        let instance_types = self.instance_types.entry(instance_id).or_insert_with(|| {
            let provider = safe_managed_alloc!(65536, CrateId::Component)
                .expect("Failed to allocate memory for instance types");
            BoundedVec::new(provider).expect("Failed to create BoundedVec")
        });

        instance_types
            .push(generative_type.clone())
            .map_err(|_| ComponentError::TooManyGenerativeTypes)?;

        Ok(generative_type)
    }

    pub fn get_generative_type(
        &self,
        type_id: TypeId,
        instance_id: ComponentInstanceId,
    ) -> Option<GenerativeResourceType> {
        let instance_types = self.instance_types.get(&instance_id)?;
        for i in 0..instance_types.len() {
            if let Ok(t) = instance_types.get(i) {
                if t.unique_type_id == type_id {
                    return Some(t);
                }
            }
        }
        None
    }

    pub fn add_type_bound(
        &mut self,
        type_id: TypeId,
        bound: TypeBound,
    ) -> core::result::Result<(), ComponentError> {
        let bounds = self.type_bounds.entry(type_id).or_insert_with(|| {
            let provider = safe_managed_alloc!(65536, CrateId::Component)
                .expect("Failed to allocate memory for type bounds");
            BoundedVec::new(provider).expect("Failed to create BoundedVec")
        });

        bounds.push(bound.clone()).map_err(|_| ComponentError::TooManyTypeBounds)?;

        self.bounds_checker.add_type_bound(bound)?;

        Ok(())
    }

    pub fn check_type_bound(
        &mut self,
        type_id: TypeId,
        target_type: TypeId,
        bound_kind: BoundKind,
    ) -> RelationResult {
        self.bounds_checker.check_type_bound(type_id, target_type, bound_kind)
    }

    pub fn check_type_bound_simple(
        &self,
        type_id: TypeId,
        target_type: TypeId,
        bound_kind: BoundKind,
    ) -> bool {
        if let Some(bounds) = self.type_bounds.get(&type_id) {
            bounds
                .iter()
                .any(|bound| bound.target_type == target_type && bound.bound_kind == bound_kind)
        } else {
            false
        }
    }

    pub fn register_resource_handle(
        &mut self,
        handle: ResourceHandle,
        generative_type: GenerativeResourceType,
    ) -> core::result::Result<(), ComponentError> {
        if self.resource_mappings.contains_key(&handle) {
            return Err(ComponentError::ResourceHandleAlreadyExists);
        }

        self.resource_mappings.insert(handle, generative_type);
        Ok(())
    }

    pub fn get_resource_type(&self, handle: ResourceHandle) -> Option<&GenerativeResourceType> {
        self.resource_mappings.get(&handle)
    }

    pub fn is_same_instance_type(
        &self,
        type1: TypeId,
        type2: TypeId,
        instance_id: ComponentInstanceId,
    ) -> bool {
        if let Some(instance_types) = self.instance_types.get(&instance_id) {
            let t1 = instance_types.iter().find(|t| t.unique_type_id == type1);
            let t2 = instance_types.iter().find(|t| t.unique_type_id == type2);

            match (t1, t2) {
                (Some(type1), Some(type2)) => type1.base_type == type2.base_type,
                _ => false,
            }
        } else {
            false
        }
    }

    pub fn get_instance_types(&self, instance_id: ComponentInstanceId) -> Vec<TypeId> {
        if let Some(types) = self.instance_types.get(&instance_id) {
            types.iter().map(|t| t.unique_type_id).collect()
        } else {
            Vec::new()
        }
    }

    pub fn cleanup_instance(&mut self, instance_id: ComponentInstanceId) {
        if let Some(types) = self.instance_types.remove(&instance_id) {
            for generative_type in types.iter() {
                self.type_bounds.remove(&generative_type.unique_type_id);

                self.resource_mappings
                    .retain(|_, mapped_type| mapped_type.instance_id != instance_id);
            }
        }
    }

    pub fn infer_type_relations(&mut self) -> core::result::Result<usize, ComponentError> {
        self.bounds_checker.infer_relations()
    }

    pub fn validate_type_consistency(&self) -> core::result::Result<(), ComponentError> {
        self.bounds_checker.validate_consistency()
    }

    pub fn get_all_supertypes(&self, type_id: TypeId) -> Vec<TypeId> {
        #[cfg(feature = "std")]
        {
            self.bounds_checker.get_all_supertypes(type_id)
        }
        #[cfg(not(feature = "std"))]
        {
            self.bounds_checker
                .get_all_supertypes(type_id)
                .map(|vec| vec.iter().copied().collect())
                .unwrap_or_else(|_| Vec::new())
        }
    }

    pub fn get_all_subtypes(&self, type_id: TypeId) -> Vec<TypeId> {
        #[cfg(feature = "std")]
        {
            self.bounds_checker.get_all_subtypes(type_id)
        }
        #[cfg(not(feature = "std"))]
        {
            self.bounds_checker
                .get_all_subtypes(type_id)
                .map(|vec| vec.iter().copied().collect())
                .unwrap_or_else(|_| Vec::new())
        }
    }

    #[cfg(feature = "std")]
    pub fn validate_type_system(&mut self) -> core::result::Result<(), ComponentError> {
        self.infer_type_relations()?;
        self.validate_type_consistency()?;

        for (type_id, bounds) in &self.type_bounds {
            for bound in bounds.iter() {
                if !self.is_valid_type_reference(bound.target_type) {
                    return Err(ComponentError::InvalidTypeReference(
                        *type_id,
                        bound.target_type,
                    ));
                }

                if let BoundKind::Sub = bound.bound_kind {
                    let result = self.bounds_checker.check_subtype(*type_id, bound.target_type);
                    if result != RelationResult::Satisfied {
                        return Err(ComponentError::InvalidSubtypeRelation(
                            *type_id,
                            bound.target_type,
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn is_valid_type_reference(&self, type_id: TypeId) -> bool {
        self.instance_types
            .values()
            .any(|types| types.iter().any(|t| t.unique_type_id == type_id))
    }

    fn check_subtype_relation(&self, sub_type: TypeId, super_type: TypeId) -> bool {
        for instance_types in self.instance_types.values() {
            let sub = instance_types.iter().find(|t| t.unique_type_id == sub_type);
            let sup = instance_types.iter().find(|t| t.unique_type_id == super_type);

            if let (Some(sub_t), Some(sup_t)) = (sub, sup) {
                return self.is_resource_subtype(&sub_t.base_type, &sup_t.base_type);
            }
        }
        false
    }

    fn is_resource_subtype(
        &self,
        sub_type: &ResourceType<ComponentProvider>,
        super_type: &ResourceType<ComponentProvider>,
    ) -> bool {
        match (sub_type, super_type) {
            (
                ResourceType::<ComponentProvider>::Handle(sub_h),
                ResourceType::<ComponentProvider>::Handle(super_h),
            ) => sub_h == super_h,
            _ => false,
        }
    }
}

impl Default for GenerativeTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
