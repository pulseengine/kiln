// WRT - wrt-runtime
// Module: Component Type Stubs
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Component type stubs for runtime integration
//!
//! These types provide the interface between the runtime
//! and the Component Model implementation.

#![allow(dead_code)] // Allow during stub phase

/// Component instance stub
#[derive(Debug, Clone)]
pub struct ComponentInstance {
    pub id: ComponentId,
}

impl ComponentInstance {
    pub fn new(id: ComponentId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> ComponentId {
        self.id
    }
}

/// Component identifier stub
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub u32);

impl ComponentId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}

/// Component type stub
#[derive(Debug, Clone)]
pub struct ComponentType {
    pub name: &'static str,
}

impl ComponentType {
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

/// Component requirements stub
#[derive(Debug, Clone)]
pub struct ComponentRequirements {
    pub component_count: usize,
    pub resource_count:  usize,
    pub memory_usage:    usize,
}

impl Default for ComponentRequirements {
    fn default() -> Self {
        Self {
            component_count: 1,
            resource_count:  0,
            memory_usage:    4096, // 4KB default
        }
    }
}

/// Component memory budget stub
#[derive(Debug, Clone)]
pub struct ComponentMemoryBudget {
    pub total_memory:       usize,
    pub component_overhead: usize,
    pub available_memory:   usize,
}

impl ComponentMemoryBudget {
    pub fn calculate(limits: &wrt_foundation::PlatformLimits) -> Result<Self, wrt_error::Error> {
        let component_overhead = limits.max_memory / 100; // 1% overhead
        let available_memory = limits.max_memory.saturating_sub(component_overhead);

        Ok(Self {
            total_memory: limits.max_memory,
            component_overhead,
            available_memory,
        })
    }
}
