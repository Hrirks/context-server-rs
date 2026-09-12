//! Graph memory types: symbol/edge nodes, traversal results, and conversation
//! deltas.

use serde::{Deserialize, Serialize};

/// A node in the code graph: a file, package, declaration, or import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSymbol {
    pub id: String,
    pub project_id: String,
    pub file_path: String,
    pub name: String,
    /// One of `file`, `package`, `import`, `class`, `function`, `method`, etc.
    pub kind: String,
    pub language: String,
    pub signature: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub created_at: String,
}

/// The kinds of relationship tracked between graph nodes.
///
/// `Contains` and `Imports` come from the structural parser's parent links and
/// import declarations. `Calls`, `Inherits`, and `References` are resolved
/// from name mentions found in each chunk's subtree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Contains,
    Imports,
    Calls,
    Inherits,
    References,
    DependsOn,
    RelatesTo,
    Mentions,
}

impl EdgeType {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeType::Contains => "contains",
            EdgeType::Imports => "imports",
            EdgeType::Calls => "calls",
            EdgeType::Inherits => "inherits",
            EdgeType::References => "references",
            EdgeType::DependsOn => "depends_on",
            EdgeType::RelatesTo => "relates_to",
            EdgeType::Mentions => "mentions",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "contains" => Some(EdgeType::Contains),
            "imports" => Some(EdgeType::Imports),
            "calls" => Some(EdgeType::Calls),
            "inherits" => Some(EdgeType::Inherits),
            "references" => Some(EdgeType::References),
            "depends_on" => Some(EdgeType::DependsOn),
            "relates_to" => Some(EdgeType::RelatesTo),
            "mentions" => Some(EdgeType::Mentions),
            _ => None,
        }
    }
}

/// A directed edge between two graph nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub project_id: String,
    pub source_id: String,
    pub target_id: String,
    pub edge_type: EdgeType,
    pub weight: f64,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
}

/// The connected fragment returned by a budgeted BFS traversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSubgraph {
    pub start_id: String,
    pub symbols: Vec<GraphSymbol>,
    pub edges: Vec<GraphEdge>,
    pub budget_exhausted: bool,
}

/// Point-in-time counts for a project's graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub project_id: String,
    pub symbol_count: u64,
    pub edge_count: u64,
}

/// Result of indexing a directory into graph memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexReport {
    pub project_id: String,
    /// Number of files parsed and (re)indexed this run.
    pub files_indexed: usize,
    /// Number of files that were unchanged and skipped.
    pub files_skipped: usize,
    /// Number of previously-indexed files no longer present on disk.
    pub files_removed: usize,
    pub symbols_indexed: usize,
    pub edges_indexed: usize,
    /// Number of symbols whose text was embedded for semantic search.
    pub embedded: usize,
    /// Number of symbols that failed to embed (backend down, empty text, etc.).
    pub embed_failures: usize,
}

/// A compact, body-free view of a single symbol.
///
/// This is the token-efficient projection used by file outlines: it carries the
/// structural facts an agent needs to navigate (name, kind, signature, line
/// range) without the symbol body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolOutline {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub signature: String,
    pub start_line: usize,
    pub end_line: usize,
}

impl From<&GraphSymbol> for SymbolOutline {
    fn from(symbol: &GraphSymbol) -> Self {
        Self {
            id: symbol.id.clone(),
            name: symbol.name.clone(),
            kind: symbol.kind.clone(),
            signature: symbol.signature.clone(),
            start_line: symbol.start_line,
            end_line: symbol.end_line,
        }
    }
}

/// A single symbol together with the exact source lines it spans.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSource {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub language: String,
    pub signature: String,
    pub start_line: usize,
    pub end_line: usize,
    /// The symbol's source, sliced from its file by line range.
    pub source: String,
}

/// One edge of a [`SymbolContext`]: the neighbour on the other end, plus the
/// relationship and its direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedSymbol {
    /// The relationship as seen from the focus symbol (`calls`, `inherits`,
    /// `contains`, ...).
    pub edge_type: EdgeType,
    /// The neighbour on the other end of the edge.
    pub symbol: SymbolOutline,
    /// `true` when the neighbour is the edge *source* (an inbound relationship:
    /// a caller, usage, or implementor).
    pub incoming: bool,
}

/// A budgeted, single-symbol context bundle assembled for an agent.
///
/// This is the "what do I need to know about this symbol" payload: its own
/// source plus the connected symbols that use it (callers) and that it uses
/// (callees), capped so it cannot balloon past the caller's token budget.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolContext {
    pub symbol: SymbolOutline,
    pub file_path: String,
    pub language: String,
    /// The symbol's own source, sliced from its file.
    pub source: String,
    /// Inbound relationships: callers, usages, implementors.
    pub callers: Vec<RelatedSymbol>,
    /// Outbound relationships: callees, supertypes, imported nodes.
    pub callees: Vec<RelatedSymbol>,
    /// `true` when `callers` or `callees` was cut off by the budget.
    pub truncated: bool,
}

/// A record in the conversation-memory delta log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMemory {
    pub id: String,
    pub session_id: String,
    pub project_id: Option<String>,
    /// e.g. `index`, `traverse`, `search`.
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}
