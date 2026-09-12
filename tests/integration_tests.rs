use tempfile::tempdir;

use context_server_rs::container::AppContainer;
use context_server_rs::db::init::init_db;

#[tokio::test]
async fn test_database_initialization() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_path_str = db_path.to_str().unwrap();

    // Test database initialization
    let result = init_db(db_path_str);
    assert!(result.is_ok(), "Database initialization should succeed");

    // Test that we can create an app container
    let container_result = AppContainer::new(db_path_str);
    assert!(
        container_result.is_ok(),
        "AppContainer creation should succeed"
    );
}

#[tokio::test]
async fn test_project_crud_operations() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_path_str = db_path.to_str().unwrap();

    // Initialize database and container
    init_db(db_path_str).unwrap();
    let container = AppContainer::new(db_path_str).unwrap();

    // Test project creation
    let project = container
        .project_service
        .create_project("Test Project", Some("A test project"), None)
        .await;

    assert!(project.is_ok(), "Project creation should succeed");
    let project = project.unwrap();
    assert_eq!(project.name, "Test Project");
    assert_eq!(project.description, Some("A test project".to_string()));

    // Test project retrieval
    let retrieved = container.project_service.get_project(&project.id).await;

    assert!(retrieved.is_ok(), "Project retrieval should succeed");
    let retrieved = retrieved.unwrap();
    assert!(retrieved.is_some(), "Project should exist");

    // Test project deletion
    let deleted = container.project_service.delete_project(&project.id).await;

    assert!(deleted.is_ok(), "Project deletion should succeed");
    assert!(deleted.unwrap(), "Project should be deleted");
}

#[tokio::test]
async fn test_framework_component_operations() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_path_str = db_path.to_str().unwrap();

    // Initialize database and container
    init_db(db_path_str).unwrap();
    let container = AppContainer::new(db_path_str).unwrap();

    // Create a test project first
    let project = container
        .project_service
        .create_project("Test Project", Some("A test project"), None)
        .await
        .unwrap();

    // Test component creation using framework_service (was component_service)
    let component = container
        .framework_service
        .create_component(
            &project.id,
            "TestWidget",
            "widget",
            "presentation",
            Some("/src/widgets/test_widget.dart"),
            None,
        )
        .await;

    assert!(component.is_ok(), "Component creation should succeed");
    let component = component.unwrap();
    assert_eq!(component.component_name, "TestWidget");
    assert_eq!(component.architecture_layer, "presentation");

    // Test component retrieval using framework_service (was component_service)
    let retrieved = container
        .framework_service
        .get_component(&component.id)
        .await;

    assert!(retrieved.is_ok(), "Component retrieval should succeed");
    assert!(retrieved.unwrap().is_some(), "Component should exist");

    // Test listing components by project using framework_service (was component_service)
    let components = container
        .framework_service
        .list_components(&project.id)
        .await;

    assert!(components.is_ok(), "Component listing should succeed");
    assert_eq!(components.unwrap().len(), 1, "Should have one component");
}

/// An embedding carries a foreign key onto `projects`, so a caller-chosen
/// project id must be registered before anything tries to embed for it.
///
/// This is the regression test for a real production failure: indexing a
/// directory wrote 1818 symbols and 14420 edges, and zero embeddings, because
/// `index_project` accepted a project id that had no `projects` row. Symbols
/// have no such key, so the graph looked healthy while semantic search stayed
/// permanently empty.
#[tokio::test]
async fn embeddings_require_a_registered_project() {
    use context_server_rs::embedding::DeterministicEmbeddingBackend;
    use context_server_rs::infrastructure::SqliteEmbeddingRepository;
    use context_server_rs::services::EmbeddingStoreService;
    use std::sync::Arc;

    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("fk.db");
    let db_path_str = db_path.to_str().unwrap();

    init_db(db_path_str).unwrap();
    let container = AppContainer::new(db_path_str).unwrap();

    let repository = Arc::new(SqliteEmbeddingRepository::new(
        container.connection_pool.clone(),
    ));
    let store = EmbeddingStoreService::new(
        repository,
        Arc::new(DeterministicEmbeddingBackend::new(32)),
        "deterministic",
        "1",
    );

    // Unregistered project id: the write is refused.
    let refused = store
        .embed_and_store(
            "sym-1",
            Some("unregistered"),
            "func main() {}",
            Some("function"),
        )
        .await;
    assert!(
        refused.is_err(),
        "an embedding for an unregistered project must not be accepted"
    );

    // Registering the id first is what makes the identical write succeed.
    container
        .project_service
        .ensure_project("unregistered")
        .await
        .unwrap();

    let stored = store
        .embed_and_store(
            "sym-1",
            Some("unregistered"),
            "func main() {}",
            Some("function"),
        )
        .await;
    assert!(
        stored.is_ok(),
        "embedding should persist once the project row exists: {stored:?}"
    );
    assert_eq!(store.count().await.unwrap(), 1);
}

#[tokio::test]
async fn ensure_project_is_idempotent() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("ensure.db");
    let db_path_str = db_path.to_str().unwrap();

    init_db(db_path_str).unwrap();
    let container = AppContainer::new(db_path_str).unwrap();

    container
        .project_service
        .ensure_project("abc")
        .await
        .unwrap();
    // A second call must not fail on the primary key.
    container
        .project_service
        .ensure_project("abc")
        .await
        .unwrap();

    let project = container
        .project_service
        .get_project("abc")
        .await
        .unwrap()
        .expect("project should be registered");
    assert_eq!(project.id, "abc");
    assert_eq!(project.name, "abc");

    assert_eq!(
        container
            .project_service
            .list_projects()
            .await
            .unwrap()
            .len(),
        1
    );
}
