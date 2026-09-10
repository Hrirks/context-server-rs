// Repository layer interfaces following Dependency Inversion Principle

pub mod architectural_decision_repository;
pub mod business_rule_repository;
pub mod development_phase_repository;
pub mod enhanced_context_repository;
pub mod framework_repository;
pub mod performance_requirement_repository;
pub mod project_repository;
pub mod specification_repository;

// Re-export repository traits
pub use architectural_decision_repository::ArchitecturalDecisionRepository;
pub use business_rule_repository::BusinessRuleRepository;
pub use development_phase_repository::DevelopmentPhaseRepository;
pub use enhanced_context_repository::EnhancedContextRepository;
pub use framework_repository::FrameworkRepository;
pub use performance_requirement_repository::PerformanceRequirementRepository;
pub use project_repository::ProjectRepository;
pub use specification_repository::SpecificationRepository;
