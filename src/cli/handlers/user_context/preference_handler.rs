use crate::models::user_context::*;
use crate::repositories::UserPreferenceRepository;
use rmcp::model::ErrorData as McpError;
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

pub struct PreferenceHandler {
    repository: Arc<dyn UserPreferenceRepository>,
}

impl PreferenceHandler {
    pub fn new(repository: Arc<dyn UserPreferenceRepository>) -> Self {
        Self { repository }
    }

    /// Create a new user preference
    pub async fn create_preference(
        &self,
        user_id: &str,
        preference_name: &str,
        preference_value: &str,
        preference_type: &str,
        applies_to_automation: bool,
        tags: Option<Vec<String>>,
    ) -> Result<UserPreference, McpError> {
        let preference = UserPreference {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            preference_name: preference_name.to_string(),
            preference_value: preference_value.to_string(),
            preference_type: PreferenceType::from_str(preference_type),
            scope: ContextScope::Global,
            applies_to_automation,
            rationale: None,
            priority: 3,
            frequency_observed: 1,
            tags: tags.unwrap_or_default(),
            created_at: Utc::now(),
            updated_at: None,
            last_referenced: None,
        };

        self.repository.create_preference(&preference).await
    }

    /// List all preferences for a user
    pub async fn list_preferences(&self, user_id: &str) -> Result<Vec<UserPreference>, McpError> {
        self.repository.find_preferences_by_user(user_id).await
    }

    /// Get a specific preference by ID
    pub async fn show_preference(&self, id: &str) -> Result<Option<UserPreference>, McpError> {
        self.repository.find_preference_by_id(id).await
    }

    /// Update an existing preference
    pub async fn update_preference(
        &self,
        id: &str,
        preference_value: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> Result<UserPreference, McpError> {
        let mut preference = self
            .repository
            .find_preference_by_id(id)
            .await?
            .ok_or_else(|| McpError::invalid_request("Preference not found", None))?;

        if let Some(val) = preference_value {
            preference.preference_value = val.to_string();
        }
        if let Some(t) = tags {
            preference.tags = t;
        }
        preference.updated_at = Some(Utc::now());

        self.repository.update_preference(&preference).await
    }

    /// Increment frequency for a preference
    pub async fn observe_preference(&self, id: &str) -> Result<(), McpError> {
        self.repository.increment_frequency(id).await
    }

    /// Find preferences by type
    pub async fn find_by_type(
        &self,
        user_id: &str,
        pref_type: &str,
    ) -> Result<Vec<UserPreference>, McpError> {
        self.repository
            .find_preferences_by_type(user_id, pref_type)
            .await
    }

    /// Find automation-applicable preferences
    pub async fn find_automation_preferences(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserPreference>, McpError> {
        self.repository
            .find_automation_applicable_preferences(user_id)
            .await
    }

    /// Delete a preference
    pub async fn delete_preference(&self, id: &str) -> Result<bool, McpError> {
        self.repository.delete_preference(id).await
    }
}
