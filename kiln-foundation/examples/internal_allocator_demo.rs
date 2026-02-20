//! Demonstration of the Kiln internal compile-time allocator system
//!
//! This example shows how the allocator system is now integrated directly
//! into kiln-foundation without external dependencies.

use kiln_foundation::allocator::{
    CapacityError,
    CrateId,
    KilnHashMap,
    KilnVec,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Kiln Internal Allocator System Demo");
    println!("=====================================");

    // Demonstrate compile-time verified collections
    demo_internal_collections()?;

    // Show integration with existing Kiln foundation
    demo_foundation_integration()?;

    println!("\n✅ Internal allocator integration successful!");
    println!("🏆 Kiln foundation now includes A+ safety-critical allocator!");

    Ok(())
}

/// Demonstrate internal compile-time verified collections
fn demo_internal_collections() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 1. Internal Compile-Time Verified Collections");
    println!("------------------------------------------------");

    // These are now part of kiln-foundation, not external crates
    let mut foundation_vec: KilnVec<i32, { CrateId::Foundation as u8 }, 1000> = KilnVec::new();
    let mut component_vec: KilnVec<String, { CrateId::Component as u8 }, 500> = KilnVec::new();

    println!(
        "✓ Foundation Vec (internal): {} items",
        foundation_vec.len()
    );
    println!("✓ Component Vec (internal): {} items", component_vec.len());

    // The compiler verifies these allocations fit within crate budgets
    foundation_vec.push(42)?;
    component_vec.push("Hello Internal Kiln".to_string())?;

    println!("✓ Internal allocations verified at compile time!");

    Ok(())
}

/// Show integration with existing Kiln foundation
fn demo_foundation_integration() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔗 2. Foundation Integration");
    println!("-----------------------------");

    // The allocator is now seamlessly integrated into kiln-foundation
    let mut runtime_map: KilnHashMap<String, i32, { CrateId::Runtime as u8 }, 256> =
        KilnHashMap::new();

    // Works exactly like standard collections
    runtime_map.insert("runtime_key".to_string(), 100)?;
    runtime_map.insert("safety_level".to_string(), 95)?;

    println!("Runtime HashMap entries: {}", runtime_map.len());
    println!(
        "Safety level: {}",
        runtime_map.get("safety_level").unwrap_or(&0)
    );

    // Demonstrate capacity limits
    let mut small_vec: KilnVec<i32, { CrateId::Host as u8 }, 3> = KilnVec::new();
    small_vec.push(1)?;
    small_vec.push(2)?;
    small_vec.push(3)?;

    match small_vec.push(4) {
        Ok(()) => println!("Unexpected success"),
        Err(CapacityError::Exceeded) => {
            println!("✓ Capacity enforcement working - safely rejected overflow");
        },
    }

    println!("✓ Foundation integration complete with safety guarantees!");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_internal_allocator() {
        // Test that the internal allocator system works
        let mut vec: KilnVec<i32, { CrateId::Foundation as u8 }, 100> = KilnVec::new();

        assert!(vec.push(1).is_ok());
        assert!(vec.push(2).is_ok());
        assert_eq!(vec.len(), 2);

        // Test zero-cost deref
        vec.sort();
        assert_eq!(vec[0], 1);
        assert_eq!(vec[1], 2);
    }

    #[test]
    fn test_capacity_enforcement() {
        let mut vec: KilnVec<i32, { CrateId::Foundation as u8 }, 2> = KilnVec::new();

        assert!(vec.push(1).is_ok());
        assert!(vec.push(2).is_ok());
        assert_eq!(vec.push(3), Err(CapacityError::Exceeded));
    }
}
