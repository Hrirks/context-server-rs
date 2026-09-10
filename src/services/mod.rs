// Service layer modules following SOLID principles

pub mod analytics_helper;
pub mod analytics_service;
pub mod architecture_validation_service;
pub mod context_crud_service;
pub mod context_query_service;
pub mod development_phase_service;
pub mod embedding_store_service;
pub mod framework_service;
pub mod project_service;
pub mod specification_analytics_service;
pub mod specification_context_linking_service;
pub mod specification_import_service;
pub mod specification_parser;
pub mod specification_service;
pub mod specification_versioning_service;

#[cfg(test)]
pub mod analytics_integration_test;
#[cfg(test)]
pub mod specification_import_integration_test;

// Re-export service traits
pub use analytics_helper::AnalyticsHelper;
pub use analytics_service::{
    AnalyticsEvent, AnalyticsEventType, AnalyticsService, DefaultAnalyticsService, ProjectInsights,
    UsageStatistics,
};
pub use architecture_validation_service::ArchitectureValidationService;
pub use context_query_service::ContextQueryService;
pub use development_phase_service::DevelopmentPhaseService;
pub use embedding_store_service::EmbeddingStoreService;
pub use framework_service::FrameworkService;
pub use project_service::ProjectService;
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
