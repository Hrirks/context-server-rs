//! Persisted dense embeddings and the helpers used for semantic search.

use serde::{Deserialize, Serialize};

/// A dense embedding vector persisted for a context item.
///
/// `vector` is the raw embedding; `model` and `version` identify which backend
/// and schema produced it. The `(context_id, model, version)` triple is unique,
/// so re-embedding a context item with the same model/version overwrites the
/// previous vector rather than appending a duplicate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEmbedding {
    pub id: String,
    pub context_id: String,
    pub project_id: Option<String>,
    pub vector: Vec<f32>,
    pub model: String,
    pub version: String,
    pub content_hash: String,
    pub content_type: Option<String>,
    pub content_length: Option<i64>,
    pub tokenization_method: Option<String>,
    pub preprocessing_steps: Option<Vec<String>>,
    pub quality_score: Option<f64>,
    pub custom_metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

/// A ranked hit from a brute-force cosine-similarity search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingSearchResult {
    pub context_id: String,
    pub similarity: f32,
    pub content_type: Option<String>,
}

/// Stable 64-bit FNV-1a hash of `text`, hex-encoded.
///
/// Deterministic across processes and runs (unlike `std::hash::DefaultHasher`,
/// whose seed is randomized per process), so the same content always maps to
/// the same hash. Used to detect content changes without re-embedding.
pub fn content_hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Cosine similarity between two equal-length vectors.
///
/// Returns 1.0 for identical direction, 0.0 for orthogonal, -1.0 for opposite.
/// Returns 0.0 when the vectors differ in length or either is zero-length.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot / (norm_a.sqrt() * norm_b.sqrt())).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_stable_and_distinct() {
        assert_eq!(content_hash("hello"), content_hash("hello"));
        assert_ne!(content_hash("hello"), content_hash("world"));
    }

    #[test]
    fn cosine_similarity_basics() {
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[0.0, 0.0]), 0.0);
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
    }
}
