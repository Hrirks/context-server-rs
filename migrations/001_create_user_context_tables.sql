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
