//! Semantic chunking: turn a source file into the isolated declarations an
//! agent actually wants, instead of a whole-file text dump.
//!
//! Parsing is CPU-bound and must never run on an async runtime thread. Use the
//! [_async] entry points; they route the parse through [offload], which is
//! tokio::task::spawn_blocking. The synchronous functions exist for tests and
//! for callers that are already on a dedicated blocking thread.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tree_sitter::{Node, Parser};

use super::languages::SourceLanguage;

/// Directories that never contain first-party sources worth chunking.
const SKIPPED_DIRECTORIES: [&str; 12] = [
    ".git",
    "target",
    "node_modules",
    "build",
    "dist",
    "out",
    ".dart_tool",
    ".idea",
    "vendor",
    "test",
    "tests",
    "integration_test",
];

/// Guard against being pointed at an enormous tree by accident.
const MAX_DISCOVERED_FILES: usize = 5_000;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("no tree-sitter grammar registered for language {0}")]
    UnknownLanguage(String),
    #[error("could not initialise grammar: {0}")]
    LanguageInit(String),
    #[error("tree-sitter produced no tree for this source")]
    NoTree,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parsing task failed: {0}")]
    Join(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkKind {
    Package,
    Import,
    Class,
    Interface,
    Enum,
    Record,
    Struct,
    Mixin,
    Extension,
    Function,
    Method,
    Constructor,
    Type,
}

impl ChunkKind {
    /// Kinds that own a body and can therefore contain other declarations.
    pub fn is_container(self) -> bool {
        matches!(
            self,
            ChunkKind::Class
                | ChunkKind::Interface
                | ChunkKind::Enum
                | ChunkKind::Record
                | ChunkKind::Struct
                | ChunkKind::Mixin
                | ChunkKind::Extension
        )
    }

    /// Containers normally serve only their header, because their members are
    /// returned as separate chunks. Structs and interfaces are the exception:
    /// their fields and member signatures live in the body and are the whole
    /// point of the chunk.
    fn serves_full_text(self) -> bool {
        matches!(self, ChunkKind::Struct | ChunkKind::Interface)
    }
}

/// The kind of relationship a [`SymbolReference`] describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    /// A call site: the chunk invokes this name (function, method, constructor).
    Call,
    /// A supertype: the chunk extends/implements this name.
    Inherit,
    /// A type mention: the chunk uses this name as a type in a field/signature.
    Reference,
}

/// A name mentioned inside a chunk, resolved into a graph edge after indexing.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolReference {
    pub kind: ReferenceKind,
    pub name: String,
    /// 1-based line where the reference occurs.
    pub line: usize,
}

/// One isolated, individually addressable piece of a source file.
#[derive(Debug, Clone, Serialize)]
pub struct SemanticChunk {
    pub kind: ChunkKind,
    pub language: SourceLanguage,
    pub name: Option<String>,
    /// Declaration header with the body stripped and whitespace collapsed.
    pub signature: String,
    /// 1-based, inclusive.
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    /// Header only for container kinds, full body for functions and methods.
    pub text: String,
    /// Index into the same result vector of the enclosing container chunk.
    pub parent: Option<usize>,
    /// Names this chunk references: calls, supertypes, and type mentions.
    pub references: Vec<SymbolReference>,
}

/// Parse one in-memory source string into semantic chunks. Synchronous and
/// CPU-bound - call it from a blocking thread.
pub fn chunk_source(
    language: SourceLanguage,
    source: &str,
) -> Result<Vec<SemanticChunk>, ParseError> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.tree_sitter_language())
        .map_err(|error| ParseError::LanguageInit(format!("{}: {error}", language.name())))?;

    let tree = parser.parse(source, None).ok_or(ParseError::NoTree)?;

    let mut chunks = Vec::new();
    visit(tree.root_node(), language, source, None, &mut chunks);
    Ok(chunks)
}

/// Run a CPU-bound closure on the blocking pool.
///
/// This is the offload primitive the whole module is built on, exposed so
/// callers can build their own parse pipelines with identical semantics.
pub async fn offload<T, F>(work: F) -> Result<T, ParseError>
where
    F: FnOnce() -> Result<T, ParseError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ParseError::Join(error.to_string()))?
}

/// Parse an in-memory source on the blocking pool.
///
/// Parsing must not run on the async runtime: a large file would stall the
/// executor and the MCP server would stop answering requests mid-parse.
#[allow(dead_code)]
pub async fn chunk_source_async(
    language: SourceLanguage,
    source: String,
) -> Result<Vec<SemanticChunk>, ParseError> {
    offload(move || chunk_source(language, &source)).await
}

/// Read and parse a file on the blocking pool.
pub async fn chunk_file_async(path: PathBuf) -> Result<Vec<SemanticChunk>, ParseError> {
    offload(move || {
        let language = SourceLanguage::from_path(&path)
            .ok_or_else(|| ParseError::UnknownLanguage(path.display().to_string()))?;
        let source = std::fs::read_to_string(&path)?;
        chunk_source(language, &source)
    })
    .await
}

/// Recursively collect parseable sources under a root directory.
///
/// Skips build output, dependency trees, and test directories. Discovery is
/// plain filesystem work and is cheap enough to run inline.
pub fn discover_sources(root: &Path) -> Result<Vec<PathBuf>, ParseError> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(directory) = stack.pop() {
        if found.len() >= MAX_DISCOVERED_FILES {
            break;
        }

        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            // Unreadable directories (permissions, broken symlinks) are not fatal.
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with('.') || SKIPPED_DIRECTORIES.contains(&name.as_ref()) {
                    continue;
                }
                stack.push(path);
            } else if file_type.is_file() && SourceLanguage::from_path(&path).is_some() {
                found.push(path);
            }
        }
    }

    found.sort();
    Ok(found)
}

/// Discover and parse every supported source under a root, off the runtime.
#[allow(dead_code)]
pub async fn chunk_directory_async(root: PathBuf) -> Result<Vec<SemanticChunk>, ParseError> {
    offload(move || {
        let mut chunks = Vec::new();
        for path in discover_sources(&root)? {
            let Some(language) = SourceLanguage::from_path(&path) else {
                continue;
            };
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            match chunk_source(language, &source) {
                Ok(mut file_chunks) => chunks.append(&mut file_chunks),
                // A file that fails to parse should not abort the whole walk.
                Err(_) => continue,
            }
        }
        Ok(chunks)
    })
    .await
}

fn visit(
    node: Node,
    language: SourceLanguage,
    source: &str,
    parent: Option<usize>,
    out: &mut Vec<SemanticChunk>,
) {
    let mut inherited_parent = parent;

    if let Some(kind) = classify(node, language) {
        let index = out.len();
        out.push(build_chunk(node, kind, language, source, parent));
        if kind.is_container() {
            inherited_parent = Some(index);
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit(child, language, source, inherited_parent, out);
    }
}

fn classify(node: Node, language: SourceLanguage) -> Option<ChunkKind> {
    match language {
        SourceLanguage::Java => match node.kind() {
            "package_declaration" => Some(ChunkKind::Package),
            "import_declaration" => Some(ChunkKind::Import),
            "class_declaration" => Some(ChunkKind::Class),
            "interface_declaration" | "annotation_type_declaration" => Some(ChunkKind::Interface),
            "enum_declaration" => Some(ChunkKind::Enum),
            "record_declaration" => Some(ChunkKind::Record),
            "method_declaration" => Some(ChunkKind::Method),
            "constructor_declaration" => Some(ChunkKind::Constructor),
            _ => None,
        },
        SourceLanguage::Go => match node.kind() {
            "package_clause" => Some(ChunkKind::Package),
            "import_declaration" => Some(ChunkKind::Import),
            "function_declaration" => Some(ChunkKind::Function),
            "method_declaration" => Some(ChunkKind::Method),
            "type_declaration" => Some(classify_go_type(node)),
            _ => None,
        },
        SourceLanguage::Dart => match node.kind() {
            "import_or_export" | "part_directive" | "library_directive" => Some(ChunkKind::Import),
            "class_declaration" => Some(ChunkKind::Class),
            "mixin_declaration" => Some(ChunkKind::Mixin),
            "enum_declaration" => Some(ChunkKind::Enum),
            "extension_declaration" => Some(ChunkKind::Extension),
            "function_declaration" => Some(ChunkKind::Function),
            "method_declaration" => Some(ChunkKind::Method),
            "constructor_signature" => Some(ChunkKind::Constructor),
            _ => None,
        },
    }
}

/// Go groups struct and interface declarations under a generic type_declaration.
fn classify_go_type(node: Node) -> ChunkKind {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_spec" => {
                return match child.child_by_field_name("type").map(|inner| inner.kind()) {
                    Some("struct_type") => ChunkKind::Struct,
                    Some("interface_type") => ChunkKind::Interface,
                    _ => ChunkKind::Type,
                };
            }
            "type_alias" => return ChunkKind::Type,
            _ => {}
        }
    }
    ChunkKind::Type
}

fn build_chunk(
    node: Node,
    kind: ChunkKind,
    language: SourceLanguage,
    source: &str,
    parent: Option<usize>,
) -> SemanticChunk {
    let signature = signature_of(node, source);
    let text = if kind.is_container() && !kind.serves_full_text() {
        signature.clone()
    } else {
        source[node.byte_range()].to_string()
    };

    SemanticChunk {
        kind,
        language,
        name: find_name(node, source, 0),
        signature,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        text,
        parent,
        references: extract_references(node, language, source),
    }
}

/// Collect the names a declaration references: call sites, supertypes, and
/// type mentions. Nested declarations are skipped so each reference is
/// attributed to exactly one chunk (the innermost one that owns it).
fn extract_references(node: Node, language: SourceLanguage, source: &str) -> Vec<SymbolReference> {
    let mut out = Vec::new();
    let mut consumed = HashSet::new();
    collect_references(node, language, source, true, &mut out, &mut consumed);

    // De-duplicate by (kind, name) while preserving first-seen order.
    let mut seen = HashSet::new();
    out.retain(|reference| seen.insert((reference.kind, reference.name.clone())));
    out
}

fn collect_references(
    node: Node,
    language: SourceLanguage,
    source: &str,
    is_root: bool,
    out: &mut Vec<SymbolReference>,
    consumed: &mut HashSet<(usize, usize)>,
) {
    let line = node.start_position().row + 1;

    match language {
        SourceLanguage::Java => match node.kind() {
            "method_invocation" => {
                if let Some(name) = node.child_by_field_name("name") {
                    push_reference(out, ReferenceKind::Call, &source[name.byte_range()], line);
                }
            }
            "object_creation_expression" => {
                if let Some(ty) = node.child_by_field_name("type") {
                    // The type is consumed as a call target, not a type mention.
                    consumed.insert((ty.start_byte(), ty.end_byte()));
                    push_reference(
                        out,
                        ReferenceKind::Call,
                        &last_identifier(&source[ty.byte_range()]),
                        line,
                    );
                }
            }
            "superclass" | "super_interfaces" => {
                for name in type_names_in(node, source) {
                    push_reference(out, ReferenceKind::Inherit, &name, line);
                }
                return; // supertypes are inheritance, not plain type mentions
            }
            "type_identifier" | "scoped_type_identifier" => {
                if !consumed.contains(&(node.start_byte(), node.end_byte())) {
                    push_reference(
                        out,
                        ReferenceKind::Reference,
                        &last_identifier(&source[node.byte_range()]),
                        line,
                    );
                }
            }
            _ => {}
        },
        SourceLanguage::Go => match node.kind() {
            "call_expression" => {
                if let Some(function) = node.child_by_field_name("function") {
                    if let Some(name) = callee_name(function, source) {
                        push_reference(out, ReferenceKind::Call, &name, line);
                    }
                }
            }
            "type_identifier" => {
                push_reference(
                    out,
                    ReferenceKind::Reference,
                    &source[node.byte_range()],
                    line,
                );
            }
            _ => {}
        },
        SourceLanguage::Dart => match node.kind() {
            "method_invocation" => {
                if let Some(name) = node.child_by_field_name("name") {
                    push_reference(out, ReferenceKind::Call, &source[name.byte_range()], line);
                }
            }
            "call_expression" => {
                if let Some(function) = node.child_by_field_name("function") {
                    if let Some(name) = callee_name(function, source) {
                        push_reference(out, ReferenceKind::Call, &name, line);
                    }
                }
            }
            "superclass" | "interfaces" => {
                for name in type_names_in(node, source) {
                    push_reference(out, ReferenceKind::Inherit, &name, line);
                }
                return;
            }
            "type_identifier" => {
                push_reference(
                    out,
                    ReferenceKind::Reference,
                    &source[node.byte_range()],
                    line,
                );
            }
            _ => {}
        },
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !is_root && classify(child, language).is_some() {
            continue;
        }
        collect_references(child, language, source, false, out, consumed);
    }
}

fn push_reference(out: &mut Vec<SymbolReference>, kind: ReferenceKind, name: &str, line: usize) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    out.push(SymbolReference {
        kind,
        name: name.to_string(),
        line,
    });
}

/// The callee name at a call site: a plain identifier, or the `field` of a
/// `selector_expression` (`obj.method()`).
fn callee_name(node: Node, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" | "field_identifier" => Some(source[node.byte_range()].to_string()),
        "selector_expression" => node
            .child_by_field_name("field")
            .map(|field| source[field.byte_range()].to_string()),
        _ => None,
    }
}

/// Every type name appearing inside a supertype clause (`extends A, B`).
fn type_names_in(node: Node, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    collect_type_names(node, source, &mut names);
    names
}

fn collect_type_names(node: Node, source: &str, out: &mut Vec<String>) {
    if node.kind() == "type_identifier" || node.kind() == "scoped_type_identifier" {
        out.push(last_identifier(&source[node.byte_range()]));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_type_names(child, source, out);
    }
}

/// The final component of a possibly-qualified type (`Outer.Inner` -> `Inner`).
fn last_identifier(text: &str) -> String {
    text.rsplit('.').next().unwrap_or(text).trim().to_string()
}

/// The declaration header: everything before the body.
fn signature_of(node: Node, source: &str) -> String {
    if let Some(body) = node
        .children(&mut node.walk())
        .find(|child| is_body_kind(child.kind()))
    {
        return collapse_whitespace(&source[node.start_byte()..body.start_byte()]);
    }

    // Declarations whose body is not a distinct child (Go structs and
    // interfaces, for example) start their body at the first opening brace.
    let raw = &source[node.byte_range()];
    if let Some(brace) = raw.find('{') {
        return collapse_whitespace(&raw[..brace]);
    }
    collapse_whitespace(raw)
}

fn is_body_kind(kind: &str) -> bool {
    matches!(
        kind,
        "block"
            | "class_body"
            | "interface_body"
            | "enum_body"
            | "constructor_body"
            | "function_body"
            | "extension_body"
    )
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Dart nests the name one or two levels down (declaration -> signature ->
/// function_signature), so search a bounded depth instead of guessing a path.
fn find_name(node: Node, source: &str, depth: usize) -> Option<String> {
    if depth > 4 {
        return None;
    }

    if matches!(
        node.kind(),
        "identifier" | "type_identifier" | "field_identifier" | "package_identifier"
    ) {
        return Some(source[node.byte_range()].to_string());
    }

    if let Some(name) = node.child_by_field_name("name") {
        return Some(source[name.byte_range()].to_string());
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "function_signature"
                | "method_signature"
                | "constructor_signature"
                | "getter_signature"
                | "setter_signature"
                | "type_spec"
                | "type_alias"
        ) {
            if let Some(found) = find_name(child, source, depth + 1) {
                return Some(found);
            }
        }
    }

    None
}
