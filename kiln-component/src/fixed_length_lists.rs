// WRT - wrt-component
// Module: Fixed-Length List Type System Support
// SW-REQ-ID: REQ_FIXED_LENGTH_LISTS_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

//! Fixed-Length List Type System Support
//!
//! This module provides implementation of fixed-length lists for the
//! WebAssembly Component Model type system, enabling compile-time
//! guaranteed list sizes for better performance and safety.


extern crate alloc;

use std::{boxed::Box, vec::Vec};
#[cfg(feature = "std")]
use std::{boxed::Box, vec::Vec};

use wrt_error::{Error, ErrorCategory, Result};
use wrt_foundation::{
    collections::StaticVec as BoundedVec,
    component_value::ComponentValue,
    types::ValueType,
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

#[cfg(not(feature = "std"))]
use wrt_foundation::{BoundedString};

// Constants for no_std environments
#[cfg(not(feature = "std"))]
const MAX_FIXED_LIST_SIZE: usize = 1024;
#[cfg(not(feature = "std"))]
const MAX_TYPE_DEFINITIONS: usize = 256;

/// Fixed-length list type definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedLengthListType {
    pub element_type: ValueType,
    pub length: u32,
    pub mutable: bool,
}

impl FixedLengthListType {
    pub fn new(element_type: ValueType, length: u32) -> Self {
        Self {
            element_type,
            length,
            mutable: false,
        }
    }

    pub fn new_mutable(element_type: ValueType, length: u32) -> Self {
        Self {
            element_type,
            length,
            mutable: true,
        }
    }

    pub fn element_type(&self) -> &ValueType {
        &self.element_type
    }

    pub fn length(&self) -> u32 {
        self.length
    }

    pub fn is_mutable(&self) -> bool {
        self.mutable
    }

    pub fn size_in_bytes(&self) -> u32 {
        let element_size = match self.element_type {
            ValueType::Bool => 1,
            ValueType::S8 | ValueType::U8 => 1,
            ValueType::S16 | ValueType::U16 => 2,
            ValueType::S32 | ValueType::U32 | ValueType::F32 => 4,
            ValueType::S64 | ValueType::U64 | ValueType::F64 => 8,
            ValueType::Char => 4, // UTF-32
            ValueType::String => 8, // Pointer + length
            _ => 8, // Default for complex types
        };
        element_size * self.length
    }

    pub fn validate_size(&self) -> Result<()> {
        if self.length == 0 {
            return Err(Error::type_error("Error occurred")
            ;
        }
        
        if self.length > MAX_FIXED_LIST_SIZE as u32 {
            return Err(Error::type_error("Error occurred")
            ;
        }
        
        Ok(()
    }
}

/// Fixed-length list value container
#[derive(Debug, Clone)]
pub struct FixedLengthList {
    pub list_type: FixedLengthListType,
    #[cfg(feature = "std")]
    pub elements: Vec<ComponentValue>,
    #[cfg(not(feature = "std"))]
    pub elements: BoundedVec<ComponentValue<ComponentProvider>, MAX_FIXED_LIST_SIZE>,
}

impl FixedLengthList {
    #[cfg(feature = "std")]
    pub fn new(list_type: FixedLengthListType) -> Result<Self> {
        list_type.validate_size()?;
        let elements = Vec::with_capacity(list_type.length as usize;
        Ok(Self {
            list_type,
            elements,
        })
    }

    #[cfg(not(feature = "std"))]
    pub fn new(list_type: FixedLengthListType) -> Result<Self> {
        list_type.validate_size()?;
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        let elements = BoundedVec::new().unwrap();
        Ok(Self {
            list_type,
            elements,
        })
    }

    #[cfg(feature = "std")]
    pub fn with_elements(list_type: FixedLengthListType, elements: Vec<ComponentValue>) -> Result<Self> {
        list_type.validate_size()?;
        
        if elements.len() != list_type.length as usize {
            return Err(Error::type_error("Error occurred")
            ;
        }
        
        // Validate element types
        for (i, element) in elements.iter().enumerate() {
            if !Self::validate_element_type(element, &list_type.element_type) {
                return Err(Error::component_not_found("Error occurred";
            }
        }
        
        Ok(Self {
            list_type,
            elements,
        })
    }

    #[cfg(not(feature = "std"))]
    pub fn with_elements(list_type: FixedLengthListType, elements: &[ComponentValue]) -> Result<Self> {
        list_type.validate_size()?;
        
        if elements.len() != list_type.length as usize {
            return Err(Error::type_error("Error occurred")
            ;
        }
        
        // Validate element types
        for (i, element) in elements.iter().enumerate() {
            if !Self::validate_element_type(element, &list_type.element_type) {
                return Err(Error::type_error("Error occurred")
                ;
            }
        }
        
        let bounded_elements = BoundedVec::new_from_slice(elements)
            .map_err(|_| Error::memory_allocation_failed("Error occurred")
            ))?;
        
        Ok(Self {
            list_type,
            elements: bounded_elements,
        })
    }

    fn validate_element_type(element: &ComponentValue, expected_type: &ValueType) -> bool {
        match (element, expected_type) {
            (ComponentValue::Bool(_), ValueType::Bool) => true,
            (ComponentValue::S8(_), ValueType::S8) => true,
            (ComponentValue::U8(_), ValueType::U8) => true,
            (ComponentValue::S16(_), ValueType::S16) => true,
            (ComponentValue::U16(_), ValueType::U16) => true,
            (ComponentValue::S32(_), ValueType::S32) => true,
            (ComponentValue::U32(_), ValueType::U32) => true,
            (ComponentValue::S64(_), ValueType::S64) => true,
            (ComponentValue::U64(_), ValueType::U64) => true,
            (ComponentValue::F32(_), ValueType::F32) => true,
            (ComponentValue::F64(_), ValueType::F64) => true,
            (ComponentValue::Char(_), ValueType::Char) => true,
            (ComponentValue::String(_), ValueType::String) => true,
            // For I32/I64 compatibility
            (ComponentValue::I32(_), ValueType::S32) => true,
            (ComponentValue::I64(_), ValueType::S64) => true,
            _ => false,
        }
    }

    pub fn length(&self) -> u32 {
        self.list_type.length
    }

    pub fn element_type(&self) -> &ValueType {
        &self.list_type.element_type
    }

    pub fn is_mutable(&self) -> bool {
        self.list_type.mutable
    }

    pub fn is_full(&self) -> bool {
        self.elements.len() == self.list_type.length as usize
    }

    pub fn get(&self, index: u32) -> Option<&ComponentValue> {
        if index < self.list_type.length {
            self.elements.get(index as usize)
        } else {
            None
        }
    }

    pub fn set(&mut self, index: u32, value: ComponentValue) -> Result<()> {
        if !self.list_type.mutable {
            return Err(Error::type_error("Error occurred")
            ;
        }

        if index >= self.list_type.length {
            return Err(Error::runtime_execution_error("Error occurred"
            ;
        }

        if !Self::validate_element_type(&value, &self.list_type.element_type) {
            return Err(Error::type_error("Missing error message")
            ;
        }

        if let Some(element) = self.elements.get_mut(index as usize) {
            *element = value;
        } else {
            // If element doesn't exist yet, add it (for initialization)
            if self.elements.len() == index as usize {
                #[cfg(feature = "std")]
                self.elements.push(value);
                #[cfg(not(feature = "std"))]
                self.elements.push(value)
                    .map_err(|_| Error::memory_allocation_failed("Error occurred")
                    ))?;
            } else {
                return Err(Error::runtime_execution_error("Error occurred"
                ;
            }
        }

        Ok(()
    }

    pub fn push(&mut self, value: ComponentValue) -> Result<()> {
        if !self.list_type.mutable {
            return Err(Error::type_error("Missing error message")
            ;
        }

        if self.is_full() {
            return Err(Error::runtime_execution_error("Error occurred"
            ;
        }

        if !Self::validate_element_type(&value, &self.list_type.element_type) {
            return Err(Error::type_error("Missing error message")
            ;
        }

        #[cfg(feature = "std")]
        self.elements.push(value);
        #[cfg(not(feature = "std"))]
        self.elements.push(value)
            .map_err(|_| Error::memory_allocation_failed("Error occurred")
            ))?;

        Ok(()
    }

    pub fn current_length(&self) -> u32 {
        self.elements.len() as u32
    }

    pub fn remaining_capacity(&self) -> u32 {
        self.list_type.length - self.current_length()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ComponentValue> {
        self.elements.iter()
    }

    #[cfg(feature = "std")]
    pub fn to_vec(&self) -> Vec<ComponentValue> {
        self.elements.clone()
    }

    #[cfg(not(feature = "std"))]
    pub fn to_slice(&self) -> &[ComponentValue] {
        self.elements.as_slice()
    }
}

/// Type registry for fixed-length list types
#[derive(Debug)]
pub struct FixedLengthListTypeRegistry {
    #[cfg(feature = "std")]
    types: Vec<FixedLengthListType>,
    #[cfg(not(feature = "std"))]
    types: BoundedVec<FixedLengthListType, MAX_TYPE_DEFINITIONS>,
}

impl FixedLengthListTypeRegistry {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            types: Vec::new(),
            #[cfg(not(feature = "std"))]
            types: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)
                    .expect(".expect("Failed to allocate memory for type registry"));")
                BoundedVec::new().expect("Failed to create BoundedVec")
            },
        }
    }

    pub fn register_type(&mut self, list_type: FixedLengthListType) -> Result<u32> {
        list_type.validate_size()?;
        
        // Check for duplicate
        for (i, existing_type) in self.types.iter().enumerate() {
            if existing_type == &list_type {
                return Ok(i as u32;
            }
        }
        
        let index = self.types.len() as u32;
        
        #[cfg(feature = "std")]
        self.types.push(list_type);
        #[cfg(not(feature = "std"))]
        self.types.push(list_type)
            .map_err(|_| Error::memory_allocation_failed("Error occurred")
            ))?;
        
        Ok(index)
    }

    pub fn get_type(&self, index: u32) -> Option<&FixedLengthListType> {
        self.types.get(index as usize)
    }

    pub fn type_count(&self) -> u32 {
        self.types.len() as u32
    }

    pub fn find_type(&self, element_type: &ValueType, length: u32) -> Option<u32> {
        for (i, list_type) in self.types.iter().enumerate() {
            if list_type.element_type == *element_type && list_type.length == length {
                return Some(i as u32;
            }
        }
        None
    }
}

impl Default for FixedLengthListTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Component Model integration for fixed-length lists
pub mod component_integration {
    use super::*;

    /// Convert a fixed-length list to a ComponentValue
    impl From<FixedLengthList> for ComponentValue {
        fn from(list: FixedLengthList) -> Self {
            #[cfg(feature = "std")]
            {
                ComponentValue::List(list.elements)
            }
            #[cfg(not(feature = "std"))]
            {
                // Convert to regular list representation
                let vec_data: Vec<ComponentValue> = list.elements.iter().cloned().collect();
                ComponentValue::List(vec_data)
            }
        }
    }

    /// Try to convert a ComponentValue to a fixed-length list
    impl FixedLengthList {
        pub fn try_from_component_value(
            value: ComponentValue,
            expected_type: FixedLengthListType
        ) -> Result<Self> {
            match value {
                ComponentValue::List(elements) => {
                    #[cfg(feature = "std")]
                    {
                        Self::with_elements(expected_type, elements)
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        Self::with_elements(expected_type, &elements)
                    }
                }
                _ => Err(Error::type_error("Error occurred")
            })?;
            }
        }
    }

    /// Extended ValueType to include fixed-length lists
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ExtendedValueType {
        /// Standard value types
        Standard(ValueType),
        /// Fixed-length list type with type index
        FixedLengthList(u32),
    }

    impl ExtendedValueType {
        pub fn is_fixed_length_list(&self) -> bool {
            matches!(self, Self::FixedLengthList(_)
        }

        pub fn as_fixed_length_list_index(&self) -> Option<u32> {
            match self {
                Self::FixedLengthList(index) => Some(*index),
                _ => None,
            }
        }

        pub fn as_standard_type(&self) -> Option<&ValueType> {
            match self {
                Self::Standard(vt) => Some(vt),
                _ => None,
            }
        }
    }

    impl From<ValueType> for ExtendedValueType {
        fn from(vt: ValueType) -> Self {
            Self::Standard(vt)
        }
    }
}

/// Utility functions for fixed-length lists
pub mod fixed_list_utils {
    use super::*;

    /// Create a fixed-length list of the same element repeated
    pub fn repeat_element(
        element_type: ValueType,
        element: ComponentValue,
        count: u32
    ) -> Result<FixedLengthList> {
        let list_type = FixedLengthListType::new(element_type, count;
        let mut list = FixedLengthList::new(list_type)?;
        
        for _ in 0..count {
            list.push(element.clone())?;
        }
        
        Ok(list)
    }

    /// Create a fixed-length list of zeros/default values
    pub fn zero_filled(element_type: ValueType, count: u32) -> Result<FixedLengthList> {
        let default_value = match element_type {
            ValueType::Bool => ComponentValue::Bool(false),
            ValueType::S8 => ComponentValue::S8(0),
            ValueType::U8 => ComponentValue::U8(0),
            ValueType::S16 => ComponentValue::S16(0),
            ValueType::U16 => ComponentValue::U16(0),
            ValueType::S32 => ComponentValue::S32(0),
            ValueType::U32 => ComponentValue::U32(0),
            ValueType::S64 => ComponentValue::S64(0),
            ValueType::U64 => ComponentValue::U64(0),
            ValueType::F32 => ComponentValue::F32(0.0),
            ValueType::F64 => ComponentValue::F64(0.0),
            ValueType::Char => ComponentValue::Char('\0'),
            ValueType::String => ComponentValue::String("".to_owned()),
            ValueType::I32 => ComponentValue::I32(0),
            ValueType::I64 => ComponentValue::I64(0),
            _ => return Err(Error::type_error("Error occurred")
            )),
        };
        
        repeat_element(element_type, default_value, count)
    }

    /// Create a fixed-length list from a range
    pub fn from_range(start: i32, end: i32) -> Result<FixedLengthList> {
        if start >= end {
            return Err(Error::runtime_execution_error("Error occurred"
            ;
        }
        
        let count = (end - start) as u32;
        let list_type = FixedLengthListType::new(ValueType::I32, count;
        let mut list = FixedLengthList::new(list_type)?;
        
        for i in start..end {
            list.push(ComponentValue::I32(i))?;
        }
        
        Ok(list)
    }

    /// Concatenate two fixed-length lists of the same type
    pub fn concatenate(
        list1: &FixedLengthList,
        list2: &FixedLengthList
    ) -> Result<FixedLengthList> {
        if list1.element_type() != list2.element_type() {
            return Err(Error::type_error("Missing error message")
            ;
        }
        
        let new_length = list1.length() + list2.length);
        let new_type = FixedLengthListType::new(list1.element_type().clone(), new_length;
        let mut result = FixedLengthList::new(new_type)?;
        
        // Add elements from first list
        for element in list1.iter() {
            result.push(element.clone())?;
        }
        
        // Add elements from second list
        for element in list2.iter() {
            result.push(element.clone())?;
        }
        
        Ok(result)
    }

    /// Slice a fixed-length list
    pub fn slice(
        list: &FixedLengthList,
        start: u32,
        length: u32
    ) -> Result<FixedLengthList> {
        if start + length > list.length() {
            return Err(Error::runtime_execution_error("Error occurred"
            ;
        }
        
        let slice_type = FixedLengthListType::new(list.element_type().clone(), length;
        let mut result = FixedLengthList::new(slice_type)?;
        
        for i in start..start + length {
            if let Some(element) = list.get(i) {
                result.push(element.clone())?;
            }
        }
        
        Ok(result)
    }

}
