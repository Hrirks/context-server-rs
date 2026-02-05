use crate::models::user_context::*;
use crate::repositories::UserGoalRepository;
use rmcp::model::ErrorData as McpError;
use std::sync::Arc;
use chrono::Utc;

pub struct GoalHandler {
    repository: Arc<dyn UserGoalRepository>,
}

impl GoalHandler {
    pub fn new(repository: Arc<dyn UserGoalRepository>) -> Self {
        Self { repository }
    }

    /// Create a new user goal
    pub async fn create_goal(
        &self,
        user_id: &str,
        goal_text: &str,
        description: Option<&str>,
        project_id: Option<&str>,
        priority: Option<u32>,
    ) -> Result<UserGoal, McpError> {
        let mut goal = UserGoal::new(user_id.to_string(), goal_text.to_string());
        if let Some(desc) = description {
            goal.description = Some(desc.to_string());
        }
        if let Some(pid) = project_id {
            goal.project_id = Some(pid.to_string());
        }
        if let Some(p) = priority {
            goal.priority = p.max(1).min(5);
        }

        self.repository.create_goal(&goal).await
    }

    /// List all goals for a user
    pub async fn list_goals(&self, user_id: &str) -> Result<Vec<UserGoal>, McpError> {
        self.repository.find_goals_by_user(user_id).await
    }

    /// Get a specific goal by ID
    pub async fn show_goal(&self, id: &str) -> Result<Option<UserGoal>, McpError> {
        self.repository.find_goal_by_id(id).await
    }

    /// Update an existing goal
    pub async fn update_goal(
        &self,
        id: &str,
        goal_text: Option<&str>,
        description: Option<&str>,
        priority: Option<u32>,
    ) -> Result<UserGoal, McpError> {
        let mut goal = self
            .repository
            .find_goal_by_id(id)
            .await?
            .ok_or_else(|| McpError::invalid_request("Goal not found", None))?;

        if let Some(text) = goal_text {
            goal.goal_text = text.to_string();
        }
        if let Some(d) = description {
            goal.description = Some(d.to_string());
        }
        if let Some(p) = priority {
            goal.priority = p.max(1).min(5);
        }
        goal.updated_at = Some(Utc::now());

        self.repository.update_goal(&goal).await
    }

    /// Add a step to a goal
    pub async fn add_step(
        &self,
        goal_id: &str,
        step_description: &str,
        step_number: u32,
    ) -> Result<UserGoal, McpError> {
        let mut goal = self
            .repository
            .find_goal_by_id(goal_id)
            .await?
            .ok_or_else(|| McpError::invalid_request("Goal not found", None))?;

        let step = GoalStep::new(step_number, step_description.to_string());
        goal.add_step(step);

        self.repository.update_goal(&goal).await
    }

    /// Mark a goal as started
    pub async fn mark_goal_started(&self, id: &str) -> Result<(), McpError> {
        let mut goal = self
            .repository
            .find_goal_by_id(id)
            .await?
            .ok_or_else(|| McpError::invalid_request("Goal not found", None))?;

        goal.mark_started();
        self.repository.update_goal(&goal).await?;
        Ok(())
    }

    /// Mark a goal as complete
    pub async fn mark_goal_complete(&self, id: &str) -> Result<(), McpError> {
        let mut goal = self
            .repository
            .find_goal_by_id(id)
            .await?
            .ok_or_else(|| McpError::invalid_request("Goal not found", None))?;

        goal.mark_completed();
        self.repository.update_goal(&goal).await?;
        Ok(())
    }

    /// Find goals by status
    pub async fn find_by_status(
        &self,
        user_id: &str,
        status: &str,
    ) -> Result<Vec<UserGoal>, McpError> {
        self.repository.find_goals_by_status(user_id, status).await
    }

    /// Delete a goal
    pub async fn delete_goal(&self, id: &str) -> Result<bool, McpError> {
        self.repository.delete_goal(id).await
    }
}
