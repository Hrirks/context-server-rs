use crate::models::user_context::*;
use crate::repositories::KnownIssueRepository;
use rmcp::model::ErrorData as McpError;
use std::sync::Arc;

pub struct IssueHandler {
    repository: Arc<dyn KnownIssueRepository>,
}

impl IssueHandler {
    pub fn new(repository: Arc<dyn KnownIssueRepository>) -> Self {
        Self { repository }
    }

    /// Create a new known issue
    pub async fn create_issue(
        &self,
        user_id: &str,
        issue_description: &str,
        category: &str,
        severity: &str,
        affected_components: Vec<String>,
    ) -> Result<KnownIssue, McpError> {
        let issue = KnownIssue::new(
            user_id.to_string(),
            issue_description.to_string(),
            IssueSeverity::from_str(severity),
            IssueCategory::from_str(category),
        );

        let mut issue = issue;
        issue.affected_components = affected_components;
        self.repository.create_issue(&issue).await
    }

    /// List all issues for a user
    pub async fn list_issues(&self, user_id: &str) -> Result<Vec<KnownIssue>, McpError> {
        self.repository.find_issues_by_user(user_id).await
    }

    /// Get a specific issue by ID
    pub async fn show_issue(&self, id: &str) -> Result<Option<KnownIssue>, McpError> {
        self.repository.find_issue_by_id(id).await
    }

    /// Update an existing issue
    pub async fn update_issue(
        &self,
        id: &str,
        issue_description: Option<&str>,
    ) -> Result<KnownIssue, McpError> {
        let mut issue = self
            .repository
            .find_issue_by_id(id)
            .await?
            .ok_or_else(|| McpError::invalid_request("Issue not found", None))?;

        if let Some(desc) = issue_description {
            issue.issue_description = desc.to_string();
        }

        self.repository.update_issue(&issue).await
    }

    /// Add a symptom to an issue
    pub async fn add_symptom(&self, issue_id: &str, symptom: &str) -> Result<KnownIssue, McpError> {
        let mut issue = self
            .repository
            .find_issue_by_id(issue_id)
            .await?
            .ok_or_else(|| McpError::invalid_request("Issue not found", None))?;

        issue.add_symptom(symptom.to_string());
        self.repository.update_issue(&issue).await
    }

    /// Add a workaround to an issue
    pub async fn add_workaround(
        &self,
        issue_id: &str,
        workaround: &str,
    ) -> Result<KnownIssue, McpError> {
        let mut issue = self
            .repository
            .find_issue_by_id(issue_id)
            .await?
            .ok_or_else(|| McpError::invalid_request("Issue not found", None))?;

        issue.workaround = Some(workaround.to_string());
        self.repository.update_issue(&issue).await
    }

    /// Mark an issue as resolved
    pub async fn mark_issue_resolved(
        &self,
        id: &str,
        status: &str,
    ) -> Result<(), McpError> {
        self.repository.mark_issue_resolved(id, status).await
    }

    /// Find issues by severity
    pub async fn find_by_severity(
        &self,
        user_id: &str,
        severity: &str,
    ) -> Result<Vec<KnownIssue>, McpError> {
        self.repository.find_issues_by_severity(user_id, severity).await
    }

    /// Find issues by category
    pub async fn find_by_category(
        &self,
        user_id: &str,
        category: &str,
    ) -> Result<Vec<KnownIssue>, McpError> {
        self.repository.find_issues_by_category(user_id, category).await
    }

    /// Find issues by status
    pub async fn find_by_status(
        &self,
        user_id: &str,
        status: &str,
    ) -> Result<Vec<KnownIssue>, McpError> {
        self.repository.find_issues_by_status(user_id, status).await
    }

    /// Delete an issue
    pub async fn delete_issue(&self, id: &str) -> Result<bool, McpError> {
        self.repository.delete_issue(id).await
    }
}
