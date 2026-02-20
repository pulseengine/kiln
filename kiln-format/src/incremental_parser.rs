//! Incremental WIT parser for efficient re-parsing
//!
//! This module provides incremental parsing capabilities for WIT files,
//! enabling efficient re-parsing when source files are modified.

#[cfg(feature = "std")]
use std::{collections::BTreeMap, vec::Vec};
#[cfg(not(feature = "std"))]
use std::{collections::BTreeMap, vec::Vec};

use wrt_error::{Error, Result};
use wrt_foundation::{BoundedString, NoStdProvider, prelude::*};

use crate::ast::*;

/// Change type for incremental parsing
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    /// Text was inserted at a position
    Insert { offset: u32, length: u32 },
    /// Text was deleted from a position
    Delete { offset: u32, length: u32 },
    /// Text was replaced at a position
    Replace {
        offset: u32,
        old_length: u32,
        new_length: u32,
    },
}

/// A change to a source file
#[derive(Debug, Clone)]
pub struct SourceChange {
    /// Type of change
    pub change_type: ChangeType,
    /// New text (for insert/replace)
    pub text: Option<BoundedString<1024>>,
}

/// Parse tree node for incremental parsing
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct ParseNode {
    /// AST node at this position
    pub node: ParseNodeKind,
    /// Source span of this node
    pub span: SourceSpan,
    /// Child nodes
    pub children: Vec<ParseNode>,
    /// Whether this node needs re-parsing
    pub dirty: bool,
}

/// Kind of parse node
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub enum ParseNodeKind {
    /// Document root
    Document,
    /// Package declaration
    Package,
    /// Use item
    UseItem,
    /// Interface declaration
    Interface,
    /// World declaration
    World,
    /// Type declaration
    TypeDecl,
    /// Function declaration
    Function,
    /// Resource declaration
    Resource,
    /// Other top-level item
    Other,
}

/// Incremental parser state
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct IncrementalParser {
    /// Current parse tree
    parse_tree: Option<ParseNode>,

    /// Source content
    source: Vec<BoundedString<1024>>,

    /// Total source length
    total_length: u32,

    /// Cached AST
    cached_ast: Option<WitDocument>,

    /// Parse statistics
    stats: ParseStats,
}

/// Statistics for incremental parsing
#[derive(Debug, Default, Clone)]
pub struct ParseStats {
    /// Total parses performed
    pub total_parses: u32,
    /// Incremental parses performed
    pub incremental_parses: u32,
    /// Full re-parses performed
    pub full_reparses: u32,
    /// Nodes reused
    pub nodes_reused: u32,
    /// Nodes re-parsed
    pub nodes_reparsed: u32,
}

#[cfg(feature = "std")]
impl IncrementalParser {
    /// Create a new incremental parser
    pub fn new() -> Self {
        Self {
            parse_tree: None,
            source: Vec::new(),
            total_length: 0,
            cached_ast: None,
            stats: ParseStats::default(),
        }
    }

    /// Set initial source content
    pub fn set_source(&mut self, content: &str) -> Result<()> {
        self.source.clear();
        self.total_length = 0;

        let _provider = wrt_foundation::safe_managed_alloc!(
            1024,
            wrt_foundation::budget_aware_provider::CrateId::Format
        )?;

        for line in content.lines() {
            let bounded_line = BoundedString::try_from_str(line)
                .map_err(|_| Error::parse_error("Line too long"))?;
            self.source.push(bounded_line);
            self.total_length += line.len() as u32 + 1; // +1 for newline
        }

        // Perform initial full parse
        self.full_parse()?;

        Ok(())
    }

    /// Apply a source change
    pub fn apply_change(&mut self, change: SourceChange) -> Result<()> {
        match change.change_type {
            ChangeType::Insert { offset, length: _ } => {
                self.apply_insert(
                    offset,
                    change
                        .text
                        .as_ref()
                        .ok_or_else(|| Error::parse_error("Insert change requires text"))?,
                )?;
            },
            ChangeType::Delete { offset, length } => {
                self.apply_delete(offset, length)?;
            },
            ChangeType::Replace {
                offset,
                old_length,
                new_length: _,
            } => {
                self.apply_replace(
                    offset,
                    old_length,
                    change
                        .text
                        .as_ref()
                        .ok_or_else(|| Error::parse_error("Replace change requires text"))?,
                )?;
            },
        }

        // Mark affected nodes as dirty
        if let Some(mut tree) = self.parse_tree.take() {
            Self::mark_dirty_nodes_static(&mut tree, &change.change_type, &mut self.stats);
            self.parse_tree = Some(tree);
        }

        // Perform incremental parse
        self.incremental_parse()?;

        Ok(())
    }

    /// Get the current AST
    pub fn get_ast(&self) -> Option<&WitDocument> {
        self.cached_ast.as_ref()
    }

    /// Get parse statistics
    pub fn stats(&self) -> &ParseStats {
        &self.stats
    }

    /// Perform a full parse
    fn full_parse(&mut self) -> Result<()> {
        self.stats.total_parses += 1;
        self.stats.full_reparses += 1;

        // Build source string
        let mut full_source = String::new();
        for line in &self.source {
            if let Ok(line_str) = line.as_str() {
                full_source.push_str(line_str);
                full_source.push('\n');
            }
        }

        // Parse using enhanced parser (when fixed) or simple parser
        // For now, create a stub AST
        let _provider = wrt_foundation::safe_managed_alloc!(
            1024,
            wrt_foundation::budget_aware_provider::CrateId::Format
        )?;
        let doc = WitDocument {
            package: None,
            #[cfg(feature = "std")]
            use_items: Vec::new(),
            #[cfg(feature = "std")]
            items: Vec::new(),
            span: SourceSpan::new(0, self.total_length, 0),
        };

        // Build parse tree
        let tree = self.build_parse_tree(&doc)?;

        self.cached_ast = Some(doc);
        self.parse_tree = Some(tree);

        Ok(())
    }

    /// Perform incremental parse on dirty nodes
    fn incremental_parse(&mut self) -> Result<()> {
        self.stats.total_parses += 1;
        self.stats.incremental_parses += 1;

        if let Some(mut tree) = self.parse_tree.take() {
            Self::reparse_dirty_nodes_static(&mut tree)?;
            self.parse_tree = Some(tree);
        }

        Ok(())
    }

    /// Apply an insert change
    fn apply_insert(&mut self, offset: u32, text: &BoundedString<1024>) -> Result<()> {
        // Find the line containing this offset
        let (_line_idx, _line_offset) = self.offset_to_line_position(offset)?;

        // Insert text into the appropriate line
        // This is simplified - real implementation would handle multi-line inserts
        // Would need to implement string insertion for BoundedString
        // For now, just mark as needing full reparse

        self.total_length += text.as_str().map(|s| s.len() as u32).unwrap_or(0);

        Ok(())
    }

    /// Apply a delete change
    fn apply_delete(&mut self, offset: u32, length: u32) -> Result<()> {
        // Find the line containing this offset
        let (_line_idx, _line_offset) = self.offset_to_line_position(offset)?;

        // Delete text from the appropriate line(s)
        // This is simplified - real implementation would handle multi-line deletes

        self.total_length = self.total_length.saturating_sub(length);

        Ok(())
    }

    /// Apply a replace change
    fn apply_replace(
        &mut self,
        offset: u32,
        old_length: u32,
        text: &BoundedString<1024>,
    ) -> Result<()> {
        self.apply_delete(offset, old_length)?;
        self.apply_insert(offset, text)?;
        Ok(())
    }

    /// Convert offset to line and position within line
    fn offset_to_line_position(&self, offset: u32) -> Result<(usize, u32)> {
        let mut current_offset = 0u32;

        for (idx, line) in self.source.iter().enumerate() {
            let line_len = line.as_str().map(|s| s.len() as u32 + 1).unwrap_or(1);

            if current_offset + line_len > offset {
                return Ok((idx, offset - current_offset));
            }

            current_offset += line_len;
        }

        Err(Error::parse_error("Offset out of bounds"))
    }

    /// Mark nodes affected by a change as dirty (static version)
    fn mark_dirty_nodes_static(node: &mut ParseNode, change: &ChangeType, stats: &mut ParseStats) {
        let change_span = match change {
            ChangeType::Insert { offset, length } => SourceSpan::new(*offset, offset + length, 0),
            ChangeType::Delete { offset, length } => SourceSpan::new(*offset, offset + length, 0),
            ChangeType::Replace {
                offset,
                old_length: _,
                new_length,
            } => SourceSpan::new(*offset, offset + new_length, 0),
        };

        // Check if this node is affected by the change
        if node.span.overlaps(&change_span) || node.span.contains_offset(change_span.start) {
            node.dirty = true;
            stats.nodes_reparsed += 1;
        } else {
            stats.nodes_reused += 1;
        }

        // Recursively mark children
        for child in &mut node.children {
            Self::mark_dirty_nodes_static(child, change, stats);
        }
    }

    /// Build parse tree from AST
    fn build_parse_tree(&self, doc: &WitDocument) -> Result<ParseNode> {
        let mut children = Vec::new();

        // Add package node if present
        if let Some(ref pkg) = doc.package {
            children.push(ParseNode {
                node: ParseNodeKind::Package,
                span: pkg.span,
                children: Vec::new(),
                dirty: false,
            });
        }

        // Add use items
        #[cfg(feature = "std")]
        for use_item in &doc.use_items {
            children.push(ParseNode {
                node: ParseNodeKind::UseItem,
                span: use_item.span,
                children: Vec::new(),
                dirty: false,
            });
        }

        // Add top-level items
        #[cfg(feature = "std")]
        for item in &doc.items {
            let (kind, span) = match item {
                TopLevelItem::Interface(i) => (ParseNodeKind::Interface, i.span),
                TopLevelItem::World(w) => (ParseNodeKind::World, w.span),
                TopLevelItem::Type(t) => (ParseNodeKind::TypeDecl, t.span),
            };

            children.push(ParseNode {
                node: kind,
                span,
                children: Vec::new(), // Would recursively build children
                dirty: false,
            });
        }

        Ok(ParseNode {
            node: ParseNodeKind::Document,
            span: doc.span,
            children,
            dirty: false,
        })
    }

    /// Reparse dirty nodes in the tree (static version)
    fn reparse_dirty_nodes_static(node: &mut ParseNode) -> Result<()> {
        if node.dirty {
            // Re-parse this node
            // In a real implementation, this would:
            // 1. Extract the source text for this node's span
            // 2. Parse just that portion
            // 3. Update the node and its children
            // 4. Update the cached AST

            node.dirty = false;
        }

        // Recursively process children
        for child in &mut node.children {
            Self::reparse_dirty_nodes_static(child)?;
        }

        Ok(())
    }
}

#[cfg(feature = "std")]
impl Default for IncrementalParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceSpan {
    /// Check if this span overlaps with another
    pub fn overlaps(&self, other: &SourceSpan) -> bool {
        self.file_id == other.file_id && !(self.end <= other.start || other.end <= self.start)
    }

    /// Check if this span contains an offset
    pub fn contains_offset(&self, offset: u32) -> bool {
        offset >= self.start && offset < self.end
    }
}

/// Incremental parsing cache for multiple files
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct IncrementalParserCache {
    /// Parsers for each file
    parsers: BTreeMap<u32, IncrementalParser>,

    /// Global statistics
    global_stats: ParseStats,
}

#[cfg(feature = "std")]
impl IncrementalParserCache {
    /// Create a new parser cache
    pub fn new() -> Self {
        Self {
            parsers: BTreeMap::new(),
            global_stats: ParseStats::default(),
        }
    }

    /// Get or create parser for a file
    pub fn get_parser(&mut self, file_id: u32) -> &mut IncrementalParser {
        self.parsers.entry(file_id).or_insert_with(IncrementalParser::new)
    }

    /// Remove parser for a file
    pub fn remove_parser(&mut self, file_id: u32) -> Option<IncrementalParser> {
        self.parsers.remove(&file_id)
    }

    /// Get global statistics
    pub fn global_stats(&self) -> ParseStats {
        let mut stats = self.global_stats.clone();

        for parser in self.parsers.values() {
            let parser_stats = parser.stats();
            stats.total_parses += parser_stats.total_parses;
            stats.incremental_parses += parser_stats.incremental_parses;
            stats.full_reparses += parser_stats.full_reparses;
            stats.nodes_reused += parser_stats.nodes_reused;
            stats.nodes_reparsed += parser_stats.nodes_reparsed;
        }

        stats
    }
}

#[cfg(feature = "std")]
impl Default for IncrementalParserCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    #[test]
    fn test_incremental_parser_creation() {
        let parser = IncrementalParser::new();
        assert!(parser.get_ast().is_none());
        assert_eq!(parser.stats().total_parses, 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_source_change_types() {
        let insert = ChangeType::Insert {
            offset: 10,
            length: 5,
        };
        let delete = ChangeType::Delete {
            offset: 20,
            length: 3,
        };
        let replace = ChangeType::Replace {
            offset: 30,
            old_length: 4,
            new_length: 6,
        };

        match insert {
            ChangeType::Insert { offset, length } => {
                assert_eq!(offset, 10);
                assert_eq!(length, 5);
            },
            _ => panic!("Wrong change type"),
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_span_operations() {
        let span1 = SourceSpan::new(10, 20, 0);
        let span2 = SourceSpan::new(15, 25, 0);
        let span3 = SourceSpan::new(25, 30, 0);

        assert!(span1.overlaps(&span2));
        assert!(!span1.overlaps(&span3));

        assert!(span1.contains_offset(15));
        assert!(!span1.contains_offset(25));
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_parser_cache() {
        let mut cache = IncrementalParserCache::new();

        let parser1 = cache.get_parser(0);
        parser1.stats.total_parses = 5;

        let parser2 = cache.get_parser(1);
        parser2.stats.total_parses = 3;

        let stats = cache.global_stats();
        assert_eq!(stats.total_parses, 8);
    }
}
