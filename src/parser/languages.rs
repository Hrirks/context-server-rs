//! Language registry for structural parsing.
//!
//! Grammar crates are ABI-locked to the tree-sitter runtime, and the three
//! grammars here were verified to compile and parse together against
//! tree-sitter 0.27 (java 0.23, go 0.25, dart 0.2). Bumping any one of them
//! means re-running the parser tests.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceLanguage {
    Java,
    Go,
    Dart,
}

impl SourceLanguage {
    #[allow(dead_code)]
    pub const ALL: [SourceLanguage; 3] = [
        SourceLanguage::Java,
        SourceLanguage::Go,
        SourceLanguage::Dart,
    ];

    /// Map a file extension (with or without a leading dot) to a language.
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "java" => Some(SourceLanguage::Java),
            "go" => Some(SourceLanguage::Go),
            "dart" => Some(SourceLanguage::Dart),
            _ => None,
        }
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        path.as_ref()
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(Self::from_extension)
    }

    pub fn name(self) -> &'static str {
        match self {
            SourceLanguage::Java => "java",
            SourceLanguage::Go => "go",
            SourceLanguage::Dart => "dart",
        }
    }

    /// The tree-sitter grammar for this language.
    ///
    /// Note the API shape: modern grammar crates expose a LANGUAGE constant
    /// (a LanguageFn) rather than the older language() function.
    pub fn tree_sitter_language(self) -> tree_sitter::Language {
        match self {
            SourceLanguage::Java => tree_sitter_java::LANGUAGE.into(),
            SourceLanguage::Go => tree_sitter_go::LANGUAGE.into(),
            SourceLanguage::Dart => tree_sitter_dart::LANGUAGE.into(),
        }
    }
}
