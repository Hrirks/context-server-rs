//! Structural (tree-sitter) awareness for Java, Go, and Dart sources.

pub mod chunker;
pub mod languages;

pub use chunker::{chunk_file_async, discover_sources, ChunkKind, ReferenceKind, SemanticChunk};
pub use languages::SourceLanguage;
