// Infrastructure layer - SQLite implementations of repositories

pub mod sqlite_analytics_repository;
pub mod sqlite_architectural_decision_repository;
pub mod sqlite_audit_trail_repository;
pub mod sqlite_business_rule_repository;
pub mod sqlite_constraint_repository;
pub mod sqlite_development_phase_repository;
pub mod sqlite_enhanced_context_repository;
pub mod sqlite_framework_repository;
pub mod sqlite_performance_requirement_repository;
pub mod sqlite_project_repository;
pub mod sqlite_specification_repository;
// Note: sqlite_component_repository was removed as it was identical to sqlite_framework_repository
// User context repositories - Phase 1 implementation
pub mod sqlite_user_decision_repository;
pub mod sqlite_user_goal_repository;
pub mod sqlite_user_preference_repository;
pub mod sqlite_known_issue_repository;
pub mod sqlite_contextual_todo_repository;
// TODO: Fix error handling in these files
// pub mod sqlite_security_policy_repository;
// pub mod sqlite_extended_repositories;

// Re-export implementations
pub use sqlite_analytics_repository::SqliteAnalyticsRepository;
pub use sqlite_architectural_decision_repository::SqliteArchitecturalDecisionRepository;
pub use sqlite_audit_trail_repository::{AuditTrailRepository, SqliteAuditTrailRepository};
pub use sqlite_business_rule_repository::SqliteBusinessRuleRepository;
pub use sqlite_constraint_repository::{
    ConstraintRepository, DependencyRepository, SqliteConstraintRepository, SqliteDependencyRepository,
};
pub use sqlite_development_phase_repository::SqliteDevelopmentPhaseRepository;
pub use sqlite_enhanced_context_repository::SqliteEnhancedContextRepository;
pub use sqlite_framework_repository::SqliteFrameworkRepository;
pub use sqlite_performance_requirement_repository::SqlitePerformanceRequirementRepository;
pub use sqlite_project_repository::SqliteProjectRepository;
pub use sqlite_specification_repository::SqliteSpecificationRepository;
// User context repository exports
pub use sqlite_user_decision_repository::SqliteUserDecisionRepository;
pub use sqlite_user_goal_repository::SqliteUserGoalRepository;
pub use sqlite_user_preference_repository::SqliteUserPreferenceRepository;
pub use sqlite_known_issue_repository::SqliteKnownIssueRepository;
pub use sqlite_contextual_todo_repository::SqliteContextualTodoRepository;
// Note: SqliteComponentRepository removed - use SqliteFrameworkRepository instead
// TODO: Re-enable when fixed
// pub use sqlite_security_policy_repository::SqliteSecurityPolicyRepository;
// pub use sqlite_extended_repositories::{SqliteProjectConventionRepository, SqliteFeatureContextRepository};
