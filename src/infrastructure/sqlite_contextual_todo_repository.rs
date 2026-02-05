use async_trait::async_trait;
use chrono::Utc;
use rmcp::model::ErrorData as McpError;
use rusqlite::{params, OptionalExtension};
use std::sync::{Arc, Mutex};
use crate::models::user_context::*;
use crate::repositories::ContextualTodoRepository;

pub struct SqliteContextualTodoRepository {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteContextualTodoRepository {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { conn }
    }

    fn row_to_todo(row: &rusqlite::Row) -> rusqlite::Result<ContextualTodo> {
        Ok(ContextualTodo {
            id: row.get(0)?,
            user_id: row.get(1)?,
            task_description: row.get(2)?,
            context_type: TodoContextType::from_str(&row.get::<_, String>(3)?),
            related_entity_id: row.get(4)?,
            related_entity_type: row
                .get::<_, Option<String>>(5)?
                .map(|s| EntityType::from_str(&s)),
            project_id: row.get(6)?,
            assigned_to: row.get(7)?,
            due_date: row
                .get::<_, Option<String>>(8)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            status: TodoStatus::from_str(&row.get::<_, String>(9)?),
            priority: row.get(10)?,
            created_from_conversation_date: row
                .get::<_, Option<String>>(11)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(12)?)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: row
                .get::<_, Option<String>>(13)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            completion_date: row
                .get::<_, Option<String>>(14)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
        })
    }
}

#[async_trait]
impl ContextualTodoRepository for SqliteContextualTodoRepository {
    async fn create_todo(&self, todo: &ContextualTodo) -> Result<ContextualTodo, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        conn.execute(
            "INSERT INTO contextual_todos (
                id, user_id, task_description, context_type, related_entity_id,
                related_entity_type, project_id, assigned_to, due_date, status,
                priority, created_from_conversation_date, created_at, updated_at,
                completion_date
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                &todo.id,
                &todo.user_id,
                &todo.task_description,
                todo.context_type.as_str(),
                &todo.related_entity_id,
                todo.related_entity_type.as_ref().map(|t| t.as_str()),
                &todo.project_id,
                &todo.assigned_to,
                todo.due_date.map(|dt| dt.to_rfc3339()),
                todo.status.as_str(),
                todo.priority,
                todo.created_from_conversation_date.map(|dt| dt.to_rfc3339()),
                todo.created_at.to_rfc3339(),
                todo.updated_at.map(|dt| dt.to_rfc3339()),
                todo.completion_date.map(|dt| dt.to_rfc3339()),
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to create todo: {}", e), None))?;

        Ok(todo.clone())
    }

    async fn find_todo_by_id(&self, id: &str) -> Result<Option<ContextualTodo>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare("SELECT * FROM contextual_todos WHERE id = ?1")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let todo = stmt
            .query_row([id], |row| Self::row_to_todo(row))
            .optional()
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?;

        Ok(todo)
    }

    async fn find_todos_by_user(&self, user_id: &str) -> Result<Vec<ContextualTodo>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare("SELECT * FROM contextual_todos WHERE user_id = ?1 ORDER BY priority ASC, due_date ASC")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let todos = stmt
            .query_map([user_id], |row| Self::row_to_todo(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(todos)
    }

    async fn find_todos_by_status(
        &self,
        user_id: &str,
        status: &str,
    ) -> Result<Vec<ContextualTodo>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM contextual_todos WHERE user_id = ?1 AND status = ?2 ORDER BY priority ASC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let todos = stmt
            .query_map(params![user_id, status], |row| Self::row_to_todo(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(todos)
    }

    async fn find_todos_by_project(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<ContextualTodo>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM contextual_todos WHERE user_id = ?1 AND project_id = ?2 ORDER BY priority ASC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let todos = stmt
            .query_map(params![user_id, project_id], |row| Self::row_to_todo(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(todos)
    }

    async fn find_todos_by_entity(&self, entity_id: &str) -> Result<Vec<ContextualTodo>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM contextual_todos WHERE related_entity_id = ?1 ORDER BY created_at DESC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let todos = stmt
            .query_map([entity_id], |row| Self::row_to_todo(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(todos)
    }

    async fn update_todo(&self, todo: &ContextualTodo) -> Result<ContextualTodo, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let updated_at = Utc::now();
        conn.execute(
            "UPDATE contextual_todos SET task_description = ?1, status = ?2,
            priority = ?3, due_date = ?4, assigned_to = ?5, updated_at = ?6,
            completion_date = ?7 WHERE id = ?8",
            params![
                &todo.task_description,
                todo.status.as_str(),
                todo.priority,
                todo.due_date.map(|dt| dt.to_rfc3339()),
                &todo.assigned_to,
                updated_at.to_rfc3339(),
                todo.completion_date.map(|dt| dt.to_rfc3339()),
                &todo.id,
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to update todo: {}", e), None))?;

        Ok(todo.clone())
    }

    async fn delete_todo(&self, id: &str) -> Result<bool, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let rows_affected = conn
            .execute("DELETE FROM contextual_todos WHERE id = ?1", [id])
            .map_err(|e| McpError::internal_error(format!("Failed to delete todo: {}", e), None))?;

        Ok(rows_affected > 0)
    }

    async fn update_todo_status(&self, id: &str, status: &str) -> Result<(), McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        conn.execute(
            "UPDATE contextual_todos SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, Utc::now().to_rfc3339(), id],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to update status: {}", e), None))?;

        Ok(())
    }
}
