use crate::models::user_context::*;
use crate::repositories::UserDecisionRepository;
use rmcp::model::ErrorData as McpError;
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

pub struct DecisionHandler {
    repository: Arc<dyn UserDecisionRepository>,
}

impl DecisionHandler {
    pub fn new(repository: Arc<dyn UserDecisionRepository>) -> Self {
        Self { repository }
    }

    /// Create a new user decision
    pub async fn create_decision(
        &self,
        user_id: &str,
        decision_text: &str,
        category: &str,
        reason: Option<&str>,
        project_id: Option<&str>,
        confidence_score: Option<f32>,
    ) -> Result<UserDecision, McpError> {
        let decision = UserDecision {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            decision_text: decision_text.to_string(),
            reason: reason.map(|s| s.to_string()),
            decision_category: DecisionCategory::from_str(category),
            related_project_id: project_id.map(|s| s.to_string()),
            confidence_score: confidence_score.unwrap_or(0.5),
            applied_count: 0,
            last_applied: None,
            created_at: Utc::now(),
            updated_at: None,
            referenced_items: vec![],
            status: EntityStatus::Active,
            scope: ContextScope::Global,
        };

        self.repository.create_decision(&decision).await
    }

    /// List all decisions for a user
    pub async fn list_decisions(&self, user_id: &str) -> Result<Vec<UserDecision>, McpError> {
        self.repository.find_decisions_by_user(user_id).await
    }

    /// Get a specific decision by ID
    pub async fn show_decision(&self, id: &str) -> Result<Option<UserDecision>, McpError> {
        self.repository.find_decision_by_id(id).await
    }

    /// Update an existing decision
    pub async fn update_decision(
        &self,
        id: &str,
        decision_text: Option<&str>,
        reason: Option<&str>,
        confidence_score: Option<f32>,
    ) -> Result<UserDecision, McpError> {
        let mut decision = self
            .repository
            .find_decision_by_id(id)
            .await?
            .ok_or_else(|| McpError::invalid_request("Decision not found", None))?;

        if let Some(text) = decision_text {
            decision.decision_text = text.to_string();
        }
        if let Some(r) = reason {
            decision.reason = Some(r.to_string());
        }
        if let Some(conf) = confidence_score {
            decision.confidence_score = conf;
        }
        decision.updated_at = Some(Utc::now());

        self.repository.update_decision(&decision).await
    }

    /// Archive a decision
    pub async fn archive_decision(&self, id: &str) -> Result<(), McpError> {
        self.repository.archive_decision(id).await
    }

    /// Delete a decision
    pub async fn delete_decision(&self, id: &str) -> Result<bool, McpError> {
        self.repository.delete_decision(id).await
    }

    /// Record that a decision was applied
    pub async fn apply_decision(&self, id: &str) -> Result<(), McpError> {
        self.repository.increment_applied_count(id).await
    }

    /// Find decisions by category
    pub async fn find_by_category(
        &self,
        user_id: &str,
        category: &str,
    ) -> Result<Vec<UserDecision>, McpError> {
        self.repository
            .find_decisions_by_category(user_id, category)
            .await
    }
}
