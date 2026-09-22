//! Structural (tree-sitter) awareness for Java, Go, and Dart sources.

pub mod chunker;
pub mod languages;

pub use chunker::{
    chunk_file_async, discover_with_options, is_test_path, ChunkKind, DiscoveryOptions,
    ReferenceKind, SemanticChunk, MAX_DISCOVERED_FILES,
};
pub use languages::SourceLanguage;
