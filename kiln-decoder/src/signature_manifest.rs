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
use core::fmt;

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

/// The export's WIT-level signature, as meld renders it.
#[derive(Debug, Clone, Deserialize)]
pub struct WitSignature {
    /// `(name, type)` per parameter, types rendered as WIT text.
    #[serde(default)]
    pub params: Vec<(String, String)>,
    /// The result type rendered as WIT text, or `None` for no result.
    #[serde(default)]
    pub result: Option<String>,
}

/// One exported canonical name and everything needed to call it.
#[derive(Debug, Clone, Deserialize)]
pub struct ExportSignature {
    /// Canonical export name, exactly as meld emits it.
    pub export: String,
    /// The WIT signature. The return-area layout carries field names and
    /// sizes but not types; the types come from here (SR-61).
    pub wit: WitSignature,
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

    /// Decode the bytes of this export's return area into named values (SR-61).
    ///
    /// `bytes` must be exactly the `return_area.size` bytes read from the
    /// pointer the export returned. Supported results are WIT records whose
    /// fields are all primitive scalars; the field types come from `wit.result`
    /// and are matched to the layout by name, in order. Every disagreement —
    /// names, sizes, a field overrunning the area, a buffer of the wrong
    /// length — is refused: a value decoded against the wrong type is a
    /// fabricated result. Compound fields are refused rather than guessed,
    /// because meld describes only the top level of the layout.
    ///
    /// # Errors
    ///
    /// Returns a [`ReturnAreaError`] naming what could not be decoded.
    pub fn decode_return_area(
        &self,
        bytes: &[u8],
    ) -> Result<Vec<(String, ReturnValue)>, ReturnAreaError> {
        let area = self.return_area.as_ref().ok_or(ReturnAreaError::NoReturnArea)?;
        let result_wit = self.wit.result.as_deref().ok_or(ReturnAreaError::NoWitResult)?;
        if bytes.len() != area.size as usize {
            return Err(ReturnAreaError::BufferSize {
                expected: area.size,
                got: bytes.len(),
            });
        }

        let wit_fields = parse_scalar_record(result_wit)?;
        if wit_fields.len() != area.layout.len() {
            return Err(ReturnAreaError::FieldCountMismatch {
                layout: area.layout.len(),
                wit: wit_fields.len(),
            });
        }

        let mut out = Vec::with_capacity(wit_fields.len());
        for (index, (slot, (wit_name, ty))) in area.layout.iter().zip(wit_fields).enumerate() {
            if slot.field != wit_name {
                return Err(ReturnAreaError::FieldMismatch {
                    index,
                    layout: slot.field.clone(),
                    wit: wit_name,
                });
            }
            if slot.size != ty.size() {
                return Err(ReturnAreaError::SizeMismatch {
                    field: wit_name,
                    layout: slot.size,
                    wit: ty.size(),
                });
            }
            let start = slot.offset as usize;
            let end = start
                .checked_add(slot.size as usize)
                .filter(|&end| end <= bytes.len())
                .ok_or_else(|| ReturnAreaError::FieldOverrun {
                    field: wit_name.clone(),
                    offset: slot.offset,
                    size: slot.size,
                    area: area.size,
                })?;
            let value =
                ty.decode(&bytes[start..end]).ok_or_else(|| ReturnAreaError::InvalidChar {
                    field: wit_name.clone(),
                    value: u32::from_le_bytes([
                        bytes[start],
                        bytes[start + 1],
                        bytes[start + 2],
                        bytes[start + 3],
                    ]),
                })?;
            out.push((wit_name, value));
        }
        Ok(out)
    }
}

/// A WIT primitive that can appear as a field of a decodable return area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarType {
    /// `bool`
    Bool,
    /// `u8`
    U8,
    /// `s8`
    S8,
    /// `u16`
    U16,
    /// `s16`
    S16,
    /// `u32`
    U32,
    /// `s32`
    S32,
    /// `u64`
    U64,
    /// `s64`
    S64,
    /// `f32`
    F32,
    /// `f64`
    F64,
    /// `char`
    Char,
}

impl ScalarType {
    fn from_wit(name: &str) -> Option<Self> {
        Some(match name {
            "bool" => Self::Bool,
            "u8" => Self::U8,
            "s8" => Self::S8,
            "u16" => Self::U16,
            "s16" => Self::S16,
            "u32" => Self::U32,
            "s32" => Self::S32,
            "u64" => Self::U64,
            "s64" => Self::S64,
            "f32" => Self::F32,
            "f64" => Self::F64,
            "char" => Self::Char,
            _ => return None,
        })
    }

    /// Size in bytes in the Canonical ABI memory layout.
    pub fn size(self) -> u32 {
        match self {
            Self::Bool | Self::U8 | Self::S8 => 1,
            Self::U16 | Self::S16 => 2,
            Self::U32 | Self::S32 | Self::F32 | Self::Char => 4,
            Self::U64 | Self::S64 | Self::F64 => 8,
        }
    }

    /// Lift a little-endian value of exactly [`Self::size`] bytes. `None` only
    /// for a `char` that is not a Unicode scalar value, which the Canonical ABI
    /// traps on.
    fn decode(self, b: &[u8]) -> Option<ReturnValue> {
        let w4 = |b: &[u8]| [b[0], b[1], b[2], b[3]];
        let w8 = |b: &[u8]| [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]];
        Some(match self {
            Self::Bool => ReturnValue::Bool(b[0] != 0),
            Self::U8 => ReturnValue::U8(b[0]),
            Self::S8 => ReturnValue::S8(b[0] as i8),
            Self::U16 => ReturnValue::U16(u16::from_le_bytes([b[0], b[1]])),
            Self::S16 => ReturnValue::S16(i16::from_le_bytes([b[0], b[1]])),
            Self::U32 => ReturnValue::U32(u32::from_le_bytes(w4(b))),
            Self::S32 => ReturnValue::S32(i32::from_le_bytes(w4(b))),
            Self::U64 => ReturnValue::U64(u64::from_le_bytes(w8(b))),
            Self::S64 => ReturnValue::S64(i64::from_le_bytes(w8(b))),
            Self::F32 => ReturnValue::F32(f32::from_le_bytes(w4(b))),
            Self::F64 => ReturnValue::F64(f64::from_le_bytes(w8(b))),
            Self::Char => ReturnValue::Char(char::from_u32(u32::from_le_bytes(w4(b)))?),
        })
    }
}

/// A value lifted from a return area.
///
/// Derives `PartialEq` but not `Eq`: floats are not totally ordered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReturnValue {
    /// `bool`
    Bool(bool),
    /// `u8`
    U8(u8),
    /// `s8`
    S8(i8),
    /// `u16`
    U16(u16),
    /// `s16`
    S16(i16),
    /// `u32`
    U32(u32),
    /// `s32`
    S32(i32),
    /// `u64`
    U64(u64),
    /// `s64`
    S64(i64),
    /// `f32`
    F32(f32),
    /// `f64`
    F64(f64),
    /// `char`
    Char(char),
}

impl fmt::Display for ReturnValue {
    /// `<wit-type> <value>`, matching how kiln prints core results.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "bool {v}"),
            Self::U8(v) => write!(f, "u8 {v}"),
            Self::S8(v) => write!(f, "s8 {v}"),
            Self::U16(v) => write!(f, "u16 {v}"),
            Self::S16(v) => write!(f, "s16 {v}"),
            Self::U32(v) => write!(f, "u32 {v}"),
            Self::S32(v) => write!(f, "s32 {v}"),
            Self::U64(v) => write!(f, "u64 {v}"),
            Self::S64(v) => write!(f, "s64 {v}"),
            Self::F32(v) => write!(f, "f32 {v}"),
            Self::F64(v) => write!(f, "f64 {v}"),
            Self::Char(v) => write!(f, "char {v:?}"),
        }
    }
}

/// Why a return area could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnAreaError {
    /// The export has no return area in the manifest.
    NoReturnArea,
    /// The manifest gives a return area but no WIT result type.
    NoWitResult,
    /// The result type is not a record (e.g. `string`, `list<..>`, `option<..>`).
    ResultNotARecord(String),
    /// The record text could not be parsed.
    MalformedRecord(String),
    /// A field's type is not a primitive scalar.
    UnsupportedFieldType {
        /// Field name.
        field: String,
        /// The field's WIT type.
        wit_type: String,
    },
    /// Layout and WIT list a different number of fields.
    FieldCountMismatch {
        /// Fields in the layout.
        layout: usize,
        /// Fields in the WIT record.
        wit: usize,
    },
    /// Layout and WIT name a field differently at the same position.
    FieldMismatch {
        /// Field position.
        index: usize,
        /// Name in the layout.
        layout: String,
        /// Name in the WIT record.
        wit: String,
    },
    /// The layout's size for a field differs from its WIT type's size.
    SizeMismatch {
        /// Field name.
        field: String,
        /// Size in the layout.
        layout: u32,
        /// Size of the WIT type.
        wit: u32,
    },
    /// A field extends past the end of the return area.
    FieldOverrun {
        /// Field name.
        field: String,
        /// Field offset.
        offset: u32,
        /// Field size.
        size: u32,
        /// Return-area size.
        area: u32,
    },
    /// The bytes supplied are not exactly the return area.
    BufferSize {
        /// `return_area.size`.
        expected: u32,
        /// Bytes supplied.
        got: usize,
    },
    /// A `char` field holds a value that is not a Unicode scalar.
    InvalidChar {
        /// Field name.
        field: String,
        /// The raw value.
        value: u32,
    },
}

impl fmt::Display for ReturnAreaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoReturnArea => write!(f, "the manifest gives this export no return area"),
            Self::NoWitResult => {
                write!(f, "the manifest gives a return area but no WIT result type")
            },
            Self::ResultNotARecord(t) => write!(
                f,
                "result type `{t}` is not a record; only records of primitive fields can be \
                 decoded"
            ),
            Self::MalformedRecord(t) => write!(f, "cannot parse the WIT record `{t}`"),
            Self::UnsupportedFieldType { field, wit_type } => write!(
                f,
                "field `{field}` has type `{wit_type}`; only primitive fields can be decoded \
                 (the manifest describes the top level of the layout only)"
            ),
            Self::FieldCountMismatch { layout, wit } => write!(
                f,
                "the return-area layout lists {layout} field(s) but the WIT result has {wit}"
            ),
            Self::FieldMismatch { index, layout, wit } => write!(
                f,
                "field {index} is `{layout}` in the return-area layout but `{wit}` in the WIT \
                 result"
            ),
            Self::SizeMismatch { field, layout, wit } => write!(
                f,
                "field `{field}` is {layout} byte(s) in the layout but its WIT type is {wit}"
            ),
            Self::FieldOverrun {
                field,
                offset,
                size,
                area,
            } => write!(
                f,
                "field `{field}` ({size} byte(s) at offset {offset}) runs past the {area}-byte \
                 return area"
            ),
            Self::BufferSize { expected, got } => {
                write!(
                    f,
                    "expected the {expected}-byte return area, got {got} byte(s)"
                )
            },
            Self::InvalidChar { field, value } => write!(
                f,
                "field `{field}` holds {value:#x}, which is not a Unicode scalar value"
            ),
        }
    }
}

impl From<ReturnAreaError> for Error {
    fn from(_: ReturnAreaError) -> Self {
        Error::runtime_type_mismatch("return area cannot be decoded against the signature manifest")
    }
}

/// Parse `record { a: f32, b: u8 }` into `[(a, F32), (b, U8)]`.
///
/// Splits on commas at nesting depth zero so a compound field type is reported
/// as unsupported by name instead of being mis-split.
fn parse_scalar_record(wit: &str) -> Result<Vec<(String, ScalarType)>, ReturnAreaError> {
    let body = wit
        .trim()
        .strip_prefix("record")
        .map(str::trim)
        .ok_or_else(|| ReturnAreaError::ResultNotARecord(String::from(wit)))?;
    let body = body
        .strip_prefix('{')
        .and_then(|b| b.strip_suffix('}'))
        .ok_or_else(|| ReturnAreaError::MalformedRecord(String::from(wit)))?;

    let mut entries = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, ch) in body.char_indices() {
        match ch {
            '<' | '{' | '(' => depth += 1,
            '>' | '}' | ')' => depth -= 1,
            ',' if depth == 0 => {
                entries.push(&body[start..i]);
                start = i + 1;
            },
            _ => {},
        }
    }
    entries.push(&body[start..]);

    let mut fields = Vec::new();
    for entry in entries.into_iter().map(str::trim).filter(|e| !e.is_empty()) {
        let (name, ty) = entry
            .split_once(':')
            .ok_or_else(|| ReturnAreaError::MalformedRecord(String::from(wit)))?;
        let (name, ty) = (name.trim(), ty.trim());
        let scalar =
            ScalarType::from_wit(ty).ok_or_else(|| ReturnAreaError::UnsupportedFieldType {
                field: String::from(name),
                wit_type: String::from(ty),
            })?;
        fields.push((String::from(name), scalar));
    }
    Ok(fields)
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
        let json = MINIMAL.replace("\"flat_param_count\":1", "\"flat_param_count\":18");
        let wasm = wasm_with_section(SIGNATURE_MANIFEST_SECTION_NAME, json.as_bytes());
        let m = extract_signature_manifest_from_binary(&wasm).unwrap().unwrap();
        let e = m.find("p:i/f@1.0.0#g").unwrap();
        assert!(
            e.params_are_indirect(),
            "18 flattened vs 1 core param is indirect"
        );

        // The flat case must not be mistaken for indirect.
        let wasm = wasm_with_section(SIGNATURE_MANIFEST_SECTION_NAME, MINIMAL.as_bytes());
        let m = extract_signature_manifest_from_binary(&wasm).unwrap().unwrap();
        assert!(!m.find("p:i/f@1.0.0#g").unwrap().params_are_indirect());
    }

    // --- SR-61: return-area decoding -------------------------------------

    fn export_with_result(result_wit: &str, area: &str) -> ExportSignature {
        let json = format!(
            r#"{{"export":"p:i/f@1.0.0#g",
            "wit":{{"params":[],"result":{result}}},
            "core":{{"params":[],"results":["i32"]}},
            "flat_param_count":0,"needs":{{"memory":true,"realloc":false}},
            "memory":"memory","realloc":null,"post_return":null,
            "return_area":{area}}}"#,
            result = serde_json::to_string(result_wit).unwrap(),
        );
        serde_json::from_str(&json).unwrap()
    }

    const PWM_WIT: &str = "record { m1: f32, m2: f32, m3: f32, m4: f32 }";
    const PWM_AREA: &str = r#"{"size":16,"align":4,"layout":[
        {"offset":0,"field":"m1","size":4},{"offset":4,"field":"m2","size":4},
        {"offset":8,"field":"m3","size":4},{"offset":12,"field":"m4","size":4}]}"#;

    fn le_f32s(vals: &[f32]) -> Vec<u8> {
        vals.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn decodes_a_record_of_f32_fields_by_name() {
        let sig = export_with_result(PWM_WIT, PWM_AREA);
        let fields = sig.decode_return_area(&le_f32s(&[0.5, 0.25, -1.0, 0.125])).unwrap();
        let rendered: Vec<String> =
            fields.iter().map(|(n, v)| alloc::format!("{n}: {v}")).collect();
        assert_eq!(
            rendered,
            ["m1: f32 0.5", "m2: f32 0.25", "m3: f32 -1", "m4: f32 0.125"]
        );
    }

    #[test]
    fn decodes_mixed_widths_at_their_layout_offsets() {
        // u8 at offset 4 followed by padding, then a u32 at 8 and an s32 at 12.
        let sig = export_with_result(
            "record { head: u32, rotor: u8, valid: u32, delta: s32 }",
            r#"{"size":16,"align":4,"layout":[
                {"offset":0,"field":"head","size":4},{"offset":4,"field":"rotor","size":1},
                {"offset":8,"field":"valid","size":4},{"offset":12,"field":"delta","size":4}]}"#,
        );
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&7u32.to_le_bytes());
        bytes.extend_from_slice(&[3, 0xAA, 0xAA, 0xAA]); // padding must be ignored
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(-5i32).to_le_bytes());
        let rendered: Vec<String> = sig
            .decode_return_area(&bytes)
            .unwrap()
            .iter()
            .map(|(n, v)| alloc::format!("{n}: {v}"))
            .collect();
        assert_eq!(
            rendered,
            [
                "head: u32 7",
                "rotor: u8 3",
                "valid: u32 1",
                "delta: s32 -5"
            ]
        );
    }

    #[test]
    fn refuses_a_compound_field_rather_than_guessing_below_the_top_level() {
        let sig = export_with_result(
            "record { pos: record { x: f32, y: f32 }, ok: u32 }",
            r#"{"size":12,"align":4,"layout":[
                {"offset":0,"field":"pos","size":8},{"offset":8,"field":"ok","size":4}]}"#,
        );
        assert!(matches!(
            sig.decode_return_area(&[0; 12]),
            Err(ReturnAreaError::UnsupportedFieldType { ref field, .. }) if field == "pos"
        ));
    }

    #[test]
    fn refuses_a_result_that_is_not_a_record() {
        let sig = export_with_result("string", r#"{"size":8,"align":4,"layout":[]}"#);
        assert!(matches!(
            sig.decode_return_area(&[0; 8]),
            Err(ReturnAreaError::ResultNotARecord(_))
        ));
    }

    #[test]
    fn refuses_when_layout_and_wit_disagree_on_field_names() {
        let sig = export_with_result("record { m1: f32, m2: f32, m3: f32, mX: f32 }", PWM_AREA);
        assert!(matches!(
            sig.decode_return_area(&[0; 16]),
            Err(ReturnAreaError::FieldMismatch { index: 3, .. })
        ));
    }

    #[test]
    fn refuses_when_layout_size_disagrees_with_the_wit_type() {
        let sig = export_with_result("record { m1: f32, m2: f32, m3: f32, m4: u8 }", PWM_AREA);
        assert!(matches!(
            sig.decode_return_area(&[0; 16]),
            Err(ReturnAreaError::SizeMismatch {
                layout: 4,
                wit: 1,
                ..
            })
        ));
    }

    #[test]
    fn refuses_a_buffer_that_is_not_exactly_the_return_area() {
        let sig = export_with_result(PWM_WIT, PWM_AREA);
        assert_eq!(
            sig.decode_return_area(&[0; 12]),
            Err(ReturnAreaError::BufferSize {
                expected: 16,
                got: 12
            })
        );
    }

    #[test]
    fn refuses_a_field_that_overruns_the_return_area() {
        let sig = export_with_result(
            "record { a: f32, b: f64 }",
            r#"{"size":8,"align":8,"layout":[
                {"offset":0,"field":"a","size":4},{"offset":4,"field":"b","size":8}]}"#,
        );
        assert!(matches!(
            sig.decode_return_area(&[0; 8]),
            Err(ReturnAreaError::FieldOverrun { ref field, .. }) if field == "b"
        ));
    }

    #[test]
    fn decodes_bool_and_char_with_canonical_abi_validation() {
        let sig = export_with_result(
            "record { on: bool, c: char }",
            r#"{"size":8,"align":4,"layout":[
                {"offset":0,"field":"on","size":1},{"offset":4,"field":"c","size":4}]}"#,
        );
        let mut bytes = alloc::vec![1, 0, 0, 0];
        bytes.extend_from_slice(&('k' as u32).to_le_bytes());
        let rendered: Vec<String> = sig
            .decode_return_area(&bytes)
            .unwrap()
            .iter()
            .map(|(n, v)| alloc::format!("{n}: {v}"))
            .collect();
        assert_eq!(rendered, ["on: bool true", "c: char 'k'"]);

        // A surrogate is not a valid Unicode scalar: lifting it traps in the
        // Canonical ABI, so the reader must refuse, not print a replacement.
        let mut bad = alloc::vec![1, 0, 0, 0];
        bad.extend_from_slice(&0xD800u32.to_le_bytes());
        assert!(matches!(
            sig.decode_return_area(&bad),
            Err(ReturnAreaError::InvalidChar { value: 0xD800, .. })
        ));
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
