use crate::db::connection_pool::ConnectionPool;
use crate::models::context::Project;
use crate::repositories::ProjectRepository;
use async_trait::async_trait;
use rmcp::model::ErrorData as McpError;
use std::sync::Arc;

/// SQLite implementation of ProjectRepository
pub struct SqliteProjectRepository {
    pool: Arc<ConnectionPool>,
}

impl SqliteProjectRepository {
    pub fn new(pool: Arc<ConnectionPool>) -> Self {
        Self { pool }
    }

    fn checkout(&self) -> Result<crate::db::connection_pool::PooledConnection, McpError> {
        self.pool.checkout().map_err(|e| {
            McpError::internal_error(format!("Failed to acquire database connection: {e}"), None)
        })
    }
}

#[async_trait]
impl ProjectRepository for SqliteProjectRepository {
    async fn create(&self, project: &Project) -> Result<Project, McpError> {
        let db = self.checkout()?;
        let db = db.lock().unwrap();

        db.execute(
            "INSERT INTO projects (id, name, description, repository_url, created_at, updated_at, allowed_root) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                &project.id,
                &project.name,
                project.description.as_deref(),
                project.repository_url.as_deref(),
                project.created_at.as_deref(),
                project.updated_at.as_deref(),
                project.allowed_root.as_deref(),
            ),
        ).map_err(|e| McpError::internal_error(format!("Database error: {}", e), None))?;

        Ok(project.clone())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Project>, McpError> {
        let db = self.checkout()?;
        let db = db.lock().unwrap();

        let mut stmt = db.prepare("SELECT id, name, description, repository_url, created_at, updated_at, allowed_root FROM projects WHERE id = ?")
            .map_err(|e| McpError::internal_error(format!("Database error: {}", e), None))?;

        let mut project_iter = stmt
            .query_map([id], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    repository_url: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    allowed_root: row.get(6)?,
                })
            })
            .map_err(|e| McpError::internal_error(format!("Database error: {}", e), None))?;

        match project_iter.next() {
            Some(Ok(project)) => Ok(Some(project)),
            Some(Err(e)) => Err(McpError::internal_error(
                format!("Database error: {}", e),
                None,
            )),
            None => Ok(None),
        }
    }

    async fn find_all(&self) -> Result<Vec<Project>, McpError> {
        let db = self.checkout()?;
        let db = db.lock().unwrap();
        let mut projects = Vec::new();

        let mut stmt = db.prepare("SELECT id, name, description, repository_url, created_at, updated_at, allowed_root FROM projects")
            .map_err(|e| McpError::internal_error(format!("Database error: {}", e), None))?;

        let project_rows = stmt
            .query_map([], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    repository_url: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    allowed_root: row.get(6)?,
                })
            })
            .map_err(|e| McpError::internal_error(format!("Database error: {}", e), None))?;

        for project in project_rows {
            match project {
                Ok(project) => projects.push(project),
                Err(e) => tracing::warn!("Failed to parse project: {}", e),
            }
        }

        Ok(projects)
    }

    async fn update(&self, project: &Project) -> Result<Project, McpError> {
        let db = self.checkout()?;
        let db = db.lock().unwrap();

        db.execute(
            "UPDATE projects SET name = ?, description = ?, repository_url = ?, updated_at = ?, allowed_root = ? WHERE id = ?",
            (
                &project.name,
                project.description.as_deref(),
                project.repository_url.as_deref(),
                project.updated_at.as_deref(),
                project.allowed_root.as_deref(),
                &project.id,
            ),
        ).map_err(|e| McpError::internal_error(format!("Database error: {}", e), None))?;

        Ok(project.clone())
    }

    async fn delete(&self, id: &str) -> Result<bool, McpError> {
        let db = self.checkout()?;
        let db = db.lock().unwrap();

        let rows_affected = db
            .execute("DELETE FROM projects WHERE id = ?", [id])
            .map_err(|e| McpError::internal_error(format!("Database error: {}", e), None))?;

        Ok(rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection_pool::ConnectionPool;
    use std::time::Duration;

    fn repository() -> SqliteProjectRepository {
        let pool = Arc::new(ConnectionPool::new(":memory:", 1, Duration::from_secs(1)).unwrap());
        {
            let conn = pool.checkout().unwrap();
            let conn = conn.lock().unwrap();
            crate::db::init::apply_schema(&conn).unwrap();
        }
        SqliteProjectRepository::new(pool)
    }

    fn project(id: &str) -> Project {
        Project {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            repository_url: None,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            allowed_root: None,
        }
    }

    #[tokio::test]
    async fn a_registered_root_survives_a_round_trip() {
        let repository = repository();
        repository.create(&project("p1")).await.unwrap();

        // A fresh project trusts nothing until a root is registered.
        let stored = repository.find_by_id("p1").await.unwrap().unwrap();
        assert_eq!(stored.allowed_root, None);

        let mut registered = stored;
        registered.allowed_root = Some("/srv/app".to_string());
        repository.update(&registered).await.unwrap();

        let stored = repository.find_by_id("p1").await.unwrap().unwrap();
        assert_eq!(stored.allowed_root.as_deref(), Some("/srv/app"));

        // The list path reads the same column, so it has to agree.
        let all = repository.find_all().await.unwrap();
        assert_eq!(all[0].allowed_root.as_deref(), Some("/srv/app"));
    }
}
