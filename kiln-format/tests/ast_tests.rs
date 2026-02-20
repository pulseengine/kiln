//! Basic tests for AST functionality

#[cfg(feature = "std")]
use wrt_format::ast::*;
use wrt_foundation::budget_aware_provider::CrateId;

#[cfg(feature = "std")]
#[test]
fn test_source_span() {
    let span = SourceSpan::new(10, 20, 1);
    assert_eq!(span.start, 10);
    assert_eq!(span.end, 20);
    assert_eq!(span.len(), 10);
    assert!(!span.is_empty());

    let empty = SourceSpan::empty();
    assert!(empty.is_empty());
}

#[cfg(feature = "std")]
#[test]
fn test_identifier() {
    use wrt_format::wit_parser::WitBoundedString;
    use wrt_foundation::{
        safe_managed_alloc,
        NoStdProvider,
    };

    let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
    let name = WitBoundedString::from_str("test", provider).unwrap();
    let span = SourceSpan::new(0, 4, 0);

    let ident = Identifier::new(name, span);
    assert_eq!(ident.span, span);
    assert_eq!(ident.name.as_str().unwrap(), "test");
}

#[cfg(feature = "std")]
#[test]
fn test_wit_document() {
    let doc = WitDocument::default();
    assert!(doc.package.is_none());
    assert!(doc.use_items.is_empty());
    assert!(doc.items.is_empty());
    assert_eq!(doc.span, SourceSpan::empty());
}

#[cfg(feature = "std")]
#[test]
fn test_primitive_types() {
    let bool_type = PrimitiveType {
        kind: PrimitiveKind::Bool,
        span: SourceSpan::empty(),
    };

    assert_eq!(format!("{}", bool_type.kind), "bool");

    let string_type = PrimitiveType {
        kind: PrimitiveKind::String,
        span: SourceSpan::empty(),
    };

    assert_eq!(format!("{}", string_type.kind), "string");
}

#[cfg(feature = "std")]
#[test]
fn test_type_expr() {
    let primitive = TypeExpr::Primitive(PrimitiveType {
        kind: PrimitiveKind::U32,
        span: SourceSpan::empty(),
    });

    assert_eq!(primitive.span(), SourceSpan::empty());

    // Test that we can create a named type
    use wrt_format::wit_parser::WitBoundedString;
    use wrt_foundation::{
        safe_managed_alloc,
        NoStdProvider,
    };

    let provider = safe_managed_alloc!(8192, CrateId::Format).unwrap();
    let name = WitBoundedString::from_str("MyType", provider).unwrap();
    let ident = Identifier::new(name, SourceSpan::new(0, 6, 0));

    let named = TypeExpr::Named(NamedType {
        package: None,
        name:    ident.clone(),
        span:    ident.span,
    });

    assert_eq!(named.span(), SourceSpan::new(0, 6, 0));
}
