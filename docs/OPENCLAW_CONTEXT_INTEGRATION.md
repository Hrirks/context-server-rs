# OpenClaw Context Integration - Implementation Phases

This document outlines the strategic implementation of a **User Context Layer** in context-server-rs for OpenClaw to learn from conversation patterns, decisions, and user preferences.

---

## Overview

The context-server-rs will be extended with new entity types to capture and persist:
- **User Decisions** - Key choices made by the user
- **User Goals** - Objectives and planned work
- **User Preferences** - Recurring habits and constraints
- **Known Issues** - Problems identified with solutions
- **Contextual To-Dos** - Actionable tasks from conversations

This enables OpenClaw to:
1. Validate actions against user constraints
2. Recommend next steps based on goals
3. Learn from past decisions
4. Avoid repeating mistakes identified in known issues

---

## Phase 1: Foundation (Database & Basic CLI)

### Objective
Establish the data persistence layer and basic CRUD CLI commands for managing user context entities.

### 1.1 Database Schema

Add four new tables to the SQLite database:

```sql
-- User/Agent Decision Registry
CREATE TABLE user_decisions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    decision_text TEXT NOT NULL,
    reason TEXT,
    decision_category TEXT NOT NULL,  -- 'architecture', 'tool_choice', 'constraint', 'workflow', 'performance'
    scope TEXT NOT NULL,              -- 'global', 'project_id:<id>'
    related_project_id TEXT,          -- NULL for global, otherwise project ID
    confidence_score FLOAT DEFAULT 0.5,  -- 0.0-1.0 based on repetition/certainty
    referenced_items TEXT,            -- JSON array of related entity IDs
    created_at TEXT NOT NULL,
    updated_at TEXT,
    applied_count INTEGER DEFAULT 0,  -- Track reuse frequency
    last_applied TEXT,
    status TEXT DEFAULT 'active'      -- 'active', 'archived', 'superseded'
);

-- User Goals & Plans
CREATE TABLE user_goals (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    goal_text TEXT NOT NULL,
    description TEXT,
    project_id TEXT,                  -- NULL for global goals
    status TEXT NOT NULL,             -- 'planned', 'in_progress', 'completed', 'blocked'
    priority INTEGER DEFAULT 3,       -- 1 (highest) to 5 (lowest)
    steps TEXT,                       -- JSON array of milestone steps with descriptions
    created_at TEXT NOT NULL,
    updated_at TEXT,
    completion_target_date TEXT,
    completion_date TEXT,
    blockers TEXT,                    -- JSON array of blocking issues/dependencies
    related_todos TEXT                -- JSON array of contextual_todo IDs
);

-- Recurring Preferences (habits/constraints)
CREATE TABLE user_preferences (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    preference_name TEXT NOT NULL,
    preference_value TEXT NOT NULL,
    preference_type TEXT NOT NULL,    -- 'tool', 'framework', 'constraint', 'pattern'
    scope TEXT NOT NULL,              -- 'global', 'project_id:<id>', 'workflow:<type>'
    applies_to_automation BOOLEAN DEFAULT true,  -- OpenClaw should consider this
    rationale TEXT,
    priority INTEGER DEFAULT 3,       -- 1 (highest) to 5 (lowest)
    frequency_observed INTEGER DEFAULT 1,  -- How many times has this been mentioned?
    tags TEXT,                        -- JSON array: ['lightweight', 'architecture', 'performance']
    created_at TEXT NOT NULL,
    updated_at TEXT,
    last_referenced TEXT
);

-- Known Issues & Solutions (learned patterns)
CREATE TABLE known_issues (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    issue_description TEXT NOT NULL,
    symptoms TEXT,                    -- JSON array of observable indicators
    root_cause TEXT,
    workaround TEXT,
    permanent_solution TEXT,
    affected_components TEXT,         -- JSON array of component/tool names
    severity TEXT NOT NULL,           -- 'critical', 'high', 'medium', 'low'
    issue_category TEXT NOT NULL,     -- 'integration', 'performance', 'deployment', 'data', 'workflow'
    learned_date TEXT NOT NULL,
    resolution_status TEXT NOT NULL,  -- 'unresolved', 'workaround_available', 'fixed', 'no_action_needed'
    resolution_date TEXT,
    prevention_notes TEXT,
    project_contexts TEXT             -- JSON array of project IDs where this was observed
);

-- Contextual To-Dos linked to conversation insights
CREATE TABLE contextual_todos (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    task_description TEXT NOT NULL,
    context_type TEXT NOT NULL,      -- 'decision_implementation', 'goal_step', 'issue_resolution', 'preference_adoption'
    related_entity_id TEXT,          -- Link back to decision/goal/issue ID
    related_entity_type TEXT,        -- 'user_decision', 'user_goal', 'known_issue', 'user_preference'
    project_id TEXT,
    assigned_to TEXT,
    due_date TEXT,
    status TEXT DEFAULT 'pending',   -- 'pending', 'in_progress', 'completed', 'blocked'
    priority INTEGER DEFAULT 3,      -- 1 (highest) to 5 (lowest)
    created_from_conversation_date TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT,
    completion_date TEXT
);

-- Create indexes for efficient querying
CREATE INDEX idx_user_decisions_user ON user_decisions(user_id);
CREATE INDEX idx_user_decisions_scope ON user_decisions(scope);
CREATE INDEX idx_user_decisions_status ON user_decisions(status);
CREATE INDEX idx_user_goals_user ON user_goals(user_id);
CREATE INDEX idx_user_goals_status ON user_goals(status);
CREATE INDEX idx_user_goals_project ON user_goals(project_id);
CREATE INDEX idx_user_preferences_user ON user_preferences(user_id);
CREATE INDEX idx_user_preferences_scope ON user_preferences(scope);
CREATE INDEX idx_known_issues_user ON known_issues(user_id);
CREATE INDEX idx_known_issues_status ON known_issues(resolution_status);
CREATE INDEX idx_contextual_todos_user ON contextual_todos(user_id);
CREATE INDEX idx_contextual_todos_status ON contextual_todos(status);
CREATE INDEX idx_contextual_todos_entity ON contextual_todos(related_entity_id);
```

### 1.2 Data Models

Create Rust structs in `/src/models/user_context.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDecision {
    pub id: String,
    pub user_id: String,
    pub decision_text: String,
    pub reason: Option<String>,
    pub decision_category: DecisionCategory,
    pub scope: ContextScope,
    pub related_project_id: Option<String>,
    pub confidence_score: f32,
    pub referenced_items: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub applied_count: i32,
    pub last_applied: Option<DateTime<Utc>>,
    pub status: EntityStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionCategory {
    Architecture,
    ToolChoice,
    Constraint,
    Workflow,
    Performance,
    Security,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserGoal {
    pub id: String,
    pub user_id: String,
    pub goal_text: String,
    pub description: Option<String>,
    pub project_id: Option<String>,
    pub status: GoalStatus,
    pub priority: u32,
    pub steps: Vec<GoalStep>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub completion_target_date: Option<DateTime<Utc>>,
    pub completion_date: Option<DateTime<Utc>>,
    pub blockers: Vec<String>,
    pub related_todos: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalStep {
    pub step_number: u32,
    pub description: String,
    pub status: GoalStatus,
    pub due_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Planned,
    InProgress,
    Completed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreference {
    pub id: String,
    pub user_id: String,
    pub preference_name: String,
    pub preference_value: String,
    pub preference_type: PreferenceType,
    pub scope: ContextScope,
    pub applies_to_automation: bool,
    pub rationale: Option<String>,
    pub priority: u32,
    pub frequency_observed: i32,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub last_referenced: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceType {
    Tool,
    Framework,
    Constraint,
    Pattern,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownIssue {
    pub id: String,
    pub user_id: String,
    pub issue_description: String,
    pub symptoms: Vec<String>,
    pub root_cause: Option<String>,
    pub workaround: Option<String>,
    pub permanent_solution: Option<String>,
    pub affected_components: Vec<String>,
    pub severity: IssueSeverity,
    pub issue_category: IssueCategory,
    pub learned_date: DateTime<Utc>,
    pub resolution_status: ResolutionStatus,
    pub resolution_date: Option<DateTime<Utc>>,
    pub prevention_notes: Option<String>,
    pub project_contexts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IssueCategory {
    Integration,
    Performance,
    Deployment,
    Data,
    Workflow,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    Unresolved,
    WorkaroundAvailable,
    Fixed,
    NoActionNeeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextualTodo {
    pub id: String,
    pub user_id: String,
    pub task_description: String,
    pub context_type: TodoContextType,
    pub related_entity_id: Option<String>,
    pub related_entity_type: Option<EntityType>,
    pub project_id: Option<String>,
    pub assigned_to: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub status: TodoStatus,
    pub priority: u32,
    pub created_from_conversation_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub completion_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoContextType {
    DecisionImplementation,
    GoalStep,
    IssueResolution,
    PreferenceAdoption,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    UserDecision,
    UserGoal,
    KnownIssue,
    UserPreference,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContextScope {
    Global,
    Project(String),
    Workflow(String),
}

impl ToString for ContextScope {
    fn to_string(&self) -> String {
        match self {
            ContextScope::Global => "global".to_string(),
            ContextScope::Project(id) => format!("project_id:{}", id),
            ContextScope::Workflow(name) => format!("workflow:{}", name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntityStatus {
    Active,
    Archived,
    Superseded,
}
```

### 1.3 Database Repository Traits

Create traits in `/src/repositories/user_context_repository.rs`:

```rust
use async_trait::async_trait;
use rmcp::model::ErrorData as McpError;
use crate::models::user_context::*;

#[async_trait]
pub trait UserDecisionRepository: Send + Sync {
    async fn create_decision(&self, decision: &UserDecision) -> Result<UserDecision, McpError>;
    async fn find_decision_by_id(&self, id: &str) -> Result<Option<UserDecision>, McpError>;
    async fn find_decisions_by_user(&self, user_id: &str) -> Result<Vec<UserDecision>, McpError>;
    async fn find_decisions_by_scope(&self, user_id: &str, scope: &str) -> Result<Vec<UserDecision>, McpError>;
    async fn find_decisions_by_category(&self, user_id: &str, category: &str) -> Result<Vec<UserDecision>, McpError>;
    async fn update_decision(&self, decision: &UserDecision) -> Result<UserDecision, McpError>;
    async fn delete_decision(&self, id: &str) -> Result<bool, McpError>;
    async fn increment_applied_count(&self, id: &str) -> Result<(), McpError>;
    async fn archive_decision(&self, id: &str) -> Result<(), McpError>;
}

#[async_trait]
pub trait UserGoalRepository: Send + Sync {
    async fn create_goal(&self, goal: &UserGoal) -> Result<UserGoal, McpError>;
    async fn find_goal_by_id(&self, id: &str) -> Result<Option<UserGoal>, McpError>;
    async fn find_goals_by_user(&self, user_id: &str) -> Result<Vec<UserGoal>, McpError>;
    async fn find_goals_by_status(&self, user_id: &str, status: &str) -> Result<Vec<UserGoal>, McpError>;
    async fn find_goals_by_project(&self, user_id: &str, project_id: &str) -> Result<Vec<UserGoal>, McpError>;
    async fn update_goal(&self, goal: &UserGoal) -> Result<UserGoal, McpError>;
    async fn delete_goal(&self, id: &str) -> Result<bool, McpError>;
    async fn update_goal_status(&self, id: &str, status: &str) -> Result<(), McpError>;
}

#[async_trait]
pub trait UserPreferenceRepository: Send + Sync {
    async fn create_preference(&self, preference: &UserPreference) -> Result<UserPreference, McpError>;
    async fn find_preference_by_id(&self, id: &str) -> Result<Option<UserPreference>, McpError>;
    async fn find_preferences_by_user(&self, user_id: &str) -> Result<Vec<UserPreference>, McpError>;
    async fn find_preferences_by_scope(&self, user_id: &str, scope: &str) -> Result<Vec<UserPreference>, McpError>;
    async fn find_preferences_by_type(&self, user_id: &str, pref_type: &str) -> Result<Vec<UserPreference>, McpError>;
    async fn update_preference(&self, preference: &UserPreference) -> Result<UserPreference, McpError>;
    async fn delete_preference(&self, id: &str) -> Result<bool, McpError>;
    async fn increment_frequency(&self, id: &str) -> Result<(), McpError>;
}

#[async_trait]
pub trait KnownIssueRepository: Send + Sync {
    async fn create_issue(&self, issue: &KnownIssue) -> Result<KnownIssue, McpError>;
    async fn find_issue_by_id(&self, id: &str) -> Result<Option<KnownIssue>, McpError>;
    async fn find_issues_by_user(&self, user_id: &str) -> Result<Vec<KnownIssue>, McpError>;
    async fn find_issues_by_status(&self, user_id: &str, status: &str) -> Result<Vec<KnownIssue>, McpError>;
    async fn find_issues_by_severity(&self, user_id: &str, severity: &str) -> Result<Vec<KnownIssue>, McpError>;
    async fn find_issues_by_component(&self, user_id: &str, component: &str) -> Result<Vec<KnownIssue>, McpError>;
    async fn update_issue(&self, issue: &KnownIssue) -> Result<KnownIssue, McpError>;
    async fn delete_issue(&self, id: &str) -> Result<bool, McpError>;
    async fn mark_issue_resolved(&self, id: &str, resolution_status: &str) -> Result<(), McpError>;
}

#[async_trait]
pub trait ContextualTodoRepository: Send + Sync {
    async fn create_todo(&self, todo: &ContextualTodo) -> Result<ContextualTodo, McpError>;
    async fn find_todo_by_id(&self, id: &str) -> Result<Option<ContextualTodo>, McpError>;
    async fn find_todos_by_user(&self, user_id: &str) -> Result<Vec<ContextualTodo>, McpError>;
    async fn find_todos_by_status(&self, user_id: &str, status: &str) -> Result<Vec<ContextualTodo>, McpError>;
    async fn find_todos_by_project(&self, user_id: &str, project_id: &str) -> Result<Vec<ContextualTodo>, McpError>;
    async fn find_todos_by_entity(&self, entity_id: &str) -> Result<Vec<ContextualTodo>, McpError>;
    async fn update_todo(&self, todo: &ContextualTodo) -> Result<ContextualTodo, McpError>;
    async fn delete_todo(&self, id: &str) -> Result<bool, McpError>;
    async fn update_todo_status(&self, id: &str, status: &str) -> Result<(), McpError>;
}
```

### 1.4 SQLite Repository Implementations

Create implementations in `/src/infrastructure/` directory:

- `sqlite_user_decision_repository.rs`
- `sqlite_user_goal_repository.rs`
- `sqlite_user_preference_repository.rs`
- `sqlite_known_issue_repository.rs`
- `sqlite_contextual_todo_repository.rs`

Each should implement the corresponding trait with SQLite CRUD operations.

### 1.5 CLI Commands

Add new CLI command group in `/src/cli/handlers/user_context_commands.rs`:

```bash
# Decision management
context-server-rs decision create --text "Use async/await for concurrency" --reason "Better error handling" --category architecture --scope global
context-server-rs decision list --user-id <user-id>
context-server-rs decision list --scope global
context-server-rs decision list --project myapp
context-server-rs decision show <decision-id>
context-server-rs decision update <decision-id> --text "Updated decision text"
context-server-rs decision archive <decision-id>
context-server-rs decision delete <decision-id>

# Goal management
context-server-rs goal create --text "Integrate MCP with context-server-rs" --priority 1 --deadline "2026-02-28"
context-server-rs goal list --user-id <user-id> --status pending
context-server-rs goal list --project myapp
context-server-rs goal show <goal-id>
context-server-rs goal update <goal-id> --status in_progress
context-server-rs goal add-step <goal-id> --step "Set up test environment" --due-date "2026-02-10"
context-server-rs goal mark-completed <goal-id>

# Preference management
context-server-rs preference create --name "Lightweight services" --value "prefer minimal dependencies" --type constraint --scope global
context-server-rs preference list --user-id <user-id>
context-server-rs preference list --scope global --tags architecture
context-server-rs preference show <preference-id>
context-server-rs preference update <preference-id>

# Issue management
context-server-rs issue create --description "MCP integration fails on port conflicts" --severity high --category integration
context-server-rs issue list --user-id <user-id> --status unresolved
context-server-rs issue show <issue-id>
context-server-rs issue add-workaround <issue-id> --workaround "Use port 9000 instead"
context-server-rs issue mark-resolved <issue-id> --status fixed

# Todo management
context-server-rs todo create --task "Implement decision validation" --relates-to decision:<decision-id>
context-server-rs todo list --user-id <user-id> --status pending
context-server-rs todo mark-done <todo-id>
```

### 1.6 Output Formatting

Enhance `/src/cli/output.rs` with formatters for:
- Decision display (decision text, reason, scope, applied count)
- Goal progress visualization (steps, completion %)
- Preference highlighting (scope, frequency_observed, tags)
- Issue severity indicators
- Todo status badges

### 1.7 Testing

Create unit tests in `/tests/user_context_tests.rs`:
- CRUD operations for each entity
- Query filtering (by scope, status, etc.)
- Relationship integrity
- Edge cases (duplicate entries, orphaned references)

---

## Phase 2: Conversation Analysis & Auto-Extraction

### Objective
Enables automatic extraction of decisions, goals, preferences, and issues from conversation text.

### 2.1 Conversation Analyzer Service

Create `/src/services/conversation_analyzer.rs`:

```rust
use async_trait::async_trait;
use regex::Regex;
use crate::models::user_context::*;

pub struct ConversationContextExtractor {
    decision_patterns: Vec<Regex>,
    goal_patterns: Vec<Regex>,
    preference_patterns: Vec<Regex>,
    issue_patterns: Vec<Regex>,
    confidence_calculator: ConfidenceCalculator,
}

impl ConversationContextExtractor {
    pub fn new() -> Self {
        Self {
            decision_patterns: Self::compile_decision_patterns(),
            goal_patterns: Self::compile_goal_patterns(),
            preference_patterns: Self::compile_preference_patterns(),
            issue_patterns: Self::compile_issue_patterns(),
            confidence_calculator: ConfidenceCalculator::new(),
        }
    }

    /// Extract decisions from conversation text
    /// Keywords: "we decided", "let's use", "prefer", "avoid", "we'll use"
    pub fn extract_decisions(&self, text: &str) -> Vec<ExtractedDecision> {
        let mut decisions = Vec::new();
        
        for pattern in &self.decision_patterns {
            if let Some(captures) = pattern.captures(text) {
                decisions.push(ExtractedDecision {
                    text: captures.get(1).map(|m| m.as_str().to_string()),
                    confidence: 0.6, // Base confidence for pattern match
                    category: Self::infer_category(text),
                    context: captures.get(0).map(|m| m.as_str().to_string()),
                });
            }
        }
        
        // Boost confidence for repeated mentions
        self.adjust_for_frequency(&mut decisions, text);
        decisions
    }

    /// Extract goals from conversation text
    /// Keywords: "goal", "objective", "need to", "want to", "should", "plan to"
    pub fn extract_goals(&self, text: &str) -> Vec<ExtractedGoal> {
        let mut goals = Vec::new();
        
        for pattern in &self.goal_patterns {
            if let Some(captures) = pattern.captures(text) {
                goals.push(ExtractedGoal {
                    text: captures.get(1).map(|m| m.as_str().to_string()),
                    has_deadline: self.extract_deadline(text),
                    has_steps: self.extract_steps(text),
                    priority: self.infer_priority(text),
                    confidence: 0.7,
                });
            }
        }
        
        goals
    }

    /// Extract preferences from conversation text
    /// Keywords: "always", "never", "typically", "usually", "prefer", "avoid"
    pub fn extract_preferences(&self, text: &str) -> Vec<ExtractedPreference> {
        let mut preferences = Vec::new();
        
        for pattern in &self.preference_patterns {
            if let Some(captures) = pattern.captures(text) {
                preferences.push(ExtractedPreference {
                    text: captures.get(1).map(|m| m.as_str().to_string()),
                    pref_type: Self::infer_preference_type(text),
                    confidence: 0.5,
                    tags: self.extract_tags(text),
                });
            }
        }
        
        preferences
    }

    /// Extract known issues from conversation text
    /// Keywords: "problem", "issue", "error", "bug", "fails", "doesn't work"
    pub fn extract_issues(&self, text: &str) -> Vec<ExtractedIssue> {
        let mut issues = Vec::new();
        
        for pattern in &self.issue_patterns {
            if let Some(captures) = pattern.captures(text) {
                issues.push(ExtractedIssue {
                    description: captures.get(1).map(|m| m.as_str().to_string()),
                    symptoms: self.extract_symptoms(text),
                    workaround: self.extract_workaround(text),
                    severity: Self::infer_severity(text),
                    confidence: 0.6,
                });
            }
        }
        
        issues
    }
}

pub struct ExtractedDecision {
    pub text: Option<String>,
    pub confidence: f32,
    pub category: DecisionCategory,
    pub context: Option<String>,
}

pub struct ExtractedGoal {
    pub text: Option<String>,
    pub has_deadline: bool,
    pub has_steps: bool,
    pub priority: u32,
    pub confidence: f32,
}

pub struct ExtractedPreference {
    pub text: Option<String>,
    pub pref_type: PreferenceType,
    pub confidence: f32,
    pub tags: Vec<String>,
}

pub struct ExtractedIssue {
    pub description: Option<String>,
    pub symptoms: Vec<String>,
    pub workaround: Option<String>,
    pub severity: IssueSeverity,
    pub confidence: f32,
}

struct ConfidenceCalculator;

impl ConfidenceCalculator {
    fn new() -> Self {
        Self
    }
}
```

### 2.2 Integration with MCP Server

Add conversation analysis to MCP tool handlers in `/src/api/`:

```rust
pub struct AnalyzeConversationTool;

#[async_trait]
impl McpTool for AnalyzeConversationTool {
    ~async fn execute(&self, input: serde_json::Value) -> Result<String> {
        let conversation_text = input["conversation_text"].as_str().ok_or("Missing conversation_text")?;
        let user_id = input["user_id"].as_str().ok_or("Missing user_id")?;
        
        let extractor = ConversationContextExtractor::new();
        
        let extracted_decisions = extractor.extract_decisions(conversation_text);
        let extracted_goals = extractor.extract_goals(conversation_text);
        let extracted_preferences = extractor.extract_preferences(conversation_text);
        let extracted_issues = extractor.extract_issues(conversation_text);
        
        // Return analysis results with high-confidence items marked for review
        Ok(serde_json::to_string(&json!({
            "decisions": extracted_decisions,
            "goals": extracted_goals,
            "preferences": extracted_preferences,
            "issues": extracted_issues,
            "action_required": true,
            "message": "Review extracted items and confirm which to save"
        }))?)
    }
}
```

### 2.3 Conversation History Storage

Add opt-in conversation logging in `/src/services/conversation_logger.rs`:

```rust
pub struct ConversationLogger {
    repo: Arc<dyn ConversationRepository>,
}

pub struct ConversationRecord {
    pub id: String,
    pub user_id: String,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub message_type: MessageType,  // 'user' | 'assistant' | 'error'
    pub message_content: String,
    pub tokens_used: Option<i32>,
    pub extracted_contexts: Option<Vec<ExtractedContext>>,  // Results from analyzer
}

#[derive(Clone, Copy)]
pub enum MessageType {
    User,
    Assistant,
    Error,
}
```

### 2.4 Pattern Library

Create `/src/services/pattern_library.rs` with configurable regex patterns:

```rust
pub struct PatternLibrary {
    patterns: HashMap<String, Pattern>,
}

pub struct Pattern {
    pub category: String,
    pub regex: String,
    pub weight: f32,
    pub examples: Vec<String>,
}

// Example patterns:
// Decision: "we (decided|chose|picked|will use) (.*)"
// Goal: "(we need to|we should|next step.*|plan to|objective.*) (.*)"
// Preference: "(always|never|typically|prefer|avoid) (.*)"
// Issue: "(problem|error|issue|bug|fails|doesn't work) with (.*)"
```

### 2.5 CLI Commands for Analysis

```bash
# Analyze conversation and see extracted contexts
context-server-rs analyze-conversation --text "<conversation text>" --user-id <user-id>

# Batch analyze from conversation logs
context-server-rs analyze-batch --session-id <session-id> --confidence-threshold 0.6

# Confirm and save extracted contexts
context-server-rs save-extracted --extraction-id <extraction-id> --confirm
```

### 2.6 Testing

Create `/tests/conversation_analysis_tests.rs`:
- Pattern matching accuracy tests
- Confidence calculation tests
- Multi-sentence extraction
- Edge cases (negation, sarcasm detection)
- Performance benchmarks

---

## Phase 3: Intelligence & Action

### Objective
Enable OpenClaw to use saved context for intelligent decision-making and validation.

### 3.1 Action Validation Service

Create `/src/services/openclaw_context_service.rs`:

```rust
pub struct OpenClawContextService {
    decision_repo: Arc<dyn UserDecisionRepository>,
    preference_repo: Arc<dyn UserPreferenceRepository>,
    issue_repo: Arc<dyn KnownIssueRepository>,
    goal_repo: Arc<dyn UserGoalRepository>,
}

pub struct ActionValidation {
    pub is_valid: bool,
    pub warnings: Vec<String>,
    pub violations: Vec<String>,
    pub applied_decisions: Vec<String>,
    pub applicable_workarounds: Vec<String>,
    pub recommendations: Vec<String>,
}

impl OpenClawContextService {
    /// Validate if an action violates user preferences or known issues
    pub async fn validate_action(
        &self,
        action: &ProposedAction,
        user_id: &str,
        project_id: Option<&str>,
    ) -> Result<ActionValidation> {
        let mut validation = ActionValidation::default();
        
        // Check against user decisions
        let decisions = self.decision_repo.find_decisions_by_user(user_id).await?;
        for decision in decisions {
            if self.matches_action(&decision, action) {
                validation.applied_decisions.push(decision.id);
            }
            if self.violates_decision(&decision, action) {
                validation.violations.push(format!(
                    "Violates decision: {}",
                    decision.decision_text
                ));
            }
        }
        
        // Check against preferences
        let preferences = self.preference_repo.find_preferences_by_user(user_id).await?;
        for preference in preferences {
            if preference.applies_to_automation && self.violates_preference(&preference, action) {
                validation.violations.push(format!(
                    "Violates preference: {}",
                    preference.preference_name
                ));
            }
            if self.matches_preference(&preference, action) {
                validation.warnings.push(format!(
                    "Note: Preference '{0}' suggests: {1}",
                    preference.preference_name,
                    preference.preference_value
                ));
            }
        }
        
        // Check for known issues with workarounds
        let issues = self.issue_repo.find_issues_by_user(user_id).await?;
        for issue in issues {
            if self.matches_issue(&issue, action) {
                if let Some(workaround) = &issue.workaround {
                    validation.applicable_workarounds.push(format!(
                        "Known issue: {}. Workaround: {}",
                        issue.issue_description,
                        workaround
                    ));
                    validation.warnings.push(workaround.clone());
                }
            }
        }
        
        // Check goal alignment
        let goals = self.goal_repo.find_goals_by_user(user_id).await?;
        for goal in goals {
            if goal.status == GoalStatus::InProgress {
                if self.supports_goal(&goal, action) {
                    validation.recommendations.push(format!(
                        "This action aligns with goal: {}",
                        goal.goal_text
                    ));
                }
            }
        }
        
        validation.is_valid = validation.violations.is_empty();
        Ok(validation)
    }

    /// Get recommended next steps based on active goals
    pub async fn recommend_next_steps(
        &self,
        user_id: &str,
        project_id: Option<&str>,
    ) -> Result<Vec<Recommendation>> {
        let mut recommendations = Vec::new();
        
        // Get active goals
        let goals = self.goal_repo.find_goals_by_user(user_id).await?;
        
        for goal in goals.iter().filter(|g| g.status == GoalStatus::InProgress) {
            // Find first incomplete step
            if let Some(step) = goal.steps.iter().find(|s| s.status != GoalStatus::Completed) {
                recommendations.push(Recommendation {
                    priority: goal.priority,
                    action: format!("Complete goal step: {}", step.description),
                    rationale: format!("Part of goal: {}", goal.goal_text),
                    related_entity_id: goal.id.clone(),
                });
            }
        }
        
        // Sort by priority
        recommendations.sort_by_key(|r| r.priority);
        
        Ok(recommendations)
    }

    /// Suggest improvements based on past decisions
    pub async fn suggest_improvements(
        &self,
        user_id: &str,
        area: &str,
    ) -> Result<Vec<SuggestionWithRationale>> {
        let mut suggestions = Vec::new();
        
        // Find past decisions in this area
        let decisions = self.decision_repo.find_decisions_by_category(user_id, area).await?;
        
        for decision in decisions.iter().filter(|d| d.applied_count > 0) {
            suggestions.push(SuggestionWithRationale {
                suggestion: decision.decision_text.clone(),
                success_rate: (decision.applied_count as f32) / 10.0, // Normalized
                times_applied: decision.applied_count,
                last_applied: decision.last_applied,
            });
        }
        
        Ok(suggestions)
    }

    /// Learn from patterns: what decisions were most successful?
    pub async fn get_decision_effectiveness(
        &self,
        user_id: &str,
    ) -> Result<Vec<DecisionEffectiveness>> {
        let decisions = self.decision_repo.find_decisions_by_user(user_id).await?;
        
        let effectiveness: Vec<_> = decisions
            .into_iter()
            .map(|d| DecisionEffectiveness {
                decision_id: d.id,
                decision_text: d.decision_text,
                times_applied: d.applied_count,
                confidence_score: d.confidence_score,
                effectiveness_rating: calculate_effectiveness(d.applied_count, d.confidence_score),
            })
            .collect();
        
        Ok(effectiveness)
    }
}

pub struct ProposedAction {
    pub action_type: String,
    pub target: String,
    pub parameters: HashMap<String, String>,
}

pub struct Recommendation {
    pub priority: u32,
    pub action: String,
    pub rationale: String,
    pub related_entity_id: String,
}

pub struct SuggestionWithRationale {
    pub suggestion: String,
    pub success_rate: f32,
    pub times_applied: i32,
    pub last_applied: Option<DateTime<Utc>>,
}

pub struct DecisionEffectiveness {
    pub decision_id: String,
    pub decision_text: String,
    pub times_applied: i32,
    pub confidence_score: f32,
    pub effectiveness_rating: f32,
}
```

### 3.2 Conflict Detection

Create `/src/services/context_conflict_detector.rs`:

```rust
pub struct ContextConflictDetector {
    preference_repo: Arc<dyn UserPreferenceRepository>,
    decision_repo: Arc<dyn UserDecisionRepository>,
}

pub struct ConflictReport {
    pub has_conflicts: bool,
    pub conflicts: Vec<Conflict>,
    pub recommendations: Vec<String>,
}

pub struct Conflict {
    pub conflict_type: ConflictType,
    pub entity_ids: Vec<String>,
    pub description: String,
    pub severity: ConflictSeverity,
}

#[derive(Debug)]
pub enum ConflictType {
    PreferenceContradiction,      // Two preferences contradict
    PreferenceViolation,          // Action violates preference
    DecisionChange,               // Newer decision contradicts older
    IssueIgnored,                 // Known issue not addressed
}

#[derive(Debug)]
pub enum ConflictSeverity {
    Critical,
    Warning,
    Info,
}

impl ContextConflictDetector {
    /// Find contradictions in saved preferences
    pub async fn detect_preference_conflicts(&self, user_id: &str) -> Result<Vec<Conflict>> {
        let preferences = self.preference_repo.find_preferences_by_user(user_id).await?;
        let mut conflicts = Vec::new();
        
        // Check for contradicting preferences
        for (i, pref1) in preferences.iter().enumerate() {
            for pref2 in preferences.iter().skip(i + 1) {
                if self.are_contradictory(pref1, pref2) {
                    conflicts.push(Conflict {
                        conflict_type: ConflictType::PreferenceContradiction,
                        entity_ids: vec![pref1.id.clone(), pref2.id.clone()],
                        description: format!(
                            "Conflicting preferences: '{}' vs '{}'",
                            pref1.preference_name,
                            pref2.preference_name
                        ),
                        severity: ConflictSeverity::Warning,
                    });
                }
            }
        }
        
        Ok(conflicts)
    }

    /// Find decisions that contradict each other
    pub async fn detect_decision_conflicts(&self, user_id: &str) -> Result<Vec<Conflict>> {
        let decisions = self.decision_repo.find_decisions_by_user(user_id).await?;
        let mut conflicts = Vec::new();
        
        // Sort by date to find contradictory changes
        let mut sorted_decisions = decisions.clone();
        sorted_decisions.sort_by_key(|d| d.created_at);
        
        for (i, decision1) in sorted_decisions.iter().enumerate() {
            for decision2 in sorted_decisions.iter().skip(i + 1) {
                if self.are_contradictory_decisions(decision1, decision2) {
                    conflicts.push(Conflict {
                        conflict_type: ConflictType::DecisionChange,
                        entity_ids: vec![decision1.id.clone(), decision2.id.clone()],
                        description: format!(
                            "Decision changed from '{}' to '{}'",
                            decision1.decision_text,
                            decision2.decision_text
                        ),
                        severity: ConflictSeverity::Info,
                    });
                }
            }
        }
        
        Ok(conflicts)
    }
}
```

### 3.3 Success Tracking

Create `/src/services/decision_effectiveness_tracker.rs`:

```rust
pub struct DecisionEffectivenessTracker {
    decision_repo: Arc<dyn UserDecisionRepository>,
}

impl DecisionEffectivenessTracker {
    /// Record that a decision was applied
    pub async fn record_decision_application(
        &self,
        decision_id: &str,
        success: bool,
    ) -> Result<()> {
        self.decision_repo.increment_applied_count(decision_id).await?;
        Ok(())
    }

    /// Get decisions ranked by effectiveness
    pub async fn get_most_effective_decisions(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<EffectiveDecision>> {
        let decisions = self.decision_repo.find_decisions_by_user(user_id).await?;
        
        let mut effective: Vec<_> = decisions
            .into_iter()
            .filter(|d| d.applied_count > 0)
            .map(|d| EffectiveDecision {
                decision: d.clone(),
                effectiveness_score: (d.applied_count as f32) * d.confidence_score,
            })
            .collect();
        
        effective.sort_by(|a, b| {
            b.effectiveness_score.partial_cmp(&a.effectiveness_score).unwrap()
        });
        
        Ok(effective.into_iter().take(limit).collect())
    }
}

pub struct EffectiveDecision {
    pub decision: UserDecision,
    pub effectiveness_score: f32,
}
```

### 3.4 MCP Tools for OpenClaw Integration

```bash
# Validate actions before executing
context-server-rs mcp-tool validate-action --action-type deploy --target production --user-id <user-id>

# Get recommendations for next steps
context-server-rs mcp-tool get-recommendations --user-id <user-id> --project-id myapp

# Check for conflicts in saved context
context-server-rs mcp-tool detect-conflicts --user-id <user-id>

# Record decision application
context-server-rs mcp-tool record-decision-application --decision-id <id> --success true
```

### 3.5 Testing

Create `/tests/openclaw_integration_tests.rs`:
- Action validation scenarios
- Conflict detection accuracy
- Recommendation ranking
- Success tracking for decisions
- Performance with large context datasets

---

## Phase 4: Collaboration & Advanced Features

### Objective
Enable team-wide context sharing, real-time collaboration, and advanced querying.

### 4.1 Multi-User Support

Extend data models to support team contexts:

```rust
pub struct TeamContext {
    pub id: String,
    pub team_id: String,
    pub scope: TeamScope,  // 'shared', 'read_only', 'restricted'
    pub entity_type: String,  // 'decision', 'preference', 'issue'
    pub entity_id: String,
    pub visibility: Visibility,  // 'public', 'team', 'restricted'
    pub created_by: String,
    pub approved_by: Option<String>,
    pub approval_status: ApprovalStatus,
}

#[derive(Clone, Copy)]
pub enum Visibility {
    Public,
    Team,
    Restricted,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
}
```

### 4.2 Real-Time WebSocket Layer

Create `/src/services/realtime_collaboration.rs`:

```rust
pub struct RealtimeCollaborationServer {
    clients: Arc<Mutex<HashMap<String, ClientSession>>>,
    event_broadcaster: broadcast::Sender<CollaborationEvent>,
}

pub struct CollaborationEvent {
    pub event_type: EventType,
    pub entity_id: String,
    pub changed_by: String,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
}

pub enum EventType {
    ContextCreated,
    ContextUpdated,
    ContextDeleted,
    ConflictDetected,
    ApprovalRequested,
}

impl RealtimeCollaborationServer {
    pub async fn subscribe_to_updates(&self, user_id: &str) -> Receiver<CollaborationEvent> {
        let (tx, rx) = broadcast::channel(100);
        let session = ClientSession {
            user_id: user_id.to_string(),
            sender: tx,
            connected_at: Utc::now(),
        };
        self.clients.lock().await.insert(user_id.to_string(), session);
        rx
    }

    pub async fn broadcast_event(&self, event: CollaborationEvent) -> Result<()> {
        self.event_broadcaster.send(event)?;
        Ok(())
    }
}
```

### 4.3 Advanced Querying

Create `/src/services/context_query_engine.rs`:

```rust
pub struct ContextQueryEngine {
    repos: RepositorySet,
}

pub struct QueryBuilder {
    filters: Vec<QueryFilter>,
    sort_by: Vec<SortCriteria>,
    limit: Option<usize>,
}

pub enum QueryFilter {
    UserScope(String),
    EntityType(String),
    Status(String),
    Tag(String),
    DateRange(DateTime<Utc>, DateTime<Utc>),
    Similarity(String, f32),  // Search text and threshold
}

impl QueryBuilder {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
            sort_by: Vec::new(),
            limit: None,
        }
    }

    pub fn with_filter(mut self, filter: QueryFilter) -> Self {
        self.filters.push(filter);
        self
    }

    pub async fn execute(&self, engine: &ContextQueryEngine) -> Result<QueryResults> {
        // Build and execute complex queries across entity types
        Ok(QueryResults::default())
    }
}

pub struct QueryResults {
    pub decisions: Vec<UserDecision>,
    pub goals: Vec<UserGoal>,
    pub preferences: Vec<UserPreference>,
    pub issues: Vec<KnownIssue>,
    pub todos: Vec<ContextualTodo>,
}
```

### 4.4 Integration with External Systems

Create APIs for integration with:
- GitHub (sync decisions/issues from GitHub Issues)
- Jira (link contexts to tickets)
- Slack (share context updates, request approvals)
- Email (digest reports of pending decisions)

### 4.5 Analytics & Reporting

Create `/src/services/context_analytics.rs`:

```rust
pub struct ContextAnalytics {
    repos: RepositorySet,
}

pub struct ContextHealthReport {
    pub organization_score: f32,    // 0.0-1.0
    pub utilization_score: f32,     // Are saved contexts actually used?
    pub consistency_score: f32,      // Are preferences consistent?
    pub coverage_score: f32,         // Percentage of decisions captured
    pub issues_unresolved: i32,
    pub outdated_decisions: i32,
    pub recommendations: Vec<String>,
}

impl ContextAnalytics {
    pub async fn generate_health_report(&self, user_id: &str) -> Result<ContextHealthReport> {
        // Analyze health of user's saved contexts
        Ok(ContextHealthReport::default())
    }

    pub async fn get_usage_statistics(&self, user_id: &str) -> Result<UsageStatistics> {
        // Track how often contexts are accessed/applied
        Ok(UsageStatistics::default())
    }

    pub async fn predictive_context_suggestions(
        &self,
        user_id: &str,
        upcomingTask: &str,
    ) -> Result<Vec<SuggestedContext>> {
        // ML-based: suggest relevant contexts based on task description
        Ok(Vec::new())
    }
}
```

### 4.6 Advanced CLI Commands

```bash
# Team collaboration
context-server-rs share-context <entity-id> --team-id <team-id> --visibility team

# Query across contexts
context-server-rs query "SELECT * FROM contexts WHERE tags LIKE '%async%' AND status='active'"

# Generate reports
context-server-rs report health --format pdf
context-server-rs report usage --user-id <user-id> --period month

# Real-time subscriptions (via WebSocket)
context-server-rs subscribe --user-id <user-id> --event-types context_updated,approval_requested
```

### 4.7 Testing

Create `/tests/advanced_features_tests.rs`:
- Multi-user scenarios
- WebSocket connection handling
- Complex query scenarios
- Team permission enforcement
- Analytics computation accuracy

---

## Implementation Checklist

### Phase 1 Deliverables:
- [ ] Database schema with 5 new tables
- [ ] 5 Rust data model structs + enums
- [ ] 5 SQLite repository implementations
- [ ] 20+ CLI commands for CRUD operations
- [ ] Output formatters for all entity types
- [ ] Unit tests (>80% coverage)

### Phase 2 Deliverables:
- [ ] Conversation analyzer service with regex patterns
- [ ] MCP tool for analyzing conversations
- [ ] Conversation logging/storage
- [ ] Pattern library with >50 patterns
- [ ] CLI commands for analysis
- [ ] Integration tests for extraction accuracy

### Phase 3 Deliverables:
- [ ] Action validation service with decision/preference checking
- [ ] Conflict detection engine
- [ ] Success tracking for decisions
- [ ] MCP tools for validation and recommendations
- [ ] Recommendation ranking algorithm
- [ ] Integration tests with realistic scenarios

### Phase 4 Deliverables:
- [ ] Team context models + permissions
- [ ] WebSocket real-time collaboration
- [ ] Advanced query engine
- [ ] External system integrations (GitHub, Jira, Slack)
- [ ] Analytics and reporting service
- [ ] CLI commands for team features
- [ ] End-to-end integration tests

---

## Success Metrics

1. **Adoption**: OpenClaw uses context for >70% of automated decisions
2. **Effectiveness**: User validates >80% of OpenClaw recommendations
3. **Learning**: Decision effectiveness improves over time (confidence scores increase)
4. **Coverage**: >50 unique decisions/preferences captured per project
5. **Conflict Resolution**: System detects and reports conflicts before execution
6. **Performance**: Context queries complete in <100ms

---

## Technical Stack Summary

- **Language**: Rust
- **Async Runtime**: Tokio
- **Database**: SQLite (embedded)
- **Web Framework**: Actix-web (for Phase 4 WebSocket)
- **Pattern Matching**: regex crate
- **Serialization**: serde + serde_json
- **Testing**: tokio::test + mock implementations
- **MCP Integration**: rmcp SDK

---

## References

- Project: context-server-rs
- MCP Specification: https://modelcontextprotocol.io
- Database Design: See schema section above
- CLI Architecture: `/src/cli/`
- Service Layer: `/src/services/`

