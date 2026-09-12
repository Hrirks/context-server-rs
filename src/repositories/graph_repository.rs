use crate::models::graph::{ConversationMemory, GraphEdge, GraphStats, GraphSymbol};
use async_trait::async_trait;
use rmcp::model::ErrorData as McpError;

/// Persistence for the code graph and the conversation-memory delta log.
#[async_trait]
pub trait GraphRepository: Send + Sync {
    /// Insert or replace a symbol node (keyed by its deterministic `id`).
    async fn upsert_symbol(&self, symbol: &GraphSymbol) -> Result<(), McpError>;

    /// Insert or replace an edge (keyed by `(source, target, edge_type)`).
    async fn upsert_edge(&self, edge: &GraphEdge) -> Result<(), McpError>;

    /// Look up a symbol by id.
    async fn find_symbol(&self, id: &str) -> Result<Option<GraphSymbol>, McpError>;

    /// Case-insensitive substring search over symbol names in a project.
    async fn find_symbols_by_name(
        &self,
        project_id: &str,
        name: &str,
        limit: usize,
    ) -> Result<Vec<GraphSymbol>, McpError>;

    /// All edges leaving a symbol.
    async fn find_outgoing_edges(&self, symbol_id: &str) -> Result<Vec<GraphEdge>, McpError>;

    /// All edges arriving at a symbol (its callers/usages/references).
    async fn find_incoming_edges(&self, symbol_id: &str) -> Result<Vec<GraphEdge>, McpError>;

    /// Remove a project's symbols and edges (used before a full re-index).
    #[allow(dead_code)]
    async fn delete_project(&self, project_id: &str) -> Result<(), McpError>;

    /// Load every symbol for a project.
    async fn list_symbols(&self, project_id: &str) -> Result<Vec<GraphSymbol>, McpError>;

    /// Load every symbol belonging to `file_path` in a project.
    async fn list_symbols_for_file(
        &self,
        project_id: &str,
        file_path: &str,
    ) -> Result<Vec<GraphSymbol>, McpError>;

    /// Remove every symbol belonging to `file_path` and all edges touching them,
    /// returning the removed symbol ids so callers can also delete embeddings.
    async fn delete_symbols_for_file(
        &self,
        project_id: &str,
        file_path: &str,
    ) -> Result<Vec<String>, McpError>;

    /// Count symbols and edges for a project.
    async fn stats(&self, project_id: &str) -> Result<GraphStats, McpError>;

    /// Append a conversation-memory delta.
    async fn record_conversation(&self, memory: &ConversationMemory) -> Result<(), McpError>;

    /// Recent conversation deltas for a session, newest first.
    async fn recent_conversation(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<ConversationMemory>, McpError>;
}
