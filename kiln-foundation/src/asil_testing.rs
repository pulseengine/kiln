// WRT - wrt-foundation
// Module: ASIL-Tagged Testing Framework
// SW-REQ-ID: REQ_TEST_ASIL_001, REQ_SAFETY_VERIFY_001, REQ_SCORE_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! ASIL-Tagged Testing Framework
//!
//! This module provides macros and utilities for categorizing tests by
//! Automotive Safety Integrity Level (ASIL) as part of the SCORE-inspired
//! safety verification framework.

#![allow(unsafe_code)]

// Import appropriate types based on environment
#[cfg(not(feature = "std"))]
use core::sync::atomic::{
    AtomicBool,
    Ordering,
};
#[cfg(feature = "std")]
use std::{
    sync::Mutex,
    vec::Vec,
};

// For no_std mode, use bounded collections
#[cfg(not(feature = "std"))]
use crate::bounded::BoundedVec;
#[cfg(not(feature = "std"))]
use crate::safe_memory::NoStdProvider;
use crate::safety_system::AsilLevel;

// For no_std environments, use simple arrays or bounded collections
#[cfg(not(feature = "std"))]
const MAX_TESTS_NO_STD: usize = 64;

// For no_std without alloc, use simple arrays
#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
type TestRegistry = [Option<AsilTestMetadata>; MAX_TESTS_NO_STD];

// For no_std with alloc, use regular Vec (simpler than BoundedVec for this use
// case)
#[cfg(all(not(feature = "std"), feature = "alloc"))]
type TestRegistry = Vec<AsilTestMetadata>;

// Add missing import for alloc case
#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::vec::Vec;

/// Test metadata for ASIL categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AsilTestMetadata {
    /// ASIL level this test validates
    pub asil_level:     AsilLevel,
    /// Requirement ID this test verifies
    pub requirement_id: &'static str,
    /// Test category
    pub category:       TestCategory,
    /// Description of what this test validates
    pub description:    &'static str,
}

impl crate::traits::Checksummable for AsilTestMetadata {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        // Include ASIL level in checksum (as discriminant)
        (self.asil_level as u8).update_checksum(checksum);
        // Include string contents (not pointers) for stable checksums
        checksum.update_slice(self.requirement_id.as_bytes());
        (self.category as u8).update_checksum(checksum);
        checksum.update_slice(self.description.as_bytes());
    }
}

/// Categories of safety tests
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestCategory {
    /// Unit test for individual components
    #[default]
    Unit,
    /// Integration test across components
    Integration,
    /// Safety-specific test for critical paths
    Safety,
    /// Performance test with safety constraints
    Performance,
    /// Memory safety validation
    Memory,
    /// Resource limit validation
    Resource,
}

// Simple storage approach that avoids complex trait implementations
// For no_std environments, we'll use a fixed-size array instead of BoundedVec

/// Global test registry for ASIL-tagged tests
#[cfg(feature = "std")]
static TEST_REGISTRY: Mutex<Option<Vec<AsilTestMetadata>>> = Mutex::new(None);

// Static registry for alloc case (no_std + alloc)
#[cfg(all(not(feature = "std"), feature = "alloc"))]
static mut TEST_REGISTRY: Option<TestRegistry> = None;

// Static registry for no-alloc case (no_std + no_alloc)
#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
static mut TEST_REGISTRY: Option<TestRegistry> = None;

// Initialization synchronization (only needed for non-std environments)
#[cfg(not(feature = "std"))]
static REGISTRY_INIT: AtomicBool = AtomicBool::new(false);

/// Initialize the test registry (no_std + no_alloc version)
#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
fn init_test_registry() {
    if !REGISTRY_INIT.swap(true, Ordering::AcqRel) {
        unsafe {
            TEST_REGISTRY = Some([None; MAX_TESTS_NO_STD]);
        }
    }
}

/// Initialize the test registry (no_std + alloc version)
#[cfg(all(not(feature = "std"), feature = "alloc"))]
fn init_test_registry() {
    if !REGISTRY_INIT.swap(true, Ordering::AcqRel) {
        unsafe {
            // Initialize with regular Vec
            TEST_REGISTRY = Some(Vec::new());
        }
    }
}

/// Register an ASIL test
pub fn register_asil_test(metadata: AsilTestMetadata) {
    #[cfg(feature = "std")]
    {
        let mut registry = TEST_REGISTRY.lock().expect("Failed to lock test registry");
        if registry.is_none() {
            *registry = Some(Vec::new());
        }
        if let Some(ref mut reg) = *registry {
            reg.push(metadata);
        }
    }

    #[cfg(all(feature = "alloc", not(feature = "std")))]
    {
        init_test_registry();
        unsafe {
            if let Some(ref mut registry) = TEST_REGISTRY {
                registry.push(metadata);
            }
        }
    }

    #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
    {
        init_test_registry();
        unsafe {
            if let Some(ref mut registry) = TEST_REGISTRY {
                // Find first empty slot
                for slot in registry.iter_mut() {
                    if slot.is_none() {
                        *slot = Some(metadata);
                        break;
                    }
                }
            }
        }
    }
}

/// Get all registered ASIL tests
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn get_asil_tests() -> Vec<AsilTestMetadata> {
    #[cfg(feature = "std")]
    {
        let registry = TEST_REGISTRY.lock().expect("Failed to lock test registry");
        registry.as_ref().map_or_else(Vec::new, |reg| reg.clone())
    }

    #[cfg(all(feature = "alloc", not(feature = "std")))]
    {
        init_test_registry();
        unsafe {
            if let Some(ref registry) = TEST_REGISTRY {
                registry.clone()
            } else {
                Vec::new()
            }
        }
    }
}

/// Get all registered ASIL tests (no_std version)
#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
pub fn get_asil_tests() -> [Option<AsilTestMetadata>; MAX_TESTS_NO_STD] {
    init_test_registry();
    unsafe {
        if let Some(ref registry) = TEST_REGISTRY {
            *registry
        } else {
            [None; MAX_TESTS_NO_STD]
        }
    }
}

/// Get tests by ASIL level
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn get_tests_by_asil(level: AsilLevel) -> Vec<AsilTestMetadata> {
    get_asil_tests().into_iter().filter(|test| test.asil_level == level).collect()
}

/// Get tests by ASIL level (no_std version)
#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
pub fn get_tests_by_asil(level: AsilLevel) -> [Option<AsilTestMetadata>; MAX_TESTS_NO_STD] {
    let all_tests = get_asil_tests();
    let mut result = [None; MAX_TESTS_NO_STD];
    let mut result_idx = 0;

    for test in all_tests.iter() {
        if let Some(test) = test {
            if test.asil_level == level && result_idx < MAX_TESTS_NO_STD {
                result[result_idx] = Some(*test);
                result_idx += 1;
            }
        }
    }
    result
}

/// Get tests by category
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn get_tests_by_category(category: TestCategory) -> Vec<AsilTestMetadata> {
    get_asil_tests().into_iter().filter(|test| test.category == category).collect()
}

/// Get tests by category (no_std version)
#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
pub fn get_tests_by_category(
    category: TestCategory,
) -> [Option<AsilTestMetadata>; MAX_TESTS_NO_STD] {
    let all_tests = get_asil_tests();
    let mut result = [None; MAX_TESTS_NO_STD];
    let mut result_idx = 0;

    for test in all_tests.iter() {
        if let Some(test) = test {
            if test.category == category && result_idx < MAX_TESTS_NO_STD {
                result[result_idx] = Some(*test);
                result_idx += 1;
            }
        }
    }
    result
}

/// Generate test statistics
pub fn get_test_statistics() -> TestStatistics {
    #[cfg(any(feature = "std", feature = "alloc"))]
    {
        let tests = get_asil_tests();
        let mut stats = TestStatistics::default();

        for test in tests {
            stats.total_count += 1;

            match test.asil_level {
                AsilLevel::QM => stats.qm_count += 1,
                AsilLevel::AsilA => stats.asil_a_count += 1,
                AsilLevel::AsilB => stats.asil_b_count += 1,
                AsilLevel::AsilC => stats.asil_c_count += 1,
                AsilLevel::AsilD => stats.asil_d_count += 1,
            }

            match test.category {
                TestCategory::Unit => stats.unit_count += 1,
                TestCategory::Integration => stats.integration_count += 1,
                TestCategory::Safety => stats.safety_count += 1,
                TestCategory::Performance => stats.performance_count += 1,
                TestCategory::Memory => stats.memory_count += 1,
                TestCategory::Resource => stats.resource_count += 1,
            }
        }

        stats
    }

    #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
    {
        let tests = get_asil_tests();
        let mut stats = TestStatistics::default();

        for test_opt in tests.iter() {
            if let Some(test) = test_opt {
                stats.total_count += 1;

                match test.asil_level {
                    AsilLevel::QM => stats.qm_count += 1,
                    AsilLevel::AsilA => stats.asil_a_count += 1,
                    AsilLevel::AsilB => stats.asil_b_count += 1,
                    AsilLevel::AsilC => stats.asil_c_count += 1,
                    AsilLevel::AsilD => stats.asil_d_count += 1,
                }

                match test.category {
                    TestCategory::Unit => stats.unit_count += 1,
                    TestCategory::Integration => stats.integration_count += 1,
                    TestCategory::Safety => stats.safety_count += 1,
                    TestCategory::Performance => stats.performance_count += 1,
                    TestCategory::Memory => stats.memory_count += 1,
                    TestCategory::Resource => stats.resource_count += 1,
                }
            }
        }

        stats
    }
}

/// Test statistics summary
#[derive(Debug, Default)]
pub struct TestStatistics {
    pub total_count:       usize,
    pub qm_count:          usize,
    pub asil_a_count:      usize,
    pub asil_b_count:      usize,
    pub asil_c_count:      usize,
    pub asil_d_count:      usize,
    pub unit_count:        usize,
    pub integration_count: usize,
    pub safety_count:      usize,
    pub performance_count: usize,
    pub memory_count:      usize,
    pub resource_count:    usize,
}

/// Macro to create ASIL-tagged tests
#[macro_export]
macro_rules! asil_test {
    (
        name:
        $test_name:ident,asil:
        $asil_level:expr,requirement:
        $req_id:expr,category:
        $category:expr,description:
        $desc:expr,test:
        $test_body:block
    ) => {
        #[test]
        fn $test_name() {
            // Register this test in the ASIL registry
            $crate::asil_testing::register_asil_test($crate::asil_testing::AsilTestMetadata {
                asil_level:     $asil_level,
                requirement_id: $req_id,
                category:       $category,
                description:    $desc,
            });

            // Run the actual test
            $test_body
        }
    };
}

/// Macro for ASIL-D (highest safety) tests
#[macro_export]
macro_rules! asil_d_test {
    (
        name:
        $test_name:ident,requirement:
        $req_id:expr,category:
        $category:expr,description:
        $desc:expr,test:
        $test_body:block
    ) => {
        $crate::asil_test! {
            name: $test_name,
            asil: $crate::safety_system::AsilLevel::AsilD,
            requirement: $req_id,
            category: $category,
            description: $desc,
            test: $test_body
        }
    };
}

/// Macro for ASIL-C tests
#[macro_export]
macro_rules! asil_c_test {
    (
        name:
        $test_name:ident,requirement:
        $req_id:expr,category:
        $category:expr,description:
        $desc:expr,test:
        $test_body:block
    ) => {
        $crate::asil_test! {
            name: $test_name,
            asil: $crate::safety_system::AsilLevel::AsilC,
            requirement: $req_id,
            category: $category,
            description: $desc,
            test: $test_body
        }
    };
}

/// Macro for memory safety tests (typically ASIL-C or higher)
#[macro_export]
macro_rules! memory_safety_test {
    (
        name:
        $test_name:ident,asil:
        $asil_level:expr,requirement:
        $req_id:expr,description:
        $desc:expr,test:
        $test_body:block
    ) => {
        $crate::asil_test! {
            name: $test_name,
            asil: $asil_level,
            requirement: $req_id,
            category: $crate::asil_testing::TestCategory::Memory,
            description: $desc,
            test: $test_body
        }
    };
}

/// Macro for resource safety tests
#[macro_export]
macro_rules! resource_safety_test {
    (
        name:
        $test_name:ident,asil:
        $asil_level:expr,requirement:
        $req_id:expr,description:
        $desc:expr,test:
        $test_body:block
    ) => {
        $crate::asil_test! {
            name: $test_name,
            asil: $asil_level,
            requirement: $req_id,
            category: $crate::asil_testing::TestCategory::Resource,
            description: $desc,
            test: $test_body
        }
    };
}

