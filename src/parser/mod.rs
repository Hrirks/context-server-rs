//! Structural (tree-sitter) awareness for Java, Go, and Dart sources.

pub mod chunker;
pub mod languages;

pub use chunker::{
    chunk_directory_async, chunk_file_async, chunk_source, chunk_source_async, discover_sources,
    offload, ChunkKind, ParseError, SemanticChunk,
};
pub use languages::SourceLanguage;
