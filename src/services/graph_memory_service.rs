//! Graph memory: turn parsed source into a queryable code graph.
//!
//! [`GraphMemoryService`] indexes tree-sitter chunks into `GraphSymbol` nodes
//! and `GraphEdge` relationships (`contains` for structural nesting, `imports`
//! for import declarations), then supports name search and budgeted BFS
//! traversal so an agent can pull a connected fragment of the codebase without
//! reserializing whole files.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;

use rmcp::model::ErrorData as McpError;

use crate::models::embedding::content_hash;
use crate::models::graph::{
    ConversationMemory, EdgeType, GraphEdge, GraphStats, GraphSubgraph, GraphSymbol, IndexReport,
};
use crate::parser::{chunk_file_async, discover_sources, ChunkKind, SemanticChunk, SourceLanguage};
use crate::repositories::GraphRepository;
use crate::services::EmbeddingStoreService;

/// Indexes source into graph memory and answers graph queries.
pub struct GraphMemoryService {
    repository: Arc<dyn GraphRepository>,
    /// Optional embedding store; when present, each symbol's text is embedded
    /// during indexing so it becomes searchable via semantic search.
    embedding_store: Option<Arc<EmbeddingStoreService>>,
}

impl GraphMemoryService {
    pub fn new(
        repository: Arc<dyn GraphRepository>,
        embedding_store: Option<Arc<EmbeddingStoreService>>,
    ) -> Self {
        Self {
            repository,
            embedding_store,
        }
    }

    /// Discover, parse, and index every supported source under `root`.
    ///
    /// The project's previous graph is cleared first, so repeated indexing is
    /// idempotent. Embedding is best-effort: an unavailable embedding backend
    /// degrades to a graph-only index rather than failing the whole run.
    pub async fn index_directory(
        &self,
        project_id: &str,
        root: &Path,
        session_id: &str,
    ) -> Result<IndexReport, McpError> {
        let files = discover_sources(root).map_err(|e| {
            McpError::internal_error(format!("Failed to discover sources: {e}"), None)
        })?;

        self.repository.delete_project(project_id).await?;

        let mut symbols_indexed = 0usize;
        let mut edges_indexed = 0usize;
        let mut embedded = 0usize;
        let mut embed_failures = 0usize;

        for file in &files {
            let Some(language) = SourceLanguage::from_path(file) else {
                continue;
            };
            let file_path = file.display().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            let file_id = symbol_id(project_id, &file_path, &file_path, "file", 0);

            let file_symbol = GraphSymbol {
                id: file_id.clone(),
                project_id: project_id.to_string(),
                file_path: file_path.clone(),
                name: file_path.clone(),
                kind: "file".to_string(),
                language: language.name().to_string(),
                signature: file_path.clone(),
                start_line: 1,
                end_line: 1,
                text: String::new(),
                created_at: now.clone(),
            };
            self.repository.upsert_symbol(&file_symbol).await?;
            symbols_indexed += 1;

            let chunks = chunk_file_async(file.clone()).await.map_err(|e| {
                McpError::internal_error(format!("Failed to parse {file_path}: {e}"), None)
            })?;

            // Build symbol nodes in chunk order, remembering their ids so the
            // `parent` index can be resolved into graph edges below.
            let mut chunk_ids = Vec::with_capacity(chunks.len());
            for chunk in &chunks {
                let name = display_name(chunk);
                let kind = kind_str(chunk.kind);
                let id = symbol_id(project_id, &file_path, &name, kind, chunk.start_line);
                let symbol = GraphSymbol {
                    id: id.clone(),
                    project_id: project_id.to_string(),
                    file_path: file_path.clone(),
                    name,
                    kind: kind.to_string(),
                    language: language.name().to_string(),
                    signature: chunk.signature.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    text: chunk.text.clone(),
                    created_at: now.clone(),
                };
                self.repository.upsert_symbol(&symbol).await?;
                symbols_indexed += 1;
                chunk_ids.push(id);
            }

            // Structural (contains) and import edges.
            for (i, chunk) in chunks.iter().enumerate() {
                let child_id = &chunk_ids[i];
                let (source, edge_type) = if chunk.kind == ChunkKind::Import {
                    (file_id.clone(), EdgeType::Imports)
                } else {
                    let parent = chunk
                        .parent
                        .map(|p| chunk_ids[p].clone())
                        .unwrap_or_else(|| file_id.clone());
                    (parent, EdgeType::Contains)
                };

                let edge = GraphEdge {
                    id: content_hash(&format!("{source}\0{child_id}\0{}", edge_type.as_str())),
                    project_id: project_id.to_string(),
                    source_id: source,
                    target_id: child_id.clone(),
                    edge_type,
                    weight: 1.0,
                    metadata: None,
                    created_at: now.clone(),
                };
                self.repository.upsert_edge(&edge).await?;
                edges_indexed += 1;
            }

            // Best-effort embedding for semantic search.
            if let Some(store) = &self.embedding_store {
                for (i, chunk) in chunks.iter().enumerate() {
                    if chunk.text.trim().is_empty() {
                        embed_failures += 1;
                        continue;
                    }
                    match store
                        .embed_and_store(
                            &chunk_ids[i],
                            Some(project_id),
                            &chunk.text,
                            Some(kind_str(chunk.kind)),
                        )
                        .await
                    {
                        Ok(_) => embedded += 1,
                        Err(_) => embed_failures += 1,
                    }
                }
            }
        }

        self.record_delta(
            session_id,
            Some(project_id),
            "index",
            serde_json::json!({
                "files": files.len(),
                "symbols": symbols_indexed,
                "edges": edges_indexed,
                "embedded": embedded,
            }),
        )
        .await;

        Ok(IndexReport {
            project_id: project_id.to_string(),
            files_indexed: files.len(),
            symbols_indexed,
            edges_indexed,
            embedded,
            embed_failures,
        })
    }

    /// Case-insensitive substring search over symbol names.
    pub async fn search_symbols(
        &self,
        project_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<GraphSymbol>, McpError> {
        self.repository
            .find_symbols_by_name(project_id, query, limit)
            .await
    }

    /// Budgeted breadth-first traversal of the outgoing edge graph.
    ///
    /// `max_depth` bounds hop count; `budget` bounds how many symbol nodes are
    /// returned. A bounded traversal lets an agent fetch the local neighborhood
    /// of a symbol without pulling the whole codebase.
    pub async fn traverse(
        &self,
        project_id: &str,
        start_id: &str,
        max_depth: usize,
        budget: usize,
        session_id: &str,
    ) -> Result<GraphSubgraph, McpError> {
        let mut symbols = Vec::new();
        let mut edges = Vec::new();
        let mut seen_edges = HashSet::new();
        let mut visited = HashSet::new();
        let mut frontier = VecDeque::new();
        frontier.push_back((start_id.to_string(), 0usize));
        let mut budget_exhausted = false;

        while let Some((id, depth)) = frontier.pop_front() {
            if !visited.insert(id.clone()) {
                continue;
            }
            if symbols.len() >= budget {
                budget_exhausted = true;
                break;
            }

            let Some(symbol) = self.repository.find_symbol(&id).await? else {
                continue;
            };
            symbols.push(symbol);

            if depth >= max_depth {
                continue;
            }

            let outgoing = self.repository.find_outgoing_edges(&id).await?;
            for edge in outgoing {
                if seen_edges.insert(edge.id.clone()) {
                    edges.push(edge.clone());
                }
                frontier.push_back((edge.target_id, depth + 1));
            }
        }

        self.record_delta(
            session_id,
            Some(project_id),
            "traverse",
            serde_json::json!({
                "start_id": start_id,
                "nodes": symbols.len(),
                "edges": edges.len(),
            }),
        )
        .await;

        Ok(GraphSubgraph {
            start_id: start_id.to_string(),
            symbols,
            edges,
            budget_exhausted,
        })
    }

    /// Symbol and edge counts for a project.
    pub async fn stats(&self, project_id: &str) -> Result<GraphStats, McpError> {
        self.repository.stats(project_id).await
    }

    /// Recent conversation deltas for a session, newest first.
    pub async fn recent_deltas(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<ConversationMemory>, McpError> {
        self.repository.recent_conversation(session_id, limit).await
    }

    async fn record_delta(
        &self,
        session_id: &str,
        project_id: Option<&str>,
        event_type: &str,
        payload: serde_json::Value,
    ) {
        let memory = ConversationMemory {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            project_id: project_id.map(str::to_string),
            event_type: event_type.to_string(),
            payload,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        if let Err(e) = self.repository.record_conversation(&memory).await {
            tracing::warn!("Failed to record conversation delta: {e}");
        }
    }
}

/// Stable, idempotent id for a symbol node.
fn symbol_id(
    project_id: &str,
    file_path: &str,
    name: &str,
    kind: &str,
    start_line: usize,
) -> String {
    content_hash(&format!(
        "{project_id}\0{file_path}\0{name}\0{kind}\0{start_line}"
    ))
}

fn kind_str(kind: ChunkKind) -> &'static str {
    match kind {
        ChunkKind::Package => "package",
        ChunkKind::Import => "import",
        ChunkKind::Class => "class",
        ChunkKind::Interface => "interface",
        ChunkKind::Enum => "enum",
        ChunkKind::Record => "record",
        ChunkKind::Struct => "struct",
        ChunkKind::Mixin => "mixin",
        ChunkKind::Extension => "extension",
        ChunkKind::Function => "function",
        ChunkKind::Method => "method",
        ChunkKind::Constructor => "constructor",
        ChunkKind::Type => "type",
    }
}

/// The display name for a chunk; derived from the parse when the grammar did
/// not expose a dedicated `name` field (imports and packages).
fn display_name(chunk: &SemanticChunk) -> String {
    if let Some(name) = &chunk.name {
        return name.clone();
    }
    match chunk.kind {
        ChunkKind::Import => import_target(chunk),
        ChunkKind::Package => package_name(chunk),
        _ => chunk.signature.clone(),
    }
}

/// Extract the imported module path from an import declaration.
fn import_target(chunk: &SemanticChunk) -> String {
    // Go and Dart quote the target (`import "fmt"`, `import 'package:...'`).
    if let Some(quoted) = first_quoted(&chunk.text) {
        return quoted;
    }
    // Java has no quotes: `import java.util.List;` / `import static ...;`.
    let s = chunk.text.trim();
    let s = s.strip_suffix(';').unwrap_or(s).trim();
    let s = s.strip_prefix("import").unwrap_or(s).trim();
    s.strip_prefix("static").unwrap_or(s).trim().to_string()
}

/// Extract the package/library name from a package declaration.
fn package_name(chunk: &SemanticChunk) -> String {
    let s = chunk.text.trim();
    let s = s.strip_suffix(';').unwrap_or(s).trim();
    s.strip_prefix("package").unwrap_or(s).trim().to_string()
}

/// The first single- or double-quoted substring, if any.
fn first_quoted(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != quote {
                j += 1;
            }
            if j < bytes.len() {
                return Some(s[i + 1..j].to_string());
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection_pool::ConnectionPool;
    use crate::embedding::DeterministicEmbeddingBackend;
    use crate::infrastructure::{SqliteEmbeddingRepository, SqliteGraphRepository};
    use std::time::Duration;

    fn build(with_embedding: bool) -> GraphMemoryService {
        let pool = Arc::new(ConnectionPool::new(":memory:", 1, Duration::from_secs(1)).unwrap());
        {
            let conn = pool.checkout().unwrap();
            conn.lock()
                .unwrap()
                .execute_batch("PRAGMA foreign_keys = OFF;")
                .unwrap();
        }
        let graph_repo = Arc::new(SqliteGraphRepository::new(pool.clone()));
        graph_repo.initialize_tables().unwrap();

        let embedding_store = if with_embedding {
            let embedding_repo = Arc::new(SqliteEmbeddingRepository::new(pool.clone()));
            embedding_repo.initialize_tables().unwrap();
            Some(Arc::new(EmbeddingStoreService::new(
                embedding_repo,
                Arc::new(DeterministicEmbeddingBackend::new(32)),
                "deterministic",
                "1",
            )))
        } else {
            None
        };

        GraphMemoryService::new(graph_repo, embedding_store)
    }

    fn write_source(dir: &Path, name: &str, source: &str) {
        std::fs::write(dir.join(name), source).unwrap();
    }

    #[tokio::test]
    async fn index_directory_builds_symbols_and_edges() {
        let dir = tempfile::tempdir().unwrap();
        write_source(
            dir.path(),
            "main.go",
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hi\")\n}\n",
        );

        let service = build(false);
        let report = service
            .index_directory("p1", dir.path(), "session-1")
            .await
            .unwrap();

        assert!(report.files_indexed >= 1);
        assert!(report.symbols_indexed >= 3); // file + package + import + function
        assert!(report.edges_indexed >= 2);

        let stats = service.stats("p1").await.unwrap();
        assert!(stats.symbol_count >= 4);
        assert!(stats.edge_count >= 2);
    }

    #[tokio::test]
    async fn traverse_returns_bounded_neighborhood() {
        let dir = tempfile::tempdir().unwrap();
        write_source(
            dir.path(),
            "main.go",
            "package main\n\nfunc outer() {\n\tinner()\n}\n\nfunc inner() {}\n",
        );

        let service = build(false);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let hits = service.search_symbols("p1", "outer", 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        let outer = &hits[0];

        let sub = service.traverse("p1", &outer.id, 2, 20, "s").await.unwrap();
        assert_eq!(sub.start_id, outer.id);
        assert!(!sub.symbols.is_empty());
        assert!(!sub.budget_exhausted);
    }

    #[tokio::test]
    async fn index_with_embedding_stores_vectors() {
        let dir = tempfile::tempdir().unwrap();
        write_source(
            dir.path(),
            "main.go",
            "package main\n\nfunc greet() {\n\tprintln(\"hello\")\n}\n",
        );

        let service = build(true);
        let report = service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        assert!(report.embedded > 0);
        assert_eq!(report.embed_failures, 0);
    }

    #[tokio::test]
    async fn conversation_deltas_are_recorded() {
        let dir = tempfile::tempdir().unwrap();
        write_source(dir.path(), "main.go", "package main\nfunc f() {}\n");

        let service = build(false);
        service
            .index_directory("p1", dir.path(), "sess")
            .await
            .unwrap();

        let deltas = service.recent_deltas("sess", 10).await.unwrap();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].event_type, "index");
    }
}
