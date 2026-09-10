//! Persistence service for semantic embeddings.
//!
//! This is distinct from [`crate::embedding::EmbeddingService`], which only
//! *computes* vectors. [`EmbeddingStoreService`] combines a compute backend with
//! an [`EmbeddingRepository`] so callers can embed text, persist the result, and
//! run brute-force cosine search over stored vectors — all behind one seam.

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

    /// Brute-force cosine search over a project's stored embeddings.
    ///
    /// At the ~50k-vector scale this engine targets, a full linear scan is a
    /// few milliseconds, so no approximate vector index is needed yet.
    pub async fn search(
        &self,
        query: &str,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<EmbeddingSearchResult>, McpError> {
        let query_vector = self.backend.embed(query).await.map_err(|e| {
            McpError::internal_error(format!("Embedding backend failed: {e}"), None)
        })?;

        let embeddings = self
            .repository
            .find_embeddings_by_project(project_id)
            .await?;

        let mut scored: Vec<EmbeddingSearchResult> = embeddings
            .into_iter()
            .map(|e| EmbeddingSearchResult {
                similarity: cosine_similarity(&query_vector, &e.vector),
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
