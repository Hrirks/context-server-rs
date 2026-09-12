# MCP Context Server for AI Code Generation

This guide explains how to use the Rust-based Model Context Protocol (MCP) server as a context provider for AI agents (such as Claude Desktop and Cursor IDE) and for integration with developer tools.

## What is Model Context Protocol (MCP)?

The [Model Context Protocol (MCP)](https://modelcontextprotocol.io/introduction) is an open protocol that standardizes how applications provide context to LLMs. MCP follows a client-server architecture where:

- **MCP Hosts**: Programs like Claude Desktop, IDEs, or AI tools that want to access data through MCP
- **MCP Clients**: Protocol clients that maintain 1:1 connections with servers  
- **MCP Servers**: Lightweight programs that expose specific capabilities through the standardized Model Context Protocol
- **Local Data Sources**: Your computer's files, databases, and services that MCP servers can securely access

## This MCP Server vs. Standard HTTP APIs

Our MCP Context Server:
- ✅ Uses the official [Model Context Protocol specification](https://spec.modelcontextprotocol.io/specification/2024-11-05/)
- ✅ Implements MCP tools for structured data exchange
- ✅ Supports automatic tool discovery by MCP clients
- ✅ Uses standard MCP transports (stdio, SSE, HTTP streaming)
- ✅ Built with the official [Rust MCP SDK (rmcp)](https://github.com/modelcontextprotocol/rust-sdk)

Benefits over HTTP APIs:
- **Standardized communication** with LLM applications
- **Tool discovery** - clients automatically discover available tools
- **Type safety** - JSON schemas define tool parameters
- **Better integration** with Claude Desktop, Cursor, and other MCP clients

## 1. Run the MCP Context Server

1. Build and start the MCP Context Server:
   ```sh
   cargo run --release
   ```
   The server will start using stdio transport (standard input/output) for MCP communication.

2. The server is now ready to accept MCP client connections.

### Embedding backend configuration

Semantic search and indexing use a text-embedding backend. By default the server
targets a local [Ollama](https://ollama.com) instance with the `nomic-embed-text`
model. Both can be overridden with environment variables so you can point at a
remote Ollama or a different model without rebuilding:

| Variable | Default | Purpose |
| --- | --- | --- |
| `OLLAMA_BASE_URL` | `http://localhost:11434` | Base URL of the Ollama server |
| `CONTEXT_EMBEDDING_MODEL` | `nomic-embed-text` | Embedding model to request |

```sh
OLLAMA_BASE_URL=http://ollama.internal:11434 \
CONTEXT_EMBEDDING_MODEL=mxbai-embed-large \
cargo run --release
```

Embeddings are computed lazily — only when an indexing or search tool is invoked —
so starting the server never requires Ollama to be reachable.

## 2. Connect MCP Clients

### Claude Desktop Integration

1. Add this server to your Claude Desktop configuration file:

**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "context-server-rs": {
      "command": "cargo",
      "args": ["run"],
      "cwd": "c:\\Users\\karki\\source\\repos\\local-chat-llm\\context-server-rs"
    }
  }
}
```

### VS Code with MCP Extension Integration

Add this to your VS Code settings.json for MCP extension support:

```json
{
  "mcp": {
    "servers": {
      "context-server-rs": {
        "command": "cargo",
        "args": ["run"],
        "cwd": "c:\\Users\\karki\\source\\repos\\local-chat-llm\\context-server-rs"
      }
    }
  }
}
```

### Cursor IDE Integration

Configure similar to VS Code with MCP support.

### MCP Inspector (for testing)

Use the [MCP Inspector](https://github.com/modelcontextprotocol/inspector) to test your server:

```bash
npx @modelcontextprotocol/inspector
```

## 3. Available MCP Tools

Once connected, clients can discover and use these MCP tools:

### Core Project Management

### `query_context`
Query project context based on feature area, task type, and components.

**Parameters:**
```json
{
  "project_id": "your-project-id",
  "feature_area": "authentication", 
  "task_type": "implement",
  "components": ["login", "signup"]
}
```

### `list_projects`
List all available projects in the context database.

### `create_project`
Create a new project in the context database.

**Parameters:**
```json
{
  "name": "LocalChat Flutter App",
  "description": "Privacy-first local LLM chat application",
  "repository_url": "https://github.com/user/localchat"
}
```

### Flutter-Specific Tools

### `create_flutter_component`
Create a new Flutter component and track its architecture layer.

**Parameters:**
```json
{
  "project_id": "your-project-id",
  "component_name": "ChatScreen",
  "component_type": "widget",
  "architecture_layer": "presentation",
  "file_path": "lib/presentation/pages/chat_screen.dart"
}
```

**Component Types:** `widget`, `provider`, `service`, `repository`, `model`, `utility`
**Architecture Layers:** `presentation`, `domain`, `data`, `core`

### `list_flutter_components`
List all Flutter components in a project with their architecture layers.

### `validate_architecture`
Validate Flutter Clean Architecture rules and detect dependency violations.

**Returns:** List of architecture violations (e.g., presentation layer importing directly from data layer)

### Development Phase Tracking

### `create_development_phase`
Create and track development phases for project management.

**Parameters:**
```json
{
  "project_id": "your-project-id",
  "phase_name": "Chat UI Implementation",
  "phase_order": 2,
  "description": "Build the main chat interface components"
}
```

### `list_development_phases`
List all development phases for a project in order.

### Graph Memory Tools

### `index_project`
Index a codebase directory into graph memory: parse sources, build a symbol
graph (contains/imports/calls/inherits/references edges), and best-effort embed
symbol text for semantic search.

**Parameters:**
```json
{
  "project_id": "your-project-id",
  "root_path": "/path/to/your/source/tree",
  "session_id": "optional-session-id"
}
```

Indexing is **incremental**. Each file's content hash is stored with its file
symbol, so on subsequent runs:

- **Unchanged files are skipped** — they are not re-parsed or re-embedded.
- **Deleted files are pruned** — their symbols, edges, and embeddings are removed.
- **Changed files are re-indexed** — only they are re-parsed and re-embedded.

This makes repeated `index_project` calls cheap, so agents can safely re-index a
large tree to pick up local edits without paying a full re-parse each time.

**Returns** an `IndexReport` with the following counters:

| Field | Meaning |
| --- | --- |
| `project_id` | The project that was indexed |
| `files_indexed` | Files that changed and were re-indexed this run |
| `files_skipped` | Files that were unchanged and skipped |
| `files_removed` | Previously-indexed files no longer present on disk |
| `symbols_indexed` | Symbols parsed/updated this run |
| `edges_indexed` | Graph edges created/updated this run |
| `embedded` | Symbols whose text was successfully embedded |
| `embed_failures` | Symbols whose embedding failed (e.g. Ollama unreachable) |

```json
{
  "project_id": "flutter-shop-app",
  "files_indexed": 3,
  "files_skipped": 1527,
  "files_removed": 1,
  "symbols_indexed": 214,
  "edges_indexed": 861,
  "embedded": 214,
  "embed_failures": 0
}
```

> Note: a changed file drops its incoming edges from *unchanged* source files
> until those sources themselves change. This is a deliberate trade-off of the
> incremental design; a full re-index (delete and re-create the project) rebuilds
> the complete graph.

### `search_symbols`
Search the indexed symbol graph by name (case-insensitive substring).

### `traverse_graph`
Budgeted breadth-first traversal of the symbol graph from a start symbol.

### `get_file_outline`
Return a token-efficient structural outline of one indexed file: every symbol's
kind, name, signature, and line range, with bodies omitted.

**Parameters:**
```json
{ "project_id": "your-project-id", "file_path": "/path/to/Svc.java" }
```

Use this to understand a file's shape for a few dozen tokens before reading any
source.

### `get_symbol_source`
Return the exact source of a single symbol, sliced from its file by the symbol's
line range. Pass an `id` obtained from `get_file_outline` or `search_symbols`.

**Parameters:**
```json
{ "project_id": "your-project-id", "symbol_id": "the-symbol-node-id" }
```

This reads one function/method instead of an entire file — the primary way to
keep an agent's context small.

### `semantic_search`
Semantic (cosine-similarity) search over embedded context for a project.

Ranking runs inside SQLite via the [sqlite-vec](https://github.com/asg017/sqlite-vec)
extension (`vec_distance_cosine`), so stored vectors are never materialized as
Rust structs. If the extension is unavailable, or a project's vectors are not
dimensionally consistent, the service transparently falls back to a brute-force
cosine scan.

## 4. Using with Claude Desktop or VS Code

Once configured, you can ask Claude or your MCP-enabled IDE to:

### Flutter-Specific Queries:
- "Create a new Flutter component called ChatScreen in the presentation layer"
- "List all Flutter components in my project and check for architecture violations"
- "Create development phases for my LocalChat Flutter app: Setup, Chat UI, Model Management, Polish"
- "Validate my Flutter Clean Architecture - are there any dependency violations?"

### General Context Queries:
- "Query the context for authentication implementation in my Flutter project"
- "What are the performance requirements for chat rendering in my app?"
- "Show me the architectural decisions for my project"
- "Create a new project called 'LocalChat Flutter App'"

The MCP client will automatically call the appropriate tools and provide context-aware responses.

## 5. Example MCP Interaction

Here's how an MCP client like Claude Desktop interacts with our server:

1. **Tool Discovery**: Client discovers available tools (`query_context`, `list_projects`, `create_project`)

2. **Context Query**: Client calls `query_context` tool:
   ```json
   {
     "project_id": "flutter-shop-app",
     "feature_area": "authentication", 
     "task_type": "implement",
     "components": ["login", "password_reset"]
   }
   ```

3. **Structured Response**: Server returns curated context:
   ```json
   {
     "business_rules": [
       {
         "rule_name": "Email Verification Required",
         "description": "All new accounts must verify email before activation"
       }
     ],
     "security_policies": [
       {
         "policy_name": "Password Requirements",
         "requirements": "Use bcrypt with 12 rounds minimum"
       }
     ],
     "architectural_decisions": [
       {
         "decision_title": "State Management",
         "decision": "Use BLoC pattern for authentication flows"
       }
     ]
   }
   ```

4. **AI Context**: The LLM uses this context to generate appropriate code that follows your project's rules and patterns.

## 6. Development and Testing

### Testing the Server

```bash
# Run the server
cargo run

# Test with MCP Inspector
npx @modelcontextprotocol/inspector

# Or test with a simple stdio client
echo '{"jsonrpc": "2.0", "method": "initialize", "params": {...}, "id": 1}' | cargo run
```

### Adding Context Data

You can add context data by:
1. Using the `create_project` MCP tool
2. Directly inserting into the SQLite database
3. Building additional MCP tools for data management (future enhancement)

## Key Features

### ✅ **Core MCP Implementation**
- **MCP Protocol Compliance**: Full Model Context Protocol implementation
- **SQLite Storage**: Embedded database with config directory support (`~/config/context-server-rs/`)
- **stdio Transport**: Standard input/output transport for MCP communication
- **Type Safety**: JSON schema validation for all tool parameters
- **Tool Discovery**: Automatic tool discovery by MCP clients

### ✅ **Flutter-Specific Features** 
- **Component Tracking**: Track Flutter widgets, providers, services, repositories
- **Architecture Layer Validation**: Enforce Clean Architecture dependency rules
- **Development Phase Management**: Track project phases (Setup → Chat UI → Model Management → Polish)
- **Privacy-First Validation**: Monitor for external network calls and API imports (planned)
- **Riverpod Provider Graph**: Map provider dependencies and scopes (planned)

### ✅ **AI-Assisted Development**
- **Context-Aware Code Generation**: AI agents get Flutter project structure context
- **Architecture Violation Detection**: Real-time validation of dependency rules
- **Project Phase Guidance**: Suggest next logical steps based on current development phase
- **Component Organization**: Automatic tracking of presentation/domain/data/core layers

### 🚧 **Coming Soon**
- **Privacy Rule Engine**: Automated detection of external API usage
- **Code Templates**: Generate boilerplate for widgets, providers, repositories
- **Performance Monitoring**: Track inference times and memory usage for LLM integration
- **Testing Context**: Store testing patterns and coverage information

## Examples and Troubleshooting

### 📖 **API Usage Examples**
See [`examples/api_usage.md`](examples/api_usage.md) for comprehensive examples including:
- Project creation and management
- Entity creation (business rules, components, decisions)
- Bulk operations
- Context queries
- Error handling patterns

### 🔧 **Troubleshooting Guide**
See [`examples/troubleshooting.md`](examples/troubleshooting.md) for solutions to common issues:
- **"Missing required parameter: data for create"** - Parameter format solutions
- **"FOREIGN KEY constraint failed"** - Entity relationship requirements
- **Best practices** for avoiding common API errors

### 💡 **Quick Start Examples**

**Create a project and add components:**
```json
// 1. Create project
{"action": "create", "data": {"name": "My Flutter App"}}

// 2. Add business rule  
{"entity_type": "business_rule", "data": {"project_id": "proj-123", "rule_name": "User Login"}}

// 3. Add component
{"entity_type": "framework_component", "data": {"project_id": "proj-123", "component_name": "LoginForm"}}
```
