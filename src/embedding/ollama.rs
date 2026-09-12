//! Ollama HTTP embedding backend.
//!
//! Calls `POST {base_url}/api/embed` with `{"model": ..., "input": ...}` and
//! reads the vectors from the `embeddings` array. `input` accepts a single
//! string or an array of strings, so a whole batch of symbol texts costs one
//! round-trip instead of one per symbol. Ollama serves plain HTTP on
//! `http://localhost:11434` by default, so no TLS is required.

use super::{EmbeddingError, EmbeddingService};
use async_trait::async_trait;
use serde_json::Value;

/// Embedding backend backed by a local or remote Ollama server.
pub struct OllamaEmbeddingBackend {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OllamaEmbeddingBackend {
    /// Create a backend pointing at `base_url` (e.g. `http://localhost:11434`).
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }

    /// The embedding model this backend targets.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The Ollama base URL this backend targets.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[async_trait]
impl EmbeddingService for OllamaEmbeddingBackend {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let mut vectors = self.embed_batch(&[text]).await?;
        vectors.pop().ok_or(EmbeddingError::NoVectors)
    }

    /// Embed every input in a single `POST /api/embed` request.
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.iter().any(|text| text.trim().is_empty()) {
            return Err(EmbeddingError::EmptyInput);
        }
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let body = serde_json::json!({
            "model": self.model,
            "input": texts,
        });

        let response = self
            .client
            .post(format!("{}/api/embed", self.base_url.trim_end_matches('/')))
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(EmbeddingError::HttpStatus(status, message));
        }

        let json: Value = response.json().await?;
        let vectors = parse_embed_responses(&json)?;

        // A truncated response would silently misalign vectors with inputs.
        if vectors.len() != texts.len() {
            return Err(EmbeddingError::NoVectors);
        }

        Ok(vectors)
    }
}

/// Parse every embedding vector out of an Ollama `/api/embed` response.
fn parse_embed_responses(json: &Value) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    let embeddings = json
        .get("embeddings")
        .and_then(Value::as_array)
        .ok_or(EmbeddingError::NoVectors)?;

    let vectors = embeddings
        .iter()
        .map(parse_vector)
        .collect::<Result<Vec<Vec<f32>>, _>>()?;

    if vectors.is_empty() {
        return Err(EmbeddingError::NoVectors);
    }
    Ok(vectors)
}

/// Parse a single JSON array of numbers into an `f32` vector.
fn parse_vector(value: &Value) -> Result<Vec<f32>, EmbeddingError> {
    let vector = value
        .as_array()
        .ok_or(EmbeddingError::NoVectors)?
        .iter()
        .map(|v| {
            v.as_f64()
                .map(|f| f as f32)
                .ok_or(EmbeddingError::NoVectors)
        })
        .collect::<Result<Vec<f32>, _>>()?;

    if vector.is_empty() {
        return Err(EmbeddingError::NoVectors);
    }
    Ok(vector)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_embed_responses_roundtrip() {
        let json = serde_json::json!({ "embeddings": [[0.1, 0.2, 0.3]] });
        assert_eq!(
            parse_embed_responses(&json).unwrap(),
            vec![vec![0.1f32, 0.2, 0.3]]
        );
    }

    #[test]
    fn parse_embed_responses_keeps_every_batched_vector() {
        let json = serde_json::json!({ "embeddings": [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]] });
        let vectors = parse_embed_responses(&json).unwrap();
        assert_eq!(vectors.len(), 3);
        assert_eq!(vectors[0], vec![1.0f32, 2.0]);
        assert_eq!(vectors[2], vec![5.0f32, 6.0]);
    }

    #[test]
    fn parse_embed_responses_rejects_missing_embeddings() {
        assert!(matches!(
            parse_embed_responses(&serde_json::json!({})),
            Err(EmbeddingError::NoVectors)
        ));
    }

    #[test]
    fn parse_embed_responses_rejects_empty_vector() {
        let json = serde_json::json!({ "embeddings": [[]] });
        assert!(matches!(
            parse_embed_responses(&json),
            Err(EmbeddingError::NoVectors)
        ));
    }
}
