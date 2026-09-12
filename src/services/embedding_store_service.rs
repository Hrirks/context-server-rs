//! Persistence service for semantic embeddings.
//!
//! This is distinct from [`crate::embedding::EmbeddingService`], which only
//! *computes* vectors. [`EmbeddingStoreService`] combines a compute backend with
//! an [`EmbeddingRepository`] so callers can embed text, persist the result, and
//! run cosine search over stored vectors — all behind one seam. Search prefers
//! ranking inside SQLite (sqlite-vec) and only falls back to a brute-force Rust
//! scan when that is unavailable.

use crate::embedding::EmbeddingService as EmbeddingBackend;
use crate::models::embedding::{
    content_hash, cosine_similarity, EmbeddingSearchResult, StoredEmbedding,
};
use crate::repositories::EmbeddingRepository;
use rmcp::model::ErrorData as McpError;
use std::sync::Arc;
use uuid::Uuid;

/// Persists embeddings and answers semantic-search queries.
pub struct EmbeddingStoreService {
    repository: Arc<dyn EmbeddingRepository>,
    backend: Arc<dyn EmbeddingBackend>,
    model: String,
    version: String,
}

#[allow(dead_code)]
impl EmbeddingStoreService {
    pub fn new(
        repository: Arc<dyn EmbeddingRepository>,
        backend: Arc<dyn EmbeddingBackend>,
        model: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            repository,
            backend,
            model: model.into(),
            version: version.into(),
        }
    }

    /// The embedding model label this store records against.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The embedding schema version this store records against.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Embed `text` and upsert the resulting vector for `context_id`.
    ///
    /// Re-embedding the same `context_id` with the same model/version replaces
    /// the previous vector rather than appending a duplicate.
    pub async fn embed_and_store(
        &self,
        context_id: &str,
        project_id: Option<&str>,
        text: &str,
        content_type: Option<&str>,
    ) -> Result<StoredEmbedding, McpError> {
        let vector = self.backend.embed(text).await.map_err(|e| {
            McpError::internal_error(format!("Embedding backend failed: {e}"), None)
        })?;

        let now = chrono::Utc::now().to_rfc3339();
        let embedding = StoredEmbedding {
            id: Uuid::new_v4().to_string(),
            context_id: context_id.to_string(),
            project_id: project_id.map(str::to_string),
            vector,
            model: self.model.clone(),
            version: self.version.clone(),
            content_hash: content_hash(text),
            content_type: content_type.map(str::to_string),
            content_length: Some(text.len() as i64),
            tokenization_method: None,
            preprocessing_steps: None,
            quality_score: None,
            custom_metadata: None,
            created_at: now.clone(),
            updated_at: Some(now),
        };

        self.repository.upsert_embedding(&embedding).await?;
        Ok(embedding)
    }

    /// Embed and persist many context items in one backend round-trip.
    ///
    /// Each item is `(context_id, text, content_type)`. Vectors are computed in
    /// a single [`EmbeddingService::embed_batch`] call and then written
    /// individually, so one bad row does not discard the rest of the batch.
    /// Returns `(stored, failed)`.
    pub async fn embed_and_store_batch(
        &self,
        project_id: Option<&str>,
        items: &[(String, String, String)],
    ) -> (usize, usize) {
        if items.is_empty() {
            return (0, 0);
        }

        let texts: Vec<&str> = items.iter().map(|(_, text, _)| text.as_str()).collect();
        let vectors = match self.backend.embed_batch(&texts).await {
            Ok(vectors) => vectors,
            Err(error) => {
                tracing::warn!("Batch embedding of {} items failed: {error}", items.len());
                return (0, items.len());
            }
        };

        if vectors.len() != items.len() {
            tracing::warn!(
                "Embedding backend returned {} vectors for {} inputs",
                vectors.len(),
                items.len()
            );
            return (0, items.len());
        }

        let mut stored = 0usize;
        let mut failed = 0usize;

        for ((context_id, text, content_type), vector) in items.iter().zip(vectors) {
            let now = chrono::Utc::now().to_rfc3339();
            let embedding = StoredEmbedding {
                id: Uuid::new_v4().to_string(),
                context_id: context_id.clone(),
                project_id: project_id.map(str::to_string),
                vector,
                model: self.model.clone(),
                version: self.version.clone(),
                content_hash: content_hash(text),
                content_type: Some(content_type.clone()),
                content_length: Some(text.len() as i64),
                tokenization_method: None,
                preprocessing_steps: None,
                quality_score: None,
                custom_metadata: None,
                created_at: now.clone(),
                updated_at: Some(now),
            };

            match self.repository.upsert_embedding(&embedding).await {
                Ok(()) => stored += 1,
                Err(error) => {
                    tracing::warn!("Failed to persist embedding for {context_id}: {error}");
                    failed += 1;
                }
            }
        }

        (stored, failed)
    }

    /// Cosine-similarity search over a project's stored embeddings.
    ///
    /// Ranking prefers sqlite-vec, which does the distance math inside SQLite
    /// without materializing every vector as a Rust struct. If the extension is
    /// unavailable (or the stored vectors are not mutually dimensionally
    /// consistent), it transparently falls back to a brute-force scan.
    pub async fn search(
        &self,
        query: &str,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<EmbeddingSearchResult>, McpError> {
        let query_vector = self.backend.embed(query).await.map_err(|e| {
            McpError::internal_error(format!("Embedding backend failed: {e}"), None)
        })?;

        match self
            .repository
            .search_similar_vectors(project_id, &query_vector, limit)
            .await
        {
            Ok(results) => Ok(results),
            Err(error) => {
                tracing::debug!(
                    "sqlite-vec search unavailable ({error}); falling back to brute-force scan"
                );
                self.search_brute_force(project_id, &query_vector, limit)
                    .await
            }
        }
    }

    /// Linear cosine scan over a project's embeddings (the pre-sqlite-vec path).
    async fn search_brute_force(
        &self,
        project_id: &str,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<EmbeddingSearchResult>, McpError> {
        let embeddings = self
            .repository
            .find_embeddings_by_project(project_id)
            .await?;

        let mut scored: Vec<EmbeddingSearchResult> = embeddings
            .into_iter()
            .map(|e| EmbeddingSearchResult {
                similarity: cosine_similarity(query_vector, &e.vector),
                context_id: e.context_id,
                content_type: e.content_type,
            })
            .collect();

        scored.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);

        Ok(scored)
    }

    /// Fetch a previously stored embedding.
    pub async fn get_embedding(
        &self,
        context_id: &str,
    ) -> Result<Option<StoredEmbedding>, McpError> {
        self.repository
            .find_embedding(context_id, &self.model, &self.version)
            .await
    }

    /// Remove every embedding attached to a context item.
    pub async fn delete_for_context(&self, context_id: &str) -> Result<usize, McpError> {
        self.repository
            .delete_embeddings_for_context(context_id)
            .await
    }

    /// Total number of stored embeddings.
    pub async fn count(&self) -> Result<u64, McpError> {
        self.repository.count().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection_pool::ConnectionPool;
    use crate::embedding::DeterministicEmbeddingBackend;
    use crate::infrastructure::SqliteEmbeddingRepository;
    use std::time::Duration;

    fn build() -> (Arc<ConnectionPool>, EmbeddingStoreService) {
        let pool = Arc::new(ConnectionPool::new(":memory:", 1, Duration::from_secs(1)).unwrap());
        {
            let conn = pool.checkout().unwrap();
            conn.lock()
                .unwrap()
                .execute_batch("PRAGMA foreign_keys = OFF;")
                .unwrap();
        }
        let repo = Arc::new(SqliteEmbeddingRepository::new(pool.clone()));
        repo.initialize_tables().unwrap();
        let service = EmbeddingStoreService::new(
            repo,
            Arc::new(DeterministicEmbeddingBackend::new(64)),
            "deterministic",
            "1",
        );
        (pool, service)
    }

    #[tokio::test]
    async fn batch_embed_stores_every_item() {
        let (_pool, service) = build();

        let items = vec![
            (
                "ctx-a".to_string(),
                "alpha".to_string(),
                "function".to_string(),
            ),
            ("ctx-b".to_string(), "beta".to_string(), "class".to_string()),
            ("ctx-c".to_string(), "gamma".to_string(), "file".to_string()),
        ];

        let (stored, failed) = service.embed_and_store_batch(Some("p1"), &items).await;
        assert_eq!(stored, 3);
        assert_eq!(failed, 0);

        for (id, text, content_type) in &items {
            let found = service.get_embedding(id).await.unwrap().unwrap();
            assert_eq!(found.context_id, *id);
            assert_eq!(found.content_hash, content_hash(text));
            assert_eq!(found.content_type.as_deref(), Some(content_type.as_str()));
        }

        // Empty input is a no-op, not an error.
        assert_eq!(service.embed_and_store_batch(Some("p1"), &[]).await, (0, 0));
    }

    #[tokio::test]
    async fn batch_embed_counts_empty_text_as_failure() {
        let (_pool, service) = build();
        // The deterministic backend rejects blank input, so the whole batch
        // fails rather than silently storing junk — and every item is counted.
        let items = vec![
            (
                "ctx-a".to_string(),
                "alpha".to_string(),
                "function".to_string(),
            ),
            (
                "ctx-b".to_string(),
                "   ".to_string(),
                "function".to_string(),
            ),
        ];
        let (stored, failed) = service.embed_and_store_batch(Some("p1"), &items).await;
        assert_eq!(stored, 0);
        assert_eq!(failed, 2);
    }

    #[tokio::test]
    async fn embed_store_and_retrieve() {
        let (_pool, service) = build();
        let stored = service
            .embed_and_store("ctx-1", Some("p1"), "create user account", Some("code"))
            .await
            .unwrap();

        assert_eq!(stored.context_id, "ctx-1");
        assert_eq!(stored.model, "deterministic");
        assert!(!stored.vector.is_empty());

        let fetched = service.get_embedding("ctx-1").await.unwrap().unwrap();
        assert_eq!(fetched.vector, stored.vector);
        assert_eq!(service.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn reembedding_same_key_overwrites() {
        let (_pool, service) = build();
        service
            .embed_and_store("ctx-1", Some("p1"), "alpha beta", None)
            .await
            .unwrap();
        service
            .embed_and_store("ctx-1", Some("p1"), "gamma delta epsilon", None)
            .await
            .unwrap();

        assert_eq!(service.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn search_ranks_similar_content_first() {
        let (_pool, service) = build();
        service
            .embed_and_store("a", Some("p1"), "create user account", None)
            .await
            .unwrap();
        service
            .embed_and_store("b", Some("p1"), "delete invoice record", None)
            .await
            .unwrap();

        let results = service
            .search("create user account", "p1", 2)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].context_id, "a");
        assert!(results[0].similarity >= results[1].similarity);
    }
}
