use async_trait::async_trait;
use rmcp::model::ErrorData as McpError;
use crate::models::user_context::*;

#[async_trait]
pub trait UserDecisionRepository: Send + Sync {
    async fn create_decision(&self, decision: &UserDecision) -> Result<UserDecision, McpError>;
    async fn find_decision_by_id(&self, id: &str) -> Result<Option<UserDecision>, McpError>;
    async fn find_decisions_by_user(&self, user_id: &str) -> Result<Vec<UserDecision>, McpError>;
    async fn find_decisions_by_scope(
        &self,
        user_id: &str,
        scope: &str,
    ) -> Result<Vec<UserDecision>, McpError>;
    async fn find_decisions_by_category(
        &self,
        user_id: &str,
        category: &str,
    ) -> Result<Vec<UserDecision>, McpError>;
    async fn update_decision(&self, decision: &UserDecision) -> Result<UserDecision, McpError>;
    async fn delete_decision(&self, id: &str) -> Result<bool, McpError>;
    async fn increment_applied_count(&self, id: &str) -> Result<(), McpError>;
    async fn archive_decision(&self, id: &str) -> Result<(), McpError>;
}

#[async_trait]
pub trait UserGoalRepository: Send + Sync {
    async fn create_goal(&self, goal: &UserGoal) -> Result<UserGoal, McpError>;
    async fn find_goal_by_id(&self, id: &str) -> Result<Option<UserGoal>, McpError>;
    async fn find_goals_by_user(&self, user_id: &str) -> Result<Vec<UserGoal>, McpError>;
    async fn find_goals_by_status(&self, user_id: &str, status: &str)
        -> Result<Vec<UserGoal>, McpError>;
    async fn find_goals_by_project(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<UserGoal>, McpError>;
    async fn update_goal(&self, goal: &UserGoal) -> Result<UserGoal, McpError>;
    async fn delete_goal(&self, id: &str) -> Result<bool, McpError>;
    async fn update_goal_status(&self, id: &str, status: &str) -> Result<(), McpError>;
}

#[async_trait]
pub trait UserPreferenceRepository: Send + Sync {
    async fn create_preference(
        &self,
        preference: &UserPreference,
    ) -> Result<UserPreference, McpError>;
    async fn find_preference_by_id(&self, id: &str) -> Result<Option<UserPreference>, McpError>;
    async fn find_preferences_by_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserPreference>, McpError>;
    async fn find_preferences_by_scope(
        &self,
        user_id: &str,
        scope: &str,
    ) -> Result<Vec<UserPreference>, McpError>;
    async fn find_preferences_by_type(
        &self,
        user_id: &str,
        pref_type: &str,
    ) -> Result<Vec<UserPreference>, McpError>;
    async fn find_automation_applicable_preferences(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserPreference>, McpError>;
    async fn update_preference(
        &self,
        preference: &UserPreference,
    ) -> Result<UserPreference, McpError>;
    async fn delete_preference(&self, id: &str) -> Result<bool, McpError>;
    async fn increment_frequency(&self, id: &str) -> Result<(), McpError>;
}

#[async_trait]
pub trait KnownIssueRepository: Send + Sync {
    async fn create_issue(&self, issue: &KnownIssue) -> Result<KnownIssue, McpError>;
    async fn find_issue_by_id(&self, id: &str) -> Result<Option<KnownIssue>, McpError>;
    async fn find_issues_by_user(&self, user_id: &str) -> Result<Vec<KnownIssue>, McpError>;
    async fn find_issues_by_status(&self, user_id: &str, status: &str)
        -> Result<Vec<KnownIssue>, McpError>;
    async fn find_issues_by_severity(
        &self,
        user_id: &str,
        severity: &str,
    ) -> Result<Vec<KnownIssue>, McpError>;
    async fn find_issues_by_category(
        &self,
        user_id: &str,
        category: &str,
    ) -> Result<Vec<KnownIssue>, McpError>;
    async fn find_issues_by_component(
        &self,
        user_id: &str,
        component: &str,
    ) -> Result<Vec<KnownIssue>, McpError>;
    async fn update_issue(&self, issue: &KnownIssue) -> Result<KnownIssue, McpError>;
    async fn delete_issue(&self, id: &str) -> Result<bool, McpError>;
    async fn mark_issue_resolved(&self, id: &str, resolution_status: &str) -> Result<(), McpError>;
}

#[async_trait]
pub trait ContextualTodoRepository: Send + Sync {
    async fn create_todo(&self, todo: &ContextualTodo) -> Result<ContextualTodo, McpError>;
    async fn find_todo_by_id(&self, id: &str) -> Result<Option<ContextualTodo>, McpError>;
    async fn find_todos_by_user(&self, user_id: &str) -> Result<Vec<ContextualTodo>, McpError>;
    async fn find_todos_by_status(
        &self,
        user_id: &str,
        status: &str,
    ) -> Result<Vec<ContextualTodo>, McpError>;
    async fn find_todos_by_project(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<ContextualTodo>, McpError>;
    async fn find_todos_by_entity(&self, entity_id: &str)
        -> Result<Vec<ContextualTodo>, McpError>;
    async fn update_todo(&self, todo: &ContextualTodo) -> Result<ContextualTodo, McpError>;
    async fn delete_todo(&self, id: &str) -> Result<bool, McpError>;
    async fn update_todo_status(&self, id: &str, status: &str) -> Result<(), McpError>;
}
