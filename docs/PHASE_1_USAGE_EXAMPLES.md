# Phase 1 User Context - Usage Examples

This document provides practical examples for using the Phase 1 User Context Layer.

---

## Quick Start

### Setting Up User Context

The user context layer provides five main entity types:
1. **Decisions** - Track architectural and technical choices
2. **Goals** - Manage objectives and milestones
3. **Preferences** - Record recurring habits and constraints
4. **Issues** - Document known problems and solutions
5. **Todos** - Create context-linked actionable tasks

---

## Entity Examples

### 1. User Decisions

#### Creating a Decision
```rust
use context_server_rs::models::user_context::*;
use chrono::Utc;
use uuid::Uuid;

let decision = UserDecision {
    id: Uuid::new_v4().to_string(),
    user_id: "alice_dev".to_string(),
    decision_text: "Always use async/await for I/O operations in Rust".to_string(),
    rationale: Some("Better performance and readability compared to callbacks".to_string()),
    decision_scope: DecisionScope::Technical,
    decision_category: DecisionCategory::Architecture,
    confidence_score: 0.95,
    tagged_items: vec![
        "async".to_string(),
        "performance".to_string(),
        "best_practice".to_string(),
    ],
    applied_count: 5,
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
```

**Use Case**: When making a significant architectural decision, record it with high confidence if you've applied it successfully multiple times.

#### Decision Scope Levels
```rust
// Technical decisions (implementation patterns)
DecisionScope::Technical  // "Use Tokio for async runtime"

// Business decisions (constraints and rules)
DecisionScope::Business   // "All user data must be encrypted at rest"

// Process decisions (workflow and conventions)
DecisionScope::ProcessRelated  // "Code reviews required before merge"
```

#### Decision Categories
```rust
DecisionCategory::Architecture  // Structural patterns: MVC, Clean Architecture
DecisionCategory::Technology    // Tool choices: Rust, PostgreSQL, React
DecisionCategory::Process       // Workflow: Git strategy, CI/CD
DecisionCategory::Pattern       // Code patterns: Observer, Factory, etc.
```

---

### 2. User Goals

#### Creating a Goal with Steps
```rust
let goal = UserGoal {
    id: Uuid::new_v4().to_string(),
    user_id: "alice_dev".to_string(),
    goal_text: "Implement authentication module".to_string(),
    project_id: Some("secure_app_v2".to_string()),
    goal_status: GoalStatus::NotStarted,
    goal_priority: GoalPriority::High,
    target_date: Some(chrono::Local::now().checked_add_days(chrono::Days::new(14)).unwrap().with_timezone(&Utc)),
    completion_percentage: 0.0,
    steps: vec![
        GoalStep {
            step_number: 1,
            description: "Design authentication flow and security model".to_string(),
            status: GoalStatus::NotStarted,
        },
        GoalStep {
            step_number: 2,
            description: "Implement JWT token generation and validation".to_string(),
            status: GoalStatus::NotStarted,
        },
        GoalStep {
            step_number: 3,
            description: "Add password hashing and verification".to_string(),
            status: GoalStatus::NotStarted,
        },
        GoalStep {
            step_number: 4,
            description: "Write integration tests for auth endpoints".to_string(),
            status: GoalStatus::NotStarted,
        },
    ],
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
```

**Use Cases**:
- Track sprint goals or milestones
- Monitor multi-step feature implementations
- Link related work in a project context
- Measure progress toward deliverables

#### Goal Status Tracking
```rust
// Track progression through lifecycle
GoalStatus::NotStarted    // Not yet begun
GoalStatus::InProgress    // Currently working on it
GoalStatus::Completed     // Successfully finished
GoalStatus::OnHold        // Temporarily paused (dependencies or blockers)
```

---

### 3. User Preferences

#### Recording Code Style Preferences
```rust
let preference = UserPreference {
    id: Uuid::new_v4().to_string(),
    user_id: "alice_dev".to_string(),
    preference_name: "Rust naming convention".to_string(),
    preference_value: "snake_case for all variables and functions".to_string(),
    applies_to_automation: true,  // OpenClaw should use this in generation
    frequency_observed: 8,         // Observed 8 times in context
    rationale: Some("Follows Rust API guidelines and team standards".to_string()),
    priority: 1,                   // Highest priority - always enforce
    last_referenced: Some(Utc::now()),
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
```

#### Recording Framework Preferences
```rust
let preference = UserPreference {
    id: Uuid::new_v4().to_string(),
    user_id: "alice_dev".to_string(),
    preference_name: "Error handling pattern".to_string(),
    preference_value: "Result<T, CustomError> with custom error types".to_string(),
    applies_to_automation: true,
    frequency_observed: 12,
    rationale: Some("Provides context-specific error information for debugging".to_string()),
    priority: 2,
    last_referenced: Some(Utc::now()),
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
```

#### Recording Constraints
```rust
let preference = UserPreference {
    id: Uuid::new_v4().to_string(),
    user_id: "alice_dev".to_string(),
    preference_name: "No synchronous I/O in async functions".to_string(),
    preference_value: "Avoid blocking_read, standard_thread operations in async context".to_string(),
    applies_to_automation: true,
    frequency_observed: 5,
    rationale: Some("Blocking calls in async context can cause executor starvation".to_string()),
    priority: 2,  // Important rule
    last_referenced: None,
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
```

**User Preference Types**:
- `applies_to_automation: true` - AI should follow this in code generation
- `priority: 1-5` - 1 is highest priority, must always follow; 5 is lowest, flexible

---

### 4. Known Issues

#### Recording a Performance Issue
```rust
let issue = KnownIssue {
    id: Uuid::new_v4().to_string(),
    user_id: "alice_dev".to_string(),
    issue_description: "Connection pool exhaustion under high load".to_string(),
    component: Some("database_connection_pool".to_string()),
    issue_category: "performance".to_string(),
    severity: IssueSeverity::High,
    symptoms: vec![
        "Server becomes unresponsive after 10 minutes of load testing".to_string(),
        "Database connections in CLOSE_WAIT state accumulate".to_string(),
        "Timeout errors from client connections".to_string(),
    ],
    workarounds: vec![
        "Increase pool size from 10 to 50 connections".to_string(),
        "Implement connection timeout of 5 seconds".to_string(),
    ],
    resolution_status: ResolutionStatus::Investigating,
    resolution_date: None,
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
```

#### Recording a Security Issue with Resolution
```rust
let issue = KnownIssue {
    id: Uuid::new_v4().to_string(),
    user_id: "alice_dev".to_string(),
    issue_description: "SQL injection vulnerability in search endpoint".to_string(),
    component: Some("api_search_endpoint".to_string()),
    issue_category: "security".to_string(),
    severity: IssueSeverity::Critical,
    symptoms: vec![
        "Unvalidated user input directly interpolated into SQL query".to_string(),
        "Database credentials potentially exposed in error messages".to_string(),
    ],
    workarounds: vec![
        "Disable search endpoint until fix deployed".to_string(),
        "Use read-only database user for API".to_string(),
    ],
    resolution_status: ResolutionStatus::Resolved,
    resolution_date: Some(Utc::now()),
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
```

**Issue Severity Levels**:
- `Critical` - System down or major security breach
- `High` - Significant functionality impaired
- `Medium` - Workaround available, needs fixing
- `Low` - Minor annoyance, can wait

**Resolution Statuses**:
- `Open` - Not yet investigated
- `Investigating` - Root cause being determined
- `Resolved` - Fixed in code
- `WorkaroundApplied` - Temporary solution in place

---

### 5. Contextual Todos

#### Creating a Code Review Task
```rust
let todo = ContextualTodo {
    id: Uuid::new_v4().to_string(),
    user_id: "alice_dev".to_string(),
    task_description: "Review authentication changes in PR #42".to_string(),
    task_context_type: TodoContextType::CodeReview,
    linked_entity_id: Some("pr_42".to_string()),
    linked_project_id: Some("secure_app_v2".to_string()),
    todo_status: TodoStatus::Pending,
    priority: 1,  // Urgent - security review
    due_date: Some(chrono::Local::now().checked_add_days(chrono::Days::new(1)).unwrap().with_timezone(&Utc)),
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
```

#### Creating a Bug Fix Task
```rust
let todo = ContextualTodo {
    id: Uuid::new_v4().to_string(),
    user_id: "alice_dev".to_string(),
    task_description: "Fix connection pool leak in production".to_string(),
    task_context_type: TodoContextType::BugFix,
    linked_entity_id: Some("issue_1023".to_string()),  // Links to known issue
    linked_project_id: Some("production_app".to_string()),
    todo_status: TodoStatus::InProgress,
    priority: 1,  // Critical - production issue
    due_date: Some(Utc::now().checked_add_signed(chrono::Duration::hours(4)).unwrap()),
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
```

#### Creating a Documentation Task
```rust
let todo = ContextualTodo {
    id: Uuid::new_v4().to_string(),
    user_id: "alice_dev".to_string(),
    task_description: "Document async/await patterns guide".to_string(),
    task_context_type: TodoContextType::Documentation,
    linked_entity_id: Some("decision_async_guide".to_string()),  // Links to decision
    linked_project_id: None,  // Team-wide knowledge
    todo_status: TodoStatus::Pending,
    priority: 3,  // Medium - not urgent
    due_date: Some(chrono::Local::now().checked_add_days(chrono::Days::new(7)).unwrap().with_timezone(&Utc)),
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
```

**Todo Context Types**:
- `CodeReview` - Review or validate code
- `BugFix` - Fix identified issue
- `ProjectPlanning` - Planning work
- `Documentation` - Write or update documentation
- `Testing` - Create or run tests

**Todo Status Lifecycle**:
- `Pending` - Not started
- `InProgress` - Currently working on it
- `Completed` - Successfully finished
- `Deferred` - Moved to later time

---

## Query Patterns

### Finding Decisions by Scope
```rust
// Get all technical decisions
decisions.iter()
    .filter(|d| d.decision_scope == DecisionScope::Technical)
    .collect::<Vec<_>>()

// Get high-confidence decisions
decisions.iter()
    .filter(|d| d.confidence_score >= 0.9)
    .collect::<Vec<_>>()
```

### Finding Goals by Status
```rust
// Get in-progress goals for a project
goals.iter()
    .filter(|g| g.project_id == Some("project_id".to_string()) && 
               g.goal_status == GoalStatus::InProgress)
    .collect::<Vec<_>>()

// Get completed goals this month
goals.iter()
    .filter(|g| g.goal_status == GoalStatus::Completed)
    .collect::<Vec<_>>()
```

### Finding Automation Preferences
```rust
// Get preferences that should guide AI automation
preferences.iter()
    .filter(|p| p.applies_to_automation)
    .sorted_by(|a, b| b.priority.cmp(&a.priority))  // Highest priority first
    .collect::<Vec<_>>()
```

### Finding Critical Issues
```rust
// Get unresolved critical issues
issues.iter()
    .filter(|i| i.severity == IssueSeverity::Critical && 
               i.resolution_status == ResolutionStatus::Open)
    .collect::<Vec<_>>()

// Get issues by component
issues.iter()
    .filter(|i| i.component == Some("database".to_string()))
    .collect::<Vec<_>>()
```

### Finding Urgent Todos
```rust
// Get high-priority pending todos
todos.iter()
    .filter(|t| t.priority <= 2 && t.todo_status == TodoStatus::Pending)
    .sorted_by(|a, b| a.priority.cmp(&b.priority))
    .collect::<Vec<_>>()
```

---

## MCP Tool Usage Examples

### Managing Decisions via MCP

```json
{
  "tool": "manage_user_decision",
  "params": {
    "action": "create",
    "user_id": "alice_dev",
    "decision_text": "Use Result<T, E> for all fallible operations",
    "decision_scope": "technical",
    "decision_category": "pattern",
    "confidence_score": 0.95
  }
}
```

### Querying User Context via MCP

```json
{
  "tool": "query_user_context",
  "params": {
    "user_id": "alice_dev",
    "context_type": "all",
    "filter": {
      "applies_to_automation": true
    },
    "limit": 50
  }
}
```

### Managing Goals via MCP

```json
{
  "tool": "manage_user_goal",
  "params": {
    "action": "update_status",
    "user_id": "alice_dev",
    "goal_id": "goal_abc123",
    "status": "in_progress",
    "completion_percentage": 35
  }
}
```

---

## Integration with OpenClaw

### Using Decisions for Validation
```
When OpenClaw generates code:
1. Query user decisions for the feature area
2. Check if generated code follows decision constraints
3. Apply workarounds for known issues in same component
4. Insert comments about relevant decisions
```

### Using Preferences for Generation
```
When generating API methods:
1. Get automation-applicable preferences (priority 1-2)
2. Apply naming conventions
3. Apply error handling patterns
4. Generate according to preferred frameworks
```

### Using Issues for Recommendations
```
When working on component with known issues:
1. Query known issues for that component
2. Suggest workarounds or permanent fixes
3. Highlight critical/high severity issues
4. Recommend issue resolution steps
```

### Using Todos for Planning
```
When analyzing project scope:
1. Check for existing todos and progress
2. Suggest completing blocking todos first
3. Break down goals into sub-todos
4. Track project-level todo completion
```

---

## Best Practices

### Recording Decisions
- ✅ **DO**: Record decisions soon after making them
- ✅ **DO**: Include rationale - why you chose this approach
- ✅ **DO**: Update confidence score as you apply it successfully
- ❌ **DON'T**: Record every small choice - focus on impactful decisions
- ❌ **DON'T**: Use confidence > 1.0 or < 0.0

### Managing Goals
- ✅ **DO**: Break large goals into meaningful steps
- ✅ **DO**: Set realistic target dates
- ✅ **DO**: Update completion percentage regularly
- ✅ **DO**: Link to related todos and decisions
- ❌ **DON'T**: Create too many goals at once (focus is better)
- ❌ **DON'T**: Leave goals in "on_hold" indefinitely

### Capturing Preferences
- ✅ **DO**: Set applies_to_automation=true for patterns AI should follow
- ✅ **DO**: Use priority 1-2 for must-follow rules
- ✅ **DO**: Record rationale - helps AI understand the "why"
- ✅ **DO**: Track frequency to identify true patterns
- ❌ **DON'T**: Record one-off exceptions as preferences

### Documenting Issues
- ✅ **DO**: Record critical and high-severity issues immediately
- ✅ **DO**: List all symptoms - helps with pattern matching
- ✅ **DO**: Provide workarounds even if not resolved
- ✅ **DO**: Update resolution status as you progress
- ❌ **DON'T**: Mix multiple issues in one record
- ❌ **DON'T**: Assume frequency - use numbers (Low/Medium/High/Critical)

### Creating Todos
- ✅ **DO**: Link todos to related entities (decisions, issues, goals)
- ✅ **DO**: Set realistic due dates
- ✅ **DO**: Use priority 1-2 for urgent/blocking work
- ✅ **DO**: Update status regularly
- ❌ **DON'T**: Create todos without context
- ❌ **DON'T**: Let todos go stale - mark as deferred or completed

---

## FAQ

**Q: How often should I update my context?**  
A: Record decisions immediately. Update goals weekly. Update issue status as you work on them. Todos when you start/complete work.

**Q: Can I archive old decisions?**  
A: Yes! Use the `archive_decision` operation to mark obsolete decisions as archived.

**Q: What's the difference between rationale and notes?**  
A: Rationale explains the WHY. Notes would be implementation details. We focus on WHY for AI understanding.

**Q: Can I link preferences to specific projects?**  
A: The current implementation is global per user. Future phases will add project-scoped preferences.

**Q: How many items should I typically have?**  
A: There's no limit, but focus quality over quantity:
- 5-15 core decisions
- 5-20 active goals
- 10-30 preferences
- 5-50 known issues
- 10-100 todos (varies by active project)

---

## Next Steps

1. **Start Recording**: Create your first decision, goal, and preference
2. **Enable Automation**: Set `applies_to_automation: true` on key preferences
3. **Track Issues**: Document known issues in your project
4. **Create Todos**: Link actionable items to relevant context
5. **Query Context**: Use MCP tools to retrieve aggregated context

