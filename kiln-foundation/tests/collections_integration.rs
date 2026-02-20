// Integration test for new collections module
// Tests collections in isolation from legacy code

use wrt_foundation::collections::{StaticVec, StaticQueue, StaticMap};
use wrt_error::Result;

#[test]
fn test_static_vec_basic() -> Result<()> {
    let mut vec = StaticVec::<u32, 10>::new();

    // Push elements
    vec.push(1)?;
    vec.push(2)?;
    vec.push(3)?;

    // Verify basic operations
    assert_eq!(vec.len(), 3);
    assert_eq!(vec.get(0), Some(&1));
    assert_eq!(vec.get(1), Some(&2));
    assert_eq!(vec.get(2), Some(&3));
    assert_eq!(vec.pop(), Some(3));
    assert_eq!(vec.len(), 2);

    Ok(())
}

#[test]
fn test_static_queue_fifo() -> Result<()> {
    let mut queue = StaticQueue::<u32, 5>::new();

    // Push elements
    queue.push(1)?;
    queue.push(2)?;
    queue.push(3)?;

    // Verify FIFO order
    assert_eq!(queue.pop(), Some(1));
    assert_eq!(queue.pop(), Some(2));
    assert_eq!(queue.pop(), Some(3));
    assert_eq!(queue.pop(), None);

    Ok(())
}

#[test]
fn test_static_map_sorted() -> Result<()> {
    let mut map = StaticMap::<u32, &str, 10>::new();

    // Insert in random order
    map.insert(5, "five")?;
    map.insert(2, "two")?;
    map.insert(8, "eight")?;
    map.insert(1, "one")?;

    // Verify sorted access
    assert_eq!(map.get(&1), Some(&"one"));
    assert_eq!(map.get(&2), Some(&"two"));
    assert_eq!(map.get(&5), Some(&"five"));
    assert_eq!(map.get(&8), Some(&"eight"));
    assert_eq!(map.len(), 4);

    Ok(())
}

#[test]
fn test_static_vec_capacity() {
    let mut vec = StaticVec::<u32, 3>::new();

    assert!(vec.push(1).is_ok());
    assert!(vec.push(2).is_ok());
    assert!(vec.push(3).is_ok());
    assert!(vec.push(4).is_err()); // Should fail - capacity exceeded
    assert_eq!(vec.len(), 3);
}

#[test]
fn test_static_queue_circular() -> Result<()> {
    let mut queue = StaticQueue::<u32, 3>::new();

    // Fill queue
    queue.push(1)?;
    queue.push(2)?;
    queue.push(3)?;

    // Pop and push to test wraparound
    assert_eq!(queue.pop(), Some(1));
    queue.push(4)?;

    // Verify correct order after wraparound
    assert_eq!(queue.pop(), Some(2));
    assert_eq!(queue.pop(), Some(3));
    assert_eq!(queue.pop(), Some(4));

    Ok(())
}

#[test]
fn test_static_map_update() -> Result<()> {
    let mut map = StaticMap::<u32, &str, 10>::new();

    map.insert(1, "one")?;
    let old = map.insert(1, "ONE")?;

    assert_eq!(old, Some("one"));
    assert_eq!(map.get(&1), Some(&"ONE"));
    assert_eq!(map.len(), 1); // Should not increase

    Ok(())
}

#[test]
fn test_collections_iterators() -> Result<()> {
    // StaticVec iterator
    let mut vec = StaticVec::<u32, 10>::new();
    vec.push(1)?;
    vec.push(2)?;
    vec.push(3)?;

    let sum: u32 = vec.iter().sum();
    assert_eq!(sum, 6);

    // StaticQueue iterator
    let mut queue = StaticQueue::<u32, 10>::new();
    queue.push(10)?;
    queue.push(20)?;

    let count = queue.iter().count();
    assert_eq!(count, 2);

    // StaticMap iterator
    let mut map = StaticMap::<u32, &str, 10>::new();
    map.insert(1, "one")?;
    map.insert(2, "two")?;

    let key_count = map.keys().count();
    assert_eq!(key_count, 2);

    Ok(())
}

#[test]
fn test_collections_clear() -> Result<()> {
    // StaticVec clear
    let mut vec = StaticVec::<u32, 10>::new();
    vec.push(1)?;
    vec.push(2)?;
    vec.clear();
    assert_eq!(vec.len(), 0);

    // StaticQueue clear
    let mut queue = StaticQueue::<u32, 10>::new();
    queue.push(1)?;
    queue.push(2)?;
    queue.clear();
    assert_eq!(queue.len(), 0);

    // StaticMap clear
    let mut map = StaticMap::<u32, &str, 10>::new();
    map.insert(1, "one")?;
    map.clear();
    assert_eq!(map.len(), 0);

    Ok(())
}
