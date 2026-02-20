#![deny(warnings)]

use wrt_component::{
    ComponentValue,
    canonical::{CanonicalABI, CanonicalOptions, ValueType},
    resources::ResourceStrategy,
};
use wrt_error::Error;
use wrt_foundation::values::{Integer, Real, Value};

/// Tests for encoding/decoding simple primitive types
#[test]
fn test_encode_decode_primitives() {
    let abi = CanonicalABI::default();

    // Test i32
    let i32_value = ComponentValue::I32(42);
    let encoded = abi.encode(&i32_value).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, ValueType::I32).unwrap();
    assert_eq!(decoded, i32_value);

    // Test i64
    let i64_value = ComponentValue::I64(9223372036854775807);
    let encoded = abi.encode(&i64_value).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, ValueType::I64).unwrap();
    assert_eq!(decoded, i64_value);

    // Test f32
    let f32_value = ComponentValue::F32(3.14);
    let encoded = abi.encode(&f32_value).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, ValueType::F32).unwrap();
    assert_eq!(decoded, f32_value);

    // Test f64
    let f64_value = ComponentValue::F64(2.71828);
    let encoded = abi.encode(&f64_value).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, ValueType::F64).unwrap();
    assert_eq!(decoded, f64_value);

    // Test bool (true)
    let bool_true = ComponentValue::Bool(true);
    let encoded = abi.encode(&bool_true).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, ValueType::Bool).unwrap();
    assert_eq!(decoded, bool_true);

    // Test bool (false)
    let bool_false = ComponentValue::Bool(false);
    let encoded = abi.encode(&bool_false).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, ValueType::Bool).unwrap();
    assert_eq!(decoded, bool_false);
}

/// Tests for encoding/decoding string values
#[test]
fn test_encode_decode_strings() {
    let abi = CanonicalABI::default();

    // Empty string
    let empty_string = ComponentValue::String("".to_string());
    let encoded = abi.encode(&empty_string).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, ValueType::String).unwrap();
    assert_eq!(decoded, empty_string);

    // ASCII string
    let ascii_string = ComponentValue::String("Hello, world!".to_string());
    let encoded = abi.encode(&ascii_string).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, ValueType::String).unwrap();
    assert_eq!(decoded, ascii_string);

    // Unicode string
    let unicode_string = ComponentValue::String("こんにちは世界".to_string());
    let encoded = abi.encode(&unicode_string).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, ValueType::String).unwrap();
    assert_eq!(decoded, unicode_string);

    // Long string
    let long_string = ComponentValue::String("a".repeat(1000));
    let encoded = abi.encode(&long_string).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, ValueType::String).unwrap();
    assert_eq!(decoded, long_string);
}

/// Tests for encoding/decoding lists
#[test]
fn test_encode_decode_lists() {
    let abi = CanonicalABI::default();

    // Empty list
    let empty_list = ComponentValue::List(vec![]);
    let encoded = abi.encode(&empty_list).unwrap();
    let decoded = abi
        .decode::<ComponentValue>(&encoded, ValueType::List(Box::new(ValueType::I32)))
        .unwrap();
    assert_eq!(decoded, empty_list);

    // List of i32
    let i32_list = ComponentValue::List(vec![
        ComponentValue::I32(1),
        ComponentValue::I32(2),
        ComponentValue::I32(3),
    ]);
    let encoded = abi.encode(&i32_list).unwrap();
    let decoded = abi
        .decode::<ComponentValue>(&encoded, ValueType::List(Box::new(ValueType::I32)))
        .unwrap();
    assert_eq!(decoded, i32_list);

    // Nested list
    let nested_list = ComponentValue::List(vec![
        ComponentValue::List(vec![ComponentValue::I32(1), ComponentValue::I32(2)]),
        ComponentValue::List(vec![ComponentValue::I32(3), ComponentValue::I32(4)]),
    ]);
    let list_type = ValueType::List(Box::new(ValueType::List(Box::new(ValueType::I32))));
    let encoded = abi.encode(&nested_list).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, list_type).unwrap();
    assert_eq!(decoded, nested_list);
}

/// Tests for encoding/decoding records
#[test]
fn test_encode_decode_records() {
    let abi = CanonicalABI::default();

    // Simple record
    let record = ComponentValue::Record(vec![
        ComponentValue::String("name".to_string()),
        ComponentValue::I32(42),
        ComponentValue::Bool(true),
    ]);

    // Define record type
    let record_type = ValueType::Record(vec![ValueType::String, ValueType::I32, ValueType::Bool]);

    let encoded = abi.encode(&record).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, record_type).unwrap();
    assert_eq!(decoded, record);

    // Nested record
    let nested_record = ComponentValue::Record(vec![
        ComponentValue::String("user".to_string()),
        ComponentValue::Record(vec![
            ComponentValue::String("name".to_string()),
            ComponentValue::I32(25),
        ]),
        ComponentValue::Bool(false),
    ]);

    // Define nested record type
    let nested_record_type = ValueType::Record(vec![
        ValueType::String,
        ValueType::Record(vec![ValueType::String, ValueType::I32]),
        ValueType::Bool,
    ]);

    let encoded = abi.encode(&nested_record).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, nested_record_type).unwrap();
    assert_eq!(decoded, nested_record);
}

/// Tests for encoding/decoding variants
#[test]
fn test_encode_decode_variants() {
    let abi = CanonicalABI::default();

    // Simple variant
    let variant = ComponentValue::Variant {
        case: 1,
        value: Box::new(ComponentValue::String("success".to_string())),
    };

    // Define variant type
    let variant_type = ValueType::Variant(vec![
        ValueType::I32,    // case 0
        ValueType::String, // case 1
        ValueType::Bool,   // case 2
    ]);

    let encoded = abi.encode(&variant).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, variant_type.clone()).unwrap();
    assert_eq!(decoded, variant);

    // Another case
    let variant2 = ComponentValue::Variant {
        case: 2,
        value: Box::new(ComponentValue::Bool(true)),
    };

    let encoded = abi.encode(&variant2).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, variant_type.clone()).unwrap();
    assert_eq!(decoded, variant2);

    // Empty variant case
    let variant0 = ComponentValue::Variant {
        case: 0,
        value: Box::new(ComponentValue::I32(404)),
    };

    let encoded = abi.encode(&variant0).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, variant_type).unwrap();
    assert_eq!(decoded, variant0);
}

/// Tests for encoding/decoding tuples
#[test]
fn test_encode_decode_tuples() {
    let abi = CanonicalABI::default();

    // Simple tuple (i32, string)
    let tuple = ComponentValue::Tuple(vec![
        ComponentValue::I32(42),
        ComponentValue::String("hello".to_string()),
    ]);

    // Define tuple type
    let tuple_type = ValueType::Tuple(vec![ValueType::I32, ValueType::String]);

    let encoded = abi.encode(&tuple).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, tuple_type).unwrap();
    assert_eq!(decoded, tuple);

    // Empty tuple
    let empty_tuple = ComponentValue::Tuple(vec![]);
    let empty_tuple_type = ValueType::Tuple(vec![]);

    let encoded = abi.encode(&empty_tuple).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, empty_tuple_type).unwrap();
    assert_eq!(decoded, empty_tuple);

    // Nested tuple
    let nested_tuple = ComponentValue::Tuple(vec![
        ComponentValue::I32(1),
        ComponentValue::Tuple(vec![
            ComponentValue::String("nested".to_string()),
            ComponentValue::Bool(true),
        ]),
    ]);

    // Define nested tuple type
    let nested_tuple_type = ValueType::Tuple(vec![
        ValueType::I32,
        ValueType::Tuple(vec![ValueType::String, ValueType::Bool]),
    ]);

    let encoded = abi.encode(&nested_tuple).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, nested_tuple_type).unwrap();
    assert_eq!(decoded, nested_tuple);
}

/// Tests for encoding/decoding options
#[test]
fn test_encode_decode_options() {
    let abi = CanonicalABI::default();

    // Some value
    let some_value = ComponentValue::Option(Some(Box::new(ComponentValue::I32(42))));
    let option_type = ValueType::Option(Box::new(ValueType::I32));

    let encoded = abi.encode(&some_value).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, option_type.clone()).unwrap();
    assert_eq!(decoded, some_value);

    // None value
    let none_value = ComponentValue::Option(None);

    let encoded = abi.encode(&none_value).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, option_type).unwrap();
    assert_eq!(decoded, none_value);

    // Option containing a complex type
    let complex_option = ComponentValue::Option(Some(Box::new(ComponentValue::List(vec![
        ComponentValue::String("item1".to_string()),
        ComponentValue::String("item2".to_string()),
    ]))));

    let complex_option_type =
        ValueType::Option(Box::new(ValueType::List(Box::new(ValueType::String))));

    let encoded = abi.encode(&complex_option).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, complex_option_type).unwrap();
    assert_eq!(decoded, complex_option);
}

/// Tests for encoding/decoding results
#[test]
fn test_encode_decode_results() {
    let abi = CanonicalABI::default();

    // Ok result
    let ok_result = ComponentValue::Result {
        is_ok: true,
        value: Box::new(ComponentValue::String("success".to_string())),
    };

    let result_type = ValueType::Result {
        ok: Box::new(ValueType::String),
        err: Box::new(ValueType::I32),
    };

    let encoded = abi.encode(&ok_result).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, result_type.clone()).unwrap();
    assert_eq!(decoded, ok_result);

    // Err result
    let err_result = ComponentValue::Result {
        is_ok: false,
        value: Box::new(ComponentValue::I32(404)),
    };

    let encoded = abi.encode(&err_result).unwrap();
    let decoded = abi.decode::<ComponentValue>(&encoded, result_type).unwrap();
    assert_eq!(decoded, err_result);
}

/// Tests for resource handling
#[test]
fn test_resource_handling() {
    // Create a custom ABI with resource strategy
    let abi = CanonicalABI::new(CanonicalOptions {
        resource_strategy: ResourceStrategy::Reference,
        ..Default::default()
    });

    // Encode a resource
    let resource_value = ComponentValue::Resource { id: 42 };
    let encoded = abi.encode_resource(&resource_value, 42).unwrap();

    // Decode the resource
    let decoded = abi.decode_resource::<ComponentValue>(&encoded, 42).unwrap();
    assert_eq!(decoded, resource_value);

    // Test invalid resource ID
    let result = abi.decode_resource::<ComponentValue>(&encoded, 43);
    assert!(result.is_err(), "Should fail with mismatched resource ID");
}

/// Tests for handling errors
#[test]
fn test_error_handling() {
    let abi = CanonicalABI::default();

    // Attempt to decode with wrong type
    let i32_value = ComponentValue::I32(42);
    let encoded = abi.encode(&i32_value).unwrap();
    let result = abi.decode::<ComponentValue>(&encoded, ValueType::F32);
    assert!(result.is_err(), "Should fail with type mismatch");

    // Malformed data
    let result = abi.decode::<ComponentValue>(&[0xFF, 0xFF], ValueType::I32);
    assert!(result.is_err(), "Should fail with malformed data");

    // Invalid variant case
    let invalid_variant = ComponentValue::Variant {
        case: 5, // Out of bounds
        value: Box::new(ComponentValue::I32(0)),
    };

    let variant_type = ValueType::Variant(vec![ValueType::I32, ValueType::String]);

    let result = abi.encode(&invalid_variant);
    assert!(result.is_err(), "Should fail with invalid variant case");
}

/// Tests for memory limits
#[test]
fn test_memory_limits() {
    // Create ABI with strict memory limits
    let abi = CanonicalABI::new(CanonicalOptions {
        max_string_len: Some(10),
        max_list_len: Some(5),
        ..Default::default()
    });

    // String within limits
    let short_string = ComponentValue::String("short".to_string());
    assert!(abi.encode(&short_string).is_ok());

    // String exceeding limits
    let long_string = ComponentValue::String("this string is too long".to_string());
    assert!(abi.encode(&long_string).is_err());

    // List within limits
    let short_list = ComponentValue::List(vec![ComponentValue::I32(1), ComponentValue::I32(2)]);
    assert!(abi.encode(&short_list).is_ok());

    // List exceeding limits
    let long_list = ComponentValue::List(vec![
        ComponentValue::I32(1),
        ComponentValue::I32(2),
        ComponentValue::I32(3),
        ComponentValue::I32(4),
        ComponentValue::I32(5),
        ComponentValue::I32(6),
    ]);
    assert!(abi.encode(&long_list).is_err());
}

/// Tests conversions from/to Rust native types
#[test]
fn test_rust_native_conversions() {
    let abi = CanonicalABI::default();

    // Convert from Rust i32
    let rust_i32: i32 = 42;
    let component_i32 = ComponentValue::from(rust_i32);
    let encoded = abi.encode(&component_i32).unwrap();
    let decoded = abi.decode::<i32>(&encoded, ValueType::I32).unwrap();
    assert_eq!(decoded, rust_i32);

    // Convert from Rust String
    let rust_string = "Hello, world!".to_string();
    let component_string = ComponentValue::from(rust_string.clone());
    let encoded = abi.encode(&component_string).unwrap();
    let decoded = abi.decode::<String>(&encoded, ValueType::String).unwrap();
    assert_eq!(decoded, rust_string);

    // Convert from Rust bool
    let rust_bool = true;
    let component_bool = ComponentValue::from(rust_bool);
    let encoded = abi.encode(&component_bool).unwrap();
    let decoded = abi.decode::<bool>(&encoded, ValueType::Bool).unwrap();
    assert_eq!(decoded, rust_bool);

    // Convert from Rust Vec
    let rust_vec = vec![1, 2, 3];
    let component_vec = ComponentValue::from_vec(rust_vec.clone());
    let encoded = abi.encode(&component_vec).unwrap();
    let decoded = abi
        .decode_vec::<i32>(&encoded, ValueType::List(Box::new(ValueType::I32)))
        .unwrap();
    assert_eq!(decoded, rust_vec);

    // Convert from Rust tuple
    let rust_tuple = (42, "answer".to_string());
    let component_tuple = ComponentValue::from_tuple((
        ComponentValue::from(rust_tuple.0),
        ComponentValue::from(rust_tuple.1),
    ));

    let tuple_type = ValueType::Tuple(vec![ValueType::I32, ValueType::String]);
    let encoded = abi.encode(&component_tuple).unwrap();
    let decoded = abi.decode_tuple::<(i32, String)>(&encoded, tuple_type).unwrap();
    assert_eq!(decoded, rust_tuple);
}

/// Helper functions for ComponentValue conversions
impl ComponentValue {
    fn from_vec<T>(vec: Vec<T>) -> Self
    where
        T: Into<ComponentValue> + Clone,
    {
        ComponentValue::List(vec.iter().map(|v| v.clone().into()).collect())
    }

    fn from_tuple<T1, T2>(tuple: (T1, T2)) -> Self
    where
        T1: Into<ComponentValue>,
        T2: Into<ComponentValue>,
    {
        ComponentValue::Tuple(vec![tuple.0.into(), tuple.1.into()])
    }
}

impl From<i32> for ComponentValue {
    fn from(value: i32) -> Self {
        ComponentValue::I32(value)
    }
}

impl From<String> for ComponentValue {
    fn from(value: String) -> Self {
        ComponentValue::String(value)
    }
}

impl From<&str> for ComponentValue {
    fn from(value: &str) -> Self {
        ComponentValue::String(value.to_string())
    }
}

impl From<bool> for ComponentValue {
    fn from(value: bool) -> Self {
        ComponentValue::Bool(value)
    }
}

/// Extend CanonicalABI with additional helper methods for testing
impl CanonicalABI {
    fn decode_vec<T>(&self, bytes: &[u8], ty: ValueType) -> Result<Vec<T>, Error> {
        // This would be implemented in the real code
        let component_value = self.decode::<ComponentValue>(bytes, ty)?;
        if let ComponentValue::List(items) = component_value {
            // Convert items to T (simplified for testing)
            Ok(vec![])
        } else {
            Err(Error::runtime_execution_error("Error occurred: Not a list"))
        }
    }

    fn decode_tuple<T>(&self, bytes: &[u8], ty: ValueType) -> Result<T, Error> {
        // This would be implemented in the real code
        let component_value = self.decode::<ComponentValue>(bytes, ty)?;
        if let ComponentValue::Tuple(_) = component_value {
            // Convert to T (simplified for testing)
            Err(Error::runtime_execution_error(
                "Error occurred: Not implemented in tests",
            ))
        } else {
            Err(Error::runtime_execution_error(
                "Error occurred: Not a tuple",
            ))
        }
    }
}

/// Tests for type conversion bidirectional functions
#[test]
fn test_type_conversion_bidirectional() {
    use wrt_component::type_conversion::bidirectional::*;
    use wrt_format::component::{
        ExternType as FormatExternType, ResourceRepresentation, ValType as FormatValType,
    };
    use wrt_foundation::{
        ExternType as TypesExternType,
        component::ResourceType,
        component_value::ValType as TypesValType,
        types::{FuncType as TypesFuncType, ValueType},
    };

    // Test conversion from core ValueType to format ValType
    assert_eq!(
        value_type_to_format_val_type(&ValueType::I32).unwrap(),
        FormatValType::S32
    );
    assert_eq!(
        value_type_to_format_val_type(&ValueType::I64).unwrap(),
        FormatValType::S64
    );
    assert_eq!(
        value_type_to_format_val_type(&ValueType::F32).unwrap(),
        FormatValType::F32
    );
    assert_eq!(
        value_type_to_format_val_type(&ValueType::F64).unwrap(),
        FormatValType::F64
    );

    // Test reference types that should return errors
    assert!(value_type_to_format_val_type(&ValueType::FuncRef).is_err());
    assert!(value_type_to_format_val_type(&ValueType::ExternRef).is_err());

    // Test conversion from format ValType to core ValueType
    assert_eq!(
        format_val_type_to_value_type(&FormatValType::S32).unwrap(),
        ValueType::I32
    );
    assert_eq!(
        format_val_type_to_value_type(&FormatValType::S64).unwrap(),
        ValueType::I64
    );
    assert_eq!(
        format_val_type_to_value_type(&FormatValType::F32).unwrap(),
        ValueType::F32
    );
    assert_eq!(
        format_val_type_to_value_type(&FormatValType::F64).unwrap(),
        ValueType::F64
    );

    // Test complex types that should return errors
    assert!(format_val_type_to_value_type(&FormatValType::String).is_err());
    assert!(format_val_type_to_value_type(&FormatValType::Bool).is_err());
    assert!(format_val_type_to_value_type(&FormatValType::Char).is_err());

    // Test conversion from core ValueType to TypesValType
    assert_eq!(
        value_type_to_types_valtype(&ValueType::I32),
        TypesValType::S32
    );
    assert_eq!(
        value_type_to_types_valtype(&ValueType::I64),
        TypesValType::S64
    );
    assert_eq!(
        value_type_to_types_valtype(&ValueType::F32),
        TypesValType::F32
    );
    assert_eq!(
        value_type_to_types_valtype(&ValueType::F64),
        TypesValType::F64
    );
    assert_eq!(
        value_type_to_types_valtype(&ValueType::FuncRef),
        TypesValType::Own(0)
    );
    assert_eq!(
        value_type_to_types_valtype(&ValueType::ExternRef),
        TypesValType::Ref(0)
    );

    // Test conversion from format ValType to TypesValType - primitive types
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::S32),
        TypesValType::S32
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::U32),
        TypesValType::U32
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::S64),
        TypesValType::S64
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::U64),
        TypesValType::U64
    );

    // Test round-trip conversion of simple types
    let test_types = vec![
        FormatValType::S32,
        FormatValType::S64,
        FormatValType::F32,
        FormatValType::F64,
        FormatValType::Bool,
        FormatValType::String,
    ];

    for original_type in test_types {
        // Convert to runtime type
        let runtime_type = format_valtype_to_types_valtype(&original_type);

        // Convert back to format type
        let roundtrip_type = types_valtype_to_format_valtype(&runtime_type);

        // For these simple types, the conversion should be lossless
        assert_eq!(original_type, roundtrip_type);
    }

    // Test conversion utility functions
    assert_eq!(
        common_to_format_val_type(&ValueType::I32).unwrap(),
        FormatValType::S32
    );

    assert_eq!(
        format_to_common_val_type(&FormatValType::S32).unwrap(),
        ValueType::I32
    );

    // Test error handling in conversion functions
    let unsupported_types = vec![
        FormatValType::ResultBoth(
            Box::new(FormatValType::S32),
            Box::new(FormatValType::String),
        ),
        FormatValType::ErrorContext,
    ];

    for val_type in unsupported_types {
        let result = format_val_type_to_value_type(&val_type);
        assert!(result.is_err());
    }

    // Test error when converting reference types to format types
    let ref_types = vec![ValueType::FuncRef, ValueType::ExternRef];

    for val_type in ref_types {
        let result = value_type_to_format_val_type(&val_type);
        assert!(result.is_err());
    }
}

/// Tests for more complex type conversion scenarios
#[test]
fn test_complex_type_conversions() {
    use wrt_component::type_conversion::bidirectional::*;
    use wrt_format::component::{
        ExternType as FormatExternType, ResourceRepresentation, ValType as FormatValType,
    };
    use wrt_foundation::{
        ExternType as TypesExternType,
        component::ResourceType,
        component_value::ValType as TypesValType,
        types::{FuncType as TypesFuncType, ValueType},
    };

    // Test compound types
    let list_type = FormatValType::List(Box::new(FormatValType::S32));
    let expected_list_type = TypesValType::List(Box::new(TypesValType::S32));
    assert_eq!(
        format_valtype_to_types_valtype(&list_type),
        expected_list_type
    );

    let option_type = FormatValType::Option(Box::new(FormatValType::S32));
    let expected_option_type = TypesValType::Option(Box::new(TypesValType::S32));
    assert_eq!(
        format_valtype_to_types_valtype(&option_type),
        expected_option_type
    );

    // Test record type
    let record_fields = vec![
        ("field1".to_string(), FormatValType::S32),
        ("field2".to_string(), FormatValType::String),
    ];
    let expected_fields = vec![
        ("field1".to_string(), TypesValType::S32),
        ("field2".to_string(), TypesValType::String),
    ];
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::Record(record_fields)),
        TypesValType::Record(expected_fields)
    );

    // Test variant type
    let variant_cases = vec![
        ("case1".to_string(), Some(FormatValType::S32)),
        ("case2".to_string(), Some(FormatValType::String)),
        ("case3".to_string(), None),
    ];
    let expected_cases = vec![
        ("case1".to_string(), Some(TypesValType::S32)),
        ("case2".to_string(), Some(TypesValType::String)),
        ("case3".to_string(), None),
    ];
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::Variant(variant_cases)),
        TypesValType::Variant(expected_cases)
    );

    // Test resource handles
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::Own(5)),
        TypesValType::Own(5)
    );
    assert_eq!(
        format_valtype_to_types_valtype(&FormatValType::Borrow(5)),
        TypesValType::Borrow(5)
    );
}

/// Tests for extern type conversion
#[test]
fn test_extern_type_conversion() {
    use wrt_component::type_conversion::bidirectional::*;
    use wrt_format::component::{
        ExternType as FormatExternType, ResourceRepresentation, ValType as FormatValType,
    };
    use wrt_foundation::{
        ExternType as TypesExternType,
        component::ResourceType,
        component_value::ValType as TypesValType,
        types::{FuncType as TypesFuncType, ValueType},
    };

    // Test function type conversion
    let format_func = FormatExternType::Function {
        params: vec![("arg".to_string(), FormatValType::S32)],
        results: vec![FormatValType::S32],
    };

    let runtime_func = format_to_runtime_extern_type(&format_func).unwrap();

    match runtime_func {
        TypesExternType::Function(func_type) => {
            assert_eq!(func_type.params.len(), 1);
            assert_eq!(func_type.results.len(), 1);
        },
        _ => panic!("Expected Function type, got {:?}", runtime_func),
    }

    // Test roundtrip conversion from runtime extern type to format extern type and
    // back Create a function type
    let params = vec![ValueType::I32, ValueType::I64];
    let results = vec![ValueType::F32];
    let func_type = TypesFuncType::new(params, results);
    let extern_type = TypesExternType::Function(func_type);

    // Convert to format type
    let format_type = runtime_to_format_extern_type(&extern_type).unwrap();

    // Convert back to runtime type
    let roundtrip_type = format_to_runtime_extern_type(&format_type).unwrap();

    // Verify the structure is preserved
    match roundtrip_type {
        TypesExternType::Function(func_type) => {
            assert_eq!(func_type.params.len(), 2);
            assert_eq!(func_type.results.len(), 1);
            assert_eq!(func_type.params[0], ValueType::I32);
            assert_eq!(func_type.params[1], ValueType::I64);
            assert_eq!(func_type.results[0], ValueType::F32);
        },
        _ => panic!("Expected Function type after roundtrip"),
    }

    // Test resource type conversion
    let resource_extern_type = FormatExternType::Resource {
        rep: ResourceRepresentation::Handle32,
        nullable: false,
    };

    let runtime_resource = format_to_runtime_extern_type(&resource_extern_type).unwrap();

    match runtime_resource {
        TypesExternType::Resource(resource_type) => {
            match resource_type {
                ResourceType::Indexed(repr, _) => {
                    assert_eq!(repr, 0); // Representation should be mapped to
                    // index
                },
                _ => panic!("Expected indexed resource type"),
            }
        },
        _ => panic!("Expected Resource type, got {:?}", runtime_resource),
    }
}

/// Tests for trait-based conversion
#[test]
fn test_conversion_traits() {
    use wrt_component::type_conversion::bidirectional::*;
    use wrt_format::component::{ExternType as FormatExternType, ValType as FormatValType};
    use wrt_foundation::{
        ExternType as TypesExternType,
        types::{FuncType as TypesFuncType, ValueType},
    };

    // Test IntoRuntimeType for FormatExternType
    let format_func = FormatExternType::Function {
        params: vec![("arg".to_string(), FormatValType::S32)],
        results: vec![FormatValType::S32],
    };

    let runtime_func: Result<TypesExternType, _> = format_func.clone().into_runtime_type();
    assert!(runtime_func.is_ok());

    // Test IntoFormatType for TypesExternType
    let func_type = TypesFuncType::new(vec![ValueType::I32], vec![ValueType::F32]);
    let extern_type = TypesExternType::Function(func_type);

    let format_type: Result<FormatExternType, _> = extern_type.into_format_type();
    assert!(format_type.is_ok());
}
