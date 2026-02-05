// User Context Output Formatters - Table and JSON rendering

use crate::models::user_context::*;
use serde_json::{json, Value};

pub mod table_formatter;
pub mod json_formatter;

pub use table_formatter::TableFormatter;
pub use json_formatter::JsonFormatter;

/// Format trait for displaying user context entities
pub trait OutputFormatter {
    /// Format a single entity as a table string
    fn format_table(&self) -> String;

    /// Format as JSON value
    fn format_json(&self) -> Value;

    /// Format as formatted JSON string
    fn format_json_string(&self) -> String {
        serde_json::to_string_pretty(&self.format_json()).unwrap_or_default()
    }
}

impl OutputFormatter for UserDecision {
    fn format_table(&self) -> String {
        format!(
            "┌─ Decision ID: {}\n│\n├─ Text: {}\n├─ Category: {}\n├─ Reason: {}\n├─ Confidence: {:.1}\n├─ Times Applied: {}\n├─ Status: {}\n└─ Created: {}",
            self.id,
            self.decision_text,
            self.decision_category.as_str(),
            self.reason.as_ref().unwrap_or(&"N/A".to_string()),
            self.confidence_score,
            self.applied_count,
            self.status.as_str(),
            self.created_at.format("%Y-%m-%d %H:%M:%S")
        )
    }

    fn format_json(&self) -> Value {
        json!({
            "id": self.id,
            "user_id": self.user_id,
            "decision_text": self.decision_text,
            "category": self.decision_category.as_str(),
            "reason": self.reason,
            "project_id": self.related_project_id,
            "confidence_score": self.confidence_score,
            "applied_count": self.applied_count,
            "last_applied": self.last_applied.map(|dt| dt.to_rfc3339()),
            "status": self.status.as_str(),
            "scope": self.scope.scope_type(),
            "created_at": self.created_at.to_rfc3339(),
            "updated_at": self.updated_at.map(|dt| dt.to_rfc3339()),
            "referenced_items": self.referenced_items,
        })
    }
}

impl OutputFormatter for UserGoal {
    fn format_table(&self) -> String {
        let progress = self.completion_percentage();
        format!(
            "┌─ Goal ID: {}\n│\n├─ Text: {}\n├─ Status: {}\n├─ Priority: {}\n├─ Progress: {:.1}%\n├─ Steps: {}/{}\n├─ Blockers: {}\n└─ Created: {}",
            self.id,
            self.goal_text,
            self.status.as_str(),
            self.priority,
            progress,
            self.steps.iter().filter(|s| s.status == GoalStatus::Completed).count(),
            self.steps.len(),
            self.blockers.len(),
            self.created_at.format("%Y-%m-%d %H:%M:%S")
        )
    }

    fn format_json(&self) -> Value {
        json!({
            "id": self.id,
            "user_id": self.user_id,
            "goal_text": self.goal_text,
            "description": self.description,
            "project_id": self.project_id,
            "priority": self.priority,
            "status": self.status.as_str(),
            "progress_percentage": self.completion_percentage(),
            "steps": self.steps.iter().map(|s| json!({
                "step_number": s.step_number,
                "description": s.description,
                "status": s.status.as_str(),
                "due_date": s.due_date.map(|dt| dt.to_rfc3339()),
            })).collect::<Vec<_>>(),
            "blockers": self.blockers,
            "related_todos": self.related_todos,
            "created_at": self.created_at.to_rfc3339(),
            "updated_at": self.updated_at.map(|dt| dt.to_rfc3339()),
            "completion_date": self.completion_date.map(|dt| dt.to_rfc3339()),
        })
    }
}

impl OutputFormatter for UserPreference {
    fn format_table(&self) -> String {
        format!(
            "┌─ Preference ID: {}\n│\n├─ Name: {}\n├─ Value: {}\n├─ Type: {}\n├─ Frequency: {}\n├─ Applies to Automation: {}\n├─ Tags: {}\n└─ Created: {}",
            self.id,
            self.preference_name,
            self.preference_value,
            self.preference_type.as_str(),
            self.frequency_observed,
            self.applies_to_automation,
            self.tags.join(", "),
            self.created_at.format("%Y-%m-%d %H:%M:%S")
        )
    }

    fn format_json(&self) -> Value {
        json!({
            "id": self.id,
            "user_id": self.user_id,
            "preference_name": self.preference_name,
            "preference_value": self.preference_value,
            "preference_type": self.preference_type.as_str(),
            "scope": self.scope.scope_type(),
            "applies_to_automation": self.applies_to_automation,
            "frequency_observed": self.frequency_observed,
            "tags": self.tags,
            "rationale": self.rationale,
            "priority": self.priority,
            "created_at": self.created_at.to_rfc3339(),
            "updated_at": self.updated_at.map(|dt| dt.to_rfc3339()),
            "last_referenced": self.last_referenced.map(|dt| dt.to_rfc3339()),
        })
    }
}

impl OutputFormatter for KnownIssue {
    fn format_table(&self) -> String {
        format!(
            "┌─ Issue ID: {}\n│\n├─ Description: {}\n├─ Category: {}\n├─ Severity: {}\n├─ Status: {}\n├─ Affected Components: {}\n├─ Symptoms: {}\n├─ Workarounds: {}\n└─ Learned: {}",
            self.id,
            &self.issue_description[..50.min(self.issue_description.len())],
            self.issue_category.as_str(),
            self.severity.as_str(),
            self.resolution_status.as_str(),
            self.affected_components.join(", "),
            self.symptoms.len(),
            if self.workaround.is_some() { "1" } else { "0" },
            self.learned_date.format("%Y-%m-%d %H:%M:%S")
        )
    }

    fn format_json(&self) -> Value {
        json!({
            "id": self.id,
            "user_id": self.user_id,
            "issue_description": self.issue_description,
            "category": self.issue_category.as_str(),
            "severity": self.severity.as_str(),
            "affected_components": self.affected_components,
            "symptoms": self.symptoms,
            "root_cause": self.root_cause,
            "workaround": self.workaround,
            "permanent_solution": self.permanent_solution,
            "resolution_status": self.resolution_status.as_str(),
            "project_contexts": self.project_contexts,
            "learned_date": self.learned_date.to_rfc3339(),
            "resolution_date": self.resolution_date.map(|dt| dt.to_rfc3339()),
            "prevention_notes": self.prevention_notes,
        })
    }
}

impl OutputFormatter for ContextualTodo {
    fn format_table(&self) -> String {
        format!(
            "┌─ Todo ID: {}\n│\n├─ Task: {}\n├─ Context Type: {}\n├─ Status: {}\n├─ Priority: {}\n├─ Related Entity: {}\n├─ Due Date: {}\n└─ Created: {}",
            self.id,
            self.task_description,
            self.context_type.as_str(),
            self.status.as_str(),
            self.priority,
            self.related_entity_id.as_ref().unwrap_or(&"None".to_string()),
            self.due_date.map(|dt| dt.format("%Y-%m-%d").to_string()).unwrap_or("None".to_string()),
            self.created_at.format("%Y-%m-%d %H:%M:%S")
        )
    }

    fn format_json(&self) -> Value {
        json!({
            "id": self.id,
            "user_id": self.user_id,
            "task_description": self.task_description,
            "context_type": self.context_type.as_str(),
            "related_entity_id": self.related_entity_id,
            "related_entity_type": self.related_entity_type.as_ref().map(|t| t.as_str()),
            "project_id": self.project_id,
            "assigned_to": self.assigned_to,
            "due_date": self.due_date.map(|dt| dt.to_rfc3339()),
            "status": self.status.as_str(),
            "priority": self.priority,
            "created_from_conversation_date": self.created_from_conversation_date.map(|dt| dt.to_rfc3339()),
            "created_at": self.created_at.to_rfc3339(),
            "updated_at": self.updated_at.map(|dt| dt.to_rfc3339()),
            "completion_date": self.completion_date.map(|dt| dt.to_rfc3339()),
        })
    }
}
