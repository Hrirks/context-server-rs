use async_trait::async_trait;
use chrono::Utc;
use rmcp::model::ErrorData as McpError;
use rusqlite::{params, OptionalExtension};
use std::sync::{Arc, Mutex};
use crate::models::user_context::*;
use crate::repositories::KnownIssueRepository;

pub struct SqliteKnownIssueRepository {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteKnownIssueRepository {
    pub fn new(conn: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { conn }
    }

    fn row_to_issue(row: &rusqlite::Row) -> rusqlite::Result<KnownIssue> {
        Ok(KnownIssue {
            id: row.get(0)?,
            user_id: row.get(1)?,
            issue_description: row.get(2)?,
            symptoms: serde_json::from_str(&row.get::<_, String>(3)?)
                .unwrap_or_default(),
            root_cause: row.get(4)?,
            workaround: row.get(5)?,
            permanent_solution: row.get(6)?,
            affected_components: serde_json::from_str(&row.get::<_, String>(7)?)
                .unwrap_or_default(),
            severity: IssueSeverity::from_str(&row.get::<_, String>(8)?),
            issue_category: IssueCategory::from_str(&row.get::<_, String>(9)?),
            learned_date: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(10)?)
                .unwrap()
                .with_timezone(&Utc),
            resolution_status: ResolutionStatus::from_str(&row.get::<_, String>(11)?),
            resolution_date: row
                .get::<_, Option<String>>(12)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            prevention_notes: row.get(13)?,
            project_contexts: serde_json::from_str(&row.get::<_, String>(14)?)
                .unwrap_or_default(),
        })
    }
}

#[async_trait]
impl KnownIssueRepository for SqliteKnownIssueRepository {
    async fn create_issue(&self, issue: &KnownIssue) -> Result<KnownIssue, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        conn.execute(
            "INSERT INTO known_issues (
                id, user_id, issue_description, symptoms, root_cause, workaround,
                permanent_solution, affected_components, severity, issue_category,
                learned_date, resolution_status, resolution_date, prevention_notes,
                project_contexts, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                &issue.id,
                &issue.user_id,
                &issue.issue_description,
                serde_json::to_string(&issue.symptoms).unwrap(),
                &issue.root_cause,
                &issue.workaround,
                &issue.permanent_solution,
                serde_json::to_string(&issue.affected_components).unwrap(),
                issue.severity.as_str(),
                issue.issue_category.as_str(),
                issue.learned_date.to_rfc3339(),
                issue.resolution_status.as_str(),
                issue.resolution_date.map(|dt| dt.to_rfc3339()),
                &issue.prevention_notes,
                serde_json::to_string(&issue.project_contexts).unwrap(),
                Utc::now().to_rfc3339(),
                None::<String>,
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to create issue: {}", e), None))?;

        Ok(issue.clone())
    }

    async fn find_issue_by_id(&self, id: &str) -> Result<Option<KnownIssue>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare("SELECT * FROM known_issues WHERE id = ?1")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let issue = stmt
            .query_row([id], |row| Self::row_to_issue(row))
            .optional()
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?;

        Ok(issue)
    }

    async fn find_issues_by_user(&self, user_id: &str) -> Result<Vec<KnownIssue>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare("SELECT * FROM known_issues WHERE user_id = ?1 ORDER BY learned_date DESC")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let issues = stmt
            .query_map([user_id], |row| Self::row_to_issue(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(issues)
    }

    async fn find_issues_by_status(
        &self,
        user_id: &str,
        status: &str,
    ) -> Result<Vec<KnownIssue>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM known_issues WHERE user_id = ?1 AND resolution_status = ?2 ORDER BY severity DESC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let issues = stmt
            .query_map(params![user_id, status], |row| Self::row_to_issue(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(issues)
    }

    async fn find_issues_by_severity(
        &self,
        user_id: &str,
        severity: &str,
    ) -> Result<Vec<KnownIssue>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM known_issues WHERE user_id = ?1 AND severity = ?2 ORDER BY learned_date DESC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let issues = stmt
            .query_map(params![user_id, severity], |row| Self::row_to_issue(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(issues)
    }

    async fn find_issues_by_component(
        &self,
        user_id: &str,
        component: &str,
    ) -> Result<Vec<KnownIssue>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        // Note: SQLite doesn't have built-in JSON array search, so we do it in memory
        let mut stmt = conn
            .prepare("SELECT * FROM known_issues WHERE user_id = ?1 ORDER BY learned_date DESC")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let issues = stmt
            .query_map([user_id], |row| Self::row_to_issue(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(issues
            .into_iter()
            .filter(|i| i.affected_components.contains(&component.to_string()))
            .collect())
    }

    async fn update_issue(&self, issue: &KnownIssue) -> Result<KnownIssue, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let updated_at = Utc::now();
        conn.execute(
            "UPDATE known_issues SET issue_description = ?1, symptoms = ?2,
            root_cause = ?3, workaround = ?4, permanent_solution = ?5,
            affected_components = ?6, severity = ?7, resolution_status = ?8,
            resolution_date = ?9, prevention_notes = ?10, updated_at = ?11 WHERE id = ?12",
            params![
                &issue.issue_description,
                serde_json::to_string(&issue.symptoms).unwrap(),
                &issue.root_cause,
                &issue.workaround,
                &issue.permanent_solution,
                serde_json::to_string(&issue.affected_components).unwrap(),
                issue.severity.as_str(),
                issue.resolution_status.as_str(),
                issue.resolution_date.map(|dt| dt.to_rfc3339()),
                &issue.prevention_notes,
                updated_at.to_rfc3339(),
                &issue.id,
            ],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to update issue: {}", e), None))?;

        Ok(issue.clone())
    }

    async fn delete_issue(&self, id: &str) -> Result<bool, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let rows_affected = conn
            .execute("DELETE FROM known_issues WHERE id = ?1", [id])
            .map_err(|e| McpError::internal_error(format!("Failed to delete issue: {}", e), None))?;

        Ok(rows_affected > 0)
    }

    async fn mark_issue_resolved(&self, id: &str, resolution_status: &str) -> Result<(), McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        conn.execute(
            "UPDATE known_issues SET resolution_status = ?1, resolution_date = ?2 WHERE id = ?3",
            params![resolution_status, Utc::now().to_rfc3339(), id],
        )
        .map_err(|e| McpError::internal_error(format!("Failed to mark resolved: {}", e), None))?;

        Ok(())
    }

    async fn find_issues_by_category(
        &self,
        user_id: &str,
        category: &str,
    ) -> Result<Vec<KnownIssue>, McpError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| McpError::internal_error(format!("Lock error: {}", e), None))?;

        let mut stmt = conn
            .prepare(
                "SELECT * FROM known_issues WHERE user_id = ?1 AND issue_category = ?2 ORDER BY learned_date DESC",
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let issues = stmt
            .query_map(params![user_id, category], |row| Self::row_to_issue(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(issues)
    }
}
