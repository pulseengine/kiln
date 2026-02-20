/// Integration tests for the TypeConversionRegistry
///
/// These tests verify that the TypeConversionRegistry implementation
/// works correctly both with simple types and with more complex
/// domain-specific conversions.
use wrt_component::type_conversion::{
    ConversionError, ConversionErrorKind, TypeConversionRegistry,
};

// Simple test types for basic validation
#[derive(Debug, PartialEq)]
struct SimpleSource(i32);

#[derive(Debug, PartialEq)]
struct SimpleTarget(i32);

#[derive(Debug, PartialEq)]
struct ComplexSource {
    value: i32,
    name: String,
}

#[derive(Debug, PartialEq)]
struct ComplexTarget {
    value: i32,
    description: String,
}

#[test]
fn test_basic_registry_functionality() {
    // Create a registry
    let mut registry = TypeConversionRegistry::new();

    // Register a simple conversion
    registry.register(
        |src: &SimpleSource| -> Result<SimpleTarget, ConversionError> {
            Ok(SimpleTarget(src.0 * 2))
        },
    );

    // Test the conversion
    let source = SimpleSource(21);
    let target = registry.convert::<SimpleSource, SimpleTarget>(&source).unwrap();

    assert_eq!(target, SimpleTarget(42));
}

#[test]
fn test_complex_conversion() {
    let mut registry = TypeConversionRegistry::new();

    // Register a more complex conversion
    registry.register(
        |src: &ComplexSource| -> Result<ComplexTarget, ConversionError> {
            Ok(ComplexTarget {
                value: src.value,
                description: format!("Converted from: {}", src.name),
            })
        },
    );

    // Test the conversion
    let source = ComplexSource {
        value: 42,
        name: "Test Source".to_string(),
    };

    let target = registry.convert::<ComplexSource, ComplexTarget>(&source).unwrap();

    assert_eq!(
        target,
        ComplexTarget {
            value: 42,
            description: "Converted from: Test Source".to_string(),
        }
    );
}

#[test]
fn test_conversion_error_handling() {
    let mut registry = TypeConversionRegistry::new();

    // Register a conversion that can fail
    registry.register(
        |src: &SimpleSource| -> Result<SimpleTarget, ConversionError> {
            if src.0 < 0 {
                return Err(ConversionError {
                    kind: ConversionErrorKind::OutOfRange,
                    source_type: std::any::type_name::<SimpleSource>(),
                    target_type: std::any::type_name::<SimpleTarget>(),
                    context: Some("Value must be non-negative".to_string()),
                    source: None,
                });
            }
            Ok(SimpleTarget(src.0))
        },
    );

    // Test successful conversion
    let good_source = SimpleSource(42);
    let success_result = registry.convert::<SimpleSource, SimpleTarget>(&good_source);
    assert!(success_result.is_ok());
    assert_eq!(success_result.unwrap(), SimpleTarget(42));

    // Test error case
    let bad_source = SimpleSource(-1);
    let error_result = registry.convert::<SimpleSource, SimpleTarget>(&bad_source);

    assert!(error_result.is_err());
    let error = error_result.unwrap_err();
    assert!(matches!(error.kind, ConversionErrorKind::OutOfRange));
    assert!(error.context.unwrap().contains("non-negative"));
}

#[test]
fn test_bidirectional_conversions() {
    let mut registry = TypeConversionRegistry::new();

    // Register conversions in both directions
    registry.register(
        |src: &SimpleSource| -> Result<SimpleTarget, ConversionError> { Ok(SimpleTarget(src.0)) },
    );

    registry.register(
        |src: &SimpleTarget| -> Result<SimpleSource, ConversionError> { Ok(SimpleSource(src.0)) },
    );

    // Test forward conversion
    let source = SimpleSource(42);
    let target = registry.convert::<SimpleSource, SimpleTarget>(&source).unwrap();
    assert_eq!(target, SimpleTarget(42));

    // Test reverse conversion
    let source_again = registry.convert::<SimpleTarget, SimpleSource>(&target).unwrap();
    assert_eq!(source_again, SimpleSource(42));
}

#[test]
fn test_missing_conversion() {
    let registry = TypeConversionRegistry::new();

    // Try a conversion that doesn't exist
    let source = SimpleSource(42);
    let result = registry.convert::<SimpleSource, SimpleTarget>(&source);

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(matches!(error.kind, ConversionErrorKind::NoConverterFound));
}

#[test]
fn test_chained_conversion_errors() {
    let mut registry = TypeConversionRegistry::new();

    // Create a nested error
    let nested_error = ConversionError {
        kind: ConversionErrorKind::InvalidVariant,
        source_type: "InnerType",
        target_type: "OuterType",
        context: Some("Inner conversion failed".to_string()),
        source: None,
    };

    // Register a conversion that returns a chained error
    registry.register(
        |_src: &SimpleSource| -> Result<SimpleTarget, ConversionError> {
            Err(ConversionError {
                kind: ConversionErrorKind::ConversionFailed,
                source_type: std::any::type_name::<SimpleSource>(),
                target_type: std::any::type_name::<SimpleTarget>(),
                context: Some("Outer conversion failed".to_string()),
                source: Some(Box::new(nested_error)),
            })
        },
    );

    // Test the error chaining
    let source = SimpleSource(42);
    let result = registry.convert::<SimpleSource, SimpleTarget>(&source);

    assert!(result.is_err());
    let error = result.unwrap_err();

    // Check outer error
    assert!(matches!(error.kind, ConversionErrorKind::ConversionFailed));
    assert!(error.context.unwrap().contains("Outer conversion failed"));

    // Check inner error
    let inner_error = error.source.unwrap();
    assert!(matches!(
        inner_error.kind,
        ConversionErrorKind::InvalidVariant
    ));
    assert!(inner_error.context.unwrap().contains("Inner conversion failed"));
}

#[test]
fn test_can_convert_check() {
    let mut registry = TypeConversionRegistry::new();

    // Initially no conversions are registered
    assert!(!registry.can_convert::<SimpleSource, SimpleTarget>());
    assert!(!registry.can_convert::<SimpleTarget, SimpleSource>());

    // Register one conversion
    registry.register(
        |src: &SimpleSource| -> Result<SimpleTarget, ConversionError> { Ok(SimpleTarget(src.0)) },
    );

    // Now one direction should work but not the other
    assert!(registry.can_convert::<SimpleSource, SimpleTarget>());
    assert!(!registry.can_convert::<SimpleTarget, SimpleSource>());
}
