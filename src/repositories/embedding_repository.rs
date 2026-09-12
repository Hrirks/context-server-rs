use crate::models::embedding::{EmbeddingSearchResult, StoredEmbedding};
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

    /// Rank a project's embeddings against `query_vector` entirely in SQL via
    /// the sqlite-vec `vec_distance_cosine()` function.
    ///
    /// This keeps every vector in SQLite instead of materializing them all as
    /// Rust structs, so ranking is both cheaper and allocation-free. Callers
    /// should fall back to [`find_embeddings_by_project`] when this returns an
    /// error (for example when the extension is unavailable).
    async fn search_similar_vectors(
        &self,
        project_id: &str,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<EmbeddingSearchResult>, McpError>;

    /// Total number of stored embeddings.
    async fn count(&self) -> Result<u64, McpError>;
}
