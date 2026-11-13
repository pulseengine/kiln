//! Which memory a host import runs against (SR-81).
//!
//! A fused `--memory multi` module has several memories. A host import reads
//! and writes the memory of the component that owns it; the engine must take
//! that from meld's import map, or refuse, on EVERY host-call path, including
//! the canon-lowered one. These tests drive a recording handler that writes a
//! marker byte into whatever memory it is handed.

#![cfg(feature = "wasi")]

use kiln_error::Result;
use kiln_foundation::{HostImportHandler, MemoryAccessor, Value, memory_init::MemoryInitializer};
use kiln_runtime::{
    engine::{CapabilityAwareEngine, CapabilityEngine, EnginePreset, InstanceHandle},
    host_import_map::{HostImportMap, ImportMapEntry, ImportMemory, ImportRealloc},
};

const MARKER: u8 = 0xAB;
const IFACE: &str = "wasi:cli/terminal-stdin@0.2.6";
const FUNC: &str = "get-terminal-stdin$3";

/// Writes `MARKER` at offset 0 of the memory it is given.
struct MarkingHandler;

impl HostImportHandler for MarkingHandler {
    fn resolves(&self, module: &str, function: &str) -> bool {
        module == IFACE && function == FUNC
    }

    fn call_import(
        &mut self,
        _module: &str,
        _function: &str,
        _args: &[Value],
        memory: Option<&dyn MemoryAccessor>,
    ) -> Result<Vec<Value>> {
        if let Some(memory) = memory {
            memory.write_bytes(0, &[MARKER])?;
        }
        Ok(vec![])
    }
}

/// Two memories; `run` calls the suffixed import (function import 0).
const TWO_MEMORIES: &str = r#"(module
    (import "wasi:cli/terminal-stdin@0.2.6" "get-terminal-stdin$3" (func))
    (memory (export "memory") 1)
    (memory (export "memory$3") 1)
    (func (export "run") call 0))"#;

fn marker_at_zero(
    engine: &CapabilityAwareEngine,
    handle: kiln_runtime::engine::InstanceHandle,
    mem: u32,
) -> Result<u8> {
    let instance = engine.get_instance(handle)?;
    let wrapper = instance.memory(mem)?;
    let mut byte = [0u8; 1];
    wrapper.0.as_ref().read_bytes(0, &mut byte)?;
    Ok(byte[0])
}

/// The canon-lowered path: the import is registered as a lowered function, so
/// the call goes through `dispatch_canon_lowered`, which used to take memory 0
/// unconditionally.
// rivet: verifies SR-81
#[serial_test::serial]
#[test]
fn canon_lowered_suffixed_call_in_a_multi_memory_module_is_refused_without_a_map() -> Result<()> {
    MemoryInitializer::ensure_initialized()?;
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;
    engine.set_host_handler(Box::new(MarkingHandler));
    let wasm = wat::parse_str(TWO_MEMORIES).map_err(|_| kiln_error::Error::runtime_error("wat"))?;
    let module = engine.load_module(&wasm)?;
    let instance = engine.instantiate(module)?;
    engine.register_lowered_function(instance.index(), 0, IFACE.into(), FUNC.into(), None, None);

    let result = engine.execute(instance, "run", &[]);

    assert!(
        result.is_err(),
        "an owner-less suffixed call must be refused on the canon-lowered path"
    );
    assert_eq!(
        marker_at_zero(&engine, instance, 0)?,
        0,
        "memory 0 must not have been written by a guess"
    );
    Ok(())
}

fn map_entry(name: &str, memory: ImportMemory) -> Result<HostImportMap> {
    HostImportMap::new(vec![ImportMapEntry {
        index: 0,
        module: IFACE.into(),
        name: name.into(),
        memory,
        realloc: ImportRealloc::NotNeeded,
        component: None,
        thread_table: None,
    }])
}

type Run = (CapabilityAwareEngine, InstanceHandle, Result<Vec<Value>>);

/// Loads `TWO_MEMORIES` with the recording handler and the given map; the
/// import is called as a plain import, or as a lowered function when `lowered`.
fn run_two_memories(map: Option<HostImportMap>, lowered: bool) -> Result<Run> {
    MemoryInitializer::ensure_initialized()?;
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;
    engine.set_host_handler(Box::new(MarkingHandler));
    if let Some(map) = map {
        engine.set_import_map(map);
    }
    let wasm = wat::parse_str(TWO_MEMORIES).map_err(|_| kiln_error::Error::runtime_error("wat"))?;
    let module = engine.load_module(&wasm)?;
    let instance = engine.instantiate(module)?;
    if lowered {
        engine.register_lowered_function(
            instance.index(),
            0,
            IFACE.into(),
            FUNC.into(),
            None,
            None,
        );
    }
    let result = engine.execute(instance, "run", &[]);
    Ok((engine, instance, result))
}

/// The map names memory 1: the handler writes there and memory 0 is untouched,
/// on the plain import path and on the canon-lowered path.
// rivet: verifies SR-81
#[serial_test::serial]
#[test]
fn the_map_selects_the_memory_on_every_host_call_path() -> Result<()> {
    for lowered in [false, true] {
        let map = map_entry(FUNC, ImportMemory::Index(1))?;
        let (engine, instance, result) = run_two_memories(Some(map), lowered)?;
        result?;
        assert_eq!(
            marker_at_zero(&engine, instance, 1)?,
            MARKER,
            "lowered={lowered}"
        );
        assert_eq!(
            marker_at_zero(&engine, instance, 0)?,
            0,
            "lowered={lowered}"
        );
    }
    Ok(())
}

const READ_IFACE: &str = "wasi:io/streams@0.2.6";
const READ_FUNC: &str = "[method]input-stream.blocking-read$3";

/// Records the guest buffer the engine pre-allocated for a read.
struct ReadRecorder(std::sync::Arc<std::sync::atomic::AtomicU32>);

impl HostImportHandler for ReadRecorder {
    fn resolves(&self, module: &str, function: &str) -> bool {
        module == READ_IFACE && function == READ_FUNC
    }

    fn call_import(
        &mut self,
        _module: &str,
        _function: &str,
        _args: &[Value],
        _memory: Option<&dyn MemoryAccessor>,
    ) -> Result<Vec<Value>> {
        Ok(vec![])
    }

    fn set_read_buffer_allocation(&mut self, ptr: u32, _size: u32) {
        self.0.store(ptr, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Two memories and two allocators: func 1 returns 100, func 2 returns 200.
const TWO_ALLOCATORS: &str = r#"(module
    (import "wasi:io/streams@0.2.6" "[method]input-stream.blocking-read$3"
      (func (param i32 i64 i32)))
    (memory 1)
    (memory 1)
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) i32.const 100)
    (func (export "cabi_realloc$3") (param i32 i32 i32 i32) (result i32) i32.const 200)
    (func (export "run")
      i32.const 0
      i64.const 8
      i32.const 0
      call 0))"#;

fn run_reader(realloc: ImportRealloc) -> Result<(u32, Result<Vec<Value>>)> {
    MemoryInitializer::ensure_initialized()?;
    let seen = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let mut engine = CapabilityAwareEngine::with_preset(EnginePreset::QM)?;
    engine.set_host_handler(Box::new(ReadRecorder(seen.clone())));
    engine.set_import_map(HostImportMap::new(vec![ImportMapEntry {
        index: 0,
        module: READ_IFACE.into(),
        name: READ_FUNC.into(),
        memory: ImportMemory::Index(1),
        realloc,
        component: None,
        thread_table: None,
    }])?);
    let wasm =
        wat::parse_str(TWO_ALLOCATORS).map_err(|_| kiln_error::Error::runtime_error("wat"))?;
    let module = engine.load_module(&wasm)?;
    let instance = engine.instantiate(module)?;
    let result = engine.execute(instance, "run", &[]);
    Ok((seen.load(std::sync::atomic::Ordering::SeqCst), result))
}

/// The guest buffer for a read comes from the allocator the map names (func 2,
/// the owning component's), not from the module's `cabi_realloc` export.
// rivet: verifies SR-81
#[serial_test::serial]
#[test]
fn the_map_selects_the_allocator_for_guest_buffers() -> Result<()> {
    let (ptr, result) = run_reader(ImportRealloc::Func(2))?;
    result?;
    assert_eq!(
        ptr, 200,
        "the mapped allocator, not the export, placed the buffer"
    );
    Ok(())
}

/// A call that needs a guest buffer is refused when the map says the import
/// allocates nothing or could not name its allocator.
// rivet: verifies SR-81
#[serial_test::serial]
#[test]
fn a_call_needing_a_buffer_is_refused_without_a_mapped_allocator() -> Result<()> {
    for realloc in [ImportRealloc::NotNeeded, ImportRealloc::Unknown("x".into())] {
        let (ptr, result) = run_reader(realloc)?;
        assert!(result.is_err());
        assert_eq!(ptr, 0, "nothing was allocated");
    }
    Ok(())
}

/// A memory index past the module's memories is an error, not memory 0.
// rivet: verifies SR-81
#[serial_test::serial]
#[test]
fn a_map_memory_index_the_module_lacks_is_an_error() -> Result<()> {
    for lowered in [false, true] {
        let map = map_entry(FUNC, ImportMemory::Index(7))?;
        let (_engine, _instance, result) = run_two_memories(Some(map), lowered)?;
        assert!(result.is_err(), "lowered={lowered}");
    }
    Ok(())
}

/// `null` and `unknown` owners refuse a handler that asks for memory; a map
/// entry for another import is an error; with no map the call is refused. Each
/// on both paths, and no memory is written.
// rivet: verifies SR-81
#[serial_test::serial]
#[test]
fn null_unknown_mismatched_and_absent_maps_refuse_on_every_path() -> Result<()> {
    for lowered in [false, true] {
        let cases = [
            Some(map_entry(FUNC, ImportMemory::NotNeeded)?),
            Some(map_entry(FUNC, ImportMemory::Unknown("not traced".into()))?),
            Some(map_entry("get-terminal-stdin$9", ImportMemory::Index(1))?),
            None,
        ];
        for map in cases {
            let (engine, instance, result) = run_two_memories(map, lowered)?;
            assert!(result.is_err(), "lowered={lowered}");
            assert_eq!(
                marker_at_zero(&engine, instance, 0)?,
                0,
                "lowered={lowered}"
            );
            assert_eq!(
                marker_at_zero(&engine, instance, 1)?,
                0,
                "lowered={lowered}"
            );
        }
    }
    Ok(())
}
