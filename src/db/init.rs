// Database initialization logic for context tables
use crate::db::connection;
use rusqlite::{Connection, Result};

pub fn init_db(db_path: &str) -> Result<Connection> {
    // Goes through the centralized opener so WAL / busy_timeout / foreign_keys
    // are applied - never a bare Connection::open.
    let conn = connection::open(db_path)?;
    apply_schema(&conn)?;
    Ok(conn)
}

/// Create every table and index the server expects on an existing connection.
///
/// Idempotent (`IF NOT EXISTS`), so it is safe against a live database. Split
/// out from [`init_db`] so tests can put the *real* schema - foreign keys
/// included - on a connection they own, instead of hand-rolling a subset and
/// discovering at runtime that a constraint they never exercised rejects the
/// write.
pub fn apply_schema(conn: &Connection) -> Result<()> {
    // Projects table
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            repository_url TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );
    "#,
    )?;

    // Business Rules
    conn.execute_batch(r#"
        CREATE TABLE IF NOT EXISTS business_rules (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            rule_name TEXT NOT NULL,
            description TEXT,
            domain_area TEXT,
            implementation_pattern TEXT,
            constraints TEXT,
            examples TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        CREATE TABLE IF NOT EXISTS architectural_decisions (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            decision_title TEXT NOT NULL,
            context TEXT,
            decision TEXT,
            consequences TEXT,
            alternatives_considered TEXT,
            status TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        CREATE TABLE IF NOT EXISTS performance_requirements (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            component_area TEXT,
            requirement_type TEXT,
            target_value TEXT,
            optimization_patterns TEXT,
            avoid_patterns TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        CREATE TABLE IF NOT EXISTS security_policies (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            policy_name TEXT NOT NULL,
            policy_area TEXT,
            requirements TEXT,
            implementation_pattern TEXT,
            forbidden_patterns TEXT,
            compliance_notes TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        CREATE TABLE IF NOT EXISTS project_conventions (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            convention_type TEXT,
            convention_rule TEXT,
            good_examples TEXT,
            bad_examples TEXT,
            rationale TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        CREATE TABLE IF NOT EXISTS feature_context (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            feature_name TEXT NOT NULL,
            business_purpose TEXT,
            user_personas TEXT,
            key_workflows TEXT,
            integration_points TEXT,
            edge_cases TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        
        -- Framework-agnostic component tables
        CREATE TABLE IF NOT EXISTS framework_components (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            component_name TEXT NOT NULL,
            component_type TEXT NOT NULL, -- 'widget', 'provider', 'service', 'repository', 'model', 'utility'
            architecture_layer TEXT NOT NULL, -- 'presentation', 'domain', 'data', 'core'
            file_path TEXT,
            dependencies TEXT, -- JSON array of dependencies
            metadata TEXT, -- JSON metadata for framework-specific properties
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        
        -- Flutter-specific context tables (legacy)
        CREATE TABLE IF NOT EXISTS flutter_components (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            component_name TEXT NOT NULL,
            component_type TEXT NOT NULL, -- 'widget', 'provider', 'service', 'repository'
            architecture_layer TEXT NOT NULL, -- 'presentation', 'domain', 'data', 'core'
            file_path TEXT,
            dependencies TEXT, -- JSON array of dependencies
            riverpod_scope TEXT, -- 'global', 'scoped', 'local'
            widget_type TEXT, -- 'StatelessWidget', 'StatefulWidget', 'ConsumerWidget'
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        
        CREATE TABLE IF NOT EXISTS development_phases (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            phase_name TEXT NOT NULL, -- 'Setup', 'Chat UI', 'Model Management', 'Polish'
            phase_order INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'in_progress', 'completed', 'blocked'
            description TEXT,
            completion_criteria TEXT, -- JSON array of criteria
            dependencies TEXT, -- JSON array of phase dependencies
            started_at TEXT,
            completed_at TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        
        CREATE TABLE IF NOT EXISTS privacy_rules (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            rule_name TEXT NOT NULL,
            rule_type TEXT NOT NULL, -- 'forbidden_import', 'required_local_storage', 'data_flow'
            pattern TEXT NOT NULL, -- import pattern, storage key, etc.
            description TEXT,
            severity TEXT DEFAULT 'error', -- 'error', 'warning', 'info'
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        
        CREATE TABLE IF NOT EXISTS privacy_violations (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            rule_id TEXT NOT NULL,
            file_path TEXT NOT NULL,
            line_number INTEGER,
            violation_text TEXT,
            status TEXT DEFAULT 'open', -- 'open', 'resolved', 'suppressed'
            detected_at TEXT DEFAULT (datetime('now')),
            resolved_at TEXT,
            FOREIGN KEY (project_id) REFERENCES projects(id),
            FOREIGN KEY (rule_id) REFERENCES privacy_rules(id)
        );
        
        CREATE TABLE IF NOT EXISTS architecture_layers (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            layer_name TEXT NOT NULL, -- 'presentation', 'domain', 'data', 'core'
            allowed_dependencies TEXT, -- JSON array of allowed layer dependencies
            forbidden_imports TEXT, -- JSON array of forbidden import patterns
            description TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        
        CREATE TABLE IF NOT EXISTS model_context (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            model_name TEXT NOT NULL,
            model_path TEXT,
            model_type TEXT, -- 'GGUF', 'ONNX', etc.
            model_size TEXT,
            performance_metrics TEXT, -- JSON with inference times, memory usage
            configuration TEXT, -- JSON with model settings
            is_active BOOLEAN DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        
        CREATE TABLE IF NOT EXISTS code_templates (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            template_name TEXT NOT NULL,
            template_type TEXT NOT NULL, -- 'widget', 'provider', 'repository', 'test'
            template_content TEXT NOT NULL,
            variables TEXT, -- JSON array of template variables
            description TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id)
        );
        
        -- Vector embeddings table for semantic search
        CREATE TABLE IF NOT EXISTS context_embeddings (
            id TEXT PRIMARY KEY,
            context_id TEXT NOT NULL,
            project_id TEXT,
            embedding_vector TEXT NOT NULL, -- JSON array of floats
            embedding_model TEXT NOT NULL,
            embedding_version TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            content_type TEXT,
            content_length INTEGER,
            tokenization_method TEXT,
            preprocessing_steps TEXT, -- JSON array
            quality_score REAL,
            custom_metadata TEXT, -- JSON object
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
        
        -- Analytics events table for usage tracking
        CREATE TABLE IF NOT EXISTS analytics_events (
            id TEXT PRIMARY KEY,
            event_type TEXT NOT NULL,
            project_id TEXT,
            entity_type TEXT,
            entity_id TEXT,
            user_agent TEXT,
            metadata TEXT,
            timestamp TEXT NOT NULL,
            duration_ms INTEGER,
            success BOOLEAN NOT NULL,
            error_message TEXT
        );

        -- Create indexes for better query performance
        CREATE INDEX IF NOT EXISTS idx_analytics_events_project_id ON analytics_events(project_id);
        CREATE INDEX IF NOT EXISTS idx_analytics_events_entity ON analytics_events(entity_type, entity_id);
        CREATE INDEX IF NOT EXISTS idx_analytics_events_timestamp ON analytics_events(timestamp);
    "#)?;
    reconcile_columns(conn)?;
    stamp_schema_version(conn)?;
    Ok(())
}

/// Columns added to existing tables after their first release.
///
/// `CREATE TABLE IF NOT EXISTS` leaves an existing table exactly as it was, so
/// a database created by an older build never gains a column the current build
/// writes to - and the failing write only happens on that machine, because a
/// test suite that builds a fresh database never sees it. Each entry is added
/// only when missing. Add to this list rather than editing an earlier entry.
const ADDED_COLUMNS: &[(&str, &str, &str)] = &[
    ("symbol_edges", "weight", "REAL NOT NULL DEFAULT 1.0"),
    ("symbol_edges", "metadata", "TEXT"),
    (
        "context_symbols",
        "created_at",
        "TEXT DEFAULT (datetime('now'))",
    ),
    (
        "context_embeddings",
        "embedding_version",
        "TEXT NOT NULL DEFAULT '1'",
    ),
    (
        "context_embeddings",
        "created_at",
        "TEXT DEFAULT (datetime('now'))",
    ),
];

/// Shape of the schema this build expects. Bump it whenever a table changes
/// shape, so a database written by an older build can be recognised.
const SCHEMA_VERSION: i64 = 1;

/// Schema version recorded in the database; 0 means it predates versioning.
pub fn schema_version(conn: &Connection) -> Result<i64> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
}

fn reconcile_columns(conn: &Connection) -> Result<()> {
    for (table, column, declaration) in ADDED_COLUMNS {
        if !table_exists(conn, table)? || column_exists(conn, table, column)? {
            continue;
        }
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {declaration};"
        ))?;
    }
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for name in rows {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn stamp_schema_version(conn: &Connection) -> Result<()> {
    let previous = schema_version(conn)?;
    if previous != SCHEMA_VERSION {
        // Worth a line in the log: a database brought forward from an older
        // build is the first thing to suspect when behaviour differs.
        tracing::info!("Database schema version {previous} brought up to {SCHEMA_VERSION}");
    }
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_database_missing_a_column_is_brought_up_to_date() {
        // A database written before `weight` existed: the table is there, the
        // column is not, and CREATE TABLE IF NOT EXISTS will never add it.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE symbol_edges (id TEXT PRIMARY KEY, edge_type TEXT NOT NULL);
             INSERT INTO symbol_edges (id, edge_type) VALUES ('e1', 'calls');",
        )
        .unwrap();

        apply_schema(&conn).unwrap();

        assert!(column_exists(&conn, "symbol_edges", "weight").unwrap());
        // The row written before the column existed picks up the default, so an
        // old database stays readable rather than failing its next query.
        let weight: f64 = conn
            .query_row(
                "SELECT weight FROM symbol_edges WHERE id = 'e1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(weight, 1.0);
    }

    #[test]
    fn applying_the_schema_twice_stamps_one_version() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(schema_version(&conn).unwrap(), 0);

        apply_schema(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);

        apply_schema(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }
}
