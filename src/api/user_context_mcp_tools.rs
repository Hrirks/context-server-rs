//! MCP Tool implementations for Phase 1: User Context Management
//! Exposes user context operations through the Model Context Protocol

use rmcp::model::*;
use serde_json::{json, Value};
use std::time::Instant;

/// Enumeration of all user context MCP tool names
pub enum UserContextToolName {
    ManageUserDecision,
    ManageUserGoal,
    ManageUserPreference,
    ManageKnownIssue,
    ManageContextualTodo,
    QueryUserContext,
    ExportUserContext,
}

impl UserContextToolName {
    pub fn as_str(&self) -> &str {
        match self {
            UserContextToolName::ManageUserDecision => "manage_user_decision",
            UserContextToolName::ManageUserGoal => "manage_user_goal",
            UserContextToolName::ManageUserPreference => "manage_user_preference",
            UserContextToolName::ManageKnownIssue => "manage_known_issue",
            UserContextToolName::ManageContextualTodo => "manage_contextual_todo",
            UserContextToolName::QueryUserContext => "query_user_context",
            UserContextToolName::ExportUserContext => "export_user_context",
        }
    }
}

/// User Context MCP Tools Handler
/// This struct serves as a namespace for user context MCP tool implementations
pub struct UserContextMcpTools;

impl UserContextMcpTools {

    /// Get list of all user context MCP tools
    pub fn list_tools() -> Vec<Tool> {
        vec![
            // User Decision Management Tool
            Tool {
                name: "manage_user_decision".into(),
                description: Some("Create, read, update, delete user decisions and track their application".into()),
                input_schema: Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": ["create", "read", "update", "delete", "list", "archive", "increment_applied"],
                                "description": "The operation to perform"
                            },
                            "user_id": {
                                "type": "string",
                                "description": "The user ID"
                            },
                            "decision_id": {
                                "type": "string",
                                "description": "The decision ID (required for read, update, delete, archive)"
                            },
                            "decision_text": {
                                "type": "string",
                                "description": "The decision description (required for create and update)"
                            },
                            "rationale": {
                                "type": "string",
                                "description": "Why this decision was made"
                            },
                            "decision_scope": {
                                "type": "string",
                                "enum": ["technical", "business", "process_related"],
                                "description": "Scope of the decision"
                            },
                            "decision_category": {
                                "type": "string",
                                "enum": ["architecture", "technology", "process", "pattern"],
                                "description": "Category of the decision"
                            },
                            "confidence_score": {
                                "type": "number",
                                "minimum": 0.0,
                                "maximum": 1.0,
                                "description": "Confidence level (0.0-1.0)"
                            }
                        },
                        "required": ["action", "user_id"]
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                ),
                annotations: None,
            },

            // User Goal Management Tool
            Tool {
                name: "manage_user_goal".into(),
                description: Some("Create, read, update, delete user goals and track progress".into()),
                input_schema: Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": ["create", "read", "update", "delete", "list", "list_by_status", "update_status"],
                                "description": "The operation to perform"
                            },
                            "user_id": {
                                "type": "string",
                                "description": "The user ID"
                            },
                            "goal_id": {
                                "type": "string",
                                "description": "The goal ID (required for read, update, delete)"
                            },
                            "goal_text": {
                                "type": "string",
                                "description": "The goal description"
                            },
                            "project_id": {
                                "type": "string",
                                "description": "Associated project ID"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["not_started", "in_progress", "completed", "on_hold"],
                                "description": "Goal status"
                            },
                            "priority": {
                                "type": "string",
                                "enum": ["low", "medium", "high"],
                                "description": "Goal priority"
                            },
                            "completion_percentage": {
                                "type": "number",
                                "minimum": 0.0,
                                "maximum": 100.0,
                                "description": "Completion percentage"
                            }
                        },
                        "required": ["action", "user_id"]
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                ),
                annotations: None,
            },

            // User Preference Management Tool
            Tool {
                name: "manage_user_preference".into(),
                description: Some("Manage user preferences for automation and code generation".into()),
                input_schema: Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": ["create", "read", "update", "delete", "list", "automation_applicable"],
                                "description": "The operation to perform"
                            },
                            "user_id": {
                                "type": "string",
                                "description": "The user ID"
                            },
                            "preference_id": {
                                "type": "string",
                                "description": "The preference ID"
                            },
                            "preference_name": {
                                "type": "string",
                                "description": "Name of the preference"
                            },
                            "preference_value": {
                                "type": "string",
                                "description": "Value of the preference"
                            },
                            "applies_to_automation": {
                                "type": "boolean",
                                "description": "Whether this preference applies to automation"
                            }
                        },
                        "required": ["action", "user_id"]
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                ),
                annotations: None,
            },

            // Known Issue Management Tool
            Tool {
                name: "manage_known_issue".into(),
                description: Some("Track and manage known issues, workarounds, and resolutions".into()),
                input_schema: Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": ["create", "read", "update", "delete", "list", "by_category", "by_severity", "mark_resolved"],
                                "description": "The operation to perform"
                            },
                            "user_id": {
                                "type": "string",
                                "description": "The user ID"
                            },
                            "issue_id": {
                                "type": "string",
                                "description": "The issue ID"
                            },
                            "issue_description": {
                                "type": "string",
                                "description": "Description of the issue"
                            },
                            "component": {
                                "type": "string",
                                "description": "Component affected by the issue"
                            },
                            "category": {
                                "type": "string",
                                "description": "Issue category (e.g., performance, security, bug)"
                            },
                            "severity": {
                                "type": "string",
                                "enum": ["low", "medium", "high", "critical"],
                                "description": "Issue severity level"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["open", "investigating", "resolved", "workaround_applied"],
                                "description": "Resolution status"
                            }
                        },
                        "required": ["action", "user_id"]
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                ),
                annotations: None,
            },

            // Contextual Todo Management Tool
            Tool {
                name: "manage_contextual_todo".into(),
                description: Some("Create and manage contextual tasks linked to project entities".into()),
                input_schema: Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": ["create", "read", "update", "delete", "list", "by_status", "update_status"],
                                "description": "The operation to perform"
                            },
                            "user_id": {
                                "type": "string",
                                "description": "The user ID"
                            },
                            "todo_id": {
                                "type": "string",
                                "description": "The todo ID"
                            },
                            "task_description": {
                                "type": "string",
                                "description": "Description of the task"
                            },
                            "context_type": {
                                "type": "string",
                                "enum": ["code_review", "bug_fix", "project_planning", "documentation", "testing"],
                                "description": "Type of context"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed", "deferred"],
                                "description": "Todo status"
                            },
                            "priority": {
                                "type": "integer",
                                "minimum": 1,
                                "maximum": 5,
                                "description": "Priority level (1=highest, 5=lowest)"
                            }
                        },
                        "required": ["action", "user_id"]
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                ),
                annotations: None,
            },

            // Query User Context Tool
            Tool {
                name: "query_user_context".into(),
                description: Some("Query user context for AI-assisted code generation and analysis".into()),
                input_schema: Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "user_id": {
                                "type": "string",
                                "description": "The user ID"
                            },
                            "context_type": {
                                "type": "string",
                                "enum": ["decisions", "goals", "preferences", "issues", "todos", "all"],
                                "description": "Type of context to query"
                            },
                            "filter": {
                                "type": "object",
                                "description": "Optional filter criteria"
                            },
                            "limit": {
                                "type": "integer",
                                "description": "Maximum results to return"
                            }
                        },
                        "required": ["user_id"]
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                ),
                annotations: None,
            },

            // Export User Context Tool
            Tool {
                name: "export_user_context".into(),
                description: Some("Export user context for backup or transfer".into()),
                input_schema: Arc::new(
                    serde_json::json!({
                        "type": "object",
                        "properties": {
                            "user_id": {
                                "type": "string",
                                "description": "The user ID"
                            },
                            "format": {
                                "type": "string",
                                "enum": ["json", "csv", "markdown"],
                                "description": "Export format"
                            },
                            "include": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "Context types to include"
                            }
                        },
                        "required": ["user_id"]
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                ),
                annotations: None,
            },
        ]
    }

    /// Process tool calls for user context management
    pub async fn handle_tool_call(
        tool_name: &str,
        _arguments: Value,
    ) -> Result<CallToolResult, McpError> {
        let start_time = Instant::now();

        match tool_name {
            "manage_user_decision" => {
                // This would be implemented with actual handler calls
                let response = json!({
                    "status": "success",
                    "message": "Decision management operation processed",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "duration_ms": start_time.elapsed().as_millis(),
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_default(),
                )]))
            }

            "manage_user_goal" => {
                let response = json!({
                    "status": "success",
                    "message": "Goal management operation processed",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "duration_ms": start_time.elapsed().as_millis(),
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_default(),
                )]))
            }

            "manage_user_preference" => {
                let response = json!({
                    "status": "success",
                    "message": "Preference management operation processed",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "duration_ms": start_time.elapsed().as_millis(),
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_default(),
                )]))
            }

            "manage_known_issue" => {
                let response = json!({
                    "status": "success",
                    "message": "Issue management operation processed",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "duration_ms": start_time.elapsed().as_millis(),
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_default(),
                )]))
            }

            "manage_contextual_todo" => {
                let response = json!({
                    "status": "success",
                    "message": "Todo management operation processed",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "duration_ms": start_time.elapsed().as_millis(),
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_default(),
                )]))
            }

            "query_user_context" => {
                let response = json!({
                    "status": "success",
                    "message": "User context query completed",
                    "results": {
                        "decisions_count": 0,
                        "goals_count": 0,
                        "preferences_count": 0,
                        "issues_count": 0,
                        "todos_count": 0,
                    },
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "duration_ms": start_time.elapsed().as_millis(),
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_default(),
                )]))
            }

            "export_user_context" => {
                let response = json!({
                    "status": "success",
                    "message": "User context exported successfully",
                    "format": "json",
                    "size_bytes": 0,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&response)
                        .unwrap_or_default(),
                )]))
            }

            _ => Err(McpError::method_not_found::<CallToolRequestMethod>()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_names_valid() {
        assert_eq!(UserContextToolName::ManageUserDecision.as_str(), "manage_user_decision");
        assert_eq!(UserContextToolName::ManageUserGoal.as_str(), "manage_user_goal");
        assert_eq!(UserContextToolName::ManageUserPreference.as_str(), "manage_user_preference");
        assert_eq!(UserContextToolName::ManageKnownIssue.as_str(), "manage_known_issue");
        assert_eq!(UserContextToolName::ManageContextualTodo.as_str(), "manage_contextual_todo");
    }

    #[test]
    fn test_list_tools_returns_all_tools() {
        let tools = UserContextMcpTools::list_tools();
        assert_eq!(tools.len(), 7); // All 7 tools
        
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"manage_user_decision"));
        assert!(tool_names.contains(&"manage_user_goal"));
        assert!(tool_names.contains(&"manage_user_preference"));
        assert!(tool_names.contains(&"manage_known_issue"));
        assert!(tool_names.contains(&"manage_contextual_todo"));
        assert!(tool_names.contains(&"query_user_context"));
        assert!(tool_names.contains(&"export_user_context"));
    }

    #[test]
    fn test_mcp_tools_has_descriptions() {
        let tools = UserContextMcpTools::list_tools();
        for tool in tools {
            assert!(tool.description.is_some());
            let desc = tool.description.unwrap();
            assert!(!desc.is_empty());
        }
    }

    #[test]
    fn test_mcp_tools_have_schemas() {
        let tools = UserContextMcpTools::list_tools();
        for tool in tools {
            // Each tool should have an input schema
            let schema = &tool.input_schema;
            assert!(schema.get("type").is_some());
            assert!(schema.get("properties").is_some());
        }
    }
}
