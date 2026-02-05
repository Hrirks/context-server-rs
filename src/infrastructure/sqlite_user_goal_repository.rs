use async_trait::async_trait;
use chrono::Utc;
use rmcp::model::ErrorData as McpError;
use rusqlite::{params, OptionalExtension};
use std::sync::{Arc, Mutex};
use crate::models::user_context::*;
use crate::repositories::UserGoalRepository;

pub struct SqliteUserGoalRepository {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteUserGoalRepository {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { conn }
    }

    fn row_to_goal(row: &rusqlite::Row) -> rusqlite::Result<UserGoal> {
        Ok(UserGoal {
            id: row.get(0)?,
            user_id: row.get(1)?,
            goal_text: row.get(2)?,
            description: row.get(3)?,
            project_id: row.get(4)?,
            status: GoalStatus::from_str(&row.get::<_, String>(5)?),
            priority: row.get(6)?,
            steps: serde_json::from_str(&row.get::<_, String>(7)?)
                .unwrap_or_default(),
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: row
                .get::<_, Option<String>>(9)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            completion_target_date: row
                .get::<_, Option<String>>(10)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            completion_date: row
                .get::<_, Option<String>>(11)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            blockers: serde_json::from_str(&row.get::<_, String>(12)?)
                .unwrap_or_default(),
            related_todos: serde_json::from_str(&row.get::<_, String>(13)?)
                .unwrap_or_default(),
        })
    }
}

#[async_trait]
impl UserGoalRepository for SqliteUserGoalRepository {
    async fn create_goal(&self, goal: &UserGoal) -> Result<UserGoal, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        conn.execute(
            "INSERT INTO user_goals (
                id, user_id, goal_text, description, project_id, status,
                priority, steps, created_at, updated_at, completion_target_date,
                completion_date, blockers, related_todos
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                &goal.id,
                &goal.user_id,
                &goal.goal_text,
                &goal.description,
                &goal.project_id,
                goal.status.as_str(),
                goal.priority,
                serde_json::to_string(&goal.steps).unwrap(),
                goal.created_at.to_rfc3339(),
                goal.updated_at.map(|dt| dt.to_rfc3339()),
                goal.completion_target_date.map(|dt| dt.to_rfc3339()),
                goal.completion_date.map(|dt| dt.to_rfc3339()),
                serde_json::to_string(&goal.blockers).unwrap(),
                serde_json::to_string(&goal.related_todos).unwrap(),
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to create goal: {}", e), None))?;

        Ok(goal.clone())
    }

    async fn find_goal_by_id(&self, id: &str) -> Result<Option<UserGoal>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare("SELECT * FROM user_goals WHERE id = ?1")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let goal = stmt
            .query_row([id], |row| Self::row_to_goal(row))
            .optional()
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?;

        Ok(goal)
    }

    async fn find_goals_by_user(&self, user_id: &str) -> Result<Vec<UserGoal>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare("SELECT * FROM user_goals WHERE user_id = ?1 ORDER BY priority ASC, created_at DESC")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let goals = stmt
            .query_map([user_id], |row| Self::row_to_goal(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(goals)
    }

    async fn find_goals_by_status(
        &self,
        user_id: &str,
        status: &str,
    ) -> Result<Vec<UserGoal>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM user_goals WHERE user_id = ?1 AND status = ?2 ORDER BY priority ASC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let goals = stmt
            .query_map(params![user_id, status], |row| Self::row_to_goal(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(goals)
    }

    async fn find_goals_by_project(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<UserGoal>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM user_goals WHERE user_id = ?1 AND project_id = ?2 ORDER BY priority ASC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let goals = stmt
            .query_map(params![user_id, project_id], |row| Self::row_to_goal(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(goals)
    }

    async fn update_goal(&self, goal: &UserGoal) -> Result<UserGoal, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let updated_at = Utc::now();
        conn.execute(
            "UPDATE user_goals SET goal_text = ?1, description = ?2, status = ?3,
            priority = ?4, steps = ?5, updated_at = ?6,
            completion_target_date = ?7, completion_date = ?8,
            blockers = ?9, related_todos = ?10 WHERE id = ?11",
            params![
                &goal.goal_text,
                &goal.description,
                goal.status.as_str(),
                goal.priority,
                serde_json::to_string(&goal.steps).unwrap(),
                updated_at.to_rfc3339(),
                goal.completion_target_date.map(|dt| dt.to_rfc3339()),
                goal.completion_date.map(|dt| dt.to_rfc3339()),
                serde_json::to_string(&goal.blockers).unwrap(),
                serde_json::to_string(&goal.related_todos).unwrap(),
                &goal.id,
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to update goal: {}", e), None))?;

        Ok(goal.clone())
    }

    async fn delete_goal(&self, id: &str) -> Result<bool, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let rows_affected = conn
            .execute("DELETE FROM user_goals WHERE id = ?1", [id])
            .map_err(|e| McpError::internal_error(format!("Failed to delete goal: {}", e), None))?;

        Ok(rows_affected > 0)
    }

    async fn update_goal_status(&self, id: &str, status: &str) -> Result<(), McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        conn.execute(
            "UPDATE user_goals SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, Utc::now().to_rfc3339(), id],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to update status: {}", e), None))?;

        Ok(())
    }
}
