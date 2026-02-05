use crate::reminder::Reminder;
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Internal store structure for pending and completed reminders
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReminderStore {
    pub pending: Vec<Reminder>,
    pub completed: Vec<Reminder>,
}

/// Merge two ReminderStores, keeping all unique tasks and preferring newer versions for conflicts
pub fn merge_stores(local: &ReminderStore, cloud: &ReminderStore) -> ReminderStore {
    // Merge pending reminders
    let mut pending_map: HashMap<i64, Reminder> = HashMap::new();

    // Add all local pending
    for r in &local.pending {
        pending_map.insert(r.id, r.clone());
    }

    // Merge cloud pending - only add if not exists or if cloud version is newer
    for r in &cloud.pending {
        if let Some(existing) = pending_map.get(&r.id) {
            // Compare and keep newer
            let a_time = existing.completed_at.as_ref().unwrap_or(&existing.created_at);
            let b_time = r.completed_at.as_ref().unwrap_or(&r.created_at);

            if let (Ok(a_dt), Ok(b_dt)) = (
                DateTime::parse_from_rfc3339(a_time),
                DateTime::parse_from_rfc3339(b_time),
            ) {
                if b_dt > a_dt {
                    pending_map.insert(r.id, r.clone());
                }
            }
        } else {
            // New task from cloud - add it
            pending_map.insert(r.id, r.clone());
        }
    }

    // Merge completed reminders
    let mut completed_map: HashMap<i64, Reminder> = HashMap::new();

    for r in &local.completed {
        completed_map.insert(r.id, r.clone());
    }

    for r in &cloud.completed {
        if !completed_map.contains_key(&r.id) {
            completed_map.insert(r.id, r.clone());
        }
        // For completed items, also check if it exists in pending - if so, it was completed
        if pending_map.contains_key(&r.id) {
            pending_map.remove(&r.id);
            completed_map.insert(r.id, r.clone());
        }
    }

    // Also check local completed against cloud pending
    for r in &local.completed {
        if cloud.pending.iter().any(|cr| cr.id == r.id) {
            // Local has it as completed, cloud has as pending - keep as completed
            pending_map.remove(&r.id);
        }
    }

    ReminderStore {
        pending: pending_map.into_values().collect(),
        completed: completed_map.into_values().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reminder::{ListType, Urgency};
    use chrono::Utc;

    fn make_reminder(id: i64, created_at: &str) -> Reminder {
        Reminder {
            id,
            message: format!("Task {}", id),
            urgency: Urgency::Today,
            list_type: ListType::Actual,
            created_at: created_at.to_string(),
            is_completed: false,
            completed_at: None,
            sort_order: 0,
        }
    }

    #[test]
    fn test_merge_adds_new_tasks_from_cloud() {
        let local = ReminderStore {
            pending: vec![make_reminder(1, "2024-01-01T00:00:00Z")],
            completed: vec![],
        };
        let cloud = ReminderStore {
            pending: vec![
                make_reminder(1, "2024-01-01T00:00:00Z"),
                make_reminder(2, "2024-01-02T00:00:00Z"),
            ],
            completed: vec![],
        };

        let merged = merge_stores(&local, &cloud);
        assert_eq!(merged.pending.len(), 2);
    }

    #[test]
    fn test_merge_keeps_newer_version() {
        let local = ReminderStore {
            pending: vec![make_reminder(1, "2024-01-01T00:00:00Z")],
            completed: vec![],
        };
        let mut newer = make_reminder(1, "2024-01-02T00:00:00Z");
        newer.message = "Updated".to_string();
        let cloud = ReminderStore {
            pending: vec![newer],
            completed: vec![],
        };

        let merged = merge_stores(&local, &cloud);
        assert_eq!(merged.pending.len(), 1);
        assert_eq!(merged.pending[0].message, "Updated");
    }
}
