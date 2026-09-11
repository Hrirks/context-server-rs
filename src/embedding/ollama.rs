//! Ollama HTTP embedding backend.
//!
//! Calls `POST {base_url}/api/embed` with `{"model": ..., "input": ...}` and
//! reads the first vector from the `embeddings` array. Ollama serves plain HTTP
//! on `http://localhost:11434` by default, so no TLS is required.

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
        if text.trim().is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        let body = serde_json::json!({
            "model": self.model,
            "input": text,
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
        parse_embed_response(&json)
    }
}

/// Parse an Ollama `/api/embed` response into its first embedding vector.
fn parse_embed_response(json: &Value) -> Result<Vec<f32>, EmbeddingError> {
    let embeddings = json
        .get("embeddings")
        .and_then(Value::as_array)
        .ok_or(EmbeddingError::NoVectors)?;

    let first = embeddings.first().ok_or(EmbeddingError::NoVectors)?;

    let vector = first
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
    fn parse_embed_response_roundtrip() {
        let json = serde_json::json!({ "embeddings": [[0.1, 0.2, 0.3]] });
        assert_eq!(parse_embed_response(&json).unwrap(), vec![0.1f32, 0.2, 0.3]);
    }

    #[test]
    fn parse_embed_response_rejects_missing_embeddings() {
        assert!(matches!(
            parse_embed_response(&serde_json::json!({})),
            Err(EmbeddingError::NoVectors)
        ));
    }

    #[test]
    fn parse_embed_response_rejects_empty_vector() {
        let json = serde_json::json!({ "embeddings": [[]] });
        assert!(matches!(
            parse_embed_response(&json),
            Err(EmbeddingError::NoVectors)
        ));
    }
}
