use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
        let completed = self
            .steps
            .iter()
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
    pub action: String,
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
