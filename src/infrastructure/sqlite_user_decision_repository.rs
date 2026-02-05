use async_trait::async_trait;
use chrono::Utc;
use rmcp::model::ErrorData as McpError;
use rusqlite::{params, OptionalExtension};
use std::sync::{Arc, Mutex};
use crate::models::user_context::*;
use crate::repositories::UserDecisionRepository;

pub struct SqliteUserDecisionRepository {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteUserDecisionRepository {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { conn }
    }

    fn row_to_decision(row: &rusqlite::Row) -> rusqlite::Result<UserDecision> {
        Ok(UserDecision {
            id: row.get(0)?,
            user_id: row.get(1)?,
            decision_text: row.get(2)?,
            reason: row.get(3)?,
            decision_category: DecisionCategory::from_str(&row.get::<_, String>(4)?),
            scope: ContextScope::from_str(&row.get::<_, String>(5)?),
            related_project_id: row.get(6)?,
            confidence_score: row.get(7)?,
            referenced_items: serde_json::from_str(&row.get::<_, String>(8)?)
                .unwrap_or_default(),
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: row
                .get::<_, Option<String>>(10)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            applied_count: row.get(11)?,
            last_applied: row
                .get::<_, Option<String>>(12)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            status: EntityStatus::from_str(&row.get::<_, String>(13)?),
        })
    }
}

#[async_trait]
impl UserDecisionRepository for SqliteUserDecisionRepository {
    async fn create_decision(&self, decision: &UserDecision) -> Result<UserDecision, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        conn.execute(
            "INSERT INTO user_decisions (
                id, user_id, decision_text, reason, decision_category, scope,
                related_project_id, confidence_score, referenced_items,
                created_at, updated_at, applied_count, last_applied, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                &decision.id,
                &decision.user_id,
                &decision.decision_text,
                &decision.reason,
                decision.decision_category.as_str(),
                decision.scope.to_string(),
                &decision.related_project_id,
                decision.confidence_score,
                serde_json::to_string(&decision.referenced_items).unwrap(),
                decision.created_at.to_rfc3339(),
                decision.updated_at.map(|dt| dt.to_rfc3339()),
                decision.applied_count,
                decision.last_applied.map(|dt| dt.to_rfc3339()),
                decision.status.as_str(),
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to create decision: {}", e), None))?;

        Ok(decision.clone())
    }

    async fn find_decision_by_id(&self, id: &str) -> Result<Option<UserDecision>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare("SELECT * FROM user_decisions WHERE id = ?1")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let decision = stmt
            .query_row([id], |row| Self::row_to_decision(row))
            .optional()
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?;

        Ok(decision)
    }

    async fn find_decisions_by_user(&self, user_id: &str) -> Result<Vec<UserDecision>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare("SELECT * FROM user_decisions WHERE user_id = ?1 ORDER BY created_at DESC")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let decisions = stmt
            .query_map([user_id], |row| Self::row_to_decision(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(decisions)
    }

    async fn find_decisions_by_scope(
        &self,
        user_id: &str,
        scope: &str,
    ) -> Result<Vec<UserDecision>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM user_decisions WHERE user_id = ?1 AND scope = ?2 ORDER BY created_at DESC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let decisions = stmt
            .query_map(params![user_id, scope], |row| Self::row_to_decision(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(decisions)
    }

    async fn find_decisions_by_category(
        &self,
        user_id: &str,
        category: &str,
    ) -> Result<Vec<UserDecision>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM user_decisions WHERE user_id = ?1 AND decision_category = ?2 ORDER BY created_at DESC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let decisions = stmt
            .query_map(params![user_id, category], |row| Self::row_to_decision(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(decisions)
    }

    async fn update_decision(&self, decision: &UserDecision) -> Result<UserDecision, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let updated_at = Utc::now();
        conn.execute(
            "UPDATE user_decisions SET decision_text = ?1, reason = ?2,
            decision_category = ?3, scope = ?4, confidence_score = ?5,
            updated_at = ?6, status = ?7 WHERE id = ?8",
            params![
                &decision.decision_text,
                &decision.reason,
                decision.decision_category.as_str(),
                decision.scope.to_string(),
                decision.confidence_score,
                updated_at.to_rfc3339(),
                decision.status.as_str(),
                &decision.id,
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to update decision: {}", e), None))?;

        Ok(decision.clone())
    }

    async fn delete_decision(&self, id: &str) -> Result<bool, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let rows_affected = conn
            .execute("DELETE FROM user_decisions WHERE id = ?1", [id])
            .map_err(|e| McpError::internal_error(format!("Failed to delete decision: {}", e), None))?;

        Ok(rows_affected > 0)
    }

    async fn increment_applied_count(&self, id: &str) -> Result<(), McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        conn.execute(
            "UPDATE user_decisions SET applied_count = applied_count + 1,
            last_applied = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to increment count: {}", e), None))?;

        Ok(())
    }

    async fn archive_decision(&self, id: &str) -> Result<(), McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        conn.execute(
            "UPDATE user_decisions SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params!["archived", Utc::now().to_rfc3339(), id],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to archive decision: {}", e), None))?;

        Ok(())
    }
}
