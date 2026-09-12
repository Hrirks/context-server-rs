//! Real semantic embeddings.
//!
//! Phase 4a replaces the deleted fake `DefaultHasher` "embedding" with a
//! pluggable [`EmbeddingService`] trait and a real HTTP backend
//! ([`OllamaEmbeddingBackend`]). A deterministic backend is provided for tests
//! and local development only — it is explicitly NOT a semantic embedding.

mod ollama;

pub use ollama::OllamaEmbeddingBackend;

use async_trait::async_trait;
use thiserror::Error;

/// Errors produced while computing or parsing embeddings.
#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("cannot embed empty input")]
    EmptyInput,
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid JSON response: {0}")]
    Json(#[from] serde_json::Error),
    #[error("embedding backend returned HTTP {0}: {1}")]
    HttpStatus(reqwest::StatusCode, String),
    #[error("embedding response did not contain any vectors")]
    NoVectors,
}

/// Pluggable text-embedding backend.
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// Compute a dense embedding vector for `text`.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    /// Embed multiple inputs in one call. The default implementation is
    /// sequential; backends with a batched wire format should override it.
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            out.push(self.embed(text).await?);
        }
        Ok(out)
    }
}

/// Deterministic, non-semantic embedding backend for tests and local dev.
///
/// It produces stable, fixed-length vectors so downstream code (storage,
/// retrieval, tests) can be exercised without a model server. It is NOT a
/// semantic embedding and must never be used for production search.
#[allow(dead_code)]
pub struct DeterministicEmbeddingBackend {
    dimensions: usize,
}

#[allow(dead_code)]
impl DeterministicEmbeddingBackend {
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions: dimensions.max(1),
        }
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

/// FNV-1a, used only to give the deterministic backend stable bucket indices.
#[allow(dead_code)]
fn fnv1a(bytes: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

#[async_trait]
impl EmbeddingService for DeterministicEmbeddingBackend {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.trim().is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        let mut vec = vec![0.0f32; self.dimensions];
        for token in text.split_whitespace() {
            let bucket = (fnv1a(token) % self.dimensions as u64) as usize;
            vec[bucket] += 1.0;
        }

        let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut vec {
                *x /= norm;
            }
        }
        Ok(vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deterministic_backend_is_stable_and_distinct() {
        let backend = DeterministicEmbeddingBackend::new(64);

        let a = backend.embed("create user account").await.unwrap();
        let a_again = backend.embed("create user account").await.unwrap();
        let b = backend.embed("delete invoice record").await.unwrap();

        assert_eq!(a, a_again);
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[tokio::test]
    async fn empty_input_is_rejected() {
        let backend = DeterministicEmbeddingBackend::new(8);
        assert!(matches!(
            backend.embed("   ").await,
            Err(EmbeddingError::EmptyInput)
        ));
    }

    #[tokio::test]
    async fn embed_batch_matches_individual_embeddings() {
        let backend = DeterministicEmbeddingBackend::new(16);
        let texts = ["alpha", "beta", "alpha"];
        let batch = backend.embed_batch(&texts).await.unwrap();

        assert_eq!(batch.len(), 3);
        assert_eq!(batch[0], batch[2]);
        assert_ne!(batch[0], batch[1]);
    }
}
