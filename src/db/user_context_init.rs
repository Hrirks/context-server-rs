use rusqlite::Connection;
use std::error::Error;

pub fn init_user_context_tables(conn: &Connection) -> Result<(), Box<dyn Error>> {
    // Read and execute migration SQL
    let migration_sql = include_str!("../../migrations/001_create_user_context_tables.sql");

    // Split by semicolon and execute each statement
    for statement in migration_sql.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            conn.execute(trimmed, [])?;
        }
    }

    tracing::info!("User context tables initialized successfully");
    Ok(())
}

pub fn verify_user_context_schema(conn: &Connection) -> Result<bool, Box<dyn Error>> {
    // Check if all tables exist
    let tables = vec![
        "user_decisions",
        "user_goals",
        "user_preferences",
        "known_issues",
        "contextual_todos",
        "user_context_audit",
    ];

    for table in tables {
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        )?;

        if count == 0 {
            tracing::warn!("Table {} not found", table);
            return Ok(false);
        }
    }

    tracing::info!("User context schema verified");
    Ok(true)
}
