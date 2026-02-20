#[cfg(feature = "std")]
use std::{collections::BTreeMap, vec::Vec};

#[cfg(not(feature = "std"))]
use wrt_foundation::collections::StaticMap as BTreeMap;

// Type aliases for no_std compatibility
#[cfg(not(feature = "std"))]
type TypeBoundsMap<K, V> = BTreeMap<K, V, 64>;

use core::fmt;

#[cfg(feature = "std")]
use wrt_foundation::component_value::ComponentValue;
use wrt_foundation::{
    bounded::MAX_GENERATIVE_TYPES,
    budget_aware_provider::CrateId,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    safe_managed_alloc,
};

#[cfg(not(feature = "std"))]
// For no_std, use a simpler ComponentValue representation
use crate::types::Value as ComponentValue;
use crate::{
    generative_types::{BoundKind, TypeBound},
    types::{ComponentError, TypeId, ValType},
};

#[derive(Debug, Clone, PartialEq)]
pub struct TypeBoundsChecker {
    #[cfg(feature = "std")]
    type_hierarchy: BTreeMap<TypeId, BoundedVec<TypeRelation, MAX_GENERATIVE_TYPES>>,
    #[cfg(not(feature = "std"))]
    type_hierarchy: BTreeMap<TypeId, BoundedVec<TypeRelation, MAX_GENERATIVE_TYPES>, 32>,
    #[cfg(feature = "std")]
    cached_relations: BTreeMap<(TypeId, TypeId), RelationResult>,
    #[cfg(not(feature = "std"))]
    cached_relations: BTreeMap<(TypeId, TypeId), RelationResult, 64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeRelation {
    pub sub_type: TypeId,
    pub super_type: TypeId,
    pub relation_kind: RelationKind,
    pub confidence: RelationConfidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RelationKind {
    /// Types are equal
    Eq,
    /// sub_type is a subtype of super_type
    Sub,
    /// Types are unrelated
    #[default]
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RelationConfidence {
    /// Relation is definitively known
    Definite,
    /// Relation is inferred from other relations
    Inferred,
    /// Relation is assumed but not verified
    #[default]
    Assumed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RelationResult {
    /// Types satisfy the bound
    Satisfied,
    /// Types do not satisfy the bound
    Violated,
    /// Relationship is unknown/undecidable
    #[default]
    Unknown,
}

// Serialization trait implementations for TypeRelation
use wrt_foundation::traits::{ReadStream, WriteStream};
use wrt_foundation::{Checksum, MemoryProvider};
use wrt_runtime::{Checksummable, FromBytes, ToBytes};

impl Checksummable for TypeRelation {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.sub_type.update_checksum(checksum);
        self.super_type.update_checksum(checksum);
        match self.relation_kind {
            RelationKind::Eq => 0u8.update_checksum(checksum),
            RelationKind::Sub => 1u8.update_checksum(checksum),
            RelationKind::None => 2u8.update_checksum(checksum),
        }
        match self.confidence {
            RelationConfidence::Definite => 0u8.update_checksum(checksum),
            RelationConfidence::Inferred => 1u8.update_checksum(checksum),
            RelationConfidence::Assumed => 2u8.update_checksum(checksum),
        }
    }
}

impl ToBytes for TypeRelation {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.sub_type.to_bytes_with_provider(writer, provider)?;
        self.super_type.to_bytes_with_provider(writer, provider)?;
        let kind_byte = match self.relation_kind {
            RelationKind::Eq => 0u8,
            RelationKind::Sub => 1u8,
            RelationKind::None => 2u8,
        };
        kind_byte.to_bytes_with_provider(writer, provider)?;
        let conf_byte = match self.confidence {
            RelationConfidence::Definite => 0u8,
            RelationConfidence::Inferred => 1u8,
            RelationConfidence::Assumed => 2u8,
        };
        conf_byte.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for TypeRelation {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let sub_type = TypeId::from_bytes_with_provider(reader, provider)?;
        let super_type = TypeId::from_bytes_with_provider(reader, provider)?;
        let kind_byte = u8::from_bytes_with_provider(reader, provider)?;
        let relation_kind = match kind_byte {
            0 => RelationKind::Eq,
            1 => RelationKind::Sub,
            _ => RelationKind::None,
        };
        let conf_byte = u8::from_bytes_with_provider(reader, provider)?;
        let confidence = match conf_byte {
            0 => RelationConfidence::Definite,
            1 => RelationConfidence::Inferred,
            _ => RelationConfidence::Assumed,
        };
        Ok(Self {
            sub_type,
            super_type,
            relation_kind,
            confidence,
        })
    }
}

// Serialization trait implementations for RelationResult
impl Checksummable for RelationResult {
    fn update_checksum(&self, checksum: &mut Checksum) {
        match self {
            RelationResult::Satisfied => 0u8.update_checksum(checksum),
            RelationResult::Violated => 1u8.update_checksum(checksum),
            RelationResult::Unknown => 2u8.update_checksum(checksum),
        }
    }
}

impl ToBytes for RelationResult {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        let byte = match self {
            RelationResult::Satisfied => 0u8,
            RelationResult::Violated => 1u8,
            RelationResult::Unknown => 2u8,
        };
        byte.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for RelationResult {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let byte = u8::from_bytes_with_provider(reader, provider)?;
        Ok(match byte {
            0 => RelationResult::Satisfied,
            1 => RelationResult::Violated,
            _ => RelationResult::Unknown,
        })
    }
}

impl TypeBoundsChecker {
    pub fn new() -> Result<Self, ComponentError> {
        #[cfg(feature = "std")]
        {
            Ok(Self {
                type_hierarchy: BTreeMap::new(),
                cached_relations: BTreeMap::new(),
            })
        }
        #[cfg(not(feature = "std"))]
        {
            Ok(Self {
                type_hierarchy: BTreeMap::new(),
                cached_relations: BTreeMap::new(),
            })
        }
    }

    pub fn add_type_bound(&mut self, bound: TypeBound) -> core::result::Result<(), ComponentError> {
        let relation = TypeRelation {
            sub_type: bound.type_id,
            super_type: bound.target_type,
            relation_kind: match bound.bound_kind {
                BoundKind::Eq => RelationKind::Eq,
                BoundKind::Sub => RelationKind::Sub,
            },
            confidence: RelationConfidence::Definite,
        };

        self.add_relation(relation)?;
        self.invalidate_cache();
        Ok(())
    }

    pub fn check_type_bound(
        &mut self,
        type1: TypeId,
        type2: TypeId,
        bound_kind: BoundKind,
    ) -> RelationResult {
        let cache_key = (type1, type2);

        if let Some(cached) = self.cached_relations.get(&cache_key) {
            return cached.clone();
        }

        let result = match bound_kind {
            BoundKind::Eq => self.check_equality(type1, type2),
            BoundKind::Sub => self.check_subtype(type1, type2),
        };

        let _ = self.cached_relations.insert(cache_key, result.clone());
        result
    }

    pub fn check_equality(&self, type1: TypeId, type2: TypeId) -> RelationResult {
        if type1 == type2 {
            return RelationResult::Satisfied;
        }

        if let Some(relations) = self.type_hierarchy.get(&type1) {
            for relation in relations.iter() {
                if relation.super_type == type2 && relation.relation_kind == RelationKind::Eq {
                    return RelationResult::Satisfied;
                }
            }
        }

        if let Some(relations) = self.type_hierarchy.get(&type2) {
            for relation in relations.iter() {
                if relation.super_type == type1 && relation.relation_kind == RelationKind::Eq {
                    return RelationResult::Satisfied;
                }
            }
        }

        RelationResult::Violated
    }

    pub fn check_subtype(&self, sub_type: TypeId, super_type: TypeId) -> RelationResult {
        if sub_type == super_type {
            return RelationResult::Satisfied;
        }

        if let Some(relations) = self.type_hierarchy.get(&sub_type) {
            for relation in relations.iter() {
                match relation.relation_kind {
                    RelationKind::Sub | RelationKind::Eq => {
                        if relation.super_type == super_type {
                            return RelationResult::Satisfied;
                        }

                        let transitive_result = self.check_subtype(relation.super_type, super_type);
                        if transitive_result == RelationResult::Satisfied {
                            return RelationResult::Satisfied;
                        }
                    },
                    RelationKind::None => {},
                }
            }
        }

        RelationResult::Violated
    }

    pub fn infer_relations(&mut self) -> core::result::Result<usize, ComponentError> {
        let mut inferred_count = 0;
        let max_iterations = 10;

        for _ in 0..max_iterations {
            #[cfg(feature = "std")]
            let mut new_relations = Vec::new();
            #[cfg(not(feature = "std"))]
            let mut new_relations: BoundedVec<TypeRelation, 64> = {
                let provider = safe_managed_alloc!(65536, CrateId::Component)
                    .map_err(|_| ComponentError::TooManyTypeBounds)?;
                BoundedVec::new().map_err(|| ComponentError::TooManyTypeBounds)?
            };

            for (type_id, relations) in self.type_hierarchy.iter() {
                for relation in relations.iter() {
                    if let Some(super_relations) = self.type_hierarchy.get(&relation.super_type) {
                        for super_relation in super_relations.iter() {
                            let new_relation = TypeRelation {
                                sub_type: *type_id,
                                super_type: super_relation.super_type,
                                relation_kind: self.combine_relations(
                                    &relation.relation_kind,
                                    &super_relation.relation_kind,
                                ),
                                confidence: RelationConfidence::Inferred,
                            };

                            if !self.relation_exists(&new_relation) {
                                let _ = new_relations.push(new_relation);
                            }
                        }
                    }
                }
            }

            if new_relations.is_empty() {
                break;
            }

            for relation in new_relations {
                self.add_relation(relation)?;
                inferred_count += 1;
            }
        }

        self.invalidate_cache();
        Ok(inferred_count)
    }

    pub fn validate_consistency(&self) -> core::result::Result<(), ComponentError> {
        for (type_id, relations) in self.type_hierarchy.iter() {
            for relation in relations.iter() {
                if *type_id == relation.super_type && relation.relation_kind == RelationKind::Sub {
                    return Err(ComponentError::InvalidSubtypeRelation(
                        *type_id,
                        relation.super_type,
                    ));
                }

                if self.creates_cycle(*type_id, relation.super_type) {
                    return Err(ComponentError::InvalidSubtypeRelation(
                        *type_id,
                        relation.super_type,
                    ));
                }
            }
        }
        Ok(())
    }

    #[cfg(feature = "std")]
    pub fn get_all_supertypes(&self, type_id: TypeId) -> Vec<TypeId> {
        let mut supertypes = Vec::new();
        self.collect_supertypes(type_id, &mut supertypes);
        supertypes
    }

    #[cfg(not(feature = "std"))]
    pub fn get_all_supertypes(
        &self,
        type_id: TypeId,
    ) -> Result<BoundedVec<TypeId, 64>, ComponentError> {
        let provider = safe_managed_alloc!(65536, CrateId::Component)
            .map_err(|_| ComponentError::TooManyTypeBounds)?;
        let mut supertypes = BoundedVec::new().map_err(|| ComponentError::TooManyTypeBounds)?;
        self.collect_supertypes(type_id, &mut supertypes)?;
        Ok(supertypes)
    }

    #[cfg(feature = "std")]
    pub fn get_all_subtypes(&self, type_id: TypeId) -> Vec<TypeId> {
        let mut subtypes = Vec::new();

        for (sub_type_id, relations) in &self.type_hierarchy {
            for relation in relations.iter() {
                if relation.super_type == type_id
                    && (relation.relation_kind == RelationKind::Sub
                        || relation.relation_kind == RelationKind::Eq)
                {
                    subtypes.push(*sub_type_id);
                }
            }
        }

        subtypes
    }

    #[cfg(not(feature = "std"))]
    pub fn get_all_subtypes(
        &self,
        type_id: TypeId,
    ) -> Result<BoundedVec<TypeId, 64>, ComponentError> {
        let mut subtypes = BoundedVec::new().map_err(|| ComponentError::TooManyTypeBounds)?;

        for (sub_type_id, relations) in self.type_hierarchy.iter() {
            for relation in relations.iter() {
                if relation.super_type == type_id
                    && (relation.relation_kind == RelationKind::Sub
                        || relation.relation_kind == RelationKind::Eq)
                {
                    subtypes.push(*sub_type_id).map_err(|_| ComponentError::TooManyTypeBounds)?;
                }
            }
        }

        Ok(subtypes)
    }

    fn add_relation(&mut self, relation: TypeRelation) -> core::result::Result<(), ComponentError> {
        #[cfg(feature = "std")]
        {
            let relations =
                self.type_hierarchy.entry(relation.sub_type).or_insert_with(BoundedVec::new);
            let _ = relations.push(relation);
        }
        #[cfg(not(feature = "std"))]
        {
            let sub_type = relation.sub_type;
            // Check if the key exists, if not insert a new BoundedVec
            if let Some(existing_relations) = self.type_hierarchy.get(&sub_type) {
                // Key exists, clone the existing vector, add the relation, and re-insert
                let mut updated_relations = existing_relations.clone();
                updated_relations
                    .push(relation)
                    .map_err(|_| ComponentError::TooManyTypeBounds)?;
                self.type_hierarchy
                    .insert(sub_type, updated_relations)
                    .map_err(|_| ComponentError::TooManyTypeBounds)?;
            } else {
                // Key doesn't exist, create a new BoundedVec
                let mut new_vec =
                    BoundedVec::new().map_err(|| ComponentError::TooManyTypeBounds)?;
                new_vec.push(relation).map_err(|_| ComponentError::TooManyTypeBounds)?;
                self.type_hierarchy
                    .insert(sub_type, new_vec)
                    .map_err(|_| ComponentError::TooManyTypeBounds)?;
            }
        }

        Ok(())
    }

    fn relation_exists(&self, relation: &TypeRelation) -> bool {
        if let Some(relations) = self.type_hierarchy.get(&relation.sub_type) {
            relations.iter().any(|r| {
                r.super_type == relation.super_type && r.relation_kind == relation.relation_kind
            })
        } else {
            false
        }
    }

    fn combine_relations(&self, rel1: &RelationKind, rel2: &RelationKind) -> RelationKind {
        match (rel1, rel2) {
            (RelationKind::Eq, RelationKind::Eq) => RelationKind::Eq,
            (RelationKind::Eq, RelationKind::Sub) | (RelationKind::Sub, RelationKind::Eq) => {
                RelationKind::Sub
            },
            (RelationKind::Sub, RelationKind::Sub) => RelationKind::Sub,
            _ => RelationKind::None,
        }
    }

    fn creates_cycle(&self, start: TypeId, target: TypeId) -> bool {
        #[cfg(feature = "std")]
        {
            self.creates_cycle_helper(start, target, &mut Vec::new())
        }
        #[cfg(not(feature = "std"))]
        {
            // For no_std, use a simple approach without dynamic allocation
            // Check immediate cycles only to avoid allocation
            if let Some(relations) = self.type_hierarchy.get(&target) {
                relations.iter().any(|r| r.super_type == start)
            } else {
                false
            }
        }
    }

    #[cfg(feature = "std")]
    fn creates_cycle_helper(
        &self,
        current: TypeId,
        target: TypeId,
        visited: &mut Vec<TypeId>,
    ) -> bool {
        if visited.contains(&current) {
            return current == target;
        }

        visited.push(current);

        if let Some(relations) = self.type_hierarchy.get(&target) {
            for relation in relations.iter() {
                if relation.super_type == current {
                    return true;
                }

                if self.creates_cycle_helper(current, relation.super_type, visited) {
                    return true;
                }
            }
        }

        visited.pop();
        false
    }

    #[cfg(feature = "std")]
    fn collect_supertypes(&self, type_id: TypeId, supertypes: &mut Vec<TypeId>) {
        if let Some(relations) = self.type_hierarchy.get(&type_id) {
            for relation in relations.iter() {
                if !supertypes.contains(&relation.super_type) {
                    supertypes.push(relation.super_type);
                    self.collect_supertypes(relation.super_type, supertypes);
                }
            }
        }
    }

    #[cfg(not(feature = "std"))]
    fn collect_supertypes(
        &self,
        type_id: TypeId,
        supertypes: &mut BoundedVec<TypeId, 64>,
    ) -> Result<(), ComponentError> {
        if let Some(relations) = self.type_hierarchy.get(&type_id) {
            for relation in relations.iter() {
                if !supertypes.iter().any(|&id| id == relation.super_type) {
                    supertypes
                        .push(relation.super_type)
                        .map_err(|_| ComponentError::TooManyTypeBounds)?;
                    self.collect_supertypes(relation.super_type, supertypes)?;
                }
            }
        }
        Ok(())
    }

    fn invalidate_cache(&mut self) {
        self.cached_relations.clear();
    }
}

impl Default for TypeBoundsChecker {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback to empty structures on allocation failure
            // This should not happen in practice but satisfies the Default trait
            panic!("Failed to allocate memory for TypeBoundsChecker")
        })
    }
}

impl fmt::Display for RelationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelationResult::Satisfied => write!(f, "satisfied"),
            RelationResult::Violated => write!(f, "violated"),
            RelationResult::Unknown => write!(f, "unknown"),
        }
    }
}

impl fmt::Display for RelationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelationKind::Eq => write!(f, "="),
            RelationKind::Sub => write!(f, "<:"),
            RelationKind::None => write!(f, "none"),
        }
    }
}
