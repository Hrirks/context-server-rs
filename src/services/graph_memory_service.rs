//! Graph memory: turn parsed source into a queryable code graph.
//!
//! [`GraphMemoryService`] indexes tree-sitter chunks into `GraphSymbol` nodes
//! and `GraphEdge` relationships (`contains` for structural nesting, `imports`
//! for import declarations, plus `calls`, `inherits`, and `references` resolved
//! from name mentions), then supports name search and budgeted BFS traversal so
//! an agent can pull a connected fragment of the codebase without reserializing
//! whole files.

use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::model::ErrorData as McpError;

use crate::models::embedding::content_hash;
use crate::models::graph::{
    CodeContext, CodeContextItem, ContextOrigin, ConversationMemory, EdgeType, GraphEdge,
    GraphStats, GraphSubgraph, GraphSymbol, IndexReport, IndexedFile, RelatedSymbol, SymbolContext,
    SymbolOutline, SymbolSource,
};
use crate::parser::{
    chunk_file_async, discover_sources, ChunkKind, ReferenceKind, SemanticChunk, SourceLanguage,
};
use crate::repositories::GraphRepository;
use crate::services::EmbeddingStoreService;

/// How many query-matching symbols seed one assembled context bundle.
const CONTEXT_SEED_LIMIT: usize = 8;
/// Semantic candidates fetched per wanted seed.
///
/// Structural chunks (packages, imports) are embedded too but make useless
/// seeds, and they are dropped *after* the result limit — so the pool has to be
/// wider than the number of seeds wanted, or a query that happens to match
/// boilerplate scores zero seeds and silently degrades to name search.
const CONTEXT_SEED_OVERFETCH: usize = 8;
/// Score multiplier applied per graph hop away from a seed.
const CONTEXT_DISTANCE_DECAY: f32 = 0.5;
/// Hard cap on bundle size, independent of the token budget.
const CONTEXT_MAX_ITEMS: usize = 64;
/// Rough per-item cost of the outline metadata, in tokens.
const CONTEXT_OUTLINE_TOKENS: usize = 24;
/// Confidence of a reference resolved to a symbol declared in the same file.
///
/// Edge weight carries resolution confidence. References are resolved by name
/// alone, so a match is evidence of nothing until a scope narrows it: the
/// narrower the scope containing the match, the more the edge is trusted.
/// `assemble_code_context` multiplies a neighbour's score by this weight, so a
/// project-wide name guess can no longer outrank a real same-package call.
const RESOLUTION_SAME_FILE: f64 = 1.0;
/// Same directory: one package in Go, one package folder in Java and Dart.
const RESOLUTION_SAME_DIR: f64 = 0.9;
/// The target's directory is named by one of the referencing file's imports.
const RESOLUTION_IMPORTED: f64 = 0.7;
/// No scoping evidence at all: the name matched elsewhere in the project. Kept,
/// because it may still be right, but heavily discounted.
const RESOLUTION_UNSCOPED: f64 = 0.3;

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
        let mut name_index: HashMap<String, Vec<NameCandidate>> = HashMap::new();
        for sym in &existing {
            if unchanged_paths.contains(&sym.file_path) && kind_indexable(&sym.kind) {
                name_index
                    .entry(sym.name.to_lowercase())
                    .or_default()
                    .push(NameCandidate::new(&sym.id, &sym.file_path, root));
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
                    name_index
                        .entry(name_key)
                        .or_default()
                        .push(NameCandidate::new(&id, &file_path, root));
                }
                chunk_ids.push(id);
            }

            // What this file can see: where it lives and what it imports. Used
            // below to resolve its references to the narrowest matching scope.
            let scope = Arc::new(ReferenceScope::new(file, root, &chunks));

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
                        scope: scope.clone(),
                        created_at: now.clone(),
                    });
                }
            }

            // Best-effort embedding for semantic search. The whole file's
            // symbols go through one backend round-trip.
            if let Some(store) = &self.embedding_store {
                let mut batch: Vec<(String, String, String)> = Vec::new();
                for (i, chunk) in chunks.iter().enumerate() {
                    // Package and import declarations are structural: they are
                    // never a useful retrieval seed. On a real Go repo they were
                    // 270 of 1678 vectors, crowding the candidate pool and
                    // forcing the over-fetch that keeps seeds useful.
                    if matches!(chunk.kind, ChunkKind::Package | ChunkKind::Import) {
                        continue;
                    }
                    if chunk.text.trim().is_empty() {
                        embed_failures += 1;
                        continue;
                    }
                    batch.push((
                        chunk_ids[i].clone(),
                        chunk.text.clone(),
                        kind_str(chunk.kind).to_string(),
                    ));
                }

                let (stored, failed) = store.embed_and_store_batch(Some(project_id), &batch).await;
                embedded += stored;
                embed_failures += failed;
            }
        }

        // Resolve deferred call/inherit/reference names against the project name
        // index. A name is linked to the *narrowest* scope that contains a match
        // — same file, then same directory (one package), then a directory the
        // referencing file imports — and only falls back to a project-wide guess
        // when nothing narrower matches. The scope reached becomes the edge
        // weight, so a guess is still recorded without being able to outrank a
        // real call when context is assembled.
        for deferred_edge in &deferred {
            let Some(candidates) = name_index.get(&deferred_edge.target_name.to_lowercase()) else {
                continue;
            };

            let mut best: Option<ResolutionScope> = None;
            let mut targets: Vec<&NameCandidate> = Vec::new();
            for candidate in candidates {
                if candidate.id == deferred_edge.source_id {
                    continue;
                }
                let candidate_scope = deferred_edge.scope.scope_for(candidate);
                if best.is_none_or(|best| candidate_scope > best) {
                    best = Some(candidate_scope);
                    targets.clear();
                    targets.push(candidate);
                } else if best == Some(candidate_scope) {
                    targets.push(candidate);
                }
            }
            // Nothing but the referencing symbol itself: no edge to draw.
            let Some(best) = best else {
                continue;
            };

            for target in targets.iter().take(MAX_EDGE_TARGETS) {
                let edge = GraphEdge {
                    id: content_hash(&format!(
                        "{}\0{}\0{}",
                        deferred_edge.source_id,
                        target.id,
                        deferred_edge.edge_type.as_str()
                    )),
                    project_id: project_id.to_string(),
                    source_id: deferred_edge.source_id.clone(),
                    target_id: target.id.clone(),
                    edge_type: deferred_edge.edge_type,
                    weight: best.weight(),
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

    /// Assemble a budgeted context bundle for a single symbol.
    ///
    /// Returns the symbol's own source plus the symbols connected to it: the
    /// inbound neighbours that reference it (callers, usages, implementors) and
    /// the outbound neighbours it references (callees, supertypes). `budget`
    /// caps the total number of neighbours returned, so the payload cannot
    /// balloon past the caller's token budget; `truncated` reports when the cap
    /// bit. Returns `None` for an unknown id or one from another project.
    pub async fn context_for_symbol(
        &self,
        project_id: &str,
        symbol_id: &str,
        budget: usize,
        session_id: &str,
    ) -> Result<Option<SymbolContext>, McpError> {
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

        let incoming_edges = self.repository.find_incoming_edges(symbol_id).await?;
        let outgoing_edges = self.repository.find_outgoing_edges(symbol_id).await?;

        let mut truncated = false;

        // Inbound first: "who uses this?" is usually the more valuable direction.
        let mut callers = Vec::new();
        let mut seen_callers = HashSet::new();
        for edge in &incoming_edges {
            if callers.len() >= budget {
                truncated = true;
                break;
            }
            if !seen_callers.insert(edge.source_id.clone()) {
                continue;
            }
            if let Some(neighbour) = self.repository.find_symbol(&edge.source_id).await? {
                if neighbour.id == symbol.id {
                    continue;
                }
                // A file "contains" its symbols, so the file node shows up as a
                // caller of everything inside it. That is structure, not usage,
                // and it drowns out real callers.
                if neighbour.kind == "file" {
                    continue;
                }
                callers.push(RelatedSymbol {
                    edge_type: edge.edge_type,
                    symbol: SymbolOutline::from(&neighbour),
                    incoming: true,
                });
            }
        }

        let mut callees = Vec::new();
        let mut seen_callees = HashSet::new();
        for edge in &outgoing_edges {
            if callers.len() + callees.len() >= budget {
                truncated = true;
                break;
            }
            if !seen_callees.insert(edge.target_id.clone()) {
                continue;
            }
            if let Some(neighbour) = self.repository.find_symbol(&edge.target_id).await? {
                if neighbour.id == symbol.id {
                    continue;
                }
                if neighbour.kind == "file" {
                    continue;
                }
                callees.push(RelatedSymbol {
                    edge_type: edge.edge_type,
                    symbol: SymbolOutline::from(&neighbour),
                    incoming: false,
                });
            }
        }

        self.record_delta(
            session_id,
            Some(project_id),
            "context",
            serde_json::json!({
                "symbol_id": symbol_id,
                "callers": callers.len(),
                "callees": callees.len(),
                "truncated": truncated,
            }),
        )
        .await;

        Ok(Some(SymbolContext {
            symbol: SymbolOutline::from(&symbol),
            file_path: symbol.file_path,
            language: symbol.language,
            source,
            callers,
            callees,
            truncated,
        }))
    }

    /// Assemble the code needed to answer a natural-language query.
    ///
    /// Retrieval runs in three steps: *seed* (semantic search over the embedded
    /// code, falling back to name search when no embedding backend is
    /// configured or it fails), *expand* (one graph hop around each seed, both
    /// inbound and outbound, so callers and callees come along), and *rank*
    /// (seed similarity discounted by hop distance). Items are appended in
    /// rank order until `token_budget` estimated tokens are used, so the caller
    /// controls payload size instead of the repository size.
    ///
    /// `truncated` is set when anything was dropped, either by the budget or by
    /// the item cap.
    pub async fn assemble_code_context(
        &self,
        project_id: &str,
        query: &str,
        token_budget: usize,
        session_id: &str,
    ) -> Result<CodeContext, McpError> {
        /// A query match: the symbol, its ranking score, and (for semantic
        /// hits) the raw cosine similarity.
        type Seed = (GraphSymbol, f32, Option<f32>);

        // --- Seed ---------------------------------------------------------
        let mut seeds: Vec<Seed> = Vec::new();
        let mut semantic = false;

        if let Some(store) = &self.embedding_store {
            let candidates = CONTEXT_SEED_LIMIT * CONTEXT_SEED_OVERFETCH;
            match store.search(query, project_id, candidates).await {
                Ok(hits) => {
                    let mut resolved: Vec<Seed> = Vec::new();
                    for hit in hits {
                        if resolved.len() >= CONTEXT_SEED_LIMIT {
                            break;
                        }
                        let Some(symbol) = self.repository.find_symbol(&hit.context_id).await?
                        else {
                            continue;
                        };
                        if symbol.project_id != project_id || !kind_indexable(&symbol.kind) {
                            continue;
                        }
                        resolved.push((symbol, hit.similarity.max(0.0), Some(hit.similarity)));
                    }
                    if !resolved.is_empty() {
                        semantic = true;
                        seeds = resolved;
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        "Semantic search failed ({error}); falling back to name search for '{query}'"
                    );
                }
            }
        }

        if seeds.is_empty() {
            for symbol in self
                .search_symbols(project_id, query, CONTEXT_SEED_LIMIT)
                .await?
            {
                if kind_indexable(&symbol.kind) {
                    seeds.push((symbol, 1.0, None));
                }
            }
        }

        // Highest similarity first; `search` already orders, name search does not.
        seeds.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        // --- Expand -------------------------------------------------------
        // One hop in both directions: inbound edges are callers/implementors,
        // outbound are callees/supertypes. Best edge wins when a neighbour is
        // reachable from several seeds.
        let mut neighbours: HashMap<String, (f32, EdgeType, GraphSymbol)> = HashMap::new();
        for (seed, score, _) in &seeds {
            let mut edges = self.repository.find_incoming_edges(&seed.id).await?;
            edges.extend(self.repository.find_outgoing_edges(&seed.id).await?);

            for edge in edges {
                let neighbour_id = if edge.target_id == seed.id {
                    &edge.source_id
                } else {
                    &edge.target_id
                };
                if neighbour_id == &seed.id {
                    continue;
                }
                let Some(symbol) = self.repository.find_symbol(neighbour_id).await? else {
                    continue;
                };
                // Whole-file nodes would drag an entire file into the bundle.
                if symbol.project_id != project_id || !kind_indexable(&symbol.kind) {
                    continue;
                }
                // Edge weight is how much the resolver trusted this link: a
                // project-wide name guess must not outrank a real call.
                let neighbour_score =
                    score * CONTEXT_DISTANCE_DECAY * edge.weight.clamp(0.0, 1.0) as f32;
                match neighbours.entry(symbol.id.clone()) {
                    Entry::Vacant(slot) => {
                        slot.insert((neighbour_score, edge.edge_type, symbol));
                    }
                    Entry::Occupied(mut slot) => {
                        if neighbour_score > slot.get().0 {
                            slot.insert((neighbour_score, edge.edge_type, symbol));
                        }
                    }
                }
            }
        }

        // Seeds already carry the best score for themselves; never re-add them.
        for (seed, _, _) in &seeds {
            neighbours.remove(&seed.id);
        }

        let mut ranked_neighbours: Vec<(f32, EdgeType, GraphSymbol)> =
            neighbours.into_values().collect();
        ranked_neighbours.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.2.name.cmp(&b.2.name))
        });

        // --- Pack ---------------------------------------------------------
        let mut items: Vec<CodeContextItem> = Vec::new();
        let mut tokens_used = 0usize;
        let mut truncated = false;
        let mut source_cache: HashMap<String, Option<String>> = HashMap::new();

        for (symbol, score, similarity) in &seeds {
            let source = self.source_for_symbol(symbol, &mut source_cache).await;
            let mut item = CodeContextItem {
                symbol: SymbolOutline::from(symbol),
                file_path: symbol.file_path.clone(),
                language: symbol.language.clone(),
                origin: ContextOrigin::Seed,
                similarity: *similarity,
                distance: 0,
                score: *score,
                via: None,
                source,
                token_estimate: 0,
            };
            let cost = item_cost(&item);
            if items.len() >= CONTEXT_MAX_ITEMS || tokens_used + cost > token_budget {
                truncated = true;
                continue;
            }
            item.token_estimate = cost;
            tokens_used += cost;
            items.push(item);
        }

        for (score, edge_type, symbol) in &ranked_neighbours {
            let source = self.source_for_symbol(symbol, &mut source_cache).await;
            let mut item = CodeContextItem {
                symbol: SymbolOutline::from(symbol),
                file_path: symbol.file_path.clone(),
                language: symbol.language.clone(),
                origin: ContextOrigin::Neighbor,
                similarity: None,
                distance: 1,
                score: *score,
                via: Some(*edge_type),
                source,
                token_estimate: 0,
            };
            let cost = item_cost(&item);
            if items.len() >= CONTEXT_MAX_ITEMS || tokens_used + cost > token_budget {
                truncated = true;
                continue;
            }
            item.token_estimate = cost;
            tokens_used += cost;
            items.push(item);
        }

        let seed_count = seeds.len();
        let neighbour_count = ranked_neighbours.len();

        self.record_delta(
            session_id,
            Some(project_id),
            "assemble_context",
            serde_json::json!({
                "query": query,
                "seeds": seed_count,
                "neighbours": neighbour_count,
                "items": items.len(),
                "truncated": truncated,
            }),
        )
        .await;

        Ok(CodeContext {
            project_id: project_id.to_string(),
            query: query.to_string(),
            token_budget,
            token_estimate: tokens_used,
            truncated,
            semantic,
            items,
        })
    }

    /// Read one symbol's source lines, caching whole files across a bundle so
    /// a file is read once even when several of its symbols are included.
    async fn source_for_symbol(
        &self,
        symbol: &GraphSymbol,
        cache: &mut HashMap<String, Option<String>>,
    ) -> String {
        let content = match cache.get(&symbol.file_path).cloned() {
            Some(cached) => cached,
            None => {
                let read = tokio::fs::read_to_string(&symbol.file_path).await.ok();
                cache.insert(symbol.file_path.clone(), read.clone());
                read
            }
        };
        match content {
            Some(text) => slice_lines(&text, symbol.start_line, symbol.end_line),
            None => symbol.text.clone(),
        }
    }

    /// Every file currently in a project's index, with its symbol count.
    ///
    /// The human-review surface for an index: answers "what did indexing
    /// actually pick up?" without walking the symbol graph.
    pub async fn indexed_files(&self, project_id: &str) -> Result<Vec<IndexedFile>, McpError> {
        self.repository.list_indexed_files(project_id).await
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
    /// Where the reference was written, used to prefer the narrowest scope.
    scope: Arc<ReferenceScope>,
    created_at: String,
}

/// A symbol a reference could resolve to, with the scope it lives in.
struct NameCandidate {
    id: String,
    file_path: String,
    /// Directory of the declaration, relative to the indexed root.
    dir: String,
}

impl NameCandidate {
    fn new(id: &str, file_path: &str, root: &Path) -> Self {
        Self {
            id: id.to_string(),
            file_path: file_path.to_string(),
            dir: relative_dir(file_path, root),
        }
    }
}

/// How well a reference's own scope matches a candidate declaration, ordered
/// from weakest to strongest evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ResolutionScope {
    /// The name matched somewhere else in the project and nothing links it here.
    Unscoped,
    /// The target's directory is named by one of the referencing file's imports.
    Imported,
    /// Same directory: one package in Go, one package folder in Java and Dart.
    Directory,
    /// Declared in the very file that references it.
    File,
}

impl ResolutionScope {
    /// Edge weight this scope earns.
    fn weight(self) -> f64 {
        match self {
            ResolutionScope::File => RESOLUTION_SAME_FILE,
            ResolutionScope::Directory => RESOLUTION_SAME_DIR,
            ResolutionScope::Imported => RESOLUTION_IMPORTED,
            ResolutionScope::Unscoped => RESOLUTION_UNSCOPED,
        }
    }
}

/// What one source file can see: its own location and the modules it imports.
struct ReferenceScope {
    file_path: String,
    dir: String,
    imports: Vec<String>,
}

impl ReferenceScope {
    fn new(file: &Path, root: &Path, chunks: &[SemanticChunk]) -> Self {
        let file_path = file.display().to_string();
        let imports = chunks
            .iter()
            .filter(|chunk| chunk.kind == ChunkKind::Import)
            .flat_map(import_targets)
            .filter(|target| !target.trim().is_empty())
            .collect();
        Self {
            dir: relative_dir(&file_path, root),
            file_path,
            imports,
        }
    }

    /// Narrowest scope that links this file to `candidate`.
    fn scope_for(&self, candidate: &NameCandidate) -> ResolutionScope {
        if candidate.file_path == self.file_path {
            return ResolutionScope::File;
        }
        if !self.dir.is_empty() && candidate.dir == self.dir {
            return ResolutionScope::Directory;
        }
        if self
            .imports
            .iter()
            .any(|import| import_matches_dir(import, &candidate.dir))
        {
            return ResolutionScope::Imported;
        }
        ResolutionScope::Unscoped
    }
}

/// Directory of `path` relative to the indexed root, `/`-separated.
fn relative_dir(path: &str, root: &Path) -> String {
    let path = Path::new(path);
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .parent()
        .map(|parent| parent.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

/// Every module path one import chunk pulls in.
///
/// A Go `import ( ... )` block is a single syntax node holding many paths, so
/// reading only the first one leaves a file's imports almost entirely invisible
/// to scope resolution — and its cross-package calls with nothing to match
/// against.
fn import_targets(chunk: &SemanticChunk) -> Vec<String> {
    let quoted = quoted_strings(&chunk.text);
    if quoted.is_empty() {
        // Java writes imports unquoted: `import java.util.List;`.
        return vec![import_target(chunk)];
    }
    quoted
}

/// Quoted literals in a declaration, in order.
fn quoted_strings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '"' && c != '\'' {
            continue;
        }
        let quote = c;
        let mut value = String::new();
        while let Some(c) = chars.next() {
            if c == quote {
                break;
            }
            if c == '\\' {
                if let Some(escaped) = chars.next() {
                    value.push(escaped);
                }
                continue;
            }
            value.push(c);
        }
        out.push(value);
    }
    out
}

/// Whether an import target names `dir`.
///
/// Import targets are written as Go module paths
/// (`github.com/acme/app/internal/booking`), Dart package URIs
/// (`package:app/internal/booking/service.dart`) or Java package names
/// (`com.acme.app.booking`), while `dir` is a plain path relative to the
/// indexed root. The two describe the same place when either one's path
/// segments contain the other's as a contiguous run.
fn import_matches_dir(import: &str, dir: &str) -> bool {
    if dir.is_empty() {
        return false;
    }
    let trimmed = import.trim().trim_matches(|c| c == '\'' || c == '"');
    let trimmed = trimmed.strip_prefix("package:").unwrap_or(trimmed);
    let trimmed = trimmed.trim_end_matches('/');
    let import_segments = path_segments(trimmed);
    let dir_segments = path_segments(dir);
    contains_segments(&import_segments, &dir_segments)
        || contains_segments(&dir_segments, &import_segments)
}

fn path_segments(path: &str) -> Vec<&str> {
    path.split(['/', '.'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty() && *segment != "..")
        .collect()
}

fn contains_segments(haystack: &[&str], needle: &[&str]) -> bool {
    !needle.is_empty()
        && needle.len() <= haystack.len()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
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

/// Estimated tokens for one item *as the caller receives it*.
///
/// Measured on the serialized JSON, not the raw source: escaping indentation,
/// newlines and quotes roughly doubles the size of source code, so budgeting on
/// the raw text silently hands the caller a bundle around twice the size it was
/// promised. The `token_estimate` field itself is written after this, so the
/// figure excludes its own digits — a rounding error, not a drift.
fn item_cost(item: &CodeContextItem) -> usize {
    serde_json::to_string(item)
        .map(|serialized| estimate_tokens(&serialized))
        .unwrap_or_else(|_| estimate_tokens(&item.source) + CONTEXT_OUTLINE_TOKENS)
}

/// Rough token estimate for source text (~4 characters per token).
///
/// Deliberately cheap and dependency-free: the budget only needs to be right
/// to within a constant factor, and an exact tokenizer would be a heavier
/// dependency than the problem warrants.
fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
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
    use crate::parser::chunker::chunk_source;
    use std::time::Duration;

    /// Test service over the *real* schema, foreign keys included.
    ///
    /// The harness deliberately does not disable foreign-key enforcement: with
    /// it off, a write that production rejects (an embedding whose project row
    /// is missing, say) persists happily and the test passes while the feature
    /// is broken.
    fn build(with_embedding: bool) -> GraphMemoryService {
        let pool = Arc::new(ConnectionPool::new(":memory:", 1, Duration::from_secs(1)).unwrap());
        {
            let conn = pool.checkout().unwrap();
            let conn = conn.lock().unwrap();
            crate::db::init::apply_schema(&conn).unwrap();
            conn.execute("INSERT INTO projects (id, name) VALUES ('p1', 'p1')", [])
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

        // The file declares a package, an import and one function. Only the
        // function is worth searching for, so only it reaches the backend:
        // structural chunks are skipped rather than embedded as noise.
        assert_eq!(
            report.embedded, 1,
            "package and import chunks must not be embedded, got {}",
            report.embedded
        );
        assert_eq!(report.embed_failures, 0);
    }

    #[tokio::test]
    async fn indexed_files_lists_parsed_files_with_counts() {
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

        let files = service.indexed_files("p1").await.unwrap();
        assert_eq!(files.len(), 1, "expected one indexed file: {files:?}");
        assert!(files[0].file_path.ends_with("main.go"));
        assert_eq!(files[0].language, "go");
        // `outer` and `inner`; the file/package nodes are excluded.
        assert_eq!(files[0].symbol_count, 2);

        // A project with nothing indexed reviews as empty, not as an error.
        assert!(service
            .indexed_files("no-such-project")
            .await
            .unwrap()
            .is_empty());
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

    #[tokio::test]
    async fn context_for_symbol_reports_callers_and_callees() {
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

        let inner = service
            .search_symbols("p1", "inner", 10)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let inner_ctx = service
            .context_for_symbol("p1", &inner.id, 20, "s")
            .await
            .unwrap()
            .unwrap();
        assert!(inner_ctx.source.contains("func inner"));

        let caller_names: Vec<&str> = inner_ctx
            .callers
            .iter()
            .map(|r| r.symbol.name.as_str())
            .collect();
        assert!(
            caller_names.contains(&"outer"),
            "expected 'outer' among callers, got {caller_names:?}"
        );
        assert!(inner_ctx.callers.iter().all(|r| r.incoming));

        let outer = service
            .search_symbols("p1", "outer", 10)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let outer_ctx = service
            .context_for_symbol("p1", &outer.id, 20, "s")
            .await
            .unwrap()
            .unwrap();
        let callee_names: Vec<&str> = outer_ctx
            .callees
            .iter()
            .map(|r| r.symbol.name.as_str())
            .collect();
        assert!(
            callee_names.contains(&"inner"),
            "expected 'inner' among callees, got {callee_names:?}"
        );
        assert!(outer_ctx.callees.iter().all(|r| !r.incoming));
    }

    #[tokio::test]
    async fn context_for_symbol_honours_budget_and_scope() {
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

        let outer = service
            .search_symbols("p1", "outer", 10)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        // A zero budget returns the symbol itself but no neighbours, flagged.
        let empty = service
            .context_for_symbol("p1", &outer.id, 0, "s")
            .await
            .unwrap()
            .unwrap();
        assert!(empty.callers.is_empty());
        assert!(empty.callees.is_empty());
        assert!(empty.truncated);

        // Unknown id and cross-project id both resolve to `None`.
        assert!(service
            .context_for_symbol("p1", "no-such-symbol", 20, "s")
            .await
            .unwrap()
            .is_none());
        assert!(service
            .context_for_symbol("other-project", &outer.id, 20, "s")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn assemble_code_context_seeds_from_semantic_search() {
        let dir = tempfile::tempdir().unwrap();
        write_source(
            dir.path(),
            "main.go",
            "package main\n\nfunc outer() {\n\tinner()\n}\n\nfunc inner() {}\n",
        );

        let service = build(true);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let context = service
            .assemble_code_context("p1", "func inner() {}", 4000, "s")
            .await
            .unwrap();

        assert!(context.semantic, "expected the semantic seeding path");
        assert!(!context.items.is_empty());
        assert_eq!(context.items[0].origin, ContextOrigin::Seed);
        assert_eq!(context.items[0].distance, 0);
        assert!(context.items[0].similarity.is_some());
        assert!(
            context.items[0].source.contains("func inner"),
            "expected the symbol's own source, got {:?}",
            context.items[0].source
        );
        assert!(context.token_estimate <= context.token_budget);
        assert!(!context.truncated);
    }

    #[tokio::test]
    async fn assemble_code_context_expands_one_hop_to_neighbours() {
        let dir = tempfile::tempdir().unwrap();
        write_source(
            dir.path(),
            "main.go",
            "package main\n\nfunc outer() {\n\tinner()\n}\n\nfunc inner() {}\n",
        );

        // No embedding store: seeds come from name search, which makes the
        // expected seeding order deterministic.
        let service = build(false);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let context = service
            .assemble_code_context("p1", "outer", 4000, "s")
            .await
            .unwrap();

        assert!(!context.semantic, "expected the name-search fallback");
        assert_eq!(context.items[0].symbol.name, "outer");

        let inner = context
            .items
            .iter()
            .find(|item| item.symbol.name == "inner")
            .expect("expected inner to be pulled in as a neighbour");
        assert_eq!(inner.origin, ContextOrigin::Neighbor);
        assert_eq!(inner.distance, 1);
        assert!(inner.via.is_some());
        assert!(inner.source.contains("func inner"));
        // Seeds rank above their neighbours.
        assert!(context.items[0].score > inner.score);
    }

    #[tokio::test]
    async fn assemble_code_context_stops_at_the_token_budget() {
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

        // A budget too small for even one symbol yields nothing, but says so.
        let starved = service
            .assemble_code_context("p1", "outer", 1, "s")
            .await
            .unwrap();
        assert!(starved.items.is_empty());
        assert!(starved.truncated);
        assert_eq!(starved.token_estimate, 0);

        // A budget that fits the seed only drops the neighbour.
        let one = service
            .assemble_code_context("p1", "outer", 40, "s")
            .await
            .unwrap();
        assert!(one.items.len() < 2 || one.truncated);
        assert!(one.token_estimate <= 40);
    }

    #[tokio::test]
    async fn assemble_code_context_is_scoped_to_the_project() {
        let dir = tempfile::tempdir().unwrap();
        write_source(
            dir.path(),
            "main.go",
            "package main\n\nfunc outer() {\n\tinner()\n}\n\nfunc inner() {}\n",
        );

        let service = build(true);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let other = service
            .assemble_code_context("p2", "outer", 4000, "s")
            .await
            .unwrap();
        assert!(other.items.is_empty());
        assert!(other.token_estimate == 0);
    }

    #[test]
    fn import_targets_match_directories_by_path_segments() {
        // A Go module path, a Dart package URI and a Java package name all name
        // the same directory shape.
        assert!(import_matches_dir(
            "github.com/acme/app/internal/booking",
            "internal/booking"
        ));
        assert!(import_matches_dir(
            "package:app/internal/booking/service.dart",
            "internal/booking"
        ));
        assert!(import_matches_dir("com.acme.app.booking", "booking"));
        assert!(import_matches_dir(
            "'./internal/booking'",
            "internal/booking"
        ));
        // A sibling package, a stdlib import and an empty dir must not match.
        assert!(!import_matches_dir(
            "github.com/acme/app/internal/auth",
            "internal/booking"
        ));
        assert!(!import_matches_dir("fmt", "internal/booking"));
        assert!(!import_matches_dir("github.com/acme/app", ""));
    }

    #[tokio::test]
    async fn resolution_prefers_the_same_package_over_a_name_match() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("internal/booking")).unwrap();
        std::fs::create_dir_all(dir.path().join("internal/auth")).unwrap();
        write_source(
            &dir.path().join("internal/booking"),
            "handler.go",
            "package booking\n\nfunc Confirm() {}\n",
        );
        write_source(
            &dir.path().join("internal/booking"),
            "service.go",
            "package booking\n\nfunc Handle() {\n\tConfirm()\n}\n",
        );
        write_source(
            &dir.path().join("internal/auth"),
            "service.go",
            "package auth\n\nfunc Confirm() {}\n",
        );

        let service = build(false);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let handle = function_named(&service, "Handle").await;
        let calls = edges_of_type(&service, &handle.id, "calls", true).await;
        assert_eq!(
            calls.len(),
            1,
            "a same-package match must win outright, got {:?}",
            calls
        );
        assert_eq!(calls[0].1, RESOLUTION_SAME_DIR);

        let auth_confirm = function_named(&service, "Confirm").await.file_path.clone();
        assert!(
            auth_confirm.contains("internal/auth"),
            "expected the call to resolve inside booking, got {auth_confirm}"
        );
    }

    #[test]
    fn an_import_block_exposes_every_path_it_pulls_in() {
        let source = "package booking\n\nimport (\n\t\"errors\"\n\t\"time\"\n\n\tplatformDB \"github.com/acme/app/internal/platform/db\"\n\t\"github.com/acme/app/internal/auth\"\n)\n\nfunc Handle() {}\n";
        let chunks = chunk_source(SourceLanguage::Go, source).unwrap();
        let imports: Vec<String> = chunks
            .iter()
            .filter(|chunk| chunk.kind == ChunkKind::Import)
            .flat_map(import_targets)
            .collect();
        assert_eq!(
            imports,
            vec![
                "errors",
                "time",
                "github.com/acme/app/internal/platform/db",
                "github.com/acme/app/internal/auth"
            ],
            "an import block must yield every path, got {imports:?}"
        );
    }

    #[tokio::test]
    async fn resolution_uses_an_import_when_no_same_package_match_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("internal/booking")).unwrap();
        std::fs::create_dir_all(dir.path().join("internal/auth")).unwrap();
        write_source(
            &dir.path().join("internal/auth"),
            "token.go",
            "package auth\n\nfunc Parse() {}\n",
        );
        write_source(
            &dir.path().join("internal/booking"),
            "service.go",
            "package booking\n\nimport \"github.com/acme/app/internal/auth\"\n\nfunc Handle() {\n\tauth.Parse()\n}\n",
        );

        let service = build(false);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let handle = function_named(&service, "Handle").await;
        let calls = edges_of_type(&service, &handle.id, "calls", true).await;
        assert_eq!(
            calls.len(),
            1,
            "only the imported package should be linked, got {:?}",
            calls
        );
        assert_eq!(calls[0].1, RESOLUTION_IMPORTED);
    }

    #[tokio::test]
    async fn resolution_scopes_calls_through_an_import_block() {
        // The shape every real Go file has: one block, many paths.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("internal/booking")).unwrap();
        std::fs::create_dir_all(dir.path().join("internal/auth")).unwrap();
        std::fs::create_dir_all(dir.path().join("internal/platform/db")).unwrap();
        write_source(
            &dir.path().join("internal/auth"),
            "token.go",
            "package auth\n\nfunc Parse() {}\n",
        );
        write_source(
            &dir.path().join("internal/platform/db"),
            "db.go",
            "package db\n\nfunc Parse() {}\n",
        );
        write_source(
            &dir.path().join("internal/booking"),
            "service.go",
            "package booking\n\nimport (\n\t\"errors\"\n\n\t\"github.com/acme/app/internal/auth\"\n\tplatformDB \"github.com/acme/app/internal/platform/db\"\n)\n\nfunc Handle() {\n\tauth.Parse()\n}\n",
        );

        let service = build(false);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let handle = function_named(&service, "Handle").await;
        let calls = edges_of_type(&service, &handle.id, "calls", true).await;
        assert_eq!(
            calls.len(),
            2,
            "both imported packages are in scope: {calls:?}"
        );
        assert!(
            calls
                .iter()
                .all(|(_, weight)| *weight == RESOLUTION_IMPORTED),
            "imported packages must not be guesses: {calls:?}"
        );
    }

    #[tokio::test]
    async fn resolution_discounts_a_project_wide_name_guess() {
        // Nothing scopes this call: no declaration in the same file or package
        // and no import naming the target. The edge is kept so the graph does
        // not lose information, but its weight marks it as a guess.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("internal/booking")).unwrap();
        std::fs::create_dir_all(dir.path().join("internal/auth")).unwrap();
        write_source(
            &dir.path().join("internal/booking"),
            "service.go",
            "package booking\n\nfunc Handle() {\n\tParse()\n}\n",
        );
        write_source(
            &dir.path().join("internal/auth"),
            "token.go",
            "package auth\n\nfunc Parse() {}\n",
        );

        let service = build(false);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let handle = function_named(&service, "Handle").await;
        let calls = edges_of_type(&service, &handle.id, "calls", true).await;
        assert_eq!(calls.len(), 1, "the guess is kept, got {:?}", calls);
        assert_eq!(calls[0].1, RESOLUTION_UNSCOPED);
    }

    #[tokio::test]
    async fn context_for_symbol_does_not_report_the_file_as_a_caller() {
        let dir = tempfile::tempdir().unwrap();
        write_source(
            dir.path(),
            "main.go",
            "package main\n\nfunc caller() {\n\tcallee()\n}\n\nfunc callee() {}\n",
        );

        let service = build(false);
        service
            .index_directory("p1", dir.path(), "s")
            .await
            .unwrap();

        let callee = function_named(&service, "callee").await;
        let context = service
            .context_for_symbol("p1", &callee.id, 20, "s")
            .await
            .unwrap()
            .expect("callee should have context");

        let callers: Vec<&str> = context
            .callers
            .iter()
            .map(|related| related.symbol.name.as_str())
            .collect();
        assert_eq!(callers, vec!["caller"], "got {callers:?}");
    }

    /// The one indexed function with this name.
    async fn function_named(service: &GraphMemoryService, name: &str) -> GraphSymbol {
        service
            .search_symbols("p1", name, 20)
            .await
            .unwrap()
            .into_iter()
            .find(|symbol| symbol.kind == "function" && symbol.name == name)
            .unwrap_or_else(|| panic!("no function named {name} was indexed"))
    }

    /// Edges of one type touching `symbol_id`, with their weight.
    async fn edges_of_type(
        service: &GraphMemoryService,
        symbol_id: &str,
        edge_type: &str,
        outgoing: bool,
    ) -> Vec<(String, f64)> {
        let edges = if outgoing {
            service
                .repository
                .find_outgoing_edges(symbol_id)
                .await
                .unwrap()
        } else {
            service
                .repository
                .find_incoming_edges(symbol_id)
                .await
                .unwrap()
        };
        edges
            .into_iter()
            .filter(|edge| edge.edge_type.as_str() == edge_type)
            .map(|edge| {
                let target = if outgoing {
                    edge.target_id.clone()
                } else {
                    edge.source_id.clone()
                };
                (target, edge.weight)
            })
            .collect()
    }
}
