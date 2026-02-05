// JSON formatters for user context entities
// Provides JSON serialization for API responses

use crate::models::user_context::*;
use serde_json::{json, Value};

pub struct JsonFormatter;

impl JsonFormatter {
    /// Format a list of decisions as JSON
    pub fn format_decisions(decisions: &[UserDecision]) -> Value {
        json!({
            "count": decisions.len(),
            "decisions": decisions.iter().map(|d| json!({
                "id": d.id,
                "decision_description": d.decision_description,
                "category": d.category.as_str(),
                "confidence_level": d.confidence_level,
                "times_applied": d.times_applied,
                "status": d.status.as_str(),
            })).collect::<Vec<_>>(),
        })
    }

    /// Format a list of goals as JSON
    pub fn format_goals(goals: &[UserGoal]) -> Value {
        json!({
            "count": goals.len(),
            "goals": goals.iter().map(|g| json!({
                "id": g.id,
                "goal_name": g.goal_name,
                "status": g.status.as_str(),
                "priority": g.priority,
                "progress_percentage": g.completion_percentage(),
                "steps_completed": g.steps.iter().filter(|s| s.completed).count(),
                "total_steps": g.steps.len(),
            })).collect::<Vec<_>>(),
        })
    }

    /// Format a list of preferences as JSON
    pub fn format_preferences(preferences: &[UserPreference]) -> Value {
        json!({
            "count": preferences.len(),
            "preferences": preferences.iter().map(|p| json!({
                "id": p.id,
                "preference_name": p.preference_name,
                "preference_value": p.preference_value,
                "preference_type": p.preference_type.as_str(),
                "frequency_observed": p.frequency_observed,
                "applies_to_automation": p.applies_to_automation,
            })).collect::<Vec<_>>(),
        })
    }

    /// Format a list of issues as JSON
    pub fn format_issues(issues: &[KnownIssue]) -> Value {
        json!({
            "count": issues.len(),
            "issues": issues.iter().map(|i| json!({
                "id": i.id,
                "issue_title": i.issue_title,
                "category": i.category.as_str(),
                "severity": i.severity.as_str(),
                "resolution_status": i.resolution_status.as_str(),
                "affected_components": i.affected_components.len(),
                "workarounds_count": i.workarounds.len(),
            })).collect::<Vec<_>>(),
        })
    }

    /// Format a list of todos as JSON
    pub fn format_todos(todos: &[ContextualTodo]) -> Value {
        json!({
            "count": todos.len(),
            "todos": todos.iter().map(|t| json!({
                "id": t.id,
                "task_description": t.task_description,
                "context_type": t.context_type.as_str(),
                "status": t.status.as_str(),
                "priority": t.priority,
            })).collect::<Vec<_>>(),
        })
    }
}
