//! Basic LSP (Language Server Protocol) infrastructure for WIT
//!
//! This module provides the foundation for WIT language server support,
//! enabling IDE features like syntax highlighting, error reporting, and more.

// LSP protocol uses specific integer types for line/column positions.
// These casts are safe within reasonable document sizes.
#![allow(clippy::cast_possible_truncation)]

#[cfg(not(feature = "std"))]
use std::{collections::BTreeMap, sync::Arc, vec::Vec};
#[cfg(feature = "std")]
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    vec::Vec,
};

use wrt_error::{Error, Result};
use wrt_foundation::{BoundedString, prelude::*};

use crate::{
    ast::*,
    incremental_parser::{ChangeType, IncrementalParserCache, SourceChange},
};

/// LSP position (line and character)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Line position (0-based)
    pub line: u32,
    /// Character position (0-based)
    pub character: u32,
}

/// LSP range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// Start position
    pub start: Position,
    /// End position
    pub end: Position,
}

/// LSP diagnostic severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

/// LSP diagnostic
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Range where the diagnostic applies
    pub range: Range,
    /// Severity of the diagnostic
    pub severity: DiagnosticSeverity,
    /// Diagnostic message
    pub message: BoundedString<512>,
    /// Optional source of the diagnostic
    pub source: Option<BoundedString<64>>,
    /// Optional diagnostic code
    pub code: Option<u32>,
}

/// Text document item
#[derive(Debug, Clone)]
pub struct TextDocumentItem {
    /// Document URI
    pub uri: BoundedString<256>,
    /// Language ID (should be "wit")
    pub language_id: BoundedString<16>,
    /// Version number
    pub version: i32,
    /// Document text
    pub text: Vec<BoundedString<1024>>,
}

/// Text document content change event
#[derive(Debug, Clone)]
pub struct TextDocumentContentChangeEvent {
    /// Range of the change
    pub range: Option<Range>,
    /// Text that is being replaced
    pub text: BoundedString<1024>,
}

/// Hover information
#[derive(Debug, Clone)]
pub struct Hover {
    /// Hover content
    pub contents: BoundedString<1024>,
    /// Optional range
    pub range: Option<Range>,
}

/// Completion item kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemKind {
    Keyword = 14,
    Function = 3,
    Interface = 7,
    Type = 22,
    Field = 5,
    EnumMember = 20,
}

/// Completion item
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// Label shown in completion list
    pub label: BoundedString<64>,
    /// Kind of completion
    pub kind: CompletionItemKind,
    /// Detail information
    pub detail: Option<BoundedString<256>>,
    /// Documentation
    pub documentation: Option<BoundedString<512>>,
    /// Text to insert
    pub insert_text: Option<BoundedString<256>>,
}

/// Location in a document
#[derive(Debug, Clone)]
pub struct Location {
    /// Document URI
    pub uri: BoundedString<256>,
    /// Range in the document
    pub range: Range,
}

/// Symbol kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Interface = 11,
    Function = 12,
    Type = 5,
    Field = 8,
    EnumMember = 22,
    Package = 4,
}

/// Document symbol
#[derive(Debug, Clone)]
pub struct DocumentSymbol {
    /// Symbol name
    pub name: BoundedString<64>,
    /// Symbol kind
    pub kind: SymbolKind,
    /// Range of the symbol
    pub range: Range,
    /// Selection range
    pub selection_range: Range,
    /// Child symbols
    #[cfg(feature = "std")]
    pub children: Vec<DocumentSymbol>,
}

/// WIT Language Server
#[cfg(feature = "std")]
pub struct WitLanguageServer {
    /// Parser cache for incremental parsing
    parser_cache: Arc<Mutex<IncrementalParserCache>>,

    /// Open documents
    documents: BTreeMap<String, TextDocumentItem>,

    /// Current diagnostics
    diagnostics: BTreeMap<String, Vec<Diagnostic>>,

    /// Server capabilities
    capabilities: ServerCapabilities,
}

/// Server capabilities
#[derive(Debug, Clone)]
pub struct ServerCapabilities {
    /// Text document sync
    pub text_document_sync: bool,
    /// Hover provider
    pub hover_provider: bool,
    /// Completion provider
    pub completion_provider: bool,
    /// Definition provider
    pub definition_provider: bool,
    /// Document symbol provider
    pub document_symbol_provider: bool,
    /// Diagnostic provider
    pub diagnostic_provider: bool,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            text_document_sync: true,
            hover_provider: true,
            completion_provider: true,
            definition_provider: true,
            document_symbol_provider: true,
            diagnostic_provider: true,
        }
    }
}

#[cfg(feature = "std")]
impl WitLanguageServer {
    /// Create a new language server
    pub fn new() -> Self {
        Self {
            parser_cache: Arc::new(Mutex::new(IncrementalParserCache::new())),
            documents: BTreeMap::new(),
            diagnostics: BTreeMap::new(),
            capabilities: ServerCapabilities::default(),
        }
    }

    /// Get server capabilities
    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    /// Open a document
    pub fn open_document(&mut self, document: TextDocumentItem) -> Result<()> {
        let uri = document
            .uri
            .as_str()
            .map_err(|_| Error::parse_error("Invalid URI"))?
            .to_string();

        // Set up parser for this document
        let file_id = self.uri_to_file_id(&uri);

        // Combine lines into full text
        let mut full_text = String::new();
        for line in &document.text {
            if let Ok(line_str) = line.as_str() {
                full_text.push_str(line_str);
                full_text.push('\n');
            }
        }

        // Parse the document
        if let Ok(mut cache) = self.parser_cache.lock() {
            let parser = cache.get_parser(file_id);
            parser.set_source(&full_text)?;
        }

        // Store document
        self.documents.insert(uri.clone(), document);

        // Run diagnostics
        self.update_diagnostics(&uri)?;

        Ok(())
    }

    /// Update document content
    pub fn update_document(
        &mut self,
        uri: &str,
        changes: Vec<TextDocumentContentChangeEvent>,
        version: i32,
    ) -> Result<()> {
        let file_id = self.uri_to_file_id(uri);

        // Apply changes to parser
        if let Ok(mut cache) = self.parser_cache.lock() {
            let parser = cache.get_parser(file_id);

            for change in changes {
                if let Some(range) = change.range {
                    // Incremental change
                    let offset = self.position_to_offset(uri, range.start)?;
                    let end_offset = self.position_to_offset(uri, range.end)?;
                    let length = end_offset - offset;

                    let source_change = SourceChange {
                        change_type: ChangeType::Replace {
                            offset,
                            old_length: length,
                            new_length: change.text.as_str().map(|s| s.len() as u32).unwrap_or(0),
                        },
                        text: Some(change.text),
                    };

                    parser.apply_change(source_change)?;
                } else {
                    // Full document change
                    parser.set_source(change.text.as_str().unwrap_or(""))?;
                }
            }
        }

        // Update document version
        if let Some(doc) = self.documents.get_mut(uri) {
            doc.version = version;
        }

        // Run diagnostics
        self.update_diagnostics(uri)?;

        Ok(())
    }

    /// Get hover information
    pub fn hover(&self, uri: &str, position: Position) -> Result<Option<Hover>> {
        let file_id = self.uri_to_file_id(uri);
        let offset = self.position_to_offset(uri, position)?;

        // Get AST from parser
        let ast = if let Ok(mut cache) = self.parser_cache.lock() {
            cache.get_parser(file_id).get_ast().cloned()
        } else {
            None
        };

        if let Some(ast) = ast {
            // Find node at position
            if let Some(node_info) = self.find_node_at_offset(&ast, offset) {
                let hover_text = match node_info {
                    NodeInfo::Function(name) => {
                        BoundedString::try_from_str(&format!("Function: {}", name)).ok()
                    },
                    NodeInfo::Type(name) => {
                        BoundedString::try_from_str(&format!("Type: {}", name)).ok()
                    },
                    NodeInfo::Interface(name) => {
                        BoundedString::try_from_str(&format!("Interface: {}", name)).ok()
                    },
                    _ => None,
                };

                if let Some(contents) = hover_text {
                    return Ok(Some(Hover {
                        contents,
                        range: None,
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Get completion items
    pub fn completion(&self, _uri: &str, _position: Position) -> Result<Vec<CompletionItem>> {
        let mut items = Vec::new();

        // Add keyword completions
        let keywords = [
            ("interface", CompletionItemKind::Keyword),
            ("world", CompletionItemKind::Keyword),
            ("package", CompletionItemKind::Keyword),
            ("use", CompletionItemKind::Keyword),
            ("type", CompletionItemKind::Keyword),
            ("record", CompletionItemKind::Keyword),
            ("variant", CompletionItemKind::Keyword),
            ("enum", CompletionItemKind::Keyword),
            ("flags", CompletionItemKind::Keyword),
            ("resource", CompletionItemKind::Keyword),
            ("func", CompletionItemKind::Keyword),
            ("import", CompletionItemKind::Keyword),
            ("export", CompletionItemKind::Keyword),
        ];

        for (keyword, kind) in keywords {
            if let Ok(label) = BoundedString::try_from_str(keyword) {
                items.push(CompletionItem {
                    label,
                    kind,
                    detail: None,
                    documentation: None,
                    insert_text: None,
                });
            }
        }

        // Add type completions
        let primitive_types = [
            "u8", "u16", "u32", "u64", "s8", "s16", "s32", "s64", "f32", "f64", "bool", "string",
            "char",
        ];

        for type_name in primitive_types {
            if let Ok(label) = BoundedString::try_from_str(type_name) {
                items.push(CompletionItem {
                    label,
                    kind: CompletionItemKind::Type,
                    detail: BoundedString::try_from_str("Primitive type").ok(),
                    documentation: None,
                    insert_text: None,
                });
            }
        }

        Ok(items)
    }

    /// Get document symbols
    pub fn document_symbols(&self, uri: &str) -> Result<Vec<DocumentSymbol>> {
        let file_id = self.uri_to_file_id(uri);
        let mut symbols = Vec::new();

        // Get AST from parser
        let ast = if let Ok(mut cache) = self.parser_cache.lock() {
            cache.get_parser(file_id).get_ast().cloned()
        } else {
            None
        };

        if let Some(ast) = ast {
            self.extract_symbols(&ast, &mut symbols)?;
        }

        Ok(symbols)
    }

    /// Update diagnostics for a document
    fn update_diagnostics(&mut self, uri: &str) -> Result<()> {
        let _file_id = self.uri_to_file_id(uri);
        let diagnostics = Vec::new();

        // Get parser errors (if any)
        // In a real implementation, the parser would provide error information

        // Store diagnostics
        self.diagnostics.insert(uri.to_string(), diagnostics);

        Ok(())
    }

    /// Convert URI to file ID
    fn uri_to_file_id(&self, uri: &str) -> u32 {
        // Simple hash of URI for file ID
        let mut hash = 0u32;
        for byte in uri.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
        }
        hash
    }

    /// Convert position to offset
    fn position_to_offset(&self, uri: &str, position: Position) -> Result<u32> {
        if let Some(doc) = self.documents.get(uri) {
            let mut offset = 0u32;

            for (line_idx, line) in doc.text.iter().enumerate() {
                if line_idx == position.line as usize {
                    return Ok(offset + position.character);
                }
                offset += line.as_str().map(|s| s.len() as u32 + 1).unwrap_or(1);
            }
        }

        Err(Error::parse_error("Position out of bounds"))
    }

    /// Find node at offset
    fn find_node_at_offset(&self, ast: &WitDocument, offset: u32) -> Option<NodeInfo> {
        // Simplified node finding - real implementation would traverse AST
        if ast.span.contains_offset(offset) { Some(NodeInfo::Document) } else { None }
    }

    /// Extract symbols from AST
    fn extract_symbols(&self, ast: &WitDocument, symbols: &mut Vec<DocumentSymbol>) -> Result<()> {
        // Extract package symbol
        if let Some(ref package) = ast.package {
            if let Ok(name) = BoundedString::try_from_str("package") {
                symbols.push(DocumentSymbol {
                    name,
                    kind: SymbolKind::Package,
                    range: self.span_to_range(package.span),
                    selection_range: self.span_to_range(package.span),
                    #[cfg(feature = "std")]
                    children: Vec::new(),
                });
            }
        }

        // Extract interface symbols
        #[cfg(feature = "std")]
        for item in &ast.items {
            match item {
                TopLevelItem::Interface(interface) => {
                    let mut children = Vec::new();

                    // Extract function symbols
                    for interface_item in &interface.items {
                        match interface_item {
                            InterfaceItem::Function(func) => {
                                children.push(DocumentSymbol {
                                    name: func.name.name.clone(),
                                    kind: SymbolKind::Function,
                                    range: self.span_to_range(func.span),
                                    selection_range: self.span_to_range(func.name.span),
                                    children: Vec::new(),
                                });
                            },
                            InterfaceItem::Type(type_decl) => {
                                children.push(DocumentSymbol {
                                    name: type_decl.name.name.clone(),
                                    kind: SymbolKind::Type,
                                    range: self.span_to_range(type_decl.span),
                                    selection_range: self.span_to_range(type_decl.name.span),
                                    children: Vec::new(),
                                });
                            },
                            InterfaceItem::Use(_use_decl) => {
                                // Skip use declarations for now
                            },
                        }
                    }

                    symbols.push(DocumentSymbol {
                        name: interface.name.name.clone(),
                        kind: SymbolKind::Interface,
                        range: self.span_to_range(interface.span),
                        selection_range: self.span_to_range(interface.name.span),
                        children,
                    });
                },
                _ => {}, // Handle other top-level items
            }
        }

        Ok(())
    }

    /// Convert SourceSpan to Range
    fn span_to_range(&self, span: SourceSpan) -> Range {
        // Simplified conversion - real implementation would use line/column mapping
        Range {
            start: Position {
                line: 0,
                character: span.start,
            },
            end: Position {
                line: 0,
                character: span.end,
            },
        }
    }
}

/// Node information for hover/navigation
enum NodeInfo {
    Document,
    Function(String),
    Type(String),
    Interface(String),
}

#[cfg(feature = "std")]
impl Default for WitLanguageServer {
    fn default() -> Self {
        Self::new()
    }
}

/// LSP request handler trait
#[cfg(feature = "std")]
pub trait LspRequestHandler {
    /// Handle hover request
    fn handle_hover(&self, uri: &str, position: Position) -> Result<Option<Hover>>;

    /// Handle completion request
    fn handle_completion(&self, uri: &str, position: Position) -> Result<Vec<CompletionItem>>;

    /// Handle document symbols request
    fn handle_document_symbols(&self, uri: &str) -> Result<Vec<DocumentSymbol>>;
}

#[cfg(feature = "std")]
impl LspRequestHandler for WitLanguageServer {
    fn handle_hover(&self, uri: &str, position: Position) -> Result<Option<Hover>> {
        self.hover(uri, position)
    }

    fn handle_completion(&self, uri: &str, position: Position) -> Result<Vec<CompletionItem>> {
        self.completion(uri, position)
    }

    fn handle_document_symbols(&self, uri: &str) -> Result<Vec<DocumentSymbol>> {
        self.document_symbols(uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_range() {
        let pos = Position {
            line: 5,
            character: 10,
        };
        let range = Range {
            start: Position {
                line: 5,
                character: 5,
            },
            end: Position {
                line: 5,
                character: 15,
            },
        };

        assert!(pos.line >= range.start.line);
        assert!(pos.character >= range.start.character);
        assert!(pos.character <= range.end.character);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_server_creation() {
        let server = WitLanguageServer::new();
        assert!(server.capabilities().text_document_sync);
        assert!(server.capabilities().hover_provider);
        assert!(server.capabilities().completion_provider);
    }

    #[test]
    fn test_diagnostic_severity() {
        assert_eq!(DiagnosticSeverity::Error as u8, 1);
        assert_eq!(DiagnosticSeverity::Warning as u8, 2);
        assert_eq!(DiagnosticSeverity::Information as u8, 3);
        assert_eq!(DiagnosticSeverity::Hint as u8, 4);
    }
}
