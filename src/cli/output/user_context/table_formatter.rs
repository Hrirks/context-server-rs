// Table formatters for user context entities
// Provides ASCII table output for CLI display

use crate::models::user_context::*;

pub struct TableFormatter;

impl TableFormatter {
    /// Format a list of decisions as an ASCII table
    pub fn format_decisions(decisions: &[UserDecision]) -> String {
        if decisions.is_empty() {
            return "No decisions found.".to_string();
        }

        let mut table = String::from(
            "┌─────────────────────┬──────────────┬──────────┬────────────┬───────────┐\n",
        );
        table.push_str("│ ID (first 16 chars) │ Text         │ Category │ Confidence│ Applied   │\n");
        table.push_str("├─────────────────────┼──────────────┼──────────┼────────────┼───────────┤\n");

        for decision in decisions {
            let id = &decision.id[..16.min(decision.id.len())];
            let text = &decision.decision_text[..12.min(decision.decision_text.len())];
            table.push_str(&format!(
                "│ {:<19} │ {:<12} │ {:<8} │ {:<10.1}│ {:<9} │\n",
                id, text, decision.decision_category.as_str(), decision.confidence_score, decision.applied_count
            ));
        }

        table.push_str("└─────────────────────┴──────────────┴──────────┴────────────┴───────────┘\n");
        table
    }

    /// Format a list of goals as an ASCII table
    pub fn format_goals(goals: &[UserGoal]) -> String {
        if goals.is_empty() {
            return "No goals found.".to_string();
        }

        let mut table = String::from(
            "┌─────────────────────┬────────────┬─────────┬──────────┬──────────┐\n",
        );
        table.push_str("│ ID (first 16 chars) │ Goal Text  │ Status  │ Priority │ Progress │\n");
        table.push_str("├─────────────────────┼────────────┼─────────┼──────────┼──────────┤\n");

        for goal in goals {
            let id = &goal.id[..16.min(goal.id.len())];
            let text = &goal.goal_text[..10.min(goal.goal_text.len())];
            let progress = goal.completion_percentage();
            table.push_str(&format!(
                "│ {:<19} │ {:<10} │ {:<7} │ {:<8} │ {:<8.0}% │\n",
                id, text, goal.status.as_str(), goal.priority, progress
            ));
        }

        table.push_str("└─────────────────────┴────────────┴─────────┴──────────┴──────────┘\n");
        table
    }

    /// Format a list of preferences as an ASCII table
    pub fn format_preferences(preferences: &[UserPreference]) -> String {
        if preferences.is_empty() {
            return "No preferences found.".to_string();
        }

        let mut table = String::from(
            "┌─────────────────────┬───────────────┬──────────────┬────────┬───────────┐\n",
        );
        table.push_str("│ ID (first 16 chars) │ Name          │ Value        │ Type   │ Frequency │\n");
        table.push_str("├─────────────────────┼───────────────┼──────────────┼────────┼───────────┤\n");

        for pref in preferences {
            let id = &pref.id[..16.min(pref.id.len())];
            let name = &pref.preference_name[..13.min(pref.preference_name.len())];
            let value = &pref.preference_value[..12.min(pref.preference_value.len())];
            table.push_str(&format!(
                "│ {:<19} │ {:<13} │ {:<12} │ {:<6} │ {:<9} │\n",
                id, name, value, pref.preference_type.as_str(), pref.frequency_observed
            ));
        }

        table.push_str("└─────────────────────┴───────────────┴──────────────┴────────┴───────────┘\n");
        table
    }

    /// Format a list of issues as an ASCII table
    pub fn format_issues(issues: &[KnownIssue]) -> String {
        if issues.is_empty() {
            return "No issues found.".to_string();
        }

        let mut table = String::from(
            "┌─────────────────────┬──────────────┬──────────┬────────────┬────────┐\n",
        );
        table.push_str("│ ID (first 16 chars) │ Description  │ Category │ Severity   │ Status │\n");
        table.push_str("├─────────────────────┼──────────────┼──────────┼────────────┼────────┤\n");

        for issue in issues {
            let id = &issue.id[..16.min(issue.id.len())];
            let desc = &issue.issue_description[..12.min(issue.issue_description.len())];
            table.push_str(&format!(
                "│ {:<19} │ {:<12} │ {:<8} │ {:<10} │ {:<6} │\n",
                id, desc, issue.issue_category.as_str(), issue.severity.as_str(), issue.resolution_status.as_str()
            ));
        }

        table.push_str("└─────────────────────┴──────────────┴──────────┴────────────┴────────┘\n");
        table
    }

    /// Format a list of todos as an ASCII table
    pub fn format_todos(todos: &[ContextualTodo]) -> String {
        if todos.is_empty() {
            return "No todos found.".to_string();
        }

        let mut table = String::from(
            "┌─────────────────────┬──────────────┬─────────┬──────────┬──────────┐\n",
        );
        table.push_str("│ ID (first 16 chars) │ Task         │ Status  │ Priority │ Context  │\n");
        table.push_str("├─────────────────────┼──────────────┼─────────┼──────────┼──────────┤\n");

        for todo in todos {
            let id = &todo.id[..16.min(todo.id.len())];
            let task = &todo.task_description[..12.min(todo.task_description.len())];
            table.push_str(&format!(
                "│ {:<19} │ {:<12} │ {:<7} │ {:<8} │ {:<8} │\n",
                id, task, todo.status.as_str(), todo.priority, todo.context_type.as_str()
            ));
        }

        table.push_str("└─────────────────────┴──────────────┴─────────┴──────────┴──────────┘\n");
        table
    }
}
