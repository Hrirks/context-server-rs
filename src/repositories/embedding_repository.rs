use crate::models::embedding::StoredEmbedding;
use async_trait::async_trait;
use rmcp::model::ErrorData as McpError;

/// Persistence for computed embeddings (Phase 4b).
#[async_trait]
#[allow(dead_code)]
pub trait EmbeddingRepository: Send + Sync {
    /// Insert or replace an embedding for its `(context_id, model, version)` key.
    async fn upsert_embedding(&self, embedding: &StoredEmbedding) -> Result<(), McpError>;

    /// Look up a single embedding by its natural key.
    async fn find_embedding(
        &self,
        context_id: &str,
        model: &str,
        version: &str,
    ) -> Result<Option<StoredEmbedding>, McpError>;

    /// Remove every embedding attached to a context item, returning how many were
    /// deleted.
    async fn delete_embeddings_for_context(&self, context_id: &str) -> Result<usize, McpError>;

    /// Load every embedding for a project (used for brute-force cosine search).
    async fn find_embeddings_by_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<StoredEmbedding>, McpError>;

    /// Total number of stored embeddings.
    async fn count(&self) -> Result<u64, McpError>;
}
