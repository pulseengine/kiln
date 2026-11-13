//! Reading meld's `--emit-import-map` JSON (SR-81).
//!
//! The map lists every function import of a fused module with the memory and
//! `cabi_realloc` of the component that owns it. The shape is meld's
//! (`version` 2): each entry has `index`, `module`, `name`, `memory` and
//! `realloc`; `memory` is `null`, `{"export", "index"}` or `{"unknown"}`, and
//! `realloc` is `null`, `{"export", "func"}` or `{"unknown"}`. `component`
//! (the owning component) and, on a `[thread-new-indirect-v0]` entry,
//! `thread_table` (`{"export", "index"}`) are read for SR-82. Anything else
//! is refused, so a map in a shape kiln does not understand is never read as
//! if it were one it does.

use kiln_error::{Error, Result};
use kiln_runtime::host_import_map::{
    HostImportMap, ImportMapEntry, ImportMemory, ImportRealloc, ThreadTable,
};
use serde_json::Value;

/// The only map version this reader understands.
const SUPPORTED_VERSION: u64 = 2;

/// Read and parse the import map at `path`.
pub(crate) fn load(path: &str) -> Result<HostImportMap> {
    let text = std::fs::read_to_string(path)
        .map_err(|_| Error::runtime_error("import map could not be read"))?;
    parse(&text)
}

/// Parse meld's import map JSON.
pub(crate) fn parse(text: &str) -> Result<HostImportMap> {
    let root: Value = serde_json::from_str(text)
        .map_err(|_| Error::validation_invalid_input("import map is not valid JSON"))?;
    if root.get("version").and_then(Value::as_u64) != Some(SUPPORTED_VERSION) {
        return Err(Error::validation_invalid_input(
            "import map version is not 2",
        ));
    }
    let imports = root
        .get("imports")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::validation_invalid_input("import map has no imports array"))?;
    let entries = imports.iter().map(entry).collect::<Result<Vec<_>>>()?;
    HostImportMap::new(entries)
}

fn entry(value: &Value) -> Result<ImportMapEntry> {
    let bad = |what: &'static str| Error::validation_invalid_input(what);
    let index = value
        .get("index")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| bad("import map entry has no valid index"))?;
    let text = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| bad("import map entry has no module or name"))
    };
    let memory = match value.get("memory") {
        Some(Value::Null) => ImportMemory::NotNeeded,
        Some(Value::Object(o)) => match (o.get("index"), o.get("unknown")) {
            (Some(n), None) => ImportMemory::Index(
                n.as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(|| bad("import map memory index is not a u32"))?,
            ),
            (None, Some(why)) => ImportMemory::Unknown(why.as_str().unwrap_or_default().to_owned()),
            _ => return Err(bad("import map memory is neither an index nor unknown")),
        },
        _ => return Err(bad("import map entry has no memory field")),
    };
    let realloc = match value.get("realloc") {
        Some(Value::Null) => ImportRealloc::NotNeeded,
        Some(Value::Object(o)) => match (o.get("func"), o.get("unknown")) {
            (Some(n), None) => ImportRealloc::Func(
                n.as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(|| bad("import map realloc func is not a u32"))?,
            ),
            (None, Some(why)) => {
                ImportRealloc::Unknown(why.as_str().unwrap_or_default().to_owned())
            },
            _ => return Err(bad("import map realloc is neither a func nor unknown")),
        },
        _ => return Err(bad("import map entry has no realloc field")),
    };
    // SR-82: the owning component (thread handles are per owner) and, on a
    // `[thread-new-indirect-v0]` entry, the table its fn_idx indexes. Both
    // may be absent or null; present in another shape they are refused.
    let component = match value.get("component") {
        None | Some(Value::Null) => None,
        Some(n) => Some(
            n.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| bad("import map component is not a u32"))?,
        ),
    };
    let thread_table = match value.get("thread_table") {
        None | Some(Value::Null) => None,
        Some(Value::Object(o)) => match (o.get("export").and_then(Value::as_str), o.get("index")) {
            (Some(export), Some(n)) => Some(ThreadTable {
                export: export.to_owned(),
                index: n
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(|| bad("import map thread_table index is not a u32"))?,
            }),
            _ => return Err(bad("import map thread_table has no export and index")),
        },
        Some(_) => return Err(bad("import map thread_table is not an object")),
    };
    Ok(ImportMapEntry {
        index,
        module: text("module")?,
        name: text("name")?,
        memory,
        realloc,
        component,
        thread_table,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{"version":2,"imports":[
        {"index":0,"module":"$root","name":"[task-return]0","kind":"p3-builtin","component":0,
         "memory":{"unknown":"not traced"},"realloc":{"unknown":"not traced"},"encoding":null},
        {"index":3,"module":"wasi:cli/environment@0.2.6","name":"get-arguments","kind":"wasi",
         "component":5,"memory":{"export":"memory$5","index":2},
         "realloc":{"export":"cabi_realloc$2","func":2179},"encoding":"utf8"},
        {"index":4,"module":"wasi:cli/exit@0.2.6","name":"exit$5","kind":"wasi","component":5,
         "memory":null,"realloc":null,"encoding":"utf8"}]}"#;

    // rivet: verifies SR-81
    #[test]
    fn a_valid_map_is_read_entry_by_entry() -> Result<()> {
        let map = parse(VALID)?;
        let args = map.entry(3, "wasi:cli/environment@0.2.6", "get-arguments")?;
        assert_eq!(args.memory, ImportMemory::Index(2));
        assert_eq!(args.realloc, ImportRealloc::Func(2179));
        let exit = map.entry(4, "wasi:cli/exit@0.2.6", "exit$5")?;
        assert_eq!(exit.memory, ImportMemory::NotNeeded);
        assert_eq!(exit.realloc, ImportRealloc::NotNeeded);
        let builtin = map.entry(0, "$root", "[task-return]0")?;
        assert_eq!(builtin.memory, ImportMemory::Unknown("not traced".into()));
        Ok(())
    }

    // rivet: verifies SR-81
    #[test]
    fn an_entry_checked_against_another_import_is_a_mismatch() -> Result<()> {
        let map = parse(VALID)?;
        assert!(map.entry(4, "wasi:cli/exit@0.2.6", "exit$2").is_err());
        assert!(map.entry(9, "wasi:cli/exit@0.2.6", "exit$5").is_err());
        Ok(())
    }

    // rivet: verifies SR-82
    #[test]
    fn component_and_thread_table_are_read() -> Result<()> {
        let map = parse(
            r#"{"version":2,"imports":[
            {"index":0,"module":"$root","name":"[thread-index]$1","kind":"p3-builtin",
             "component":1,"memory":null,"realloc":null,"encoding":null},
            {"index":1,"module":"$root","name":"[thread-new-indirect-v0]","kind":"p3-builtin",
             "component":0,"memory":null,"realloc":null,"encoding":null,
             "thread_table":{"export":"__meld_thread_table","index":1}}]}"#,
        )?;
        let index = map.entry(0, "$root", "[thread-index]$1")?;
        assert_eq!(index.component, Some(1));
        assert_eq!(index.thread_table, None);
        let new = map.entry(1, "$root", "[thread-new-indirect-v0]")?;
        assert_eq!(new.component, Some(0));
        assert_eq!(
            new.thread_table,
            Some(ThreadTable {
                export: "__meld_thread_table".into(),
                index: 1
            })
        );
        for bad in [
            r#"{"version":2,"imports":[{"index":0,"module":"a","name":"b","memory":null,"realloc":null,"component":-1}]}"#,
            r#"{"version":2,"imports":[{"index":0,"module":"a","name":"b","memory":null,"realloc":null,"thread_table":{"index":1}}]}"#,
            r#"{"version":2,"imports":[{"index":0,"module":"a","name":"b","memory":null,"realloc":null,"thread_table":3}]}"#,
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        Ok(())
    }

    // rivet: verifies SR-81
    #[test]
    fn maps_in_an_unknown_shape_are_refused() {
        for bad in [
            "{not json",
            r#"{"imports":[]}"#,
            r#"{"version":3,"imports":[]}"#,
            r#"{"version":2}"#,
            // memory missing, or neither index nor unknown
            r#"{"version":2,"imports":[{"index":0,"module":"a","name":"b","realloc":null}]}"#,
            r#"{"version":2,"imports":[{"index":0,"module":"a","name":"b","memory":{},"realloc":null}]}"#,
            // negative and oversized indices
            r#"{"version":2,"imports":[{"index":-1,"module":"a","name":"b","memory":null,"realloc":null}]}"#,
            r#"{"version":2,"imports":[{"index":0,"module":"a","name":"b","memory":{"index":4294967296},"realloc":null}]}"#,
            // the same import index twice
            r#"{"version":2,"imports":[
                {"index":0,"module":"a","name":"b","memory":null,"realloc":null},
                {"index":0,"module":"a","name":"c","memory":null,"realloc":null}]}"#,
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
