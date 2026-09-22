//! Structural (tree-sitter) awareness for Java, Go, and Dart sources.

pub mod chunker;
pub mod languages;

pub use chunker::{
    chunk_file_async, discover_with_limit, ChunkKind, ReferenceKind, SemanticChunk,
    MAX_DISCOVERED_FILES,
};
pub use languages::SourceLanguage;
