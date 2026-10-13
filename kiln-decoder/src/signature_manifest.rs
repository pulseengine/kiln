//! Reader for meld's `meld.signature-manifest` custom section (SR-60, #480).
//!
//! A meld-fused core module carries no WIT type information — `wasm-tools`
//! reports "no `component-type*` custom sections meaning that there is not WIT
//! information", because `wasm-tools component new` consumes that section long
//! before meld sees the input. meld therefore emits a signature manifest
//! describing, per exported canonical name, what the Canonical ABI did to it.
//!
//! The manifest exists so a runtime can **check** rather than **re-derive**.
//! `core` is read back by meld from the function type it actually emitted, not
//! computed from the same WIT tree that produces `wit`; if the two ever agree by
//! construction the field is decoration and a passing cross-check proves
//! nothing. That property is the whole reason kiln consumes this instead of a
//! synthesized `component-type` section (see meld#400).
//!
//! Reading discipline, matching [`crate::resource_limits_section`]:
//! absent section → `Ok(None)`; present but malformed → `Err`, never silently
//! treated as absent.

use alloc::{string::String, vec::Vec};

use kiln_error::Error;
use serde::Deserialize;

/// Custom section name meld emits.
pub const SIGNATURE_MANIFEST_SECTION_NAME: &str = "meld.signature-manifest";

/// Manifest major version this reader understands.
///
/// An unknown version is refused outright rather than parsed best-effort:
/// ignoring a field we do not understand is how a reader silently
/// misinterprets a shape.
pub const SUPPORTED_MANIFEST_VERSION: u32 = 1;

/// What the export needs from the host to be callable at all.
#[derive(Debug, Clone, Deserialize)]
pub struct Needs {
    /// The export reads or writes guest linear memory.
    #[serde(default)]
    pub memory: bool,
    /// Arguments or results must be allocated through the callee's `realloc`.
    #[serde(default)]
    pub realloc: bool,
}

/// One field of the return area, as stored by the callee.
#[derive(Debug, Clone, Deserialize)]
pub struct ReturnAreaField {
    /// Byte offset from the returned pointer.
    pub offset: u32,
    /// Field name from the WIT type.
    pub field: String,
    /// Field size in bytes.
    pub size: u32,
}

/// Where the callee writes its result when the result is returned indirectly.
#[derive(Debug, Clone, Deserialize)]
pub struct ReturnArea {
    /// Total size in bytes.
    pub size: u32,
    /// Required alignment.
    pub align: u32,
    /// Top-level field layout. meld emits only the top level for now; nested
    /// layouts are deliberately absent rather than guessed, so a reader must
    /// not infer below the level described here.
    #[serde(default)]
    pub layout: Vec<ReturnAreaField>,
}

/// The core function type meld actually emitted for this export.
#[derive(Debug, Clone, Deserialize)]
pub struct CoreSignature {
    /// Core parameter types, in order.
    pub params: Vec<String>,
    /// Core result types.
    pub results: Vec<String>,
}

/// One exported canonical name and everything needed to call it.
#[derive(Debug, Clone, Deserialize)]
pub struct ExportSignature {
    /// Canonical export name, exactly as meld emits it.
    pub export: String,
    /// Number of values the Canonical ABI flattened the parameters into.
    ///
    /// This is the field that distinguishes a genuine scalar parameter from a
    /// pointer to a lowered argument block: when it exceeds the flattening
    /// limit the core signature collapses to a single `i32` that is an
    /// address, and `core` alone cannot tell the two cases apart.
    pub flat_param_count: u32,
    /// The core type read back from the emitted module.
    pub core: CoreSignature,
    /// Host requirements for this export.
    pub needs: Needs,
    /// Name of the memory export this lift uses, resolved per-lift.
    #[serde(default)]
    pub memory: Option<String>,
    /// Name of the exported allocator this lift uses.
    ///
    /// `None` with `needs.realloc == true` is meld stating a contradiction it
    /// found in the artifact rather than hiding it: the allocator the spec says
    /// to use for this export is not reachable, so the export is not callable.
    #[serde(default)]
    pub realloc: Option<String>,
    /// Name of the post-return export, when the guest emits one.
    #[serde(default)]
    pub post_return: Option<String>,
    /// Return-area description, when the result is returned indirectly.
    #[serde(default)]
    pub return_area: Option<ReturnArea>,
}

impl ExportSignature {
    /// Whether the parameters are passed indirectly, as a pointer to a lowered
    /// argument block, rather than as flat core values.
    ///
    /// Detected by disagreement between what the ABI flattened to and what the
    /// core signature actually takes — not by re-deriving the flattening limit,
    /// which is the derivation that is easy to get wrong.
    pub fn params_are_indirect(&self) -> bool {
        self.flat_param_count as usize != self.core.params.len()
    }

    /// Whether the export is callable at all given what the module exports.
    ///
    /// An export needing an allocator that is not reachable cannot be invoked
    /// correctly, so a reader must refuse rather than substitute one that
    /// merely looks like an allocator.
    pub fn realloc_is_unreachable(&self) -> bool {
        self.needs.realloc && self.realloc.is_none()
    }
}

/// The parsed manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct SignatureManifest {
    /// Manifest format version.
    pub version: u32,
    /// One entry per exported canonical name.
    pub exports: Vec<ExportSignature>,
}

impl SignatureManifest {
    /// Look up an export by its canonical name.
    pub fn find(&self, export: &str) -> Option<&ExportSignature> {
        self.exports.iter().find(|e| e.export == export)
    }
}

/// Extract and parse the signature manifest from a core module.
///
/// Returns `Ok(None)` when the section is absent — a module fused without
/// `--emit-manifest` is not an error. Returns `Err` when the section is present
/// but cannot be understood, including an unrecognised `version`: a manifest we
/// cannot read is refused, never treated as if it were missing.
#[cfg(feature = "std")]
pub fn extract_signature_manifest_from_binary(
    binary: &[u8],
) -> Result<Option<SignatureManifest>, Error> {
    use kiln_format::binary::read_leb128_u32;

    if binary.len() < 8 || &binary[0..4] != b"\0asm" {
        return Err(Error::parse_error(
            "Not a WebAssembly binary (bad magic) while scanning for the signature manifest",
        ));
    }

    let mut offset = 8;
    while offset < binary.len() {
        let section_id = binary[offset];
        offset += 1;
        let (payload_len, len_size) = read_leb128_u32(binary, offset)?;
        offset += len_size;
        let payload_end = offset
            .checked_add(payload_len as usize)
            .ok_or_else(|| Error::parse_error("Section size overflows while scanning"))?;
        if payload_end > binary.len() {
            return Err(Error::parse_error(
                "Section extends past end of binary while scanning for the signature manifest",
            ));
        }
        let payload = &binary[offset..payload_end];
        offset = payload_end;

        if section_id != 0 {
            continue;
        }

        let (name_len, name_len_size) = read_leb128_u32(payload, 0)?;
        let name_end = name_len_size
            .checked_add(name_len as usize)
            .ok_or_else(|| Error::parse_error("Custom section name overflows"))?;
        if name_end > payload.len() {
            return Err(Error::parse_error(
                "Custom section name extends past section payload",
            ));
        }
        let name = core::str::from_utf8(&payload[name_len_size..name_end])
            .map_err(|_| Error::parse_error("Custom section name is not valid UTF-8"))?;
        if name != SIGNATURE_MANIFEST_SECTION_NAME {
            continue;
        }

        // Section is PRESENT: every failure from here is loud.
        let manifest: SignatureManifest = serde_json::from_slice(&payload[name_end..])
            .map_err(|_| Error::parse_error("signature manifest is not valid JSON"))?;

        if manifest.version != SUPPORTED_MANIFEST_VERSION {
            return Err(Error::parse_error(
                "signature manifest version is not supported by this runtime",
            ));
        }

        return Ok(Some(manifest));
    }

    Ok(None)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    fn wasm_with_section(name: &str, contents: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"\0asm\x01\0\0\0");
        let mut payload = Vec::new();
        payload.push(name.len() as u8); // LEB128 for small lengths
        payload.extend_from_slice(name.as_bytes());
        payload.extend_from_slice(contents);
        out.push(0); // custom section id
        // LEB128 length (payloads here stay under 2^21)
        let mut n = payload.len();
        loop {
            let mut byte = (n & 0x7f) as u8;
            n >>= 7;
            if n != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if n == 0 {
                break;
            }
        }
        out.extend_from_slice(&payload);
        out
    }

    const MINIMAL: &str = r#"{"version":1,"exports":[{"export":"p:i/f@1.0.0#g",
        "wit":{"params":[],"result":"u32"},
        "core":{"params":["i32"],"results":["i32"]},
        "flat_param_count":1,"needs":{"memory":false,"realloc":false},
        "memory":null,"realloc":null,"post_return":null,"return_area":null}]}"#;

    #[test]
    fn absent_section_is_none_not_an_error() {
        let wasm = wasm_with_section("something.else", b"{}");
        assert!(
            extract_signature_manifest_from_binary(&wasm).unwrap().is_none(),
            "a module fused without --emit-manifest is not an error"
        );
    }

    #[test]
    fn present_section_parses() {
        let wasm = wasm_with_section(SIGNATURE_MANIFEST_SECTION_NAME, MINIMAL.as_bytes());
        let m = extract_signature_manifest_from_binary(&wasm).unwrap().unwrap();
        assert_eq!(m.version, 1);
        assert_eq!(m.exports.len(), 1);
        assert!(m.find("p:i/f@1.0.0#g").is_some());
    }

    #[test]
    fn malformed_json_fails_loud_rather_than_reading_as_absent() {
        let wasm = wasm_with_section(SIGNATURE_MANIFEST_SECTION_NAME, b"{not json");
        assert!(
            extract_signature_manifest_from_binary(&wasm).is_err(),
            "a present-but-broken manifest must not be silently treated as absent"
        );
    }

    #[test]
    fn unknown_version_is_refused_not_best_effort_parsed() {
        let bumped = MINIMAL.replace("\"version\":1", "\"version\":2");
        let wasm = wasm_with_section(SIGNATURE_MANIFEST_SECTION_NAME, bumped.as_bytes());
        assert!(
            extract_signature_manifest_from_binary(&wasm).is_err(),
            "an unknown major version must refuse the whole manifest"
        );
    }

    #[test]
    fn indirect_params_detected_by_disagreement_not_by_rederiving_the_limit() {
        // 18 flattened values collapsed to a single i32 pointer.
        let json = MINIMAL
            .replace("\"flat_param_count\":1", "\"flat_param_count\":18");
        let wasm = wasm_with_section(SIGNATURE_MANIFEST_SECTION_NAME, json.as_bytes());
        let m = extract_signature_manifest_from_binary(&wasm).unwrap().unwrap();
        let e = m.find("p:i/f@1.0.0#g").unwrap();
        assert!(e.params_are_indirect(), "18 flattened vs 1 core param is indirect");

        // The flat case must not be mistaken for indirect.
        let wasm = wasm_with_section(SIGNATURE_MANIFEST_SECTION_NAME, MINIMAL.as_bytes());
        let m = extract_signature_manifest_from_binary(&wasm).unwrap().unwrap();
        assert!(!m.find("p:i/f@1.0.0#g").unwrap().params_are_indirect());
    }

    #[test]
    fn unreachable_realloc_is_recognised() {
        let json = MINIMAL.replace(
            "\"needs\":{\"memory\":false,\"realloc\":false}",
            "\"needs\":{\"memory\":true,\"realloc\":true}",
        );
        let wasm = wasm_with_section(SIGNATURE_MANIFEST_SECTION_NAME, json.as_bytes());
        let m = extract_signature_manifest_from_binary(&wasm).unwrap().unwrap();
        assert!(
            m.find("p:i/f@1.0.0#g").unwrap().realloc_is_unreachable(),
            "needs.realloc with realloc:null is the contradiction meld states deliberately"
        );
    }
}
