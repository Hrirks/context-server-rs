# Phase 1 Implementation Guide: Foundation (Database & Basic CLI)

This document provides detailed, code-ready implementations for Phase 1 of the OpenClaw Context Integration system.

---

## Table of Contents

1. [Database Setup](#database-setup)
2. [Data Models](#data-models)
3. [Repository Implementations](#repository-implementations)
4. [CLI Handlers](#cli-handlers)
5. [Output Formatters](#output-formatters)
6. [Testing](#testing)
7. [Configuration](#configuration)

---

## Database Setup

### 1.1 Migration Script

Create `migrations/001_create_user_context_tables.sql`:

```sql
-- User/Agent Decision Registry
CREATE TABLE IF NOT EXISTS user_decisions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    decision_text TEXT NOT NULL,
    reason TEXT,
    decision_category TEXT NOT NULL,
    scope TEXT NOT NULL,
    related_project_id TEXT,
    confidence_score REAL DEFAULT 0.5,
    referenced_items TEXT DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT,
    applied_count INTEGER DEFAULT 0,
    last_applied TEXT,
    status TEXT DEFAULT 'active',
    FOREIGN KEY(related_project_id) REFERENCES projects(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_user_decisions_user 
    ON user_decisions(user_id);
CREATE INDEX IF NOT EXISTS idx_user_decisions_scope 
    ON user_decisions(scope);
CREATE INDEX IF NOT EXISTS idx_user_decisions_status 
    ON user_decisions(status);
CREATE INDEX IF NOT EXISTS idx_user_decisions_category 
    ON user_decisions(decision_category);
CREATE INDEX IF NOT EXISTS idx_user_decisions_created 
    ON user_decisions(created_at DESC);

-- User Goals & Plans
CREATE TABLE IF NOT EXISTS user_goals (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    goal_text TEXT NOT NULL,
    description TEXT,
    project_id TEXT,
    status TEXT NOT NULL,
    priority INTEGER DEFAULT 3,
    steps TEXT DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT,
    completion_target_date TEXT,
    completion_date TEXT,
    blockers TEXT DEFAULT '[]',
    related_todos TEXT DEFAULT '[]',
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_user_goals_user 
    ON user_goals(user_id);
CREATE INDEX IF NOT EXISTS idx_user_goals_status 
    ON user_goals(status);
CREATE INDEX IF NOT EXISTS idx_user_goals_project 
    ON user_goals(project_id);
CREATE INDEX IF NOT EXISTS idx_user_goals_priority 
    ON user_goals(priority);

-- Recurring Preferences (habits/constraints)
CREATE TABLE IF NOT EXISTS user_preferences (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    preference_name TEXT NOT NULL,
    preference_value TEXT NOT NULL,
    preference_type TEXT NOT NULL,
    scope TEXT NOT NULL,
    applies_to_automation BOOLEAN DEFAULT 1,
    rationale TEXT,
    priority INTEGER DEFAULT 3,
    frequency_observed INTEGER DEFAULT 1,
    tags TEXT DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT,
    last_referenced TEXT,
    UNIQUE(user_id, preference_name, scope)
);

CREATE INDEX IF NOT EXISTS idx_user_preferences_user 
    ON user_preferences(user_id);
CREATE INDEX IF NOT EXISTS idx_user_preferences_scope 
    ON user_preferences(scope);
CREATE INDEX IF NOT EXISTS idx_user_preferences_type 
    ON user_preferences(preference_type);
CREATE INDEX IF NOT EXISTS idx_user_preferences_automation 
    ON user_preferences(applies_to_automation);

-- Known Issues & Solutions
CREATE TABLE IF NOT EXISTS known_issues (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    issue_description TEXT NOT NULL,
    symptoms TEXT DEFAULT '[]',
    root_cause TEXT,
    workaround TEXT,
    permanent_solution TEXT,
    affected_components TEXT DEFAULT '[]',
    severity TEXT NOT NULL,
    issue_category TEXT NOT NULL,
    learned_date TEXT NOT NULL,
    resolution_status TEXT NOT NULL,
    resolution_date TEXT,
    prevention_notes TEXT,
    project_contexts TEXT DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_known_issues_user 
    ON known_issues(user_id);
CREATE INDEX IF NOT EXISTS idx_known_issues_status 
    ON known_issues(resolution_status);
CREATE INDEX IF NOT EXISTS idx_known_issues_severity 
    ON known_issues(severity);
CREATE INDEX IF NOT EXISTS idx_known_issues_category 
    ON known_issues(issue_category);
CREATE INDEX IF NOT EXISTS idx_known_issues_learned 
    ON known_issues(learned_date DESC);

-- Contextual To-Dos
CREATE TABLE IF NOT EXISTS contextual_todos (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    task_description TEXT NOT NULL,
    context_type TEXT NOT NULL,
    related_entity_id TEXT,
    related_entity_type TEXT,
    project_id TEXT,
    assigned_to TEXT,
    due_date TEXT,
    status TEXT DEFAULT 'pending',
    priority INTEGER DEFAULT 3,
    created_from_conversation_date TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT,
    completion_date TEXT,
    FOREIGN KEY(related_entity_id) REFERENCES user_decisions(id) ON DELETE SET NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_contextual_todos_user 
    ON contextual_todos(user_id);
CREATE INDEX IF NOT EXISTS idx_contextual_todos_status 
    ON contextual_todos(status);
CREATE INDEX IF NOT EXISTS idx_contextual_todos_entity 
    ON contextual_todos(related_entity_id);
CREATE INDEX IF NOT EXISTS idx_contextual_todos_project 
    ON contextual_todos(project_id);
CREATE INDEX IF NOT EXISTS idx_contextual_todos_due 
    ON contextual_todos(due_date);

-- Audit log for context changes
CREATE TABLE IF NOT EXISTS user_context_audit (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    action TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT,
    changed_by TEXT,
    changed_at TEXT NOT NULL,
    reason TEXT
);

CREATE INDEX IF NOT EXISTS idx_audit_user 
    ON user_context_audit(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_entity 
    ON user_context_audit(entity_id);
CREATE INDEX IF NOT EXISTS idx_audit_timestamp 
    ON user_context_audit(changed_at DESC);
```

### 1.2 Database Initialization Code

Create `src/db/user_context_init.rs`:

```rust
use rusqlite::Connection;
use std::error::Error;

pub fn init_user_context_tables(conn: &Connection) -> Result<(), Box<dyn Error>> {
    // Read and execute migration
    let migrations = vec![
        include_str!("../../migrations/001_create_user_context_tables.sql"),
    ];

    for migration in migrations {
        // Split by semicolon and execute each statement
        for statement in migration.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                conn.execute(trimmed, [])?;
            }
        }
    }

    Ok(())
}

pub fn verify_user_context_schema(conn: &Connection) -> Result<bool, Box<dyn Error>> {
    // Check if all tables exist
    let tables = vec![
        "user_decisions",
        "user_goals",
        "user_preferences",
        "known_issues",
        "contextual_todos",
        "user_context_audit",
    ];

    for table in tables {
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        )?;

        if count == 0 {
            return Ok(false);
        }
    }

    Ok(true)
}
```

### 1.3 Connection Pool Integration

Update `src/db/connection_pool.rs`:

```rust
use crate::db::user_context_init::init_user_context_tables;

pub async fn initialize_database(pool: &ConnectionPool) -> Result<(), Box<dyn Error>> {
    let conn = pool.get()?;
    
    // Initialize base tables
    init_project_tables(&conn)?;
    
    // Initialize user context tables
    init_user_context_tables(&conn)?;
    
    tracing::info!("User context tables initialized successfully");
    Ok(())
}
```

---

## Data Models

### 2.1 User Context Models

Create `src/models/user_context.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ============ User Decision ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl UserDecision {
    pub fn new(
        user_id: String,
        decision_text: String,
        decision_category: DecisionCategory,
        scope: ContextScope,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            decision_text,
            reason: None,
            decision_category,
            scope,
            related_project_id: None,
            confidence_score: 0.5,
            referenced_items: Vec::new(),
            created_at: Utc::now(),
            updated_at: None,
            applied_count: 0,
            last_applied: None,
            status: EntityStatus::Active,
        }
    }

    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }

    pub fn with_project(mut self, project_id: String) -> Self {
        self.related_project_id = Some(project_id);
        self
    }

    pub fn with_confidence(mut self, score: f32) -> Self {
        self.confidence_score = score.max(0.0).min(1.0);
        self
    }

    pub fn increment_applied_count(&mut self) {
        self.applied_count += 1;
        self.last_applied = Some(Utc::now());
        self.updated_at = Some(Utc::now());
    }

    pub fn archive(&mut self) {
        self.status = EntityStatus::Archived;
        self.updated_at = Some(Utc::now());
    }
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
    #[serde(other)]
    Other,
}

impl DecisionCategory {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Architecture => "architecture",
            Self::ToolChoice => "tool_choice",
            Self::Constraint => "constraint",
            Self::Workflow => "workflow",
            Self::Performance => "performance",
            Self::Security => "security",
            Self::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "architecture" => Self::Architecture,
            "tool_choice" => Self::ToolChoice,
            "constraint" => Self::Constraint,
            "workflow" => Self::Workflow,
            "performance" => Self::Performance,
            "security" => Self::Security,
            _ => Self::Other,
        }
    }
}

// ============ User Goal ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl UserGoal {
    pub fn new(user_id: String, goal_text: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            goal_text,
            description: None,
            project_id: None,
            status: GoalStatus::Planned,
            priority: 3,
            steps: Vec::new(),
            created_at: Utc::now(),
            updated_at: None,
            completion_target_date: None,
            completion_date: None,
            blockers: Vec::new(),
            related_todos: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: String) -> Self {
        self.description = Some(desc);
        self
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority.max(1).min(5);
        self
    }

    pub fn add_step(&mut self, step: GoalStep) {
        self.steps.push(step);
        self.updated_at = Some(Utc::now());
    }

    pub fn mark_started(&mut self) {
        self.status = GoalStatus::InProgress;
        self.updated_at = Some(Utc::now());
    }

    pub fn mark_completed(&mut self) {
        self.status = GoalStatus::Completed;
        self.completion_date = Some(Utc::now());
        self.updated_at = Some(Utc::now());
    }

    pub fn completion_percentage(&self) -> f32 {
        if self.steps.is_empty() {
            return 0.0;
        }
        let completed = self.steps.iter()
            .filter(|s| s.status == GoalStatus::Completed)
            .count() as f32;
        (completed / self.steps.len() as f32) * 100.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GoalStep {
    pub step_number: u32,
    pub description: String,
    pub status: GoalStatus,
    pub due_date: Option<DateTime<Utc>>,
}

impl GoalStep {
    pub fn new(step_number: u32, description: String) -> Self {
        Self {
            step_number,
            description,
            status: GoalStatus::Planned,
            due_date: None,
        }
    }

    pub fn with_due_date(mut self, date: DateTime<Utc>) -> Self {
        self.due_date = Some(date);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Planned,
    InProgress,
    Completed,
    Blocked,
}

impl GoalStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Planned => "planned",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "in_progress" => Self::InProgress,
            "completed" => Self::Completed,
            "blocked" => Self::Blocked,
            _ => Self::Planned,
        }
    }
}

// ============ User Preference ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl UserPreference {
    pub fn new(
        user_id: String,
        preference_name: String,
        preference_value: String,
        preference_type: PreferenceType,
        scope: ContextScope,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            preference_name,
            preference_value,
            preference_type,
            scope,
            applies_to_automation: true,
            rationale: None,
            priority: 3,
            frequency_observed: 1,
            tags: Vec::new(),
            created_at: Utc::now(),
            updated_at: None,
            last_referenced: None,
        }
    }

    pub fn with_rationale(mut self, rationale: String) -> Self {
        self.rationale = Some(rationale);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn increment_frequency(&mut self) {
        self.frequency_observed += 1;
        self.last_referenced = Some(Utc::now());
        self.updated_at = Some(Utc::now());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceType {
    Tool,
    Framework,
    Constraint,
    Pattern,
    #[serde(other)]
    Other,
}

impl PreferenceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Tool => "tool",
            Self::Framework => "framework",
            Self::Constraint => "constraint",
            Self::Pattern => "pattern",
            Self::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "framework" => Self::Framework,
            "constraint" => Self::Constraint,
            "pattern" => Self::Pattern,
            "tool" => Self::Tool,
            _ => Self::Other,
        }
    }
}

// ============ Known Issue ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl KnownIssue {
    pub fn new(
        user_id: String,
        issue_description: String,
        severity: IssueSeverity,
        issue_category: IssueCategory,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            issue_description,
            symptoms: Vec::new(),
            root_cause: None,
            workaround: None,
            permanent_solution: None,
            affected_components: Vec::new(),
            severity,
            issue_category,
            learned_date: Utc::now(),
            resolution_status: ResolutionStatus::Unresolved,
            resolution_date: None,
            prevention_notes: None,
            project_contexts: Vec::new(),
        }
    }

    pub fn add_symptom(&mut self, symptom: String) {
        self.symptoms.push(symptom);
    }

    pub fn with_workaround(mut self, workaround: String) -> Self {
        self.workaround = Some(workaround);
        self
    }

    pub fn mark_resolved(&mut self, status: ResolutionStatus) {
        self.resolution_status = status;
        self.resolution_date = Some(Utc::now());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl IssueSeverity {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "high" => Self::High,
            "medium" => Self::Medium,
            "low" => Self::Low,
            _ => Self::Critical,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IssueCategory {
    Integration,
    Performance,
    Deployment,
    Data,
    Workflow,
    #[serde(other)]
    Other,
}

impl IssueCategory {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Integration => "integration",
            Self::Performance => "performance",
            Self::Deployment => "deployment",
            Self::Data => "data",
            Self::Workflow => "workflow",
            Self::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "performance" => Self::Performance,
            "deployment" => Self::Deployment,
            "data" => Self::Data,
            "workflow" => Self::Workflow,
            "integration" => Self::Integration,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    Unresolved,
    WorkaroundAvailable,
    Fixed,
    NoActionNeeded,
}

impl ResolutionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Unresolved => "unresolved",
            Self::WorkaroundAvailable => "workaround_available",
            Self::Fixed => "fixed",
            Self::NoActionNeeded => "no_action_needed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "workaround_available" => Self::WorkaroundAvailable,
            "fixed" => Self::Fixed,
            "no_action_needed" => Self::NoActionNeeded,
            _ => Self::Unresolved,
        }
    }
}

// ============ Contextual Todo ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl ContextualTodo {
    pub fn new(
        user_id: String,
        task_description: String,
        context_type: TodoContextType,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            task_description,
            context_type,
            related_entity_id: None,
            related_entity_type: None,
            project_id: None,
            assigned_to: None,
            due_date: None,
            status: TodoStatus::Pending,
            priority: 3,
            created_from_conversation_date: None,
            created_at: Utc::now(),
            updated_at: None,
            completion_date: None,
        }
    }

    pub fn mark_started(&mut self) {
        self.status = TodoStatus::InProgress;
        self.updated_at = Some(Utc::now());
    }

    pub fn mark_completed(&mut self) {
        self.status = TodoStatus::Completed;
        self.completion_date = Some(Utc::now());
        self.updated_at = Some(Utc::now());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoContextType {
    DecisionImplementation,
    GoalStep,
    IssueResolution,
    PreferenceAdoption,
    #[serde(other)]
    Other,
}

impl TodoContextType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::DecisionImplementation => "decision_implementation",
            Self::GoalStep => "goal_step",
            Self::IssueResolution => "issue_resolution",
            Self::PreferenceAdoption => "preference_adoption",
            Self::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "goal_step" => Self::GoalStep,
            "issue_resolution" => Self::IssueResolution,
            "preference_adoption" => Self::PreferenceAdoption,
            "decision_implementation" => Self::DecisionImplementation,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

impl TodoStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "in_progress" => Self::InProgress,
            "completed" => Self::Completed,
            "blocked" => Self::Blocked,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    UserDecision,
    UserGoal,
    KnownIssue,
    UserPreference,
}

impl EntityType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::UserDecision => "user_decision",
            Self::UserGoal => "user_goal",
            Self::KnownIssue => "known_issue",
            Self::UserPreference => "user_preference",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "user_goal" => Self::UserGoal,
            "known_issue" => Self::KnownIssue,
            "user_preference" => Self::UserPreference,
            _ => Self::UserDecision,
        }
    }
}

// ============ Context Scope ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ContextScope {
    Global,
    Project(String),
    Workflow(String),
}

impl ContextScope {
    pub fn to_string(&self) -> String {
        match self {
            ContextScope::Global => "global".to_string(),
            ContextScope::Project(id) => format!("project_id:{}", id),
            ContextScope::Workflow(name) => format!("workflow:{}", name),
        }
    }

    pub fn from_str(s: &str) -> Self {
        if s == "global" {
            ContextScope::Global
        } else if let Some(id) = s.strip_prefix("project_id:") {
            ContextScope::Project(id.to_string())
        } else if let Some(name) = s.strip_prefix("workflow:") {
            ContextScope::Workflow(name.to_string())
        } else {
            ContextScope::Global
        }
    }

    pub fn scope_type(&self) -> &str {
        match self {
            ContextScope::Global => "global",
            ContextScope::Project(_) => "project",
            ContextScope::Workflow(_) => "workflow",
        }
    }
}

// ============ Entity Status ============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntityStatus {
    Active,
    Archived,
    Superseded,
}

impl EntityStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Superseded => "superseded",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "archived" => Self::Archived,
            "superseded" => Self::Superseded,
            _ => Self::Active,
        }
    }
}

// ============ Audit Trail ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContextAuditEntry {
    pub id: String,
    pub user_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub action: String,           // 'create', 'update', 'delete', 'archive'
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub changed_by: String,
    pub changed_at: DateTime<Utc>,
    pub reason: Option<String>,
}

impl UserContextAuditEntry {
    pub fn create(
        user_id: String,
        entity_type: String,
        entity_id: String,
        new_value: String,
        changed_by: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            entity_type,
            entity_id,
            action: "create".to_string(),
            old_value: None,
            new_value: Some(new_value),
            changed_by,
            changed_at: Utc::now(),
            reason: None,
        }
    }
}
```

---

## Repository Implementations

### 3.1 User Decision Repository

Create `src/infrastructure/sqlite_user_decision_repository.rs`:

```rust
use async_trait::async_trait;
use chrono::Utc;
use rmcp::model::ErrorData as McpError;
use rusqlite::{params, Connection};
use serde_json::json;
use std::sync::Arc;
use crate::models::user_context::*;
use crate::repositories::UserDecisionRepository;

pub struct SqliteUserDecisionRepository {
    pool: Arc<rusqlite::Connection>,
}

impl SqliteUserDecisionRepository {
    pub fn new(pool: Arc<rusqlite::Connection>) -> Self {
        Self { pool }
    }

    fn row_to_decision(row: &rusqlite::Row) -> rusqlite::Result<UserDecision> {
        Ok(UserDecision {
            id: row.get(0)?,
            user_id: row.get(1)?,
            decision_text: row.get(2)?,
            reason: row.get(3)?,
            decision_category: DecisionCategory::from_str(&row.get::<_, String>(4)?),
            scope: ContextScope::from_str(&row.get::<_, String>(5)?),
            related_project_id: row.get(6)?,
            confidence_score: row.get(7)?,
            referenced_items: serde_json::from_str(&row.get::<_, String>(8)?)
                .unwrap_or_default(),
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: row
                .get::<_, Option<String>>(10)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            applied_count: row.get(11)?,
            last_applied: row
                .get::<_, Option<String>>(12)?
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            status: EntityStatus::from_str(&row.get::<_, String>(13)?),
        })
    }
}

#[async_trait]
impl UserDecisionRepository for SqliteUserDecisionRepository {
    async fn create_decision(&self, decision: &UserDecision) -> Result<UserDecision, McpError> {
        self.pool
            .execute(
                "INSERT INTO user_decisions (
                    id, user_id, decision_text, reason, decision_category, scope,
                    related_project_id, confidence_score, referenced_items,
                    created_at, updated_at, applied_count, last_applied, status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    &decision.id,
                    &decision.user_id,
                    &decision.decision_text,
                    &decision.reason,
                    decision.decision_category.as_str(),
                    decision.scope.to_string(),
                    &decision.related_project_id,
                    decision.confidence_score,
                    serde_json::to_string(&decision.referenced_items).unwrap(),
                    decision.created_at.to_rfc3339(),
                    decision.updated_at.map(|dt| dt.to_rfc3339()),
                    decision.applied_count,
                    decision.last_applied.map(|dt| dt.to_rfc3339()),
                    decision.status.as_str(),
                ],
            )
            .map_err(|e| McpError::internal_error(format!("Failed to create decision: {}", e), None))?;

        Ok(decision.clone())
    }

    async fn find_decision_by_id(&self, id: &str) -> Result<Option<UserDecision>, McpError> {
        let mut stmt = self
            .pool
            .prepare("SELECT * FROM user_decisions WHERE id = ?1")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let decision = stmt
            .query_row([id], |row| Self::row_to_decision(row))
            .optional()
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?;

        Ok(decision)
    }

    async fn find_decisions_by_user(&self, user_id: &str) -> Result<Vec<UserDecision>, McpError> {
        let mut stmt = self
            .pool
            .prepare("SELECT * FROM user_decisions WHERE user_id = ?1 ORDER BY created_at DESC")
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let decisions = stmt
            .query_map([user_id], |row| Self::row_to_decision(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(decisions)
    }

    async fn find_decisions_by_scope(
        &self,
        user_id: &str,
        scope: &str,
    ) -> Result<Vec<UserDecision>, McpError> {
        let mut stmt = self.pool
            .prepare(
                "SELECT * FROM user_decisions WHERE user_id = ?1 AND scope = ?2 ORDER BY created_at DESC"
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let decisions = stmt
            .query_map(params![user_id, scope], |row| Self::row_to_decision(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(decisions)
    }

    async fn find_decisions_by_category(
        &self,
        user_id: &str,
        category: &str,
    ) -> Result<Vec<UserDecision>, McpError> {
        let mut stmt = self.pool
            .prepare(
                "SELECT * FROM user_decisions WHERE user_id = ?1 AND decision_category = ?2 ORDER BY created_at DESC"
            )
            .map_err(|e| McpError::internal_error(format!("Prepare error: {}", e), None))?;

        let decisions = stmt
            .query_map(params![user_id, category], |row| Self::row_to_decision(row))
            .map_err(|e| McpError::internal_error(format!("Query error: {}", e), None))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| McpError::internal_error(format!("Collection error: {}", e), None))?;

        Ok(decisions)
    }

    async fn update_decision(&self, decision: &UserDecision) -> Result<UserDecision, McpError> {
        let updated_at = Utc::now();
        self.pool
            .execute(
                "UPDATE user_decisions SET decision_text = ?1, reason = ?2,
                decision_category = ?3, scope = ?4, confidence_score = ?5,
                updated_at = ?6, status = ?7 WHERE id = ?8",
                params![
                    &decision.decision_text,
                    &decision.reason,
                    decision.decision_category.as_str(),
                    decision.scope.to_string(),
                    decision.confidence_score,
                    updated_at.to_rfc3339(),
                    decision.status.as_str(),
                    &decision.id,
                ],
            )
            .map_err(|e| McpError::internal_error(format!("Failed to update decision: {}", e), None))?;

        Ok(decision.clone())
    }

    async fn delete_decision(&self, id: &str) -> Result<bool, McpError> {
        let rows_affected = self
            .pool
            .execute("DELETE FROM user_decisions WHERE id = ?1", [id])
            .map_err(|e| McpError::internal_error(format!("Failed to delete decision: {}", e), None))?;

        Ok(rows_affected > 0)
    }

    async fn increment_applied_count(&self, id: &str) -> Result<(), McpError> {
        self.pool
            .execute(
                "UPDATE user_decisions SET applied_count = applied_count + 1,
                last_applied = ?1 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), id],
            )
            .map_err(|e| McpError::internal_error(format!("Failed to increment count: {}", e), None))?;

        Ok(())
    }

    async fn archive_decision(&self, id: &str) -> Result<(), McpError> {
        self.pool
            .execute(
                "UPDATE user_decisions SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params!["archived", Utc::now().to_rfc3339(), id],
            )
            .map_err(|e| McpError::internal_error(format!("Failed to archive decision: {}", e), None))?;

        Ok(())
    }
}
```

### 3.2 Similar implementations for other repositories

Create repository implementations for:
- `src/infrastructure/sqlite_user_goal_repository.rs`
- `src/infrastructure/sqlite_user_preference_repository.rs`
- `src/infrastructure/sqlite_known_issue_repository.rs`
- `src/infrastructure/sqlite_contextual_todo_repository.rs`

(Follow the same pattern as above, implementing the trait methods with SQLite queries)

---

## CLI Handlers

### 4.1 Decision CLI Handler

Create `src/cli/handlers/user_context/decision_handler.rs`:

```rust
use clap::{Parser, Subcommand};
use chrono::Utc;
use std::sync::Arc;
use crate::models::user_context::*;
use crate::repositories::UserDecisionRepository;
use crate::cli::output::OutputFormatter;

#[derive(Parser)]
pub struct DecisionCommand {
    #[command(subcommand)]
    pub action: DecisionAction,
}

#[derive(Subcommand)]
pub enum DecisionAction {
    /// Create a new decision
    Create {
        /// Decision text
        #[arg(long)]
        text: String,

        /// Reason for the decision
        #[arg(long)]
        reason: Option<String>,

        /// Category: architecture, tool_choice, constraint, workflow, performance, security
        #[arg(long, default_value = "constraint")]
        category: String,

        /// Scope: global, project:<project-id>
        #[arg(long, default_value = "global")]
        scope: String,

        /// User ID
        #[arg(long, env = "CONTEXT_USER_ID")]
        user_id: String,

        /// Confidence score (0.0-1.0)
        #[arg(long, default_value = "0.5")]
        confidence: f32,
    },

    /// List decisions
    List {
        /// User ID
        #[arg(long, env = "CONTEXT_USER_ID")]
        user_id: String,

        /// Filter by scope
        #[arg(long)]
        scope: Option<String>,

        /// Filter by category
        #[arg(long)]
        category: Option<String>,

        /// Filter by status: active, archived, superseded
        #[arg(long)]
        status: Option<String>,

        /// Output format: table, json
        #[arg(long, default_value = "table")]
        format: String,
    },

    /// Show a decision
    Show {
        /// Decision ID
        id: String,

        /// Output format: table, json
        #[arg(long, default_value = "table")]
        format: String,
    },

    /// Update a decision
    Update {
        /// Decision ID
        id: String,

        /// New decision text
        #[arg(long)]
        text: Option<String>,

        /// New reason
        #[arg(long)]
        reason: Option<String>,

        /// New confidence score
        #[arg(long)]
        confidence: Option<f32>,
    },

    /// Archive a decision
    Archive {
        /// Decision ID
        id: String,
    },

    /// Delete a decision
    Delete {
        /// Decision ID
        id: String,

        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
}

pub struct DecisionHandler {
    repo: Arc<dyn UserDecisionRepository>,
}

impl DecisionHandler {
    pub fn new(repo: Arc<dyn UserDecisionRepository>) -> Self {
        Self { repo }
    }

    pub async fn handle(&self, action: DecisionAction) -> Result<String> {
        match action {
            DecisionAction::Create {
                text,
                reason,
                category,
                scope,
                user_id,
                confidence,
            } => self.create_decision(text, reason, category, scope, user_id, confidence).await,

            DecisionAction::List {
                user_id,
                scope,
                category,
                status,
                format,
            } => self.list_decisions(user_id, scope, category, status, &format).await,

            DecisionAction::Show { id, format } => self.show_decision(&id, &format).await,

            DecisionAction::Update {
                id,
                text,
                reason,
                confidence,
            } => self.update_decision(&id, text, reason, confidence).await,

            DecisionAction::Archive { id } => self.archive_decision(&id).await,

            DecisionAction::Delete { id, force } => self.delete_decision(&id, force).await,
        }
    }

    async fn create_decision(
        &self,
        text: String,
        reason: Option<String>,
        category_str: String,
        scope_str: String,
        user_id: String,
        confidence: f32,
    ) -> Result<String> {
        let category = DecisionCategory::from_str(&category_str);
        let scope = ContextScope::from_str(&scope_str);

        let mut decision = UserDecision::new(user_id, text, category, scope);
        decision.reason = reason;
        decision.confidence_score = confidence;

        let created = self.repo.create_decision(&decision).await?;

        Ok(format!(
            "✓ Decision created with ID: {}\n{}",
            created.id,
            OutputFormatter::format_decision(&created, "json")
        ))
    }

    async fn list_decisions(
        &self,
        user_id: String,
        scope: Option<String>,
        category: Option<String>,
        status: Option<String>,
        format: &str,
    ) -> Result<String> {
        let mut decisions = self.repo.find_decisions_by_user(&user_id).await?;

        // Apply filters
        if let Some(scope_filter) = scope {
            decisions.retain(|d| d.scope.to_string() == scope_filter);
        }
        if let Some(cat_filter) = category {
            decisions.retain(|d| d.decision_category.as_str() == cat_filter);
        }
        if let Some(status_filter) = status {
            decisions.retain(|d| d.status.as_str() == status_filter);
        }

        Ok(OutputFormatter::format_decisions(&decisions, format))
    }

    async fn show_decision(&self, id: &str, format: &str) -> Result<String> {
        let decision = self
            .repo
            .find_decision_by_id(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Decision not found"))?;

        Ok(OutputFormatter::format_decision(&decision, format))
    }

    async fn update_decision(
        &self,
        id: &str,
        text: Option<String>,
        reason: Option<String>,
        confidence: Option<f32>,
    ) -> Result<String> {
        let mut decision = self
            .repo
            .find_decision_by_id(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Decision not found"))?;

        if let Some(t) = text {
            decision.decision_text = t;
        }
        if let Some(r) = reason {
            decision.reason = Some(r);
        }
        if let Some(c) = confidence {
            decision.confidence_score = c;
        }
        decision.updated_at = Some(Utc::now());

        let updated = self.repo.update_decision(&decision).await?;

        Ok(format!(
            "✓ Decision updated\n{}",
            OutputFormatter::format_decision(&updated, "json")
        ))
    }

    async fn archive_decision(&self, id: &str) -> Result<String> {
        self.repo.archive_decision(id).await?;
        Ok(format!("✓ Decision archived: {}", id))
    }

    async fn delete_decision(&self, id: &str, force: bool) -> Result<String> {
        if !force {
            println!("Are you sure? (y/n)");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                return Ok("Cancelled".to_string());
            }
        }

        self.repo.delete_decision(id).await?;
        Ok(format!("✓ Decision deleted: {}", id))
    }
}

use anyhow::Result;
```

(Similar handlers for goals, preferences, issues, and todos...)

---

(Continuation follows in next sections...)

---

## Output Formatters

### 5.1 Decision Formatter

Create `src/cli/output/user_context_formatters.rs`:

```rust
use crate::models::user_context::*;
use colored::*;

pub struct UserContextFormatter;

impl UserContextFormatter {
    pub fn format_decision(decision: &UserDecision, format: &str) -> String {
        match format {
            "json" => serde_json::to_string_pretty(decision).unwrap_or_default(),
            _ => Self::format_decision_table(decision),
        }
    }

    fn format_decision_table(decision: &UserDecision) -> String {
        format!(
            "{}
ID:                  {}
User ID:             {}
Decision:            {}
Reason:              {}
Category:            {}
Scope:               {}
Confidence Score:    {:.2}%
Applied Count:       {}
Last Applied:        {}
Status:              {}
Created:             {}
Updated:             {}
            ",
            "=== DECISION ===".bold(),
            decision.id.cyan(),
            decision.user_id,
            decision.decision_text.bright_white(),
            decision.reason.as_ref().unwrap_or(&"N/A".to_string()),
            decision.decision_category.as_str().yellow(),
            decision.scope.to_string().green(),
            decision.confidence_score * 100.0,
            decision.applied_count.to_string().bright_blue(),
            decision
                .last_applied
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "Never".to_string()),
            Self::status_colored(decision.status.as_str()),
            decision.created_at.to_rfc3339(),
            decision
                .updated_at
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "N/A".to_string()),
        )
    }

    pub fn format_decisions(decisions: &[UserDecision], format: &str) -> String {
        match format {
            "json" => serde_json::to_string_pretty(decisions).unwrap_or_default(),
            _ => Self::format_decisions_table(decisions),
        }
    }

    fn format_decisions_table(decisions: &[UserDecision]) -> String {
        let mut output = String::new();
        output.push_str(&format!("{}\n", "=== DECISIONS ===".bold()));

        for decision in decisions {
            output.push_str(&format!(
                "  {} | {} | {} | Count: {}\n",
                decision.id.cyan(),
                decision.decision_category.as_str().yellow(),
                decision.decision_text.bright_white(),
                decision.applied_count.to_string().bright_blue(),
            ));
        }

        output
    }

    pub fn format_goal(goal: &UserGoal, format: &str) -> String {
        match format {
            "json" => serde_json::to_string_pretty(goal).unwrap_or_default(),
            _ => Self::format_goal_table(goal),
        }
    }

    fn format_goal_table(goal: &UserGoal) -> String {
        format!(
            "{}
ID:                  {}
Goal:                {}
Status:              {}
Priority:            {}
Completion:          {:.1}%
Steps:               {}
Target Date:         {}
Created:             {}
            ",
            "=== GOAL ===".bold(),
            goal.id.cyan(),
            goal.goal_text.bright_white(),
            Self::status_colored(goal.status.as_str()),
            goal.priority.to_string().yellow(),
            goal.completion_percentage(),
            goal.steps.len(),
            goal
                .completion_target_date
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "No deadline".to_string()),
            goal.created_at.to_rfc3339(),
        )
    }

    fn status_colored(status: &str) -> ColoredString {
        match status {
            "active" => "active".green(),
            "completed" => "completed".bright_green(),
            "in_progress" => "in_progress".bright_blue(),
            "blocked" => "blocked".red(),
            "planned" => "planned".yellow(),
            _ => status.normal(),
        }
    }
}
```

---

## Testing

### 6.1 Unit Tests

Create `tests/user_context_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use context_server_rs::models::user_context::*;
    use chrono::Utc;

    #[test]
    fn test_user_decision_creation() {
        let decision = UserDecision::new(
            "user-123".to_string(),
            "Use async/await for concurrency".to_string(),
            DecisionCategory::Architecture,
            ContextScope::Global,
        );

        assert_eq!(decision.user_id, "user-123");
        assert_eq!(decision.confidence_score, 0.5);
        assert_eq!(decision.applied_count, 0);
        assert_eq!(decision.status, EntityStatus::Active);
    }

    #[test]
    fn test_decision_increment_applied() {
        let mut decision = UserDecision::new(
            "user-123".to_string(),
            "Use async/await".to_string(),
            DecisionCategory::Architecture,
            ContextScope::Global,
        );

        decision.increment_applied_count();
        assert_eq!(decision.applied_count, 1);
        assert!(decision.last_applied.is_some());
    }

    #[test]
    fn test_goal_completion_percentage() {
        let mut goal = UserGoal::new("user-123".to_string(), "Complete project".to_string());

        let step1 = GoalStep::new(1, "Design".to_string());
        let mut step2 = GoalStep::new(2, "Implementation".to_string());
        step2.status = GoalStatus::Completed;
        let step3 = GoalStep::new(3, "Testing".to_string());

        goal.add_step(step1);
        goal.add_step(step2);
        goal.add_step(step3);

        assert_eq!(goal.completion_percentage(), 33.33..);
    }

    #[test]
    fn test_context_scope_serialization() {
        let global = ContextScope::Global;
        assert_eq!(global.to_string(), "global");

        let project = ContextScope::Project("proj-123".to_string());
        assert_eq!(project.to_string(), "project_id:proj-123");

        let workflow = ContextScope::Workflow("deployment".to_string());
        assert_eq!(workflow.to_string(), "workflow:deployment");
    }

    #[test]
    fn test_context_scope_deserialization() {
        assert_eq!(ContextScope::from_str("global"), ContextScope::Global);
        assert_eq!(
            ContextScope::from_str("project_id:proj-123"),
            ContextScope::Project("proj-123".to_string())
        );
    }

    #[test]
    fn test_issue_severity_ordering() {
        assert_eq!(IssueSeverity::Critical.as_str(), "critical");
        assert_eq!(IssueSeverity::High.as_str(), "high");
        assert_eq!(IssueSeverity::Medium.as_str(), "medium");
        assert_eq!(IssueSeverity::Low.as_str(), "low");
    }

    #[test]
    fn test_preference_frequency_increment() {
        let mut pref = UserPreference::new(
            "user-123".to_string(),
            "Use lightweight services".to_string(),
            "prefer".to_string(),
            PreferenceType::Constraint,
            ContextScope::Global,
        );

        assert_eq!(pref.frequency_observed, 1);
        pref.increment_frequency();
        assert_eq!(pref.frequency_observed, 2);
    }
}
```

---

## Configuration

### 7.1 Add to lib.rs

Update `src/lib.rs`:

```rust
pub mod models;
pub mod repositories;
pub mod infrastructure;
pub mod db;
pub mod cli;
pub mod api;
pub mod services;

// User context modules
pub mod repositories {
    pub use crate::repositories::user_context_repository::*;
}

// Add to module tree
pub mod db {
    pub mod user_context_init;
}
```

---

## Implementation Checklist - Phase 1

- [ ] Create migration SQL file
- [ ] Create database initialization code
- [ ] Create all 5 data model structs with builders
- [ ] Implement 5 repository trait files
- [ ] Implement 5 SQLite repository implementations
- [ ] Create CLI handlers for all 5 entity types
- [ ] Create output formatters
- [ ] Write comprehensive unit tests
- [ ] Integration test with real database
- [ ] Documentation for each command
- [ ] Error handling and validation
- [ ] Audit trail logging

---

## Next Steps

1. **Begin implementation** starting with database migration
2. **Test database schema** with manual SQLite queries
3. **Implement models** with full builder patterns
4. **Implement repositories** with proper error handling
5. **Create CLI commands** with comprehensive help text
6. **Add output formatters** for both table and JSON output
7. **Run tests** to verify all operations
8. **Update main CLI router** to integrate new commands

