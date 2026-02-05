use async_trait::async_trait;
use chrono::Utc;
use rmcp::model::ErrorData as McpError;
use rusqlite::{params, OptionalExtension};
use std::sync::{Arc, Mutex};
use crate::models::user_context::*;
use crate::repositories::UserPreferenceRepository;

pub struct SqliteUserPreferenceRepository {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteUserPreferenceRepository {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { conn }
    }

    fn row_to_preference(row: &rusqlite::Row) -> rusqlite::Result<UserPreference> {
        Ok(UserPreference {
            id: row.get(0)?,
            user_id: row.get(1)?,
            preference_name: row.get(2)?,
            preference_value: row.get(3)?,
            preference_type: PreferenceType::from_str(&row.get::<_, String>(4)?),
            scope: ContextScope::from_str(&row.get::<_, String>(5)?),
            applies_to_automation: row.get(6)?,
            rationale: row.get(7)?,
            priority: row.get(8)?,
            frequency_observed: row.get(9)?,
            tags: serde_json::from_str(&row.get::<_, String>(10)?)
                .unwrap_or_default(),
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: row
                .get::<_, Option<String>>(12)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            last_referenced: row
                .get::<_, Option<String>>(13)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
        })
    }
}

#[async_trait]
impl UserPreferenceRepository for SqliteUserPreferenceRepository {
    async fn create_preference(
        &self,
        preference: &UserPreference,
    ) -> Result<UserPreference, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        conn.execute(
            "INSERT INTO user_preferences (
                id, user_id, preference_name, preference_value, preference_type, scope,
                applies_to_automation, rationale, priority, frequency_observed,
                tags, created_at, updated_at, last_referenced
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                &preference.id,
                &preference.user_id,
                &preference.preference_name,
                &preference.preference_value,
                preference.preference_type.as_str(),
                preference.scope.to_string(),
                preference.applies_to_automation,
                &preference.rationale,
                preference.priority,
                preference.frequency_observed,
                serde_json::to_string(&preference.tags).unwrap(),
                preference.created_at.to_rfc3339(),
                preference.updated_at.map(|dt| dt.to_rfc3339()),
                preference.last_referenced.map(|dt| dt.to_rfc3339()),
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to create preference: {}", e), None))?;

        Ok(preference.clone())
    }

    async fn find_preference_by_id(&self, id: &str) -> Result<Option<UserPreference>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare("SELECT * FROM user_preferences WHERE id = ?1")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let pref = stmt
            .query_row([id], |row| Self::row_to_preference(row))
            .optional()
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?;

        Ok(pref)
    }

    async fn find_preferences_by_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserPreference>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare("SELECT * FROM user_preferences WHERE user_id = ?1 ORDER BY priority ASC")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let prefs = stmt
            .query_map([user_id], |row| Self::row_to_preference(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(prefs)
    }

    async fn find_preferences_by_scope(
        &self,
        user_id: &str,
        scope: &str,
    ) -> Result<Vec<UserPreference>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM user_preferences WHERE user_id = ?1 AND scope = ?2 ORDER BY priority ASC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let prefs = stmt
            .query_map(params![user_id, scope], |row| Self::row_to_preference(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(prefs)
    }

    async fn find_preferences_by_type(
        &self,
        user_id: &str,
        pref_type: &str,
    ) -> Result<Vec<UserPreference>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM user_preferences WHERE user_id = ?1 AND preference_type = ?2 ORDER BY priority ASC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let prefs = stmt
            .query_map(params![user_id, pref_type], |row| Self::row_to_preference(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(prefs)
    }

    async fn update_preference(
        &self,
        preference: &UserPreference,
    ) -> Result<UserPreference, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let updated_at = Utc::now();
        conn.execute(
            "UPDATE user_preferences SET preference_value = ?1, rationale = ?2,
            priority = ?3, frequency_observed = ?4, tags = ?5,
            updated_at = ?6, applies_to_automation = ?7 WHERE id = ?8",
            params![
                &preference.preference_value,
                &preference.rationale,
                preference.priority,
                preference.frequency_observed,
                serde_json::to_string(&preference.tags).unwrap(),
                updated_at.to_rfc3339(),
                preference.applies_to_automation,
                &preference.id,
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to update preference: {}", e), None))?;

        Ok(preference.clone())
    }

    async fn delete_preference(&self, id: &str) -> Result<bool, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let rows_affected = conn
            .execute("DELETE FROM user_preferences WHERE id = ?1", [id])
            .map_err(|e| McpError::internal_error(format!("Failed to delete preference: {}", e), None))?;

        Ok(rows_affected > 0)
    }

    async fn increment_frequency(&self, id: &str) -> Result<(), McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        conn.execute(
            "UPDATE user_preferences SET frequency_observed = frequency_observed + 1,
            last_referenced = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to increment frequency: {}", e), None))?;

        Ok(())
    }

    async fn find_automation_applicable_preferences(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserPreference>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM user_preferences WHERE user_id = ?1 AND applies_to_automation = 1 ORDER BY frequency_observed DESC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let prefs = stmt
            .query_map([user_id], |row| Self::row_to_preference(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(prefs)
    }
}
