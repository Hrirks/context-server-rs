use crate::db::connection_pool::{ConnectionPool, PooledConnection};
use crate::models::embedding::StoredEmbedding;
use crate::repositories::EmbeddingRepository;
use async_trait::async_trait;
use rmcp::model::ErrorData as McpError;
use rusqlite::{params, OptionalExtension, Row};
use std::sync::Arc;

/// SQLite implementation of [`EmbeddingRepository`].
pub struct SqliteEmbeddingRepository {
    pool: Arc<ConnectionPool>,
}

impl SqliteEmbeddingRepository {
    pub fn new(pool: Arc<ConnectionPool>) -> Self {
        Self { pool }
    }

    fn checkout(&self) -> Result<PooledConnection, McpError> {
        self.pool.checkout().map_err(|e| {
            McpError::internal_error(format!("Failed to acquire database connection: {e}"), None)
        })
    }

    /// Create the `context_embeddings` table and its indexes if absent.
    ///
    /// This mirrors the schema declared in `db/init.rs` but is kept here too so
    /// the repository is usable standalone (e.g. in unit tests) without first
    /// running the full database bootstrap.
    pub fn initialize_tables(&self) -> Result<(), McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS context_embeddings (
                id TEXT PRIMARY KEY,
                context_id TEXT NOT NULL,
                project_id TEXT,
                embedding_vector TEXT NOT NULL,
                embedding_model TEXT NOT NULL,
                embedding_version TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                content_type TEXT,
                content_length INTEGER,
                tokenization_method TEXT,
                preprocessing_steps TEXT,
                quality_score REAL,
                custom_metadata TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (project_id) REFERENCES projects(id),
                UNIQUE(context_id, embedding_model, embedding_version)
            );

            CREATE INDEX IF NOT EXISTS idx_embeddings_context_id ON context_embeddings(context_id);
            CREATE INDEX IF NOT EXISTS idx_embeddings_project_id ON context_embeddings(project_id);
            CREATE INDEX IF NOT EXISTS idx_embeddings_model ON context_embeddings(embedding_model);
            CREATE INDEX IF NOT EXISTS idx_embeddings_content_hash ON context_embeddings(content_hash);
            CREATE INDEX IF NOT EXISTS idx_embeddings_created_at ON context_embeddings(created_at);
            "#,
        )
        .map_err(|e| {
            McpError::internal_error(format!("Failed to create context_embeddings table: {e}"), None)
        })
    }
}

const SELECT_COLUMNS: &str = "id, context_id, project_id, embedding_vector, embedding_model, \
     embedding_version, content_hash, content_type, content_length, tokenization_method, \
     preprocessing_steps, quality_score, custom_metadata, created_at, updated_at";

fn row_to_embedding(row: &Row) -> rusqlite::Result<StoredEmbedding> {
    let vector_json: String = row.get(3)?;
    let vector: Vec<f32> = serde_json::from_str(&vector_json).unwrap_or_default();
    let preprocessing_steps: Option<Vec<String>> = row
        .get::<_, Option<String>>(10)?
        .and_then(|s| serde_json::from_str(&s).ok());
    let custom_metadata: Option<serde_json::Value> = row
        .get::<_, Option<String>>(12)?
        .and_then(|s| serde_json::from_str(&s).ok());

    Ok(StoredEmbedding {
        id: row.get(0)?,
        context_id: row.get(1)?,
        project_id: row.get(2)?,
        vector,
        model: row.get(4)?,
        version: row.get(5)?,
        content_hash: row.get(6)?,
        content_type: row.get(7)?,
        content_length: row.get(8)?,
        tokenization_method: row.get(9)?,
        preprocessing_steps,
        quality_score: row.get(11)?,
        custom_metadata,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

#[async_trait]
impl EmbeddingRepository for SqliteEmbeddingRepository {
    async fn upsert_embedding(&self, embedding: &StoredEmbedding) -> Result<(), McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let vector_json = serde_json::to_string(&embedding.vector).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize embedding vector: {e}"), None)
        })?;
        let preprocessing_json = embedding
            .preprocessing_steps
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| {
                McpError::internal_error(
                    format!("Failed to serialize preprocessing steps: {e}"),
                    None,
                )
            })?;
        let metadata_json = embedding
            .custom_metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| {
                McpError::internal_error(format!("Failed to serialize custom metadata: {e}"), None)
            })?;

        db.execute(
            r#"
            INSERT INTO context_embeddings (
                id, context_id, project_id, embedding_vector, embedding_model,
                embedding_version, content_hash, content_type, content_length,
                tokenization_method, preprocessing_steps, quality_score,
                custom_metadata, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(context_id, embedding_model, embedding_version) DO UPDATE SET
                embedding_vector = excluded.embedding_vector,
                content_hash = excluded.content_hash,
                content_type = excluded.content_type,
                content_length = excluded.content_length,
                tokenization_method = excluded.tokenization_method,
                preprocessing_steps = excluded.preprocessing_steps,
                quality_score = excluded.quality_score,
                custom_metadata = excluded.custom_metadata,
                updated_at = excluded.updated_at
            "#,
            params![
                embedding.id,
                embedding.context_id,
                embedding.project_id,
                vector_json,
                embedding.model,
                embedding.version,
                embedding.content_hash,
                embedding.content_type,
                embedding.content_length,
                embedding.tokenization_method,
                preprocessing_json,
                embedding.quality_score,
                metadata_json,
                embedding.created_at,
                embedding.updated_at,
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to upsert embedding: {e}"), None))?;

        Ok(())
    }

    async fn find_embedding(
        &self,
        context_id: &str,
        model: &str,
        version: &str,
    ) -> Result<Option<StoredEmbedding>, McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let query = format!(
            "SELECT {SELECT_COLUMNS} FROM context_embeddings \
             WHERE context_id = ?1 AND embedding_model = ?2 AND embedding_version = ?3"
        );
        let result = db
            .query_row(
                &query,
                params![context_id, model, version],
                row_to_embedding,
            )
            .optional()
            .map_err(|e| {
                McpError::internal_error(format!("Failed to find embedding: {e}"), None)
            })?;

        Ok(result)
    }

    async fn delete_embeddings_for_context(&self, context_id: &str) -> Result<usize, McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let deleted = db
            .execute(
                "DELETE FROM context_embeddings WHERE context_id = ?1",
                params![context_id],
            )
            .map_err(|e| {
                McpError::internal_error(format!("Failed to delete embeddings: {e}"), None)
            })?;

        Ok(deleted)
    }

    async fn find_embeddings_by_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<StoredEmbedding>, McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let query =
            format!("SELECT {SELECT_COLUMNS} FROM context_embeddings WHERE project_id = ?1");
        let mut stmt = db
            .prepare(&query)
            .map_err(|e| McpError::internal_error(format!("Failed to prepare query: {e}"), None))?;

        let embeddings = stmt
            .query_map(params![project_id], row_to_embedding)
            .map_err(|e| {
                McpError::internal_error(format!("Failed to query embeddings: {e}"), None)
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                McpError::internal_error(format!("Failed to read embeddings: {e}"), None)
            })?;

        Ok(embeddings)
    }

    async fn count(&self) -> Result<u64, McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM context_embeddings", [], |row| {
                row.get(0)
            })
            .map_err(|e| {
                McpError::internal_error(format!("Failed to count embeddings: {e}"), None)
            })?;

        Ok(count as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn test_pool() -> Arc<ConnectionPool> {
        let pool = ConnectionPool::new(":memory:", 1, Duration::from_secs(1)).unwrap();
        // The FK to projects(id) is irrelevant here; disable enforcement so the
        // tests don't need a parent projects table.
        {
            let conn = pool.checkout().unwrap();
            conn.lock()
                .unwrap()
                .execute_batch("PRAGMA foreign_keys = OFF;")
                .unwrap();
        }
        Arc::new(pool)
    }

    fn sample_embedding(context_id: &str) -> StoredEmbedding {
        StoredEmbedding {
            id: format!("id-{context_id}"),
            context_id: context_id.to_string(),
            project_id: Some("p1".to_string()),
            vector: vec![0.1, 0.2, 0.3],
            model: "test-model".to_string(),
            version: "1".to_string(),
            content_hash: "abc".to_string(),
            content_type: Some("code".to_string()),
            content_length: Some(3),
            tokenization_method: None,
            preprocessing_steps: None,
            quality_score: Some(0.9),
            custom_metadata: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: None,
        }
    }

    #[tokio::test]
    async fn upsert_and_find_roundtrip() {
        let repo = SqliteEmbeddingRepository::new(test_pool());
        repo.initialize_tables().unwrap();

        let e = sample_embedding("ctx-1");
        repo.upsert_embedding(&e).await.unwrap();

        let found = repo
            .find_embedding("ctx-1", "test-model", "1")
            .await
            .unwrap()
            .expect("should be found");
        assert_eq!(found.context_id, "ctx-1");
        assert_eq!(found.vector, vec![0.1, 0.2, 0.3]);
    }

    #[tokio::test]
    async fn upsert_replaces_existing_key() {
        let repo = SqliteEmbeddingRepository::new(test_pool());
        repo.initialize_tables().unwrap();

        let mut e = sample_embedding("ctx-1");
        repo.upsert_embedding(&e).await.unwrap();

        e.vector = vec![0.9, 0.8, 0.7];
        repo.upsert_embedding(&e).await.unwrap();

        assert_eq!(repo.count().await.unwrap(), 1);
        let found = repo
            .find_embedding("ctx-1", "test-model", "1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.vector, vec![0.9, 0.8, 0.7]);
    }

    #[tokio::test]
    async fn delete_by_context_removes_only_that_context() {
        let repo = SqliteEmbeddingRepository::new(test_pool());
        repo.initialize_tables().unwrap();

        repo.upsert_embedding(&sample_embedding("ctx-1"))
            .await
            .unwrap();
        repo.upsert_embedding(&sample_embedding("ctx-2"))
            .await
            .unwrap();

        let deleted = repo.delete_embeddings_for_context("ctx-1").await.unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(repo.count().await.unwrap(), 1);
        assert!(repo
            .find_embedding("ctx-1", "test-model", "1")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn find_by_project_filters_correctly() {
        let repo = SqliteEmbeddingRepository::new(test_pool());
        repo.initialize_tables().unwrap();

        let mut e1 = sample_embedding("ctx-1");
        e1.project_id = Some("p1".to_string());
        let mut e2 = sample_embedding("ctx-2");
        e2.project_id = Some("p2".to_string());

        repo.upsert_embedding(&e1).await.unwrap();
        repo.upsert_embedding(&e2).await.unwrap();

        let p1 = repo.find_embeddings_by_project("p1").await.unwrap();
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].context_id, "ctx-1");
    }
}
