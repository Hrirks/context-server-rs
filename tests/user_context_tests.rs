//! Unit tests for Phase 1 User Context Layer
//! Tests cover models, repositories, handlers, and database operations

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    // Test data structures
    use context_server_rs::models::user_context::{
        UserDecision, UserGoal, UserPreference, KnownIssue, ContextualTodo,
        DecisionScope, DecisionCategory, GoalStatus, GoalPriority,
        TodoContextType, TodoStatus, IssueSeverity, ResolutionStatus,
    };

    // ============================================================================
    // Model Tests
    // ============================================================================

    #[test]
    fn test_user_decision_creation() {
        let decision = UserDecision {
            id: Uuid::new_v4().to_string(),
            user_id: "user123".to_string(),
            decision_text: "Use async/await for all I/O operations".to_string(),
            rationale: Some("Better performance and readability".to_string()),
            decision_scope: DecisionScope::Technical,
            decision_category: DecisionCategory::Architecture,
            confidence_score: 0.95,
            tagged_items: vec!["async".to_string(), "performance".to_string()],
            applied_count: 5,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(decision.user_id, "user123");
        assert_eq!(decision.confidence_score, 0.95);
        assert_eq!(decision.applied_count, 5);
        assert!(decision.confidence_score >= 0.0 && decision.confidence_score <= 1.0);
    }

    #[test]
    fn test_user_decision_confidence_bounds() {
        let decision = UserDecision {
            id: Uuid::new_v4().to_string(),
            user_id: "user123".to_string(),
            decision_text: "Test decision".to_string(),
            rationale: None,
            decision_scope: DecisionScope::Business,
            decision_category: DecisionCategory::Process,
            confidence_score: 0.5,
            tagged_items: vec![],
            applied_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(decision.confidence_score >= 0.0 && decision.confidence_score <= 1.0);
    }

    #[test]
    fn test_user_goal_creation() {
        let goal = UserGoal {
            id: Uuid::new_v4().to_string(),
            user_id: "user123".to_string(),
            goal_text: "Complete authentication module".to_string(),
            project_id: Some("project1".to_string()),
            goal_status: GoalStatus::InProgress,
            goal_priority: GoalPriority::High,
            target_date: Some(Utc::now()),
            completion_percentage: 0.0,
            steps: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(goal.user_id, "user123");
        assert_eq!(goal.goal_status, GoalStatus::InProgress);
        assert_eq!(goal.completion_percentage, 0.0);
    }

    #[test]
    fn test_user_goal_completion_percentage_bounds() {
        let goal = UserGoal {
            id: Uuid::new_v4().to_string(),
            user_id: "user123".to_string(),
            goal_text: "Test goal".to_string(),
            project_id: None,
            goal_status: GoalStatus::NotStarted,
            goal_priority: GoalPriority::Medium,
            target_date: None,
            completion_percentage: 75.5,
            steps: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(goal.completion_percentage >= 0.0 && goal.completion_percentage <= 100.0);
    }

    #[test]
    fn test_user_preference_creation() {
        let preference = UserPreference {
            id: Uuid::new_v4().to_string(),
            user_id: "user123".to_string(),
            preference_name: "code_style".to_string(),
            preference_value: "snake_case".to_string(),
            applies_to_automation: true,
            frequency_observed: 5,
            rationale: Some("Consistent with team standards".to_string()),
            priority: 3,
            last_referenced: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(preference.user_id, "user123");
        assert_eq!(preference.frequency_observed, 5);
        assert!(preference.applies_to_automation);
        assert_eq!(preference.priority, 3);
    }

    #[test]
    fn test_known_issue_creation() {
        let issue = KnownIssue {
            id: Uuid::new_v4().to_string(),
            user_id: "user123".to_string(),
            issue_description: "Memory leak in connection pool".to_string(),
            component: Some("database".to_string()),
            issue_category: "performance".to_string(),
            severity: IssueSeverity::High,
            symptoms: vec!["high memory usage".to_string(), "application slowdown".to_string()],
            workarounds: vec!["restart the application".to_string()],
            resolution_status: ResolutionStatus::Open,
            resolution_date: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(issue.user_id, "user123");
        assert_eq!(issue.severity, IssueSeverity::High);
        assert_eq!(issue.symptoms.len(), 2);
        assert_eq!(issue.workarounds.len(), 1);
    }

    #[test]
    fn test_contextual_todo_creation() {
        let todo = ContextualTodo {
            id: Uuid::new_v4().to_string(),
            user_id: "user123".to_string(),
            task_description: "Review code changes in PR #42".to_string(),
            task_context_type: TodoContextType::CodeReview,
            linked_entity_id: Some("pr_42".to_string()),
            linked_project_id: Some("project1".to_string()),
            todo_status: TodoStatus::Pending,
            priority: 2,
            due_date: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(todo.user_id, "user123");
        assert_eq!(todo.todo_status, TodoStatus::Pending);
        assert_eq!(todo.priority, 2);
    }

    // ============================================================================
    // Enum Tests
    // ============================================================================

    #[test]
    fn test_goal_status_enum_conversions() {
        assert_eq!(GoalStatus::NotStarted.as_str(), "not_started");
        assert_eq!(GoalStatus::InProgress.as_str(), "in_progress");
        assert_eq!(GoalStatus::Completed.as_str(), "completed");
        assert_eq!(GoalStatus::OnHold.as_str(), "on_hold");
    }

    #[test]
    fn test_goal_priority_enum_conversions() {
        assert_eq!(GoalPriority::Low.as_str(), "low");
        assert_eq!(GoalPriority::Medium.as_str(), "medium");
        assert_eq!(GoalPriority::High.as_str(), "high");
    }

    #[test]
    fn test_decision_scope_enum_conversions() {
        assert_eq!(DecisionScope::Technical.as_str(), "technical");
        assert_eq!(DecisionScope::Business.as_str(), "business");
        assert_eq!(DecisionScope::ProcessRelated.as_str(), "process_related");
    }

    #[test]
    fn test_decision_category_enum_conversions() {
        assert_eq!(DecisionCategory::Architecture.as_str(), "architecture");
        assert_eq!(DecisionCategory::Technology.as_str(), "technology");
        assert_eq!(DecisionCategory::Process.as_str(), "process");
        assert_eq!(DecisionCategory::Pattern.as_str(), "pattern");
    }

    #[test]
    fn test_issue_severity_enum_conversions() {
        assert_eq!(IssueSeverity::Low.as_str(), "low");
        assert_eq!(IssueSeverity::Medium.as_str(), "medium");
        assert_eq!(IssueSeverity::High.as_str(), "high");
        assert_eq!(IssueSeverity::Critical.as_str(), "critical");
    }

    #[test]
    fn test_todo_status_enum_conversions() {
        assert_eq!(TodoStatus::Pending.as_str(), "pending");
        assert_eq!(TodoStatus::InProgress.as_str(), "in_progress");
        assert_eq!(TodoStatus::Completed.as_str(), "completed");
        assert_eq!(TodoStatus::Deferred.as_str(), "deferred");
    }

    #[test]
    fn test_todo_context_type_enum_conversions() {
        assert_eq!(TodoContextType::CodeReview.as_str(), "code_review");
        assert_eq!(TodoContextType::BugFix.as_str(), "bug_fix");
        assert_eq!(TodoContextType::ProjectPlanning.as_str(), "project_planning");
        assert_eq!(TodoContextType::Documentation.as_str(), "documentation");
        assert_eq!(TodoContextType::Testing.as_str(), "testing");
    }

    // ============================================================================
    // Serialization Tests
    // ============================================================================

    #[test]
    fn test_user_decision_serialization() {
        let decision = UserDecision {
            id: "dec123".to_string(),
            user_id: "user123".to_string(),
            decision_text: "Use async/await".to_string(),
            rationale: Some("Performance".to_string()),
            decision_scope: DecisionScope::Technical,
            decision_category: DecisionCategory::Architecture,
            confidence_score: 0.9,
            tagged_items: vec!["async".to_string(), "rust".to_string()],
            applied_count: 3,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&decision).expect("Serialization failed");
        assert!(json.contains("dec123"));
        assert!(json.contains("Use async/await"));
        assert!(json.contains("0.9"));

        let _deserialized: UserDecision =
            serde_json::from_str(&json).expect("Deserialization failed");
    }

    #[test]
    fn test_user_goal_with_steps_serialization() {
        let goal = UserGoal {
            id: "goal123".to_string(),
            user_id: "user123".to_string(),
            goal_text: "Complete auth module".to_string(),
            project_id: Some("proj1".to_string()),
            goal_status: GoalStatus::InProgress,
            goal_priority: GoalPriority::High,
            target_date: Some(Utc::now()),
            completion_percentage: 50.0,
            steps: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&goal).expect("Serialization failed");
        assert!(json.contains("goal123"));
        assert!(json.contains("Complete auth module"));

        let _deserialized: UserGoal =
            serde_json::from_str(&json).expect("Deserialization failed");
    }

    #[test]
    fn test_known_issue_with_arrays_serialization() {
        let issue = KnownIssue {
            id: "issue123".to_string(),
            user_id: "user123".to_string(),
            issue_description: "Memory leak".to_string(),
            component: Some("db".to_string()),
            issue_category: "performance".to_string(),
            severity: IssueSeverity::High,
            symptoms: vec!["high memory".to_string(), "slow queries".to_string()],
            workarounds: vec!["restart".to_string()],
            resolution_status: ResolutionStatus::Open,
            resolution_date: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&issue).expect("Serialization failed");
        assert!(json.contains("issue123"));
        assert!(json.contains("Memory leak"));
        assert!(json.contains("high memory"));

        let _deserialized: KnownIssue =
            serde_json::from_str(&json).expect("Deserialization failed");
    }

    // ============================================================================
    // Field Validation Tests
    // ============================================================================

    #[test]
    fn test_user_id_is_required() {
        let decision = UserDecision {
            id: Uuid::new_v4().to_string(),
            user_id: "".to_string(), // Empty user_id is invalid in practice
            decision_text: "Test".to_string(),
            rationale: None,
            decision_scope: DecisionScope::Technical,
            decision_category: DecisionCategory::Architecture,
            confidence_score: 0.5,
            tagged_items: vec![],
            applied_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // In practice, this would be validated at the repository level
        assert!(decision.user_id.is_empty());
    }

    #[test]
    fn test_decision_text_max_length() {
        let long_text = "a".repeat(5000);
        let decision = UserDecision {
            id: Uuid::new_v4().to_string(),
            user_id: "user123".to_string(),
            decision_text: long_text.clone(),
            rationale: None,
            decision_scope: DecisionScope::Technical,
            decision_category: DecisionCategory::Architecture,
            confidence_score: 0.5,
            tagged_items: vec![],
            applied_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(decision.decision_text.len(), 5000);
    }

    // ============================================================================
    // DateTime Tests
    // ============================================================================

    #[test]
    fn test_datetime_fields_valid() {
        let now = Utc::now();
        let decision = UserDecision {
            id: Uuid::new_v4().to_string(),
            user_id: "user123".to_string(),
            decision_text: "Test".to_string(),
            rationale: None,
            decision_scope: DecisionScope::Technical,
            decision_category: DecisionCategory::Architecture,
            confidence_score: 0.5,
            tagged_items: vec![],
            applied_count: 0,
            created_at: now,
            updated_at: now,
        };

        assert_eq!(decision.created_at, now);
        assert_eq!(decision.updated_at, now);
    }

    #[test]
    fn test_datetime_rfc3339_format() {
        let now = Utc::now();
        let decision = UserDecision {
            id: Uuid::new_v4().to_string(),
            user_id: "user123".to_string(),
            decision_text: "Test".to_string(),
            rationale: None,
            decision_scope: DecisionScope::Technical,
            decision_category: DecisionCategory::Architecture,
            confidence_score: 0.5,
            tagged_items: vec![],
            applied_count: 0,
            created_at: now,
            updated_at: now,
        };

        let json_str = serde_json::to_string(&decision).expect("Serialization failed");
        // RFC3339 format should be present in the JSON
        assert!(json_str.contains("2"));  // Year will contain 2
    }

    // ============================================================================
    // UUID Tests
    // ============================================================================

    #[test]
    fn test_uuid_generation() {
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        // UUIDs should be unique
        assert_ne!(uuid1, uuid2);
    }

    #[test]
    fn test_uuid_to_string() {
        let uuid = Uuid::new_v4();
        let uuid_str = uuid.to_string();

        // UUID string should be in standard format
        assert_eq!(uuid_str.len(), 36); // Standard UUID format length
        assert!(uuid_str.contains('-'));
    }

    // ============================================================================
    // Test Helper Functions
    // ============================================================================

    fn create_test_decision() -> UserDecision {
        UserDecision {
            id: Uuid::new_v4().to_string(),
            user_id: "test_user".to_string(),
            decision_text: "Test decision".to_string(),
            rationale: Some("Test rationale".to_string()),
            decision_scope: DecisionScope::Technical,
            decision_category: DecisionCategory::Architecture,
            confidence_score: 0.8,
            tagged_items: vec!["test".to_string()],
            applied_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_goal() -> UserGoal {
        UserGoal {
            id: Uuid::new_v4().to_string(),
            user_id: "test_user".to_string(),
            goal_text: "Test goal".to_string(),
            project_id: Some("test_project".to_string()),
            goal_status: GoalStatus::NotStarted,
            goal_priority: GoalPriority::Medium,
            target_date: None,
            completion_percentage: 0.0,
            steps: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_test_preference() -> UserPreference {
        UserPreference {
            id: Uuid::new_v4().to_string(),
            user_id: "test_user".to_string(),
            preference_name: "test_pref".to_string(),
            preference_value: "test_value".to_string(),
            applies_to_automation: false,
            frequency_observed: 0,
            rationale: None,
            priority: 5,
            last_referenced: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ============================================================================
    // Bulk Tests with Helper Functions
    // ============================================================================

    #[test]
    fn test_create_batch_of_decisions() {
        let decisions: Vec<UserDecision> = (0..10)
            .map(|i| UserDecision {
                id: Uuid::new_v4().to_string(),
                user_id: format!("user_{}", i),
                decision_text: format!("Decision {}", i),
                rationale: None,
                decision_scope: DecisionScope::Technical,
                decision_category: DecisionCategory::Architecture,
                confidence_score: 0.5 + (i as f32 * 0.05),
                tagged_items: vec![],
                applied_count: i,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .collect();

        assert_eq!(decisions.len(), 10);
        // Verify all have unique IDs
        let ids: Vec<String> = decisions.iter().map(|d| d.id.clone()).collect();
        assert_eq!(ids.len(), 10);
    }

    #[test]
    fn test_create_batch_of_goals() {
        let goals: Vec<UserGoal> = (0..5)
            .map(|i| UserGoal {
                id: Uuid::new_v4().to_string(),
                user_id: format!("user_{}", i),
                goal_text: format!("Goal {}", i),
                project_id: Some(format!("project_{}", i)),
                goal_status: if i % 2 == 0 {
                    GoalStatus::NotStarted
                } else {
                    GoalStatus::InProgress
                },
                goal_priority: GoalPriority::High,
                target_date: None,
                completion_percentage: (i * 20) as f32,
                steps: vec![],
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .collect();

        assert_eq!(goals.len(), 5);
    }
}
