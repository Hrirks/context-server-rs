use crate::models::context::Project;
use crate::repositories::ProjectRepository;
use async_trait::async_trait;
use rmcp::model::ErrorData as McpError;
use uuid::Uuid;

/// Service for Project operations following Single Responsibility Principle
#[async_trait]
pub trait ProjectService: Send + Sync {
    async fn create_project(
        &self,
        name: &str,
        description: Option<&str>,
        repository_url: Option<&str>,
    ) -> Result<Project, McpError>;
    #[allow(dead_code)]
    async fn get_project(&self, id: &str) -> Result<Option<Project>, McpError>;
    /// Register `id` if it is not already a project.
    ///
    /// Several tables hang off `projects(id)` by foreign key — `context_embeddings`
    /// among them. Features that accept a caller-chosen project id (indexing a
    /// directory, for example) must make that row exist first, otherwise their
    /// writes are rejected by a constraint while unrelated writes to the same id
    /// succeed. Idempotent, so it is safe to call before every such operation.
    async fn ensure_project(&self, id: &str) -> Result<(), McpError>;
    async fn list_projects(&self) -> Result<Vec<Project>, McpError>;
    #[allow(dead_code)]
    async fn update_project(&self, project: &Project) -> Result<Project, McpError>;
    #[allow(dead_code)]
    async fn delete_project(&self, id: &str) -> Result<bool, McpError>;
}

/// Implementation of ProjectService
pub struct ProjectServiceImpl<R: ProjectRepository> {
    repository: R,
}

impl<R: ProjectRepository> ProjectServiceImpl<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl<R: ProjectRepository> ProjectService for ProjectServiceImpl<R> {
    async fn create_project(
        &self,
        name: &str,
        description: Option<&str>,
        repository_url: Option<&str>,
    ) -> Result<Project, McpError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let project = Project {
            id,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            repository_url: repository_url.map(|s| s.to_string()),
            created_at: Some(now.clone()),
            updated_at: Some(now),
        };

        self.repository.create(&project).await
    }

    async fn get_project(&self, id: &str) -> Result<Option<Project>, McpError> {
        self.repository.find_by_id(id).await
    }

    async fn ensure_project(&self, id: &str) -> Result<(), McpError> {
        if self.repository.find_by_id(id).await?.is_some() {
            return Ok(());
        }
        let now = chrono::Utc::now().to_rfc3339();
        let project = Project {
            id: id.to_string(),
            name: id.to_string(),
            description: Some(format!("Auto-registered when '{id}' was first used")),
            repository_url: None,
            created_at: Some(now.clone()),
            updated_at: Some(now),
        };
        // `create` inserts; a concurrent caller may have won the race, in which
        // case the row already exists and that is exactly the desired state.
        match self.repository.create(&project).await {
            Ok(_) => Ok(()),
            Err(error) => {
                if self.repository.find_by_id(id).await?.is_some() {
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn list_projects(&self) -> Result<Vec<Project>, McpError> {
        self.repository.find_all().await
    }

    async fn update_project(&self, project: &Project) -> Result<Project, McpError> {
        self.repository.update(project).await
    }

    async fn delete_project(&self, id: &str) -> Result<bool, McpError> {
        self.repository.delete(id).await
    }
}
