use crate::storage::legacy::try_migrate_legacy_data;
use crate::storage::merge::ReminderStore;
use std::fs;
use std::path::PathBuf;

/// Load reminders from local JSON file
pub fn load_local(app_data_path: &PathBuf) -> Result<ReminderStore, String> {
    let path = app_data_path.join("reminders.json");

    if !path.exists() {
        return Ok(ReminderStore::default());
    }

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    // Try to parse as new format first
    if let Ok(data) = serde_json::from_str::<ReminderStore>(&content) {
        return Ok(data);
    }

    // Try migration from legacy format
    let backup_path = app_data_path.join("reminders_backup_v1.json");
    if let Some(migrated) = try_migrate_legacy_data(&content, Some(&backup_path)) {
        // Save migrated data immediately
        save_local(app_data_path, &migrated)?;
        return Ok(migrated);
    }

    Ok(ReminderStore::default())
}

/// Save reminders to local JSON file
pub fn save_local(app_data_path: &PathBuf, data: &ReminderStore) -> Result<(), String> {
    let path = app_data_path.join("reminders.json");
    let content = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_load_nonexistent_returns_empty() {
        let temp_dir = env::temp_dir().join("test_load_nonexistent");
        let _ = fs::create_dir_all(&temp_dir);

        let result = load_local(&temp_dir);
        assert!(result.is_ok());
        let store = result.unwrap();
        assert!(store.pending.is_empty());
        assert!(store.completed.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        use crate::reminder::{ListType, Reminder, Urgency};
        use chrono::Utc;

        let temp_dir = env::temp_dir().join("test_roundtrip");
        let _ = fs::create_dir_all(&temp_dir);

        let store = ReminderStore {
            pending: vec![Reminder {
                id: 1,
                message: "Test".to_string(),
                urgency: Urgency::Today,
                list_type: ListType::Actual,
                created_at: Utc::now().to_rfc3339(),
                is_completed: false,
                completed_at: None,
                sort_order: 0,
            }],
            completed: vec![],
        };

        save_local(&temp_dir, &store).unwrap();
        let loaded = load_local(&temp_dir).unwrap();

        assert_eq!(loaded.pending.len(), 1);
        assert_eq!(loaded.pending[0].message, "Test");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
