// Service layer modules following SOLID principles

pub mod advanced_query_service;
pub mod analytics_helper;
pub mod analytics_service;
pub mod architecture_validation_service;
pub mod change_broadcaster;
pub mod change_detection_service;
pub mod conflict_resolution_engine;
pub mod conflict_resolution_ui;
pub mod context_crud_service;
pub mod context_intelligence_service;
pub mod context_quality_service;
pub mod context_query_service;
pub mod context_relationship_engine;
pub mod development_phase_service;
pub mod embedding_service;
pub mod extended_context_crud_service;
pub mod framework_service;
pub mod hybrid_search_service;
pub mod project_service;
pub mod search_index_manager;
pub mod semantic_search_service;
pub mod specification_analytics_service;
pub mod specification_context_linking_service;
pub mod specification_import_service;
pub mod specification_parser;
pub mod specification_service;
pub mod specification_versioning_service;
pub mod sync_engine;
pub mod vector_embedding_integration;
pub mod websocket_manager;
pub mod websocket_server;
pub mod websocket_types;
// #[cfg(test)]
// pub mod advanced_query_service_test;
#[cfg(test)]
pub mod advanced_query_service_simple_test;
#[cfg(test)]
pub mod analytics_integration_test;
#[cfg(test)]
pub mod change_broadcaster_test;
#[cfg(test)]
pub mod change_broadcasting_integration_test;
#[cfg(test)]
pub mod conflict_resolution_integration_test;
#[cfg(test)]
pub mod search_index_manager_test;
#[cfg(test)]
pub mod semantic_search_integration_test;
#[cfg(test)]
pub mod specification_import_integration_test;
#[cfg(test)]
pub mod websocket_manager_test;
// Note: component_service removed as it was identical to framework_service
// Note: flutter_service and flutter_advanced_crud_service modules don't exist yet

// Re-export service traits
// Temporarily commented out to debug compilation issues
// pub use advanced_query_service::AdvancedQueryConfig;
pub use analytics_helper::AnalyticsHelper;
pub use analytics_service::{
    AnalyticsEvent, AnalyticsEventType, AnalyticsService, DefaultAnalyticsService, ProjectInsights,
    UsageStatistics,
};
pub use architecture_validation_service::ArchitectureValidationService;
pub use change_broadcaster::{BroadcastMetrics, ChangeBroadcaster, ChangeEvent, QueuedChange};
pub use change_detection_service::{ChangeDetectionService, ChangeEmitter};
pub use conflict_resolution_engine::{
    ConflictInfo, ConflictResolutionEngine, ConflictResolutionResult, ConflictType,
    ManualResolutionRequest,
};
pub use conflict_resolution_ui::{
    ConflictResolutionSession, ConflictResolutionUI, StartResolutionRequest,
    StartResolutionResponse, UpdateUIStateRequest, UpdateUIStateResponse,
};
pub use context_intelligence_service::{
    ContextIntelligenceService, DefaultContextIntelligenceService,
};
pub use context_quality_service::{ContextQualityService, DefaultContextQualityService};
pub use context_query_service::ContextQueryService;
pub use context_relationship_engine::{
    ContextRelationshipEngine, DefaultContextRelationshipEngine,
};
pub use development_phase_service::DevelopmentPhaseService;
pub use embedding_service::{EmbeddingService, EmbeddingServiceFactory};
pub use framework_service::FrameworkService;
pub use hybrid_search_service::{HybridSearchService, HybridSearchServiceImpl};
pub use project_service::ProjectService;
pub use search_index_manager::{IndexManagerConfig, SearchIndexManager, SearchIndexManagerImpl};
pub use semantic_search_service::SemanticSearchService;
pub use specification_analytics_service::{
    DefaultSpecificationAnalyticsService, SpecificationAnalyticsService,
};
pub use specification_context_linking_service::{
    DefaultSpecificationContextLinkingService, SpecificationContextLinkingService,
};
pub use specification_import_service::{
    ChangeType, DefaultSpecificationImportService, SpecificationChange, SpecificationImportService,
};
pub use specification_parser::SpecificationParser;
pub use specification_service::{DefaultSpecificationService, SpecificationService};
pub use specification_versioning_service::{
    DifferenceType, SpecificationVersion, SpecificationVersioningService,
    SqliteSpecificationVersioningService, VersionChangeType, VersionComparison, VersionDifference,
};
pub use sync_engine::{Resolution, SyncConflict, SyncEngine, SyncStream};
pub use websocket_manager::WebSocketManager;
pub use websocket_server::{WebSocketConfig, WebSocketServer, WebSocketService};
pub use websocket_types::*;
// Note: ComponentService removed as it was identical to FrameworkService
// The following services are currently commented out because their corresponding endpoints
// have not yet been implemented. These services will be re-enabled once the necessary
// functionality is added to the application. The expected timeline for implementation
// is tracked in the project roadmap. Please refer to the roadmap for updates.
