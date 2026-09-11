use crate::db::connection_pool::{ConnectionPool, PooledConnection};
use crate::models::graph::{ConversationMemory, EdgeType, GraphEdge, GraphStats, GraphSymbol};
use crate::repositories::GraphRepository;
use async_trait::async_trait;
use rmcp::model::ErrorData as McpError;
use rusqlite::{params, OptionalExtension, Row};
use std::sync::Arc;

/// SQLite implementation of [`GraphRepository`].
pub struct SqliteGraphRepository {
    pool: Arc<ConnectionPool>,
}

impl SqliteGraphRepository {
    pub fn new(pool: Arc<ConnectionPool>) -> Self {
        Self { pool }
    }

    fn checkout(&self) -> Result<PooledConnection, McpError> {
        self.pool.checkout().map_err(|e| {
            McpError::internal_error(format!("Failed to acquire database connection: {e}"), None)
        })
    }

    /// Create the graph tables if absent.
    pub fn initialize_tables(&self) -> Result<(), McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS context_symbols (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                language TEXT NOT NULL,
                signature TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                text TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_symbols_project ON context_symbols(project_id);
            CREATE INDEX IF NOT EXISTS idx_symbols_file ON context_symbols(file_path);
            CREATE INDEX IF NOT EXISTS idx_symbols_name ON context_symbols(name);

            CREATE TABLE IF NOT EXISTS symbol_edges (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                metadata TEXT,
                created_at TEXT NOT NULL,
                UNIQUE(source_id, target_id, edge_type)
            );

            CREATE INDEX IF NOT EXISTS idx_edges_source ON symbol_edges(source_id);
            CREATE INDEX IF NOT EXISTS idx_edges_target ON symbol_edges(target_id);
            CREATE INDEX IF NOT EXISTS idx_edges_type ON symbol_edges(edge_type);

            CREATE TABLE IF NOT EXISTS conversation_memory (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                project_id TEXT,
                event_type TEXT NOT NULL,
                payload TEXT,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_conversation_session ON conversation_memory(session_id, created_at);
            "#,
        )
        .map_err(|e| McpError::internal_error(format!("Failed to create graph tables: {e}"), None))
    }
}

const SYMBOL_COLUMNS: &str =
    "id, project_id, file_path, name, kind, language, signature, start_line, end_line, text, created_at";

fn row_to_symbol(row: &Row) -> rusqlite::Result<GraphSymbol> {
    Ok(GraphSymbol {
        id: row.get(0)?,
        project_id: row.get(1)?,
        file_path: row.get(2)?,
        name: row.get(3)?,
        kind: row.get(4)?,
        language: row.get(5)?,
        signature: row.get(6)?,
        start_line: row.get::<_, i64>(7)? as usize,
        end_line: row.get::<_, i64>(8)? as usize,
        text: row.get(9)?,
        created_at: row.get(10)?,
    })
}

fn row_to_edge(row: &Row) -> rusqlite::Result<GraphEdge> {
    let edge_type_str: String = row.get(4)?;
    // Only known values are ever written; fall back harmlessly if a future
    // writer adds a type this build predates.
    let edge_type = EdgeType::parse(&edge_type_str).unwrap_or(EdgeType::Mentions);

    Ok(GraphEdge {
        id: row.get(0)?,
        project_id: row.get(1)?,
        source_id: row.get(2)?,
        target_id: row.get(3)?,
        edge_type,
        weight: row.get(5)?,
        metadata: row
            .get::<_, Option<String>>(6)?
            .and_then(|s| serde_json::from_str(&s).ok()),
        created_at: row.get(7)?,
    })
}

fn row_to_memory(row: &Row) -> rusqlite::Result<ConversationMemory> {
    Ok(ConversationMemory {
        id: row.get(0)?,
        session_id: row.get(1)?,
        project_id: row.get(2)?,
        event_type: row.get(3)?,
        payload: row
            .get::<_, Option<String>>(4)?
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::Value::Null),
        created_at: row.get(5)?,
    })
}

#[async_trait]
impl GraphRepository for SqliteGraphRepository {
    async fn upsert_symbol(&self, symbol: &GraphSymbol) -> Result<(), McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        db.execute(
            r#"
            INSERT INTO context_symbols (
                id, project_id, file_path, name, kind, language, signature,
                start_line, end_line, text, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                signature = excluded.signature,
                text = excluded.text,
                end_line = excluded.end_line
            "#,
            params![
                symbol.id,
                symbol.project_id,
                symbol.file_path,
                symbol.name,
                symbol.kind,
                symbol.language,
                symbol.signature,
                symbol.start_line as i64,
                symbol.end_line as i64,
                symbol.text,
                symbol.created_at,
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to upsert symbol: {e}"), None))?;

        Ok(())
    }

    async fn upsert_edge(&self, edge: &GraphEdge) -> Result<(), McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let metadata_json = edge
            .metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| {
                McpError::internal_error(format!("Failed to serialize edge metadata: {e}"), None)
            })?;

        db.execute(
            r#"
            INSERT INTO symbol_edges (
                id, project_id, source_id, target_id, edge_type, weight, metadata, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(source_id, target_id, edge_type) DO UPDATE SET
                weight = excluded.weight,
                metadata = excluded.metadata
            "#,
            params![
                edge.id,
                edge.project_id,
                edge.source_id,
                edge.target_id,
                edge.edge_type.as_str(),
                edge.weight,
                metadata_json,
                edge.created_at,
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to upsert edge: {e}"), None))?;

        Ok(())
    }

    async fn find_symbol(&self, id: &str) -> Result<Option<GraphSymbol>, McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let query = format!("SELECT {SYMBOL_COLUMNS} FROM context_symbols WHERE id = ?1");
        let result = db
            .query_row(&query, params![id], row_to_symbol)
            .optional()
            .map_err(|e| McpError::internal_error(format!("Failed to find symbol: {e}"), None))?;

        Ok(result)
    }

    async fn find_symbols_by_name(
        &self,
        project_id: &str,
        name: &str,
        limit: usize,
    ) -> Result<Vec<GraphSymbol>, McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let pattern = format!("%{name}%");
        let query = format!(
            "SELECT {SYMBOL_COLUMNS} FROM context_symbols \
             WHERE project_id = ?1 AND name LIKE ?2 COLLATE NOCASE ORDER BY name LIMIT ?3"
        );
        let mut stmt = db.prepare(&query).map_err(|e| {
            McpError::internal_error(format!("Failed to prepare symbol search: {e}"), None)
        })?;

        let symbols = stmt
            .query_map(params![project_id, pattern, limit as i64], row_to_symbol)
            .map_err(|e| McpError::internal_error(format!("Failed to search symbols: {e}"), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Failed to read symbols: {e}"), None))?;

        Ok(symbols)
    }

    async fn find_outgoing_edges(&self, symbol_id: &str) -> Result<Vec<GraphEdge>, McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let mut stmt = db
            .prepare(
                "SELECT id, project_id, source_id, target_id, edge_type, weight, metadata, created_at \
                 FROM symbol_edges WHERE source_id = ?1",
            )
            .map_err(|e| McpError::internal_error(format!("Failed to prepare edge query: {e}"), None))?;

        let edges = stmt
            .query_map(params![symbol_id], row_to_edge)
            .map_err(|e| McpError::internal_error(format!("Failed to query edges: {e}"), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Failed to read edges: {e}"), None))?;

        Ok(edges)
    }

    async fn delete_project(&self, project_id: &str) -> Result<(), McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        db.execute(
            "DELETE FROM symbol_edges WHERE project_id = ?1",
            params![project_id],
        )
        .map_err(|e| {
            McpError::internal_error(format!("Failed to delete project edges: {e}"), None)
        })?;
        db.execute(
            "DELETE FROM context_symbols WHERE project_id = ?1",
            params![project_id],
        )
        .map_err(|e| {
            McpError::internal_error(format!("Failed to delete project symbols: {e}"), None)
        })?;

        Ok(())
    }

    async fn list_symbols(&self, project_id: &str) -> Result<Vec<GraphSymbol>, McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let query = format!("SELECT {SYMBOL_COLUMNS} FROM context_symbols WHERE project_id = ?1");
        let mut stmt = db.prepare(&query).map_err(|e| {
            McpError::internal_error(format!("Failed to prepare symbol list: {e}"), None)
        })?;

        let symbols = stmt
            .query_map(params![project_id], row_to_symbol)
            .map_err(|e| McpError::internal_error(format!("Failed to list symbols: {e}"), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Failed to read symbols: {e}"), None))?;

        Ok(symbols)
    }

    async fn delete_symbols_for_file(
        &self,
        project_id: &str,
        file_path: &str,
    ) -> Result<Vec<String>, McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let ids: Vec<String> = {
            let mut stmt = db
                .prepare("SELECT id FROM context_symbols WHERE project_id = ?1 AND file_path = ?2")
                .map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to prepare symbol id query: {e}"),
                        None,
                    )
                })?;
            let rows = stmt
                .query_map(params![project_id, file_path], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|e| {
                    McpError::internal_error(format!("Failed to query symbol ids: {e}"), None)
                })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|e| {
                McpError::internal_error(format!("Failed to read symbol ids: {e}"), None)
            })?
        };

        db.execute(
            "DELETE FROM symbol_edges \
             WHERE source_id IN (SELECT id FROM context_symbols WHERE project_id = ?1 AND file_path = ?2) \
                OR target_id IN (SELECT id FROM context_symbols WHERE project_id = ?1 AND file_path = ?2)",
            params![project_id, file_path],
        )
        .map_err(|e| {
            McpError::internal_error(format!("Failed to delete file edges: {e}"), None)
        })?;

        db.execute(
            "DELETE FROM context_symbols WHERE project_id = ?1 AND file_path = ?2",
            params![project_id, file_path],
        )
        .map_err(|e| {
            McpError::internal_error(format!("Failed to delete file symbols: {e}"), None)
        })?;

        Ok(ids)
    }

    async fn stats(&self, project_id: &str) -> Result<GraphStats, McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let symbol_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM context_symbols WHERE project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .map_err(|e| McpError::internal_error(format!("Failed to count symbols: {e}"), None))?;
        let edge_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM symbol_edges WHERE project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .map_err(|e| McpError::internal_error(format!("Failed to count edges: {e}"), None))?;

        Ok(GraphStats {
            project_id: project_id.to_string(),
            symbol_count: symbol_count as u64,
            edge_count: edge_count as u64,
        })
    }

    async fn record_conversation(&self, memory: &ConversationMemory) -> Result<(), McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let payload_json =
            serde_json::to_string(&memory.payload).unwrap_or_else(|_| "null".to_string());

        db.execute(
            "INSERT INTO conversation_memory (id, session_id, project_id, event_type, payload, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                memory.id,
                memory.session_id,
                memory.project_id,
                memory.event_type,
                payload_json,
                memory.created_at,
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to record conversation: {e}"), None))?;

        Ok(())
    }

    async fn recent_conversation(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<ConversationMemory>, McpError> {
        let conn = self.checkout()?;
        let db = conn.lock().unwrap();

        let mut stmt = db
            .prepare(
                "SELECT id, session_id, project_id, event_type, payload, created_at \
                 FROM conversation_memory WHERE session_id = ?1 ORDER BY created_at DESC LIMIT ?2",
            )
            .map_err(|e| {
                McpError::internal_error(format!("Failed to prepare conversation query: {e}"), None)
            })?;

        let memories = stmt
            .query_map(params![session_id, limit as i64], row_to_memory)
            .map_err(|e| {
                McpError::internal_error(format!("Failed to query conversation: {e}"), None)
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                McpError::internal_error(format!("Failed to read conversation: {e}"), None)
            })?;

        Ok(memories)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn test_repo() -> SqliteGraphRepository {
        let pool = ConnectionPool::new(":memory:", 1, Duration::from_secs(1)).unwrap();
        let repo = SqliteGraphRepository::new(Arc::new(pool));
        repo.initialize_tables().unwrap();
        repo
    }

    fn symbol(id: &str, project: &str, name: &str) -> GraphSymbol {
        GraphSymbol {
            id: id.to_string(),
            project_id: project.to_string(),
            file_path: "src/main.rs".to_string(),
            name: name.to_string(),
            kind: "function".to_string(),
            language: "go".to_string(),
            signature: format!("fn {name}()"),
            start_line: 1,
            end_line: 2,
            text: format!("fn {name}() {{}}"),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn edge(id: &str, project: &str, source: &str, target: &str, ty: EdgeType) -> GraphEdge {
        GraphEdge {
            id: id.to_string(),
            project_id: project.to_string(),
            source_id: source.to_string(),
            target_id: target.to_string(),
            edge_type: ty,
            weight: 1.0,
            metadata: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn symbol_roundtrip_and_search() {
        let repo = test_repo();
        repo.upsert_symbol(&symbol("s1", "p1", "handleRequest"))
            .await
            .unwrap();
        repo.upsert_symbol(&symbol("s2", "p1", "handleResponse"))
            .await
            .unwrap();

        let found = repo.find_symbol("s1").await.unwrap().unwrap();
        assert_eq!(found.name, "handleRequest");

        let hits = repo.find_symbols_by_name("p1", "handle", 10).await.unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[tokio::test]
    async fn edge_roundtrip_and_stats() {
        let repo = test_repo();
        repo.upsert_symbol(&symbol("s1", "p1", "a")).await.unwrap();
        repo.upsert_symbol(&symbol("s2", "p1", "b")).await.unwrap();
        repo.upsert_edge(&edge("e1", "p1", "s1", "s2", EdgeType::Contains))
            .await
            .unwrap();

        let edges = repo.find_outgoing_edges("s1").await.unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_type, EdgeType::Contains);

        let stats = repo.stats("p1").await.unwrap();
        assert_eq!(stats.symbol_count, 2);
        assert_eq!(stats.edge_count, 1);
    }

    #[tokio::test]
    async fn delete_project_clears_graph() {
        let repo = test_repo();
        repo.upsert_symbol(&symbol("s1", "p1", "a")).await.unwrap();
        repo.upsert_edge(&edge("e1", "p1", "s1", "s1", EdgeType::Contains))
            .await
            .unwrap();

        repo.delete_project("p1").await.unwrap();
        let stats = repo.stats("p1").await.unwrap();
        assert_eq!(stats.symbol_count, 0);
        assert_eq!(stats.edge_count, 0);
    }

    #[tokio::test]
    async fn conversation_memory_roundtrip() {
        let repo = test_repo();
        let mem = ConversationMemory {
            id: "m1".to_string(),
            session_id: "session-1".to_string(),
            project_id: Some("p1".to_string()),
            event_type: "index".to_string(),
            payload: serde_json::json!({"symbols": 5}),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        repo.record_conversation(&mem).await.unwrap();

        let recent = repo.recent_conversation("session-1", 10).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].event_type, "index");
    }
}
