use crate::reminder::{ListType, Reminder, Urgency};
use crate::storage::merge::ReminderStore;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// Legacy reminder structure for migration from older app versions
#[derive(Debug, Clone, Deserialize)]
pub struct LegacyReminder {
    pub id: i64,
    pub message: String,
    pub due_time: String,
    pub created_at: String,
    #[serde(default)]
    pub recurrence: String,
    pub is_completed: bool,
    #[serde(default)]
    pub is_snoozed: bool,
    pub original_due_time: Option<String>,
    pub completed_at: Option<String>,
    #[serde(default)]
    pub sort_order: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LegacyReminderStore {
    pub pending: Vec<LegacyReminder>,
    pub completed: Vec<LegacyReminder>,
}

/// Migrate a legacy reminder to the new format
pub fn migrate_legacy_reminder(legacy: LegacyReminder) -> Reminder {
    // Determine urgency based on due_time
    let urgency = if let Ok(due) = DateTime::parse_from_rfc3339(&legacy.due_time) {
        let now = Utc::now();
        let due_utc = due.with_timezone(&Utc);
        let hours_until = (due_utc - now).num_hours();

        if hours_until <= 1 {
            Urgency::Now
        } else if hours_until <= 24 {
            Urgency::Today
        } else if hours_until <= 168 {
            // 7 days
            Urgency::Soon
        } else {
            Urgency::Whenever
        }
    } else {
        Urgency::Whenever
    };

    Reminder {
        id: legacy.id,
        message: legacy.message,
        urgency,
        list_type: ListType::Actual, // All migrated tasks go to actual
        created_at: legacy.created_at,
        is_completed: legacy.is_completed,
        completed_at: legacy.completed_at,
        sort_order: legacy.sort_order,
    }
}

/// Try to parse content as legacy format and migrate if needed
pub fn try_migrate_legacy_data(content: &str, backup_path: Option<&PathBuf>) -> Option<ReminderStore> {
    // Try to parse as legacy format
    if let Ok(legacy_store) = serde_json::from_str::<LegacyReminderStore>(content) {
        // Check if it's actually legacy format (has due_time field)
        // If the new format parses successfully, don't migrate
        if serde_json::from_str::<ReminderStore>(content).is_ok() {
            return None;
        }

        eprintln!("Detected legacy data format, migrating...");

        // Create backup if path provided
        if let Some(backup) = backup_path {
            if let Err(e) = fs::write(backup, content) {
                eprintln!("Warning: Failed to create backup: {}", e);
            } else {
                eprintln!("Created backup at {:?}", backup);
            }
        }

        // Migrate reminders
        let pending: Vec<Reminder> = legacy_store
            .pending
            .into_iter()
            .map(migrate_legacy_reminder)
            .collect();

        let completed: Vec<Reminder> = legacy_store
            .completed
            .into_iter()
            .map(migrate_legacy_reminder)
            .collect();

        eprintln!(
            "Migrated {} pending, {} completed reminders",
            pending.len(),
            completed.len()
        );

        return Some(ReminderStore { pending, completed });
    }

    None
}
