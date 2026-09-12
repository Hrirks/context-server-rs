//! Graph memory: turn parsed source into a queryable code graph.
//!
//! [`GraphMemoryService`] indexes tree-sitter chunks into `GraphSymbol` nodes
//! and `GraphEdge` relationships (`contains` for structural nesting, `imports`
//! for import declarations, plus `calls`, `inherits`, and `references` resolved
//! from name mentions), then supports name search and budgeted BFS traversal so
//! an agent can pull a connected fragment of the codebase without reserializing
//! whole files.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::model::ErrorData as McpError;

use crate::models::embedding::content_hash;
use crate::models::graph::{
    ConversationMemory, EdgeType, GraphEdge, GraphStats, GraphSubgraph, GraphSymbol, IndexReport,
    SymbolOutline, SymbolSource,
};
use crate::parser::{
    chunk_file_async, discover_sources, ChunkKind, ReferenceKind, SemanticChunk, SourceLanguage,
};
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
    /// Incremental: each file is hashed and only new/changed files are re-parsed
    /// and re-embedded; unchanged files are skipped and deleted files are pruned.
    /// Cross-file call/inherit/reference edges are resolved against the full
    /// project symbol set. Embedding is best-effort: an unavailable backend
    /// degrades to a graph-only index rather than failing the whole run.
    ///
    /// Note: when a file changes, edges that point *into* it from unchanged files
    /// are dropped (those source files are not re-parsed) and restored the next
    /// time those source files change.
    pub async fn index_directory(
        &self,
        project_id: &str,
        root: &Path,
        session_id: &str,
    ) -> Result<IndexReport, McpError> {
        let files = discover_sources(root).map_err(|e| {
            McpError::internal_error(format!("Failed to discover sources: {e}"), None)
        })?;

        // Stable content hash per discovered file. `None` means the file could
        // not be read; it is treated as changed so the parse path surfaces the
        // real error.
        let mut file_hashes: HashMap<String, Option<String>> = HashMap::new();
        for file in &files {
            let path = file.display().to_string();
            let hash = match tokio::fs::read_to_string(file).await {
                Ok(content) => Some(content_hash(&content)),
                Err(_) => None,
            };
            file_hashes.insert(path, hash);
        }

        // Existing file-level state: file symbols carry their last content hash
        // in `text`, so changed/unchanged is decidable without re-parsing.
        let existing = self.repository.list_symbols(project_id).await?;
        let prev_file_hash: HashMap<String, String> = existing
            .iter()
            .filter(|s| s.kind == "file")
            .map(|s| (s.file_path.clone(), s.text.clone()))
            .collect();

        let current_paths: HashSet<String> =
            files.iter().map(|f| f.display().to_string()).collect();

        let mut changed_files: Vec<PathBuf> = Vec::new();
        let mut unchanged_paths: HashSet<String> = HashSet::new();
        let mut files_skipped = 0usize;
        let mut files_removed = 0usize;

        for file in &files {
            let path = file.display().to_string();
            let unchanged = match (
                file_hashes.get(&path).and_then(|h| h.clone()),
                prev_file_hash.get(&path),
            ) {
                (Some(current), Some(prev)) => current == *prev,
                _ => false,
            };
            if unchanged {
                unchanged_paths.insert(path);
                files_skipped += 1;
            } else {
                changed_files.push(file.clone());
            }
        }

        let mut deleted_paths: Vec<String> = Vec::new();
        for path in prev_file_hash.keys() {
            if !current_paths.contains(path) {
                deleted_paths.push(path.clone());
            }
        }

        // Prune symbols/edges/embeddings for deleted and changed files.
        for path in &deleted_paths {
            let removed = self
                .repository
                .delete_symbols_for_file(project_id, path)
                .await?;
            self.delete_embeddings(&removed).await;
            files_removed += 1;
        }
        for file in &changed_files {
            let path = file.display().to_string();
            let removed = self
                .repository
                .delete_symbols_for_file(project_id, &path)
                .await?;
            self.delete_embeddings(&removed).await;
        }

        // Seed the name index from unchanged files so cross-file references in
        // newly indexed files resolve against the rest of the project.
        let mut name_index: HashMap<String, Vec<String>> = HashMap::new();
        for sym in &existing {
            if unchanged_paths.contains(&sym.file_path) && kind_indexable(&sym.kind) {
                name_index
                    .entry(sym.name.to_lowercase())
                    .or_default()
                    .push(sym.id.clone());
            }
        }

        let mut symbols_indexed = 0usize;
        let mut edges_indexed = 0usize;
        let mut embedded = 0usize;
        let mut embed_failures = 0usize;

        // References whose target id cannot be resolved until the whole project
        // has been indexed (a call may target a symbol defined in another file).
        let mut deferred: Vec<DeferredEdge> = Vec::new();

        for file in &changed_files {
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
                text: file_hashes
                    .get(&file_path)
                    .and_then(|h| h.clone())
                    .unwrap_or_default(),
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
                let name_key = name.to_lowercase();
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
                if indexable_kind(chunk.kind) {
                    name_index.entry(name_key).or_default().push(id.clone());
                }
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

                // Defer call/inherit/reference resolution until every symbol in
                // the project has been indexed (targets may live in other files).
                for reference in &chunk.references {
                    let edge_type = match reference.kind {
                        ReferenceKind::Call => EdgeType::Calls,
                        ReferenceKind::Inherit => EdgeType::Inherits,
                        ReferenceKind::Reference => EdgeType::References,
                    };
                    deferred.push(DeferredEdge {
                        source_id: chunk_ids[i].clone(),
                        edge_type,
                        target_name: reference.name.clone(),
                        created_at: now.clone(),
                    });
                }
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

        // Resolve deferred call/inherit/reference names against the full
        // project name index, emitting one edge per candidate target. A soft
        // cap keeps a very common name from producing an edge storm.
        for deferred_edge in &deferred {
            let Some(targets) = name_index.get(&deferred_edge.target_name.to_lowercase()) else {
                continue;
            };
            for target_id in targets.iter().take(MAX_EDGE_TARGETS) {
                if *target_id == deferred_edge.source_id {
                    continue;
                }
                let edge = GraphEdge {
                    id: content_hash(&format!(
                        "{}\0{}\0{}",
                        deferred_edge.source_id,
                        target_id,
                        deferred_edge.edge_type.as_str()
                    )),
                    project_id: project_id.to_string(),
                    source_id: deferred_edge.source_id.clone(),
                    target_id: target_id.clone(),
                    edge_type: deferred_edge.edge_type,
                    weight: 1.0,
                    metadata: None,
                    created_at: deferred_edge.created_at.clone(),
                };
                self.repository.upsert_edge(&edge).await?;
                edges_indexed += 1;
            }
        }

        self.record_delta(
            session_id,
            Some(project_id),
            "index",
            serde_json::json!({
                "files_indexed": changed_files.len(),
                "files_skipped": files_skipped,
                "files_removed": files_removed,
                "symbols": symbols_indexed,
                "edges": edges_indexed,
                "embedded": embedded,
            }),
        )
        .await;

        Ok(IndexReport {
            project_id: project_id.to_string(),
            files_indexed: changed_files.len(),
            files_skipped,
            files_removed,
            symbols_indexed,
            edges_indexed,
            embedded,
            embed_failures,
        })
    }

    /// Token-efficient structural outline of one indexed file.
    ///
    /// Returns every symbol in `file_path` (excluding the synthetic `file` node)
    /// with its kind, signature, and line range but *not* its body, ordered by
    /// position. This lets an agent see a file's shape for a few dozen tokens
    /// instead of reading the whole file.
    pub async fn file_outline(
        &self,
        project_id: &str,
        file_path: &str,
    ) -> Result<Vec<SymbolOutline>, McpError> {
        let symbols = self
            .repository
            .list_symbols_for_file(project_id, file_path)
            .await?;

        let outlines = symbols
            .iter()
            .filter(|symbol| symbol.kind != "file")
            .map(SymbolOutline::from)
            .collect();

        Ok(outlines)
    }

    /// Exact source for a single symbol, sliced from its file by line range.
    ///
    /// Line ranges are 1-based and inclusive. When the file can no longer be
    /// read (moved, deleted, permissions) this falls back to the symbol text
    /// captured at index time.
    pub async fn symbol_source(
        &self,
        project_id: &str,
        symbol_id: &str,
    ) -> Result<Option<SymbolSource>, McpError> {
        let Some(symbol) = self.repository.find_symbol(symbol_id).await? else {
            return Ok(None);
        };
        if symbol.project_id != project_id {
            return Ok(None);
        }

        let source = match tokio::fs::read_to_string(&symbol.file_path).await {
            Ok(content) => slice_lines(&content, symbol.start_line, symbol.end_line),
            Err(_) => symbol.text.clone(),
        };

        Ok(Some(SymbolSource {
            id: symbol.id,
            name: symbol.name,
            kind: symbol.kind,
            file_path: symbol.file_path,
            language: symbol.language,
            signature: symbol.signature,
            start_line: symbol.start_line,
            end_line: symbol.end_line,
            source,
        }))
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

    async fn delete_embeddings(&self, ids: &[String]) {
        if let Some(store) = &self.embedding_store {
            for id in ids {
                if let Err(e) = store.delete_for_context(id).await {
                    tracing::warn!("Failed to delete embedding for {id}: {e}");
                }
            }
        }
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

/// A call/inherit/reference name to resolve into an edge after indexing.
struct DeferredEdge {
    source_id: String,
    edge_type: EdgeType,
    target_name: String,
    created_at: String,
}

/// Upper bound on how many same-named symbols a single reference links to.
const MAX_EDGE_TARGETS: usize = 25;

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

/// Whether a chunk kind names something that can be the target of a
/// call/inherit/reference edge. Imports and packages are excluded: they are
/// not callable or referenceable types.
fn indexable_kind(kind: ChunkKind) -> bool {
    !matches!(kind, ChunkKind::Package | ChunkKind::Import)
}

/// Whether a stored symbol kind can be the target of a call/inherit/reference
/// edge. Mirrors [`indexable_kind`] for the string kinds persisted in the graph.
fn kind_indexable(kind: &str) -> bool {
    !matches!(kind, "file" | "package" | "import")
}

/// Slice 1-based inclusive line range `start..=end` out of `source`.
///
/// `start == 0` (or an otherwise unusable range) yields the whole source, which
/// keeps the caller from returning an empty body for unset line numbers.
fn slice_lines(source: &str, start_line: usize, end_line: usize) -> String {
    if start_line == 0 || end_line < start_line {
        return source.to_string();
    }
    source
        .lines()
        .skip(start_line - 1)
        .take(end_line - start_line + 1)
        .collect::<Vec<_>>()
        .join("\n")
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
    async fn index_resolves_calls_inherits_and_references_edges() {
        let dir = tempfile::tempdir().unwrap();
        write_source(
            dir.path(),
            "App.java",
            "class WidgetBase {}\nclass User {}\nclass AlphaService extends WidgetBase {\n  User fetch() { helper(); return new User(); }\n  void helper() {}\n}\n",
        );

        let service = build(false);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        // AlphaService inherits WidgetBase.
        let alpha = &service
            .search_symbols("p1", "AlphaService", 10)
            .await
            .unwrap()[0];
        let sub = service.traverse("p1", &alpha.id, 1, 50, "s").await.unwrap();
        let names: Vec<&str> = sub.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"WidgetBase"),
            "expected inherits edge, got {names:?}"
        );

        // fetch() calls helper() and references User.
        let fetch = &service.search_symbols("p1", "fetch", 10).await.unwrap()[0];
        let sub = service.traverse("p1", &fetch.id, 1, 50, "s").await.unwrap();
        let names: Vec<&str> = sub.symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"helper"),
            "expected calls edge, got {names:?}"
        );
        assert!(
            names.contains(&"User"),
            "expected references edge, got {names:?}"
        );
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

    #[tokio::test]
    async fn incremental_index_skips_unchanged_and_prunes_deleted() {
        let dir = tempfile::tempdir().unwrap();
        write_source(dir.path(), "a.go", "package main\nfunc a() {}\n");
        write_source(dir.path(), "b.go", "package main\nfunc b() {}\n");

        let service = build(false);

        let first = service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();
        assert_eq!(first.files_indexed, 2);
        assert_eq!(first.files_skipped, 0);
        assert_eq!(first.files_removed, 0);

        let second = service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();
        assert_eq!(second.files_indexed, 0);
        assert_eq!(second.files_skipped, 2);
        assert_eq!(second.files_removed, 0);

        write_source(dir.path(), "a.go", "package main\nfunc aChanged() {}\n");
        std::fs::remove_file(dir.path().join("b.go")).unwrap();

        let third = service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();
        assert_eq!(third.files_indexed, 1);
        assert_eq!(third.files_skipped, 0);
        assert_eq!(third.files_removed, 1);

        let stats = service.stats("p1").await.unwrap();
        assert_eq!(stats.symbol_count, 3); // file + package + function (aChanged)
    }

    #[tokio::test]
    async fn file_outline_lists_symbols_without_bodies() {
        let dir = tempfile::tempdir().unwrap();
        write_source(
            dir.path(),
            "svc.go",
            "package main\n\nfunc alpha() {\n\tprintln(\"a\")\n}\n\nfunc beta() {}\n",
        );

        let service = build(false);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let file_path = dir.path().join("svc.go").display().to_string();
        let outline = service.file_outline("p1", &file_path).await.unwrap();

        let names: Vec<&str> = outline.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
        // The synthetic `file` node is excluded from outlines.
        assert!(!names.iter().any(|n| *n == file_path));
        // Ordered by position.
        let alpha = outline.iter().find(|s| s.name == "alpha").unwrap();
        let beta = outline.iter().find(|s| s.name == "beta").unwrap();
        assert!(alpha.start_line < beta.start_line);
    }

    #[tokio::test]
    async fn symbol_source_slices_exact_lines() {
        let dir = tempfile::tempdir().unwrap();
        write_source(
            dir.path(),
            "svc.go",
            "package main\n\nfunc alpha() {\n\tprintln(\"a\")\n}\n\nfunc beta() {}\n",
        );

        let service = build(false);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let symbol = service
            .search_symbols("p1", "alpha", 10)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        let source = service
            .symbol_source("p1", &symbol.id)
            .await
            .unwrap()
            .expect("symbol should exist");
        assert_eq!(source.name, "alpha");
        assert!(source.source.contains("println(\"a\")"));
        assert!(!source.source.contains("func beta"));
    }

    #[tokio::test]
    async fn symbol_source_is_scoped_to_project() {
        let dir = tempfile::tempdir().unwrap();
        write_source(dir.path(), "svc.go", "package main\n\nfunc alpha() {}\n");

        let service = build(false);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let symbol = service
            .search_symbols("p1", "alpha", 10)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        assert!(service
            .symbol_source("other-project", &symbol.id)
            .await
            .unwrap()
            .is_none());
    }
}
