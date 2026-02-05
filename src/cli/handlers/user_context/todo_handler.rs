use crate::models::user_context::*;
use crate::repositories::ContextualTodoRepository;
use rmcp::model::ErrorData as McpError;
use std::sync::Arc;
use chrono::Utc;

pub struct TodoHandler {
    repository: Arc<dyn ContextualTodoRepository>,
}

impl TodoHandler {
    pub fn new(repository: Arc<dyn ContextualTodoRepository>) -> Self {
        Self { repository }
    }

    /// Create a new contextual todo
    pub async fn create_todo(
        &self,
        user_id: &str,
        task_description: &str,
        context_type: &str,
        related_entity_id: Option<&str>,
        priority: Option<u32>,
    ) -> Result<ContextualTodo, McpError> {
        let todo = ContextualTodo::new(
            user_id.to_string(),
            task_description.to_string(),
            TodoContextType::from_str(context_type),
        );

        let mut todo = todo;
        todo.related_entity_id = related_entity_id.map(|s| s.to_string());
        if let Some(p) = priority {
            todo.priority = p;
        }

        self.repository.create_todo(&todo).await
    }

    /// List all todos for a user
    pub async fn list_todos(&self, user_id: &str) -> Result<Vec<ContextualTodo>, McpError> {
        self.repository.find_todos_by_user(user_id).await
    }

    /// Get a specific todo by ID
    pub async fn show_todo(&self, id: &str) -> Result<Option<ContextualTodo>, McpError> {
        self.repository.find_todo_by_id(id).await
    }

    /// Update an existing todo
    pub async fn update_todo(
        &self,
        id: &str,
        task_description: Option<&str>,
        priority: Option<u32>,
    ) -> Result<ContextualTodo, McpError> {
        let mut todo = self
            .repository
            .find_todo_by_id(id)
            .await?
            .ok_or_else(|| McpError::invalid_request("Todo not found", None))?;

        if let Some(desc) = task_description {
            todo.task_description = desc.to_string();
        }
        if let Some(p) = priority {
            todo.priority = p;
        }
        todo.updated_at = Some(Utc::now());

        self.repository.update_todo(&todo).await
    }

    /// Mark a todo as in progress
    pub async fn mark_todo_started(&self, id: &str) -> Result<(), McpError> {
        self.repository
            .update_todo_status(id, TodoStatus::InProgress.as_str())
            .await
    }

    /// Mark a todo as done
    pub async fn mark_todo_done(&self, id: &str) -> Result<(), McpError> {
        self.repository
            .update_todo_status(id, TodoStatus::Completed.as_str())
            .await
    }

    /// Update todo status
    pub async fn update_todo_status(&self, id: &str, status: &str) -> Result<(), McpError> {
        self.repository.update_todo_status(id, status).await
    }

    /// Find todos by status
    pub async fn find_by_status(
        &self,
        user_id: &str,
        status: &str,
    ) -> Result<Vec<ContextualTodo>, McpError> {
        self.repository
            .find_todos_by_status(user_id, status)
            .await
    }

    /// Find todos by project
    pub async fn find_by_project(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<ContextualTodo>, McpError> {
        self.repository
            .find_todos_by_project(user_id, project_id)
            .await
    }

    /// Find todos related to an entity
    pub async fn find_by_entity(&self, entity_id: &str) -> Result<Vec<ContextualTodo>, McpError> {
        self.repository.find_todos_by_entity(entity_id).await
    }

    /// Delete a todo
    pub async fn delete_todo(&self, id: &str) -> Result<bool, McpError> {
        self.repository.delete_todo(id).await
    }
}
