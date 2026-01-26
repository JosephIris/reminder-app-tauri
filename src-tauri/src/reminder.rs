use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Urgency {
    Now,
    Today,
    Soon,
    Whenever,
}

impl Default for Urgency {
    fn default() -> Self {
        Urgency::Today
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ListType {
    Actual,
    Backlog,
}

impl Default for ListType {
    fn default() -> Self {
        ListType::Actual
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: i64,
    pub message: String,
    pub urgency: Urgency,
    pub list_type: ListType,
    pub created_at: String,
    pub is_completed: bool,
    pub completed_at: Option<String>,
    #[serde(default)]
    pub sort_order: i64, // Lower = higher priority (shown first in list / leftmost on bar)
}

impl Reminder {
    pub fn new(message: String, urgency: Urgency, list_type: ListType) -> Self {
        Self {
            id: 0, // Will be set by storage
            message,
            urgency,
            list_type,
            created_at: Utc::now().to_rfc3339(),
            is_completed: false,
            completed_at: None,
            sort_order: 0, // New reminders go to top (most important)
        }
    }
}
