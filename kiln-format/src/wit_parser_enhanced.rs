//! Enhanced WIT parser with full AST support
//!
//! This module provides a comprehensive WIT parser that generates proper AST nodes
//! with source location tracking, supporting the full WIT grammar specification.
//! 
//! This parser requires allocation support and is only available with std or alloc features.

#[cfg(feature = "std")]
use std::{collections::BTreeMap, vec::Vec, boxed::Box, format, vec, string::String};
#[cfg(all(not(feature = "std")))]
use std::{collections::BTreeMap, vec::Vec, boxed::Box, format, vec, string::String};

#[cfg(not(any(feature = "std", )))]
compile_error!("Enhanced WIT parser requires std or alloc feature";

use core::fmt;

use wrt_foundation::{
    BoundedVec, BoundedString,
    bounded::MAX_GENERATIVE_TYPES,
    NoStdProvider,
};

use wrt_error::Error;

use crate::ast_simple::*;
use crate::wit_parser::{WitBoundedString, WitBoundedStringSmall, WitParseError};

/// Token types for lexical analysis
#[derive(Debug, Clone, PartialEq)]
enum Token {
    // Keywords
    Package,
    Use,
    Type,
    Record,
    Variant,
    Enum,
    Flags,
    Resource,
    Func,
    Constructor,
    Static,
    Method,
    Interface,
    World,
    Import,
    Export,
    Include,
    With,
    As,
    From,
    
    // Identifiers and literals
    Identifier(String),
    Version(String),
    StringLiteral(String),
    
    // Punctuation
    Colon,
    Semicolon,
    Comma,
    Dot,
    Arrow,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftAngle,
    RightAngle,
    Slash,
    At,
    Equals,
    
    // Special
    Eof,
    Newline,
    Comment(String),
}

/// Lexer for tokenizing WIT source code
struct Lexer {
    input: Vec<char>,
    position: usize,
    current_char: Option<char>,
    line: u32,
    column: u32,
    file_id: u32,
}

impl Lexer {
    fn new(input: &str, file_id: u32) -> Self {
        let chars: Vec<char> = input.chars().collect();
        let current_char = chars.get(0).copied);
        Self {
            input: chars,
            position: 0,
            current_char,
            line: 1,
            column: 1,
            file_id,
        }
    }
    
    fn current_position(&self) -> u32 {
        self.position as u32
    }
    
    fn advance(&mut self) {
        if self.current_char == Some('\n') {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        
        self.position += 1;
        self.current_char = self.input.get(self.position).copied);
    }
    
    fn peek(&self, offset: usize) -> Option<char> {
        self.input.get(self.position + offset).copied()
    }
    
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char {
            if ch.is_whitespace() && ch != '\n' {
                self.advance);
            } else {
                break;
            }
        }
    }
    
    fn read_identifier(&mut self) -> String {
        let mut result = String::new();
        
        while let Some(ch) = self.current_char {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                result.push(ch);
                self.advance);
            } else {
                break;
            }
        }
        
        result
    }
    
    fn read_version(&mut self) -> String {
        let mut result = String::new();
        
        while let Some(ch) = self.current_char {
            if ch.is_numeric() || ch == '.' || ch == '-' || ch.is_alphanumeric() {
                result.push(ch);
                self.advance);
            } else {
                break;
            }
        }
        
        result
    }
    
    fn read_string_literal(&mut self) -> Result<String, WitParseError> {
        let mut result = String::new();
        self.advance(); // Skip opening quote
        
        while let Some(ch) = self.current_char {
            if ch == '"' {
                self.advance(); // Skip closing quote
                return Ok(result;
            } else if ch == '\\' {
                self.advance);
                match self.current_char {
                    Some('n') => result.push('\n'),
                    Some('r') => result.push('\r'),
                    Some('t') => result.push('\t'),
                    Some('\\') => result.push('\\'),
                    Some('"') => result.push('"'),
                    _ => return Err(WitParseError::InvalidSyntax(
                        WitBoundedString::try_from_str("Invalid escape sequence", NoStdProvider::default()).unwrap()
                    )),
                }
                self.advance);
            } else {
                result.push(ch);
                self.advance);
            }
        }
        
        Err(WitParseError::InvalidSyntax(
            WitBoundedString::try_from_str("Unterminated string literal", NoStdProvider::default()).unwrap()
        ))
    }
    
    fn read_comment(&mut self) -> String {
        let mut result = String::new();
        
        // Skip the // or ///
        self.advance);
        self.advance);
        if self.current_char == Some('/') {
            self.advance);
        }
        
        // Skip leading space
        if self.current_char == Some(' ') {
            self.advance);
        }
        
        while let Some(ch) = self.current_char {
            if ch == '\n' {
                break;
            }
            result.push(ch);
            self.advance);
        }
        
        result
    }
    
    fn next_token(&mut self) -> Result<Token, WitParseError> {
        self.skip_whitespace);
        
        match self.current_char {
            None => Ok(Token::Eof),
            Some('\n') => {
                self.advance);
                Ok(Token::Newline)
            }
            Some('/') if self.peek(1) == Some('/') => {
                let comment = self.read_comment);
                Ok(Token::Comment(comment))
            }
            Some(':') => {
                self.advance);
                Ok(Token::Colon)
            }
            Some(');') => {
                self.advance);
                Ok(Token::Semicolon)
            }
            Some(',') => {
                self.advance);
                Ok(Token::Comma)
            }
            Some('.') => {
                self.advance);
                Ok(Token::Dot)
            }
            Some('(') => {
                self.advance);
                Ok(Token::LeftParen)
            }
            Some(')') => {
                self.advance);
                Ok(Token::RightParen)
            }
            Some('{') => {
                self.advance);
                Ok(Token::LeftBrace)
            }
            Some('}') => {
                self.advance);
                Ok(Token::RightBrace)
            }
            Some('<') => {
                self.advance);
                Ok(Token::LeftAngle)
            }
            Some('>') => {
                self.advance);
                Ok(Token::RightAngle)
            }
            Some('@') => {
                self.advance);
                Ok(Token::At)
            }
            Some('=') => {
                self.advance);
                Ok(Token::Equals)
            }
            Some('/') => {
                self.advance);
                Ok(Token::Slash)
            }
            Some('-') if self.peek(1) == Some('>') => {
                self.advance);
                self.advance);
                Ok(Token::Arrow)
            }
            Some('"') => {
                let s = self.read_string_literal()?;
                Ok(Token::StringLiteral(s))
            }
            Some(ch) if ch.is_alphabetic() || ch == '_' => {
                let ident = self.read_identifier);
                
                let token = match ident.as_str() {
                    "package" => Token::Package,
                    "use" => Token::Use,
                    "type" => Token::Type,
                    "record" => Token::Record,
                    "variant" => Token::Variant,
                    "enum" => Token::Enum,
                    "flags" => Token::Flags,
                    "resource" => Token::Resource,
                    "func" => Token::Func,
                    "constructor" => Token::Constructor,
                    "static" => Token::Static,
                    "method" => Token::Method,
                    "interface" => Token::Interface,
                    "world" => Token::World,
                    "import" => Token::Import,
                    "export" => Token::Export,
                    "include" => Token::Include,
                    "with" => Token::With,
                    "as" => Token::As,
                    "from" => Token::From,
                    _ => Token::Identifier(ident),
                };
                
                Ok(token)
            }
            Some(ch) if ch.is_numeric() => {
                let version = self.read_version);
                Ok(Token::Version(version))
            }
            Some(ch) => {
                Err(WitParseError::InvalidSyntax(
                    WitBoundedString::try_from_str(&format!("Unexpected character: {}", ch), NoStdProvider::default()).unwrap()
                ))
            }
        }
    }
}

/// Enhanced WIT parser with full AST generation
pub struct EnhancedWitParser {
    lexer: Lexer,
    current_token: Token,
    peek_token: Option<Token>,
    provider: NoStdProvider<1024>,
    documentation_buffer: Vec<String>,
}

impl EnhancedWitParser {
    /// Create a new enhanced WIT parser
    pub fn new() -> Self {
        Self {
            lexer: Lexer::new("", 0),
            current_token: Token::Eof,
            peek_token: None,
            provider: NoStdProvider::default(),
            documentation_buffer: Vec::new(),
        }
    }
    
    /// Parse a complete WIT document
    pub fn parse_document(&mut self, source: &str, file_id: u32) -> Result<WitDocument, WitParseError> {
        self.lexer = Lexer::new(source, file_id;
        self.advance()?;
        
        let start = self.lexer.current_position);
        
        let mut package = None;
        let mut use_items = Vec::new();
        let mut items = Vec::new();
        
        // Collect any leading documentation
        self.collect_documentation);
        
        // Parse package declaration if present
        if matches!(self.current_token, Token::Package) {
            package = Some(self.parse_package_decl()?;
        }
        
        // Parse top-level items
        while !matches!(self.current_token, Token::Eof) {
            self.collect_documentation);
            
            match &self.current_token {
                Token::Use => {
                    let use_decl = self.parse_use_decl()?;
                    use_items.push(use_decl);
                }
                Token::Type => {
                    let type_decl = self.parse_type_decl()?;
                    items.push(TopLevelItem::Type(type_decl);
                }
                Token::Interface => {
                    let interface = self.parse_interface_decl()?;
                    items.push(TopLevelItem::Interface(interface))
                        
                }
                Token::World => {
                    let world = self.parse_world_decl()?;
                    items.push(TopLevelItem::World(world))
                        
                }
                Token::Newline | Token::Comment(_) => {
                    self.advance()?;
                }
                _ => {
                    return Err(WitParseError::InvalidSyntax(
                        WitBoundedString::try_from_str("Expected top-level declaration", self.provider.clone()).unwrap()
                    ;
                }
            }
        }
        
        let end = self.lexer.current_position);
        let span = SourceSpan::new(start, end, file_id;
        
        Ok(WitDocument {
            package,
            use_items,
            items,
            span,
        })
    }
    
    fn advance(&mut self) -> Result<(), WitParseError> {
        if let Some(peek) = self.peek_token.take() {
            self.current_token = peek;
        } else {
            self.current_token = self.lexer.next_token()?;
        }
        Ok(())
    }
    
    fn peek(&mut self) -> Result<&Token, WitParseError> {
        if self.peek_token.is_none() {
            self.peek_token = Some(self.lexer.next_token()?;
        }
        Ok(self.peek_token.as_ref().unwrap())
    }
    
    fn expect(&mut self, expected: Token) -> Result<(), WitParseError> {
        if self.current_token == expected {
            self.advance()?;
            Ok(())
        } else {
            Err(WitParseError::InvalidSyntax(
                WitBoundedString::try_from_str(&format!("Expected {:?}, found {:?}", expected, self.current_token), self.provider.clone()).unwrap()
            ))
        }
    }
    
    fn collect_documentation(&mut self) {
        self.documentation_buffer.clear);
        
        while let Token::Comment(text) = &self.current_token {
            if text.starts_with('/') {
                // Doc comment
                self.documentation_buffer.push(text.clone();
            }
            self.advance().unwrap_or();
        }
    }
    
    fn take_documentation(&mut self) -> Option<Documentation> {
        if self.documentation_buffer.is_empty() {
            None
        } else {
            // Note: In a real implementation, we'd convert the strings to bounded strings
            Some(Documentation {
                #[cfg(feature = "std")]
                lines: Vec::new(), // For now, just return empty vec
                span: SourceSpan::empty(),
            };
        }
    }
    
    fn parse_identifier(&mut self) -> Result<Identifier, WitParseError> {
        let start = self.lexer.current_position);
        
        if let Token::Identifier(name) = &self.current_token {
            let name_str = name.clone();
            self.advance()?;
            let end = self.lexer.current_position);
            
            Ok(Identifier {
                name: WitBoundedString::try_from_str(&name_str, self.provider.clone()).unwrap(),
                span: SourceSpan::new(start, end, self.lexer.file_id),
            };
        } else {
            Err(WitParseError::InvalidSyntax(
                WitBoundedString::try_from_str("Expected identifier", self.provider.clone()).unwrap()
            ))
        }
    }
    
    fn parse_package_decl(&mut self) -> Result<PackageDecl, WitParseError> {
        let start = self.lexer.current_position);
        
        self.expect(Token::Package)?);
        
        let namespace = self.parse_identifier()?;
        self.expect(Token::Colon)?);
        let name = self.parse_identifier()?;
        
        let mut version = None;
        if matches!(self.current_token, Token::At) {
            self.advance()?;
            version = Some(self.parse_version()?;
        }
        
        let end = self.lexer.current_position);
        
        Ok(PackageDecl {
            namespace,
            name,
            version,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_version(&mut self) -> Result<Version, WitParseError> {
        let start = self.lexer.current_position);
        
        if let Token::Version(v) = &self.current_token {
            let parts: Vec<&str> = v.split('.').collect();
            if parts.len() < 3 {
                return Err(WitParseError::InvalidSyntax(
                    WitBoundedString::try_from_str("Invalid version format", self.provider.clone()).unwrap()
                ;
            }
            
            let major = parts[0].parse().map_err(|_| WitParseError::InvalidSyntax(
                WitBoundedString::try_from_str("Invalid major version", self.provider.clone()).unwrap()
            ))?;
            let minor = parts[1].parse().map_err(|_| WitParseError::InvalidSyntax(
                WitBoundedString::try_from_str("Invalid minor version", self.provider.clone()).unwrap()
            ))?;
            
            let (patch_str, pre) = if let Some(dash_pos) = parts[2].find('-') {
                let (patch, pre) = parts[2].split_at(dash_pos;
                (patch, Some(WitBoundedStringSmall::from_str(&pre[1..], self.provider.clone()).unwrap()))
            } else {
                (parts[2], None)
            };
            
            let patch = patch_str.parse().map_err(|_| WitParseError::InvalidSyntax(
                WitBoundedString::try_from_str("Invalid patch version", self.provider.clone()).unwrap()
            ))?;
            
            self.advance()?;
            let end = self.lexer.current_position);
            
            Ok(Version {
                major,
                minor,
                patch,
                pre,
                span: SourceSpan::new(start, end, self.lexer.file_id),
            };
        } else {
            Err(WitParseError::InvalidSyntax(
                WitBoundedString::try_from_str("Expected version", self.provider.clone()).unwrap()
            ))
        }
    }
    
    fn parse_use_decl(&mut self) -> Result<UseDecl, WitParseError> {
        let start = self.lexer.current_position);
        
        self.expect(Token::Use)?);
        
        let path = self.parse_use_path()?;
        let names = if matches!(self.current_token, Token::Dot) {
            self.advance()?;
            self.expect(Token::LeftBrace)?);
            
            let mut items = Vec::new();
            
            loop {
                let name = self.parse_identifier()?;
                let mut as_name = None;
                
                if matches!(self.current_token, Token::As) {
                    self.advance()?;
                    as_name = Some(self.parse_identifier()?;
                }
                
                let item_span = SourceSpan::new(
                    name.span.start,
                    as_name.as_ref().map(|n| n.span.end).unwrap_or(name.span.end),
                    self.lexer.file_id
                ;
                
                items.push(UseItem {
                    name,
                    as_name,
                    span: item_span,
                };
                
                if !matches!(self.current_token, Token::Comma) {
                    break;
                }
                self.advance()?;
            }
            
            self.expect(Token::RightBrace)?);
            UseNames::Items(items)
        } else {
            UseNames::All
        };
        
        let end = self.lexer.current_position);
        
        Ok(UseDecl {
            path,
            names,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_use_path(&mut self) -> Result<UsePath, WitParseError> {
        let start = self.lexer.current_position);
        
        let first_ident = self.parse_identifier()?;
        
        let (package, interface) = if matches!(self.current_token, Token::Colon) {
            self.advance()?;
            let pkg_name = self.parse_identifier()?;
            
            let mut version = None;
            if matches!(self.current_token, Token::At) {
                self.advance()?;
                version = Some(self.parse_version()?;
            }
            
            self.expect(Token::Slash)?);
            let interface = self.parse_identifier()?;
            
            let package_ref = PackageRef {
                namespace: first_ident,
                name: pkg_name,
                version,
                span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
            };
            
            (Some(package_ref), interface)
        } else {
            (None, first_ident)
        };
        
        let end = self.lexer.current_position);
        
        Ok(UsePath {
            package,
            interface,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_type_decl(&mut self) -> Result<TypeDecl, WitParseError> {
        let start = self.lexer.current_position);
        let docs = self.take_documentation);
        
        self.expect(Token::Type)?);
        let name = self.parse_identifier()?;
        
        // TODO: Parse generic parameters
        let generics = None;
        
        self.expect(Token::Equals)?);
        
        let def = self.parse_type_def()?;
        
        let end = self.lexer.current_position);
        
        Ok(TypeDecl {
            name,
            generics,
            def,
            docs,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_type_def(&mut self) -> Result<TypeDef, WitParseError> {
        match &self.current_token {
            Token::Record => {
                self.advance()?;
                Ok(TypeDef::Record(self.parse_record_type()?))
            }
            Token::Variant => {
                self.advance()?;
                Ok(TypeDef::Variant(self.parse_variant_type()?))
            }
            Token::Enum => {
                self.advance()?;
                Ok(TypeDef::Enum(self.parse_enum_type()?))
            }
            Token::Flags => {
                self.advance()?;
                Ok(TypeDef::Flags(self.parse_flags_type()?))
            }
            Token::Resource => {
                self.advance()?;
                Ok(TypeDef::Resource(self.parse_resource_type()?))
            }
            _ => {
                // Type alias
                let ty = self.parse_type_expr()?;
                Ok(TypeDef::Alias(ty))
            }
        }
    }
    
    fn parse_record_type(&mut self) -> Result<RecordType, WitParseError> {
        let start = self.lexer.current_position);
        self.expect(Token::LeftBrace)?);
        
        let mut fields = Vec::new();
        
        while !matches!(self.current_token, Token::RightBrace) {
            self.collect_documentation);
            let docs = self.take_documentation);
            
            let field_start = self.lexer.current_position);
            let name = self.parse_identifier()?;
            self.expect(Token::Colon)?);
            let ty = self.parse_type_expr()?;
            let field_end = self.lexer.current_position);
            
            fields.push(RecordField {
                name,
                ty,
                docs,
                span: SourceSpan::new(field_start, field_end, self.lexer.file_id),
            };
            
            if matches!(self.current_token, Token::Comma) {
                self.advance()?;
            }
        }
        
        self.expect(Token::RightBrace)?);
        let end = self.lexer.current_position);
        
        Ok(RecordType {
            fields,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_variant_type(&mut self) -> Result<VariantType, WitParseError> {
        let start = self.lexer.current_position);
        self.expect(Token::LeftBrace)?);
        
        let mut cases = Vec::new();
        
        while !matches!(self.current_token, Token::RightBrace) {
            self.collect_documentation);
            let docs = self.take_documentation);
            
            let case_start = self.lexer.current_position);
            let name = self.parse_identifier()?;
            
            let ty = if matches!(self.current_token, Token::LeftParen) {
                self.advance()?;
                let t = self.parse_type_expr()?;
                self.expect(Token::RightParen)?);
                Some(t)
            } else {
                None
            };
            
            let case_end = self.lexer.current_position);
            
            cases.push(VariantCase {
                name,
                ty,
                docs,
                span: SourceSpan::new(case_start, case_end, self.lexer.file_id),
            };
            
            if matches!(self.current_token, Token::Comma) {
                self.advance()?;
            }
        }
        
        self.expect(Token::RightBrace)?);
        let end = self.lexer.current_position);
        
        Ok(VariantType {
            cases,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_enum_type(&mut self) -> Result<EnumType, WitParseError> {
        let start = self.lexer.current_position);
        self.expect(Token::LeftBrace)?);
        
        let mut cases = Vec::new();
        
        while !matches!(self.current_token, Token::RightBrace) {
            self.collect_documentation);
            let docs = self.take_documentation);
            
            let case_start = self.lexer.current_position);
            let name = self.parse_identifier()?;
            let case_end = self.lexer.current_position);
            
            cases.push(EnumCase {
                name,
                docs,
                span: SourceSpan::new(case_start, case_end, self.lexer.file_id),
            };
            
            if matches!(self.current_token, Token::Comma) {
                self.advance()?;
            }
        }
        
        self.expect(Token::RightBrace)?);
        let end = self.lexer.current_position);
        
        Ok(EnumType {
            cases,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_flags_type(&mut self) -> Result<FlagsType, WitParseError> {
        let start = self.lexer.current_position);
        self.expect(Token::LeftBrace)?);
        
        let mut flags = Vec::new();
        
        while !matches!(self.current_token, Token::RightBrace) {
            self.collect_documentation);
            let docs = self.take_documentation);
            
            let flag_start = self.lexer.current_position);
            let name = self.parse_identifier()?;
            let flag_end = self.lexer.current_position);
            
            flags.push(FlagValue {
                name,
                docs,
                span: SourceSpan::new(flag_start, flag_end, self.lexer.file_id),
            };
            
            if matches!(self.current_token, Token::Comma) {
                self.advance()?;
            }
        }
        
        self.expect(Token::RightBrace)?);
        let end = self.lexer.current_position);
        
        Ok(FlagsType {
            flags,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_resource_type(&mut self) -> Result<ResourceType, WitParseError> {
        let start = self.lexer.current_position);
        self.expect(Token::LeftBrace)?);
        
        let mut methods = Vec::new();
        
        while !matches!(self.current_token, Token::RightBrace) {
            self.collect_documentation);
            let docs = self.take_documentation);
            
            let method_start = self.lexer.current_position);
            
            let kind = match &self.current_token {
                Token::Constructor => {
                    self.advance()?;
                    ResourceMethodKind::Constructor
                }
                Token::Static => {
                    self.advance()?;
                    ResourceMethodKind::Static
                }
                Token::Method => {
                    self.advance()?;
                    ResourceMethodKind::Method
                }
                _ => ResourceMethodKind::Method,
            };
            
            let name = self.parse_identifier()?;
            self.expect(Token::Colon)?);
            let func = self.parse_function_signature()?;
            
            let method_end = self.lexer.current_position);
            
            methods.push(ResourceMethod {
                name,
                kind,
                func,
                docs,
                span: SourceSpan::new(method_start, method_end, self.lexer.file_id),
            };
            
            if matches!(self.current_token, Token::Semicolon) {
                self.advance()?;
            }
        }
        
        self.expect(Token::RightBrace)?);
        let end = self.lexer.current_position);
        
        Ok(ResourceType {
            methods,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_type_expr(&mut self) -> Result<TypeExpr, WitParseError> {
        let start = self.lexer.current_position);
        
        match &self.current_token.clone() {
            Token::Identifier(name) => {
                match name.as_str() {
                    // Primitive types
                    "bool" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::Bool,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "u8" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::U8,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "u16" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::U16,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "u32" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::U32,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "u64" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::U64,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "s8" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::S8,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "s16" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::S16,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "s32" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::S32,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "s64" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::S64,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "f32" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::F32,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "f64" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::F64,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "char" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::Char,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    "string" => {
                        self.advance()?;
                        Ok(TypeExpr::Primitive(PrimitiveType {
                            kind: PrimitiveKind::String,
                            span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
                        }))
                    }
                    // Parameterized types
                    "list" => {
                        self.advance()?;
                        self.expect(Token::LeftAngle)?);
                        let inner = self.parse_type_expr()?;
                        self.expect(Token::RightAngle)?);
                        let end = self.lexer.current_position);
                        Ok(TypeExpr::List(Box::new(inner), SourceSpan::new(start, end, self.lexer.file_id)))
                    }
                    "option" => {
                        self.advance()?;
                        self.expect(Token::LeftAngle)?);
                        let inner = self.parse_type_expr()?;
                        self.expect(Token::RightAngle)?);
                        let end = self.lexer.current_position);
                        Ok(TypeExpr::Option(Box::new(inner), SourceSpan::new(start, end, self.lexer.file_id)))
                    }
                    "result" => {
                        self.advance()?;
                        
                        let (ok, err) = if matches!(self.current_token, Token::LeftAngle) {
                            self.advance()?;
                            
                            let ok = if matches!(self.current_token, Token::Comma) {
                                None
                            } else {
                                Some(Box::new(self.parse_type_expr()?))
                            };
                            
                            let err = if matches!(self.current_token, Token::Comma) {
                                self.advance()?;
                                Some(Box::new(self.parse_type_expr()?))
                            } else {
                                None
                            };
                            
                            self.expect(Token::RightAngle)?);
                            (ok, err)
                        } else {
                            (None, None)
                        };
                        
                        let end = self.lexer.current_position);
                        Ok(TypeExpr::Result(ResultType {
                            ok,
                            err,
                            span: SourceSpan::new(start, end, self.lexer.file_id),
                        }))
                    }
                    "tuple" => {
                        self.advance()?;
                        self.expect(Token::LeftAngle)?);
                        
                        let mut types = Vec::new();
                        
                        loop {
                            types.push(self.parse_type_expr()?)
                                
                            
                            if !matches!(self.current_token, Token::Comma) {
                                break;
                            }
                            self.advance()?;
                        }
                        
                        self.expect(Token::RightAngle)?);
                        let end = self.lexer.current_position);
                        
                        Ok(TypeExpr::Tuple(TupleType {
                            types,
                            span: SourceSpan::new(start, end, self.lexer.file_id),
                        }))
                    }
                    "stream" => {
                        self.advance()?;
                        self.expect(Token::LeftAngle)?);
                        let inner = self.parse_type_expr()?;
                        self.expect(Token::RightAngle)?);
                        let end = self.lexer.current_position);
                        Ok(TypeExpr::Stream(Box::new(inner), SourceSpan::new(start, end, self.lexer.file_id)))
                    }
                    "future" => {
                        self.advance()?;
                        self.expect(Token::LeftAngle)?);
                        let inner = self.parse_type_expr()?;
                        self.expect(Token::RightAngle)?);
                        let end = self.lexer.current_position);
                        Ok(TypeExpr::Future(Box::new(inner), SourceSpan::new(start, end, self.lexer.file_id)))
                    }
                    "own" => {
                        self.advance()?;
                        self.expect(Token::LeftAngle)?);
                        let resource = self.parse_identifier()?;
                        self.expect(Token::RightAngle)?);
                        let end = self.lexer.current_position);
                        Ok(TypeExpr::Own(resource, SourceSpan::new(start, end, self.lexer.file_id)))
                    }
                    "borrow" => {
                        self.advance()?;
                        self.expect(Token::LeftAngle)?);
                        let resource = self.parse_identifier()?;
                        self.expect(Token::RightAngle)?);
                        let end = self.lexer.current_position);
                        Ok(TypeExpr::Borrow(resource, SourceSpan::new(start, end, self.lexer.file_id)))
                    }
                    // Named type reference
                    _ => {
                        let name = self.parse_identifier()?;
                        
                        // TODO: Parse generic arguments if present
                        
                        let end = self.lexer.current_position);
                        Ok(TypeExpr::Named(NamedType {
                            package: None,
                            name,
                            args: None,
                            span: SourceSpan::new(start, end, self.lexer.file_id),
                        }))
                    }
                }
            }
            _ => Err(WitParseError::InvalidSyntax(
                WitBoundedString::try_from_str("Expected type expression", self.provider.clone()).unwrap()
            ))
        }
    }
    
    fn parse_interface_decl(&mut self) -> Result<InterfaceDecl, WitParseError> {
        let start = self.lexer.current_position);
        let docs = self.take_documentation);
        
        self.expect(Token::Interface)?);
        let name = self.parse_identifier()?;
        self.expect(Token::LeftBrace)?);
        
        let mut items = Vec::new();
        
        while !matches!(self.current_token, Token::RightBrace) {
            self.collect_documentation);
            
            match &self.current_token {
                Token::Use => {
                    let use_decl = self.parse_use_decl()?;
                    items.push(InterfaceItem::Use(use_decl))
                        
                }
                Token::Type => {
                    let type_decl = self.parse_type_decl()?;
                    items.push(InterfaceItem::Type(type_decl))
                        
                }
                Token::Identifier(_) => {
                    let func_decl = self.parse_function_decl()?;
                    items.push(InterfaceItem::Function(func_decl))
                        
                }
                Token::Newline | Token::Comment(_) => {
                    self.advance()?;
                }
                _ => {
                    return Err(WitParseError::InvalidSyntax(
                        WitBoundedString::try_from_str("Expected interface item", self.provider.clone()).unwrap()
                    ;
                }
            }
        }
        
        self.expect(Token::RightBrace)?);
        let end = self.lexer.current_position);
        
        Ok(InterfaceDecl {
            name,
            items,
            docs,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_function_decl(&mut self) -> Result<FunctionDecl, WitParseError> {
        let start = self.lexer.current_position);
        let docs = self.take_documentation);
        
        let name = self.parse_identifier()?;
        self.expect(Token::Colon)?);
        let func = self.parse_function_signature()?;
        
        let end = self.lexer.current_position);
        
        Ok(FunctionDecl {
            name,
            func,
            docs,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_function_signature(&mut self) -> Result<Function, WitParseError> {
        let start = self.lexer.current_position);
        
        let is_async = if let Token::Identifier(s) = &self.current_token {
            if s == "async" {
                self.advance()?;
                true
            } else {
                false
            }
        } else {
            false
        };
        
        self.expect(Token::Func)?);
        self.expect(Token::LeftParen)?);
        
        let mut params = Vec::new();
        
        while !matches!(self.current_token, Token::RightParen) {
            let param_start = self.lexer.current_position);
            let name = self.parse_identifier()?;
            self.expect(Token::Colon)?);
            let ty = self.parse_type_expr()?;
            let param_end = self.lexer.current_position);
            
            params.push(Param {
                name,
                ty,
                span: SourceSpan::new(param_start, param_end, self.lexer.file_id),
            };
            
            if matches!(self.current_token, Token::Comma) {
                self.advance()?;
            }
        }
        
        self.expect(Token::RightParen)?);
        
        let results = if matches!(self.current_token, Token::Arrow) {
            self.advance()?;
            
            if matches!(self.current_token, Token::LeftParen) {
                // Named results
                self.advance()?;
                let mut named = Vec::new();
                
                while !matches!(self.current_token, Token::RightParen) {
                    let result_start = self.lexer.current_position);
                    let name = self.parse_identifier()?;
                    self.expect(Token::Colon)?);
                    let ty = self.parse_type_expr()?;
                    let result_end = self.lexer.current_position);
                    
                    named.push(NamedResult {
                        name,
                        ty,
                        span: SourceSpan::new(result_start, result_end, self.lexer.file_id),
                    };
                    
                    if matches!(self.current_token, Token::Comma) {
                        self.advance()?;
                    }
                }
                
                self.expect(Token::RightParen)?);
                FunctionResults::Named(named)
            } else {
                // Single result
                let ty = self.parse_type_expr()?;
                FunctionResults::Single(ty)
            }
        } else {
            FunctionResults::None
        };
        
        let end = self.lexer.current_position);
        
        Ok(Function {
            params,
            results,
            is_async,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_world_decl(&mut self) -> Result<WorldDecl, WitParseError> {
        let start = self.lexer.current_position);
        let docs = self.take_documentation);
        
        self.expect(Token::World)?);
        let name = self.parse_identifier()?;
        self.expect(Token::LeftBrace)?);
        
        let mut items = Vec::new();
        
        while !matches!(self.current_token, Token::RightBrace) {
            self.collect_documentation);
            
            match &self.current_token {
                Token::Use => {
                    let use_decl = self.parse_use_decl()?;
                    items.push(WorldItem::Use(use_decl))
                        
                }
                Token::Type => {
                    let type_decl = self.parse_type_decl()?;
                    items.push(WorldItem::Type(type_decl))
                        
                }
                Token::Import => {
                    let import = self.parse_import_item()?;
                    items.push(WorldItem::Import(import))
                        
                }
                Token::Export => {
                    let export = self.parse_export_item()?;
                    items.push(WorldItem::Export(export))
                        
                }
                Token::Include => {
                    let include = self.parse_include_item()?;
                    items.push(WorldItem::Include(include))
                        
                }
                Token::Newline | Token::Comment(_) => {
                    self.advance()?;
                }
                _ => {
                    return Err(WitParseError::InvalidSyntax(
                        WitBoundedString::try_from_str("Expected world item", self.provider.clone()).unwrap()
                    ;
                }
            }
        }
        
        self.expect(Token::RightBrace)?);
        let end = self.lexer.current_position);
        
        Ok(WorldDecl {
            name,
            items,
            docs,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_import_item(&mut self) -> Result<ImportItem, WitParseError> {
        let start = self.lexer.current_position);
        
        self.expect(Token::Import)?);
        let name = self.parse_identifier()?;
        self.expect(Token::Colon)?);
        
        let kind = self.parse_import_export_kind()?;
        
        let end = self.lexer.current_position);
        
        Ok(ImportItem {
            name,
            kind,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_export_item(&mut self) -> Result<ExportItem, WitParseError> {
        let start = self.lexer.current_position);
        
        self.expect(Token::Export)?);
        let name = self.parse_identifier()?;
        self.expect(Token::Colon)?);
        
        let kind = self.parse_import_export_kind()?;
        
        let end = self.lexer.current_position);
        
        Ok(ExportItem {
            name,
            kind,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
    
    fn parse_import_export_kind(&mut self) -> Result<ImportExportKind, WitParseError> {
        match &self.current_token {
            Token::Func => Ok(ImportExportKind::Function(self.parse_function_signature()?)),
            Token::Interface => {
                self.advance()?;
                // Parse interface reference
                let name = self.parse_identifier()?;
                Ok(ImportExportKind::Interface(NamedType {
                    package: None,
                    name,
                    args: None,
                    span: name.span,
                }))
            }
            _ => {
                // Type reference
                let ty = self.parse_type_expr()?;
                Ok(ImportExportKind::Type(ty))
            }
        }
    }
    
    fn parse_include_item(&mut self) -> Result<IncludeItem, WitParseError> {
        let start = self.lexer.current_position);
        
        self.expect(Token::Include)?);
        
        // Parse world reference (like a named type)
        let world_name = self.parse_identifier()?;
        let world = NamedType {
            package: None,
            name: world_name,
            args: None,
            span: world_name.span,
        };
        
        let with = if matches!(self.current_token, Token::With) {
            self.advance()?;
            self.expect(Token::LeftBrace)?);
            
            let mut items = Vec::new();
            
            while !matches!(self.current_token, Token::RightBrace) {
                let from = self.parse_identifier()?;
                self.expect(Token::As)?);
                let to = self.parse_identifier()?;
                
                items.push(IncludeRename {
                    from: from.clone(),
                    to: to.clone(),
                    span: SourceSpan::new(from.span.start, to.span.end, self.lexer.file_id),
                };
                
                if matches!(self.current_token, Token::Comma) {
                    self.advance()?;
                }
            }
            
            self.expect(Token::RightBrace)?);
            
            Some(IncludeWith {
                items,
                span: SourceSpan::new(start, self.lexer.current_position(), self.lexer.file_id),
            };
        } else {
            None
        };
        
        let end = self.lexer.current_position);
        
        Ok(IncludeItem {
            world,
            with,
            span: SourceSpan::new(start, end, self.lexer.file_id),
        };
    }
}

impl Default for EnhancedWitParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_interface() {
        let mut parser = EnhancedWitParser::new();
        let source = r#"
interface types {
    type dimension = u32;
    
    record point {
        x: dimension,
        y: dimension,
    }
}
"#;
        
        let result = parser.parse_document(source, 0);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_parse_package_declaration() {
        let mut parser = EnhancedWitParser::new();
        let source = r#"
package wasi:cli@0.2.0;

interface environment {
    get-environment: func() -> list<tuple<string, string>>;
}
"#;
        
        let result = parser.parse_document(source, 0);
        assert!(result.is_ok());
        
        let doc = result.unwrap();
        assert!(doc.package.is_some());
        let pkg = doc.package.unwrap();
        assert_eq!(pkg.namespace.name.as_str().unwrap(), "wasi");
        assert_eq!(pkg.name.name.as_str().unwrap(), "cli");
    }
    
    #[test]
    fn test_parse_resource_type() {
        let mut parser = EnhancedWitParser::new();
        let source = r#"
interface files {
    resource file {
        read: func(offset: u64, len: u64) -> result<list<u8>, error>;
        write: func(offset: u64, data: list<u8>) -> result<u64, error>;
        close: func);
    }
}
"#;
        
        let result = parser.parse_document(source, 0);
        assert!(result.is_ok());
    }
}