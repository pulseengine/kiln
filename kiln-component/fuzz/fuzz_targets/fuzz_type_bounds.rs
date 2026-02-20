#![no_main]

use libfuzzer_sys::fuzz_target;
use wrt_component::{
    type_bounds::{TypeBoundsChecker, TypeBound, TypeBoundKind},
    generative_types::{GenerativeTypeRegistry, BoundKind},
    ComponentInstanceId, TypeId,
};

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    
    let mut type_registry = GenerativeTypeRegistry::new();
    let mut bounds_checker = TypeBoundsChecker::new();
    
    // Use fuzzer data to create component IDs and type operations
    let component_id = ComponentInstanceId::new((data[0] as u32) % 100;
    let num_types = (data[1] % 10) as usize;
    let num_bounds = (data[2] % 20) as usize;
    
    // Create some resource types
    let mut type_ids = Vec::new();
    for i in 0..num_types.min(data.len() / 4) {
        let name = format!("type_{}", i;
        if let Ok(resource_type) = type_registry.create_resource_type(component_id, &name) {
            type_ids.push(resource_type.type_id);
        }
    }
    
    // Add type bounds based on fuzzer data
    let mut data_offset = 3;
    for _ in 0..num_bounds {
        if data_offset + 3 >= data.len() || type_ids.len() < 2 {
            break;
        }
        
        let sub_idx = (data[data_offset] as usize) % type_ids.len();
        let super_idx = (data[data_offset + 1] as usize) % type_ids.len();
        
        if sub_idx != super_idx {
            let kind = if data[data_offset + 2] & 0x01 == 0 {
                TypeBoundKind::Sub
            } else {
                TypeBoundKind::Eq
            };
            
            let bound = TypeBound {
                sub_type: type_ids[sub_idx],
                super_type: type_ids[super_idx],
                kind,
            };
            
            let _ = bounds_checker.add_bound(bound;
        }
        
        data_offset += 3;
    }
    
    // Test various type checking operations
    for i in 0..type_ids.len() {
        for j in 0..type_ids.len() {
            // These should not panic, just return true/false
            let _ = bounds_checker.is_subtype(type_ids[i], type_ids[j];
            let _ = bounds_checker.is_eq_type(type_ids[i], type_ids[j];
            let _ = bounds_checker.get_relation(type_ids[i], type_ids[j];
        }
    }
    
    // Test transitive relations
    let _ = bounds_checker.compute_transitive_closure);
    
    // Test with direct TypeId values from fuzzer data
    if data_offset + 4 < data.len() {
        let type1 = TypeId(u32::from_le_bytes([
            data[data_offset],
            data[data_offset + 1],
            data.get(data_offset + 2).copied().unwrap_or(0),
            data.get(data_offset + 3).copied().unwrap_or(0),
        ];
        let type2 = TypeId(u32::from_le_bytes([
            data.get(data_offset + 4).copied().unwrap_or(0),
            data.get(data_offset + 5).copied().unwrap_or(0),
            data.get(data_offset + 6).copied().unwrap_or(0),
            data.get(data_offset + 7).copied().unwrap_or(0),
        ];
        
        // These operations should handle arbitrary type IDs gracefully
        let _ = bounds_checker.is_subtype(type1, type2;
        let _ = bounds_checker.is_eq_type(type1, type2;
        let _ = bounds_checker.get_relation(type1, type2;
    }
    
    // Test removing bounds
    if !type_ids.is_empty() && data.len() > data_offset {
        let remove_idx = (data[data_offset] as usize) % type_ids.len();
        let _ = bounds_checker.remove_bounds_for_type(type_ids[remove_idx];
    }
    
    // Clear and verify empty state
    bounds_checker.clear);
    for i in 0..type_ids.len().min(2) {
        for j in 0..type_ids.len().min(2) {
            if i != j {
                assert!(!bounds_checker.is_subtype(type_ids[i], type_ids[j]);
            }
        }
    }
};