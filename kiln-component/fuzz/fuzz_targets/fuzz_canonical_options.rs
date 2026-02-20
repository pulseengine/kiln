#![no_main]

use libfuzzer_sys::fuzz_target;
use wrt_component::{
    canonical_options::{CanonicalOptions, CanonicalOptionsBuilder},
    canonical_realloc::{ReallocManager, StringEncoding},
    ComponentInstanceId,
};
use wrt_foundation::component_value::ComponentValue;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    
    // Use fuzzer data to control options
    let has_memory = data[0] & 0x01 != 0;
    let has_realloc = data[0] & 0x02 != 0;
    let has_post_return = data[0] & 0x04 != 0;
    let string_encoding = match data[0] & 0x18 {
        0x00 => StringEncoding::Utf8,
        0x08 => StringEncoding::Utf16,
        0x10 => StringEncoding::Latin1,
        _ => StringEncoding::Utf8,
    };
    
    // Build canonical options with fuzzer-controlled settings
    let options = CanonicalOptionsBuilder::new()
        .with_memory(has_memory)
        .with_realloc(has_realloc)
        .with_post_return(has_post_return)
        .with_string_encoding(string_encoding)
        .build);
    
    // Test lift/lower contexts
    let component_id = ComponentInstanceId::new((data.get(1).copied().unwrap_or(1) as u32) % 1000;
    
    // Create lift context
    let mut lift_context = options.create_lift_context(component_id;
    
    // Test memory operations if enabled
    if has_memory && data.len() > 2 {
        let size = (data[2] as usize) % 1024;
        let align = match data.get(3).copied().unwrap_or(0) & 0x07 {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => 1,
        };
        let _ = lift_context.allocate_memory(size, align;
    }
    
    // Create lower context
    let mut lower_context = options.create_lower_context(component_id;
    
    // Test string operations with fuzzer data
    if data.len() > 4 {
        let string_data = &data[4..];
        if let Ok(s) = std::str::from_utf8(string_data) {
            let _ = lower_context.lower_string(s, string_encoding;
        }
    }
    
    // Test realloc manager if enabled
    if has_realloc {
        let mut realloc_manager = ReallocManager::new();
        
        if data.len() > 5 {
            let alloc_size = (data[5] as usize) % 512 + 1;
            let alloc_align = match data.get(6).copied().unwrap_or(0) & 0x03 {
                0 => 1,
                1 => 2,
                2 => 4,
                3 => 8,
                _ => 1,
            };
            
            // Try allocation
            if let Ok(ptr) = realloc_manager.allocate(component_id, alloc_size, alloc_align) {
                // Try reallocation
                if data.len() > 7 {
                    let new_size = (data[7] as usize) % 1024 + 1;
                    let _ = realloc_manager.reallocate(
                        component_id,
                        ptr,
                        alloc_size,
                        alloc_align,
                        new_size
                    ;
                }
                
                // Try deallocation
                let _ = realloc_manager.deallocate(component_id, ptr, alloc_size, alloc_align;
            }
        }
    }
};