//! Basic tests for WIT AST functionality
//!
//! These tests verify the core AST functionality without relying on
//! BoundedString creation which has current implementation issues.

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    #[cfg(feature = "std")]
    use std::vec::Vec;

    use crate::ast_simple::*;

    #[test]
    fn test_source_span_creation() {
        let span1 = SourceSpan::new(0, 10, 0);
        let span2 = SourceSpan::new(10, 20, 0);

        assert_eq!(span1.start, 0);
        assert_eq!(span1.end, 10);
        assert_eq!(span1.file_id, 0);

        assert_eq!(span2.start, 10);
        assert_eq!(span2.end, 20);
        assert_eq!(span2.file_id, 0);
    }

    #[test]
    fn test_source_span_merge() {
        let span1 = SourceSpan::new(0, 10, 0);
        let span2 = SourceSpan::new(10, 20, 0);

        let merged = span1.merge(&span2);
        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 20);
        assert_eq!(merged.file_id, 0);
    }

    #[test]
    fn test_source_span_empty() {
        let empty = SourceSpan::empty();
        assert_eq!(empty.start, 0);
        assert_eq!(empty.end, 0);
        assert_eq!(empty.file_id, 0);
    }

    #[test]
    fn test_primitive_types() {
        let span = SourceSpan::new(0, 10, 0);

        let string_type = PrimitiveType {
            kind: PrimitiveKind::String,
            span,
        };

        let u32_type = PrimitiveType {
            kind: PrimitiveKind::U32,
            span,
        };

        let bool_type = PrimitiveType {
            kind: PrimitiveKind::Bool,
            span,
        };

        assert_eq!(string_type.kind, PrimitiveKind::String);
        assert_eq!(u32_type.kind, PrimitiveKind::U32);
        assert_eq!(bool_type.kind, PrimitiveKind::Bool);
    }

    #[test]
    fn test_type_expressions() {
        let span = SourceSpan::new(0, 10, 0);

        let string_type = PrimitiveType {
            kind: PrimitiveKind::String,
            span,
        };

        let type_expr = TypeExpr::Primitive(string_type);

        // Verify we can pattern match on the type expression
        match type_expr {
            TypeExpr::Primitive(prim) => {
                assert_eq!(prim.kind, PrimitiveKind::String);
            },
            _ => panic!("Expected primitive type expression"),
        }
    }

    #[test]
    fn test_function_results() {
        let span = SourceSpan::new(0, 10, 0);

        let u32_type = PrimitiveType {
            kind: PrimitiveKind::U32,
            span,
        };

        // Test None results
        let no_results = FunctionResults::None;
        match no_results {
            FunctionResults::None => {}, // Expected
            _ => panic!("Expected None results"),
        }

        // Test Single result
        let single_result = FunctionResults::Single(TypeExpr::Primitive(u32_type));
        match single_result {
            FunctionResults::Single(TypeExpr::Primitive(prim)) => {
                assert_eq!(prim.kind, PrimitiveKind::U32);
            },
            _ => panic!("Expected single primitive result"),
        }
    }

    #[test]
    fn test_wit_document() {
        let span = SourceSpan::new(0, 100, 0);

        // Create a simple WIT document
        let document = WitDocument {
            package: None,
            #[cfg(feature = "std")]
            use_items: Vec::new(),
            #[cfg(feature = "std")]
            items: Vec::new(),
            span,
        };

        assert_eq!(document.span.start, 0);
        assert_eq!(document.span.end, 100);
        assert!(document.package.is_none());

        #[cfg(feature = "std")]
        {
            assert!(document.use_items.is_empty());
            assert!(document.items.is_empty());
        }
    }

    #[test]
    fn test_primitive_kind_all_variants() {
        // Test all primitive kinds exist and can be created
        let kinds = [
            PrimitiveKind::Bool,
            PrimitiveKind::U8,
            PrimitiveKind::U16,
            PrimitiveKind::U32,
            PrimitiveKind::U64,
            PrimitiveKind::S8,
            PrimitiveKind::S16,
            PrimitiveKind::S32,
            PrimitiveKind::S64,
            PrimitiveKind::F32,
            PrimitiveKind::F64,
            PrimitiveKind::Char,
            PrimitiveKind::String,
        ];

        // Verify each kind can be created and compared
        for &kind in &kinds {
            let span = SourceSpan::new(0, 5, 0);
            let prim_type = PrimitiveType { kind, span };
            assert_eq!(prim_type.kind, kind);
        }
    }

    #[test]
    fn test_function_definition() {
        let span = SourceSpan::new(0, 50, 0);

        // Create a simple function definition
        let function = Function {
            #[cfg(feature = "std")]
            params: Vec::new(),
            results: FunctionResults::None,
            is_async: false,
            span,
        };

        assert!(!function.is_async);
        assert_eq!(function.span.start, 0);
        assert_eq!(function.span.end, 50);

        #[cfg(feature = "std")]
        assert!(function.params.is_empty());

        match function.results {
            FunctionResults::None => {}, // Expected
            _ => panic!("Expected no results"),
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_ast_structure_without_strings() {
        // Test that we can work with the AST structure even without BoundedString
        // creation
        let interface_items: Vec<InterfaceItem> = Vec::new();
        assert!(interface_items.is_empty());

        let top_level_items: Vec<TopLevelItem> = Vec::new();
        assert!(top_level_items.is_empty());

        // Test that default implementations work
        let function_results = FunctionResults::default();
        match function_results {
            FunctionResults::None => {}, // Expected default
            _ => panic!("Expected None as default for FunctionResults"),
        }
    }
}
