use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: i64,
    pub message: String,
    pub due_time: String,
    pub created_at: String,
    pub recurrence: String,
    pub is_completed: bool,
    pub is_snoozed: bool,
    pub original_due_time: Option<String>,
    pub completed_at: Option<String>,
    #[serde(default)]
    pub sort_order: i64, // Lower = higher priority (shown first in list / rightmost in bar)
}

impl Reminder {
    pub fn new(message: String, due_time: String, recurrence: String) -> Self {
        Self {
            id: 0, // Will be set by storage
            message,
            due_time,
            created_at: Utc::now().to_rfc3339(),
            recurrence,
            is_completed: false,
            is_snoozed: false,
            original_due_time: None,
            completed_at: None,
            sort_order: i64::MAX, // New reminders go to end by default
        }
    }

    pub fn is_due(&self) -> bool {
        if self.is_completed {
            return false;
        }
        if let Ok(due) = DateTime::parse_from_rfc3339(&self.due_time) {
            return due <= Utc::now();
        }
        false
    }
}
