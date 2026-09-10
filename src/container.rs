// Dependency Injection Container following SOLID principles
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;

use crate::db::connection_pool::ConnectionPool;
use crate::embedding::{EmbeddingService, OllamaEmbeddingBackend};

// Infrastructure layer
use crate::infrastructure::{
    SqliteAnalyticsRepository,
    SqliteArchitecturalDecisionRepository,
    SqliteBusinessRuleRepository,
    SqliteDevelopmentPhaseRepository,
    SqliteEmbeddingRepository,
    SqliteFrameworkRepository,
    SqliteGraphRepository,
    // Note: SqliteComponentRepository removed as it was identical to SqliteFrameworkRepository
    SqlitePerformanceRequirementRepository,
    SqliteProjectRepository,
    SqliteSpecificationRepository,
};

// Service layer
use crate::services::{
    analytics_service::{AnalyticsService, DefaultAnalyticsService},
    architecture_validation_service::ArchitectureValidationServiceImpl,
    context_crud_service::{ContextCrudService, ContextCrudServiceImpl},
    context_query_service::ContextQueryServiceImpl,
    development_phase_service::DevelopmentPhaseServiceImpl,
    framework_service::FrameworkServiceImpl,
    // Note: ComponentService removed as it was identical to FrameworkService
    project_service::ProjectServiceImpl,
    specification_analytics_service::{
        DefaultSpecificationAnalyticsService, SpecificationAnalyticsService,
    },
    ArchitectureValidationService,
    ContextQueryService,
    DefaultSpecificationImportService,
    DefaultSpecificationService,
    DevelopmentPhaseService,
    EmbeddingStoreService,
    FrameworkService,
    GraphMemoryService,
    ProjectService,
    SpecificationImportService,
    SpecificationService,
    SpecificationVersioningService,
    SqliteSpecificationVersioningService,
};

/// Maximum number of SQLite connections the container keeps open.
///
/// Each connection gets its own mutex, so up to this many in-process reads can
/// proceed concurrently (writes are still serialized by SQLite's write lock).
const POOL_MAX_CONNECTIONS: usize = 8;

/// How long a checkout waits for a free connection before returning an error.
const POOL_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

/// Default Ollama embedding model. Serving locally over plain HTTP.
const DEFAULT_EMBEDDING_MODEL: &str = "nomic-embed-text";

/// Schema version recorded against stored embeddings.
const DEFAULT_EMBEDDING_VERSION: &str = "1";

/// Application container holding all dependencies
pub struct AppContainer {
    // Services (following Dependency Inversion Principle)
    pub project_service: Box<dyn ProjectService>,
    #[allow(dead_code)]
    pub development_phase_service: Box<dyn DevelopmentPhaseService>,
    pub context_query_service: Box<dyn ContextQueryService>,
    pub architecture_validation_service: Box<dyn ArchitectureValidationService>,
    pub context_crud_service: Box<dyn ContextCrudService>,
    pub framework_service: Box<dyn FrameworkService>,
    pub analytics_service: Box<dyn AnalyticsService>,
    #[allow(dead_code)]
    pub specification_service: Arc<dyn SpecificationService>,
    pub specification_import_service: Arc<dyn SpecificationImportService>,
    pub specification_versioning_service: Arc<dyn SpecificationVersioningService>,
    pub specification_analytics_service: Arc<dyn SpecificationAnalyticsService>,
    pub embedding_store_service: Arc<EmbeddingStoreService>,
    pub graph_memory_service: Arc<GraphMemoryService>,
    // Note: component_service removed as it was identical to framework_service
}

impl AppContainer {
    /// Create a new application container with all dependencies injected.
    ///
    /// Uses a local Ollama embedding backend by default. Embeddings are computed
    /// lazily (only when an indexing/search tool runs), so constructing the
    /// container never makes a network call.
    pub fn new(db_path: &str) -> Result<Self> {
        Self::with_embedding_backend(
            db_path,
            Arc::new(OllamaEmbeddingBackend::local(DEFAULT_EMBEDDING_MODEL)),
        )
    }

    /// Create a container with a custom embedding backend (tests, non-Ollama).
    pub fn with_embedding_backend(
        db_path: &str,
        embedding_backend: Arc<dyn EmbeddingService>,
    ) -> Result<Self> {
        let pool = Arc::new(ConnectionPool::new(
            db_path,
            POOL_MAX_CONNECTIONS,
            POOL_ACQUIRE_TIMEOUT,
        )?);

        // Create repositories (infrastructure layer)
        let project_repository = SqliteProjectRepository::new(pool.clone());
        let development_phase_repository = SqliteDevelopmentPhaseRepository::new(pool.clone());
        let business_rule_repository = SqliteBusinessRuleRepository::new(pool.clone());
        let architectural_decision_repository =
            SqliteArchitecturalDecisionRepository::new(pool.clone());
        let performance_requirement_repository =
            SqlitePerformanceRequirementRepository::new(pool.clone());

        // Create services (application layer) - dependency injection
        let project_service = Box::new(ProjectServiceImpl::new(project_repository));

        let development_phase_service = Box::new(DevelopmentPhaseServiceImpl::new(
            development_phase_repository,
        ));

        let context_query_service = Box::new(ContextQueryServiceImpl::new(
            business_rule_repository,
            architectural_decision_repository,
            performance_requirement_repository,
        ));

        // Create framework service for architecture validation
        // Note: In a real application, you might want to use Arc<dyn FrameworkService> instead
        let framework_repository_for_validation = SqliteFrameworkRepository::new(pool.clone());
        let framework_service_for_validation =
            FrameworkServiceImpl::new(framework_repository_for_validation);
        let architecture_validation_service = Box::new(ArchitectureValidationServiceImpl::new(
            framework_service_for_validation,
        ));

        // Create CRUD services with their repositories
        let context_crud_service = Box::new(ContextCrudServiceImpl::new(
            SqliteBusinessRuleRepository::new(pool.clone()),
            SqliteArchitecturalDecisionRepository::new(pool.clone()),
            SqlitePerformanceRequirementRepository::new(pool.clone()),
        ));

        // Create framework service
        let framework_repository = SqliteFrameworkRepository::new(pool.clone());
        let framework_service = Box::new(FrameworkServiceImpl::new(framework_repository));

        // Create analytics service
        let analytics_repository = SqliteAnalyticsRepository::new(pool.clone());
        // Initialize analytics tables
        analytics_repository.init_tables()?;
        let analytics_service =
            Box::new(DefaultAnalyticsService::new(Box::new(analytics_repository)));

        // Create specification services
        let specification_repository = Arc::new(SqliteSpecificationRepository::new(pool.clone()));
        specification_repository.initialize_tables()?;

        let specification_service = Arc::new(DefaultSpecificationService::new(
            specification_repository.clone(),
        ));

        let specification_import_service = Arc::new(DefaultSpecificationImportService::new(
            specification_service.clone(),
            specification_repository.clone(),
        ));

        let specification_versioning_service =
            Arc::new(SqliteSpecificationVersioningService::new(pool.clone()));
        specification_versioning_service.initialize_tables()?;

        // Create specification analytics service
        let specification_analytics_service = Arc::new(DefaultSpecificationAnalyticsService::new(
            specification_repository.clone(),
            Arc::new(DefaultAnalyticsService::new(Box::new(
                SqliteAnalyticsRepository::new(pool.clone()),
            ))),
        ));

        // Create the embedding persistence service (Phase 4b)
        let embedding_repository = Arc::new(SqliteEmbeddingRepository::new(pool.clone()));
        embedding_repository.initialize_tables()?;
        let embedding_store_service = Arc::new(EmbeddingStoreService::new(
            embedding_repository,
            embedding_backend,
            DEFAULT_EMBEDDING_MODEL,
            DEFAULT_EMBEDDING_VERSION,
        ));

        // Create the graph memory service (Phase 5/6)
        let graph_repository = Arc::new(SqliteGraphRepository::new(pool.clone()));
        graph_repository.initialize_tables()?;
        let graph_memory_service = Arc::new(GraphMemoryService::new(
            graph_repository,
            Some(embedding_store_service.clone()),
        ));

        // Note: component_service removed as it was identical to framework_service

        Ok(AppContainer {
            project_service,
            development_phase_service,
            context_query_service,
            architecture_validation_service,
            context_crud_service,
            framework_service,
            analytics_service,
            specification_service,
            specification_import_service,
            specification_versioning_service,
            specification_analytics_service,
            embedding_store_service,
            graph_memory_service,
            // Note: component_service removed
        })
    }
}

/// Factory pattern for creating the container with proper error handling
#[allow(dead_code)]
pub struct ContainerFactory;

impl ContainerFactory {
    #[allow(dead_code)]
    pub fn create(db_path: &str) -> Result<AppContainer> {
        AppContainer::new(db_path)
    }
}
