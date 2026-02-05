mod drive;
mod legacy;
mod local;
pub mod merge;
pub mod oauth;

use crate::config::MAX_ACTUAL_TASKS;
use crate::reminder::{ListType, Reminder, Urgency};
use chrono::{DateTime, Datelike, Timelike, Utc};
use merge::{merge_stores, ReminderStore};
use std::fs;
use std::path::PathBuf;

pub use oauth::OAuthCredentials;

/// Main storage struct managing both local and cloud persistence
pub struct Storage {
    data: ReminderStore,
    app_data_path: PathBuf,
    use_drive: bool,
    access_token: Option<String>,
    refresh_token: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    folder_id: Option<String>,
    file_id: Option<String>,
}

impl Storage {
    pub fn new() -> Result<Self, String> {
        let app_data_path = dirs::data_local_dir()
            .ok_or("Failed to get local data dir")?
            .join("ReminderApp");

        fs::create_dir_all(&app_data_path).map_err(|e| e.to_string())?;

        let mut storage = Self {
            data: ReminderStore::default(),
            app_data_path,
            use_drive: false,
            access_token: None,
            refresh_token: None,
            client_id: None,
            client_secret: None,
            folder_id: None,
            file_id: None,
        };

        // Try to initialize Drive storage
        if let Err(e) = storage.init_drive() {
            eprintln!("Drive initialization failed, using local storage: {}", e);
            storage.use_drive = false;
            storage.data = local::load_local(&storage.app_data_path)?;
        }

        Ok(storage)
    }

    fn init_drive(&mut self) -> Result<(), String> {
        // Load local data first so we can merge with cloud
        match local::load_local(&self.app_data_path) {
            Ok(data) => {
                self.data = data;
                eprintln!(
                    "Loaded {} local pending, {} local completed",
                    self.data.pending.len(),
                    self.data.completed.len()
                );
            }
            Err(e) => {
                eprintln!("No local data to load ({}), will use cloud data only", e);
            }
        }

        // Load OAuth state
        let oauth_state = oauth::load_oauth_state(&self.app_data_path)?;
        self.access_token = Some(oauth_state.access_token);
        self.refresh_token = oauth_state.refresh_token;
        self.client_id = oauth_state.client_id;
        self.client_secret = oauth_state.client_secret;
        self.folder_id = Some(oauth_state.folder_id);
        self.use_drive = true;

        // Find or create reminders.json in Drive
        let folder_id = self.folder_id.as_ref().ok_or("No folder ID")?;
        let access_token = self.access_token.as_ref().ok_or("No access token")?;

        match drive::find_or_create_drive_file(access_token, folder_id, &self.data) {
            Ok(file_id) => {
                self.file_id = Some(file_id);
            }
            Err(e) if e.contains("expired") => {
                eprintln!("Drive file search failed: {}, trying token refresh...", e);
                self.refresh_access_token()?;
                let access_token = self.access_token.as_ref().ok_or("No access token")?;
                self.file_id =
                    Some(drive::find_or_create_drive_file(access_token, folder_id, &self.data)?);
            }
            Err(e) => return Err(e),
        }

        // Load from Drive and merge
        if let Err(e) = self.load_from_drive() {
            eprintln!("Drive load failed: {}, trying token refresh...", e);
            self.refresh_access_token()?;
            self.load_from_drive()?;
        }

        // Push merged data back to cloud and local
        if let Err(e) = self.save_to_drive() {
            eprintln!("Warning: Failed to sync merged data to cloud: {}", e);
        }
        if let Err(e) = self.save_local() {
            eprintln!("Warning: Failed to save merged data locally: {}", e);
        }

        eprintln!(
            "Drive sync initialized successfully. Found {} pending, {} completed reminders.",
            self.data.pending.len(),
            self.data.completed.len()
        );

        Ok(())
    }

    fn refresh_access_token(&mut self) -> Result<(), String> {
        let refresh_token = self.refresh_token.as_ref().ok_or("No refresh token")?;
        let client_id = self.client_id.as_ref().ok_or("No client ID")?;
        let client_secret = self.client_secret.as_ref().ok_or("No client secret")?;

        let new_token = oauth::refresh_access_token(
            &self.app_data_path,
            refresh_token,
            client_id,
            client_secret,
        )?;

        self.access_token = Some(new_token);
        Ok(())
    }

    fn load_from_drive(&mut self) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("No access token")?;
        let file_id = self.file_id.as_ref().ok_or("No file ID")?;

        let cloud_data = drive::load_from_drive(token, file_id)?;

        // Merge cloud data with local data
        let local_count = self.data.pending.len() + self.data.completed.len();
        let cloud_count = cloud_data.pending.len() + cloud_data.completed.len();

        if local_count > 0 && cloud_count > 0 {
            eprintln!(
                "Merging {} local items with {} cloud items",
                local_count, cloud_count
            );
            self.data = merge_stores(&self.data, &cloud_data);
            eprintln!(
                "After merge: {} pending, {} completed",
                self.data.pending.len(),
                self.data.completed.len()
            );
        } else if cloud_count > 0 {
            self.data = cloud_data;
        }

        Ok(())
    }

    fn save_to_drive(&mut self) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("No access token")?.clone();
        let file_id = self.file_id.as_ref().ok_or("No file ID")?.clone();

        match drive::save_to_drive(&token, &file_id, &self.data) {
            Ok(_) => Ok(()),
            Err(e) if e.contains("expired") => {
                self.refresh_access_token()?;
                let new_token = self.access_token.as_ref().ok_or("No token after refresh")?;
                drive::save_to_drive(new_token, &file_id, &self.data)
            }
            Err(e) => Err(e),
        }
    }

    fn save_local(&self) -> Result<(), String> {
        local::save_local(&self.app_data_path, &self.data)
    }

    fn save(&mut self) -> Result<(), String> {
        self.save_local()?;

        if self.use_drive {
            if let Err(e) = self.save_to_drive() {
                eprintln!("Failed to save to Drive: {}", e);
                return Err(format!("Saved locally but cloud sync failed: {}", e));
            }
        }

        Ok(())
    }

    fn save_local_only(&mut self) -> Result<(), String> {
        self.save_local()
    }

    fn next_id(&self) -> i64 {
        let max_pending = self.data.pending.iter().map(|r| r.id).max().unwrap_or(0);
        let max_completed = self.data.completed.iter().map(|r| r.id).max().unwrap_or(0);
        max_pending.max(max_completed) + 1
    }

    // ============ Public API ============

    pub fn get_pending_reminders(&self) -> Vec<Reminder> {
        let mut reminders = self.data.pending.clone();
        reminders.sort_by(|a, b| a.sort_order.cmp(&b.sort_order));
        reminders
    }

    pub fn get_actual_reminders(&self) -> Vec<Reminder> {
        let mut reminders: Vec<Reminder> = self
            .data
            .pending
            .iter()
            .filter(|r| r.list_type == ListType::Actual)
            .cloned()
            .collect();
        reminders.sort_by(|a, b| a.sort_order.cmp(&b.sort_order));
        reminders
    }

    pub fn get_backlog_reminders(&self) -> Vec<Reminder> {
        let mut reminders: Vec<Reminder> = self
            .data
            .pending
            .iter()
            .filter(|r| r.list_type == ListType::Backlog)
            .cloned()
            .collect();
        reminders.sort_by(|a, b| a.sort_order.cmp(&b.sort_order));
        reminders
    }

    pub fn get_completed_reminders(&self) -> Vec<Reminder> {
        let mut reminders = self.data.completed.clone();
        reminders.sort_by(|a, b| {
            let a_time = a.completed_at.as_deref().unwrap_or("");
            let b_time = b.completed_at.as_deref().unwrap_or("");
            b_time.cmp(a_time)
        });
        reminders
    }

    pub fn add_reminder(&mut self, mut reminder: Reminder) -> Result<i64, String> {
        reminder.id = self.next_id();
        let id = reminder.id;

        if reminder.list_type == ListType::Actual {
            let actual_count = self
                .data
                .pending
                .iter()
                .filter(|r| r.list_type == ListType::Actual)
                .count();

            if actual_count >= MAX_ACTUAL_TASKS {
                self.bump_least_important_to_backlog();
            }

            for r in self.data.pending.iter_mut() {
                if r.list_type == ListType::Actual {
                    r.sort_order += 1;
                }
            }
            reminder.sort_order = 0;
        } else {
            let min_backlog_order = self
                .data
                .pending
                .iter()
                .filter(|r| r.list_type == ListType::Backlog)
                .map(|r| r.sort_order)
                .min()
                .unwrap_or(0);
            reminder.sort_order = min_backlog_order - 1;
        }

        self.data.pending.push(reminder);
        self.save()?;
        Ok(id)
    }

    fn bump_least_important_to_backlog(&mut self) {
        if let Some(idx) = self
            .data
            .pending
            .iter()
            .enumerate()
            .filter(|(_, r)| r.list_type == ListType::Actual)
            .max_by_key(|(_, r)| r.sort_order)
            .map(|(i, _)| i)
        {
            self.data.pending[idx].list_type = ListType::Backlog;
            let min_backlog_order = self
                .data
                .pending
                .iter()
                .filter(|r| r.list_type == ListType::Backlog)
                .map(|r| r.sort_order)
                .min()
                .unwrap_or(0);
            self.data.pending[idx].sort_order = min_backlog_order - 1;
        }
    }

    fn promote_from_backlog_if_room(&mut self) {
        let actual_count = self
            .data
            .pending
            .iter()
            .filter(|r| r.list_type == ListType::Actual)
            .count();

        if actual_count >= MAX_ACTUAL_TASKS {
            return;
        }

        let first_backlog_idx = self
            .data
            .pending
            .iter()
            .enumerate()
            .filter(|(_, r)| r.list_type == ListType::Backlog)
            .min_by_key(|(_, r)| r.sort_order)
            .map(|(i, _)| i);

        if let Some(idx) = first_backlog_idx {
            let max_actual_order = self
                .data
                .pending
                .iter()
                .filter(|r| r.list_type == ListType::Actual)
                .map(|r| r.sort_order)
                .max()
                .unwrap_or(-1);

            self.data.pending[idx].list_type = ListType::Actual;
            self.data.pending[idx].sort_order = max_actual_order + 1;
        }
    }

    pub fn update_reminder(
        &mut self,
        id: i64,
        message: String,
        urgency: Urgency,
    ) -> Result<(), String> {
        if let Some(reminder) = self.data.pending.iter_mut().find(|r| r.id == id) {
            reminder.message = message;
            reminder.urgency = urgency;
            self.save()?;
        }
        Ok(())
    }

    pub fn move_reminder(&mut self, id: i64, to_list: ListType) -> Result<(), String> {
        let current_list = self
            .data
            .pending
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.list_type.clone());

        let current_list = match current_list {
            Some(list) if list == to_list => return Ok(()),
            Some(list) => list,
            None => return Ok(()),
        };

        if to_list == ListType::Actual {
            let actual_count = self
                .data
                .pending
                .iter()
                .filter(|r| r.list_type == ListType::Actual && r.id != id)
                .count();

            if actual_count >= MAX_ACTUAL_TASKS {
                self.bump_least_important_to_backlog();
            }

            for r in self.data.pending.iter_mut() {
                if r.list_type == ListType::Actual {
                    r.sort_order += 1;
                }
            }

            if let Some(r) = self.data.pending.iter_mut().find(|r| r.id == id) {
                r.list_type = ListType::Actual;
                r.sort_order = 0;
            }
        } else {
            let min_backlog_order = self
                .data
                .pending
                .iter()
                .filter(|r| r.list_type == ListType::Backlog)
                .map(|r| r.sort_order)
                .min()
                .unwrap_or(0);

            if let Some(r) = self.data.pending.iter_mut().find(|r| r.id == id) {
                r.list_type = ListType::Backlog;
                r.sort_order = min_backlog_order - 1;
            }
        }

        self.save_local_only()?;
        Ok(())
    }

    pub fn set_urgency(&mut self, id: i64, urgency: Urgency) -> Result<(), String> {
        if let Some(reminder) = self.data.pending.iter_mut().find(|r| r.id == id) {
            reminder.urgency = urgency;
            self.save_local_only()?;
        }
        Ok(())
    }

    pub fn delete_reminder(&mut self, id: i64) -> Result<(), String> {
        let was_actual = self
            .data
            .pending
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.list_type == ListType::Actual)
            .unwrap_or(false);

        self.data.pending.retain(|r| r.id != id);
        self.data.completed.retain(|r| r.id != id);

        if was_actual {
            self.promote_from_backlog_if_room();
        }

        self.save()?;
        Ok(())
    }

    pub fn complete_reminder(&mut self, id: i64) -> Result<(), String> {
        if let Some(pos) = self.data.pending.iter().position(|r| r.id == id) {
            let was_actual = self.data.pending[pos].list_type == ListType::Actual;
            let mut reminder = self.data.pending.remove(pos);
            reminder.is_completed = true;
            reminder.completed_at = Some(Utc::now().to_rfc3339());
            self.data.completed.push(reminder);

            if was_actual {
                self.promote_from_backlog_if_room();
            }

            self.save()?;
        }
        Ok(())
    }

    pub fn uncomplete_reminder(&mut self, id: i64) -> Result<(), String> {
        if let Some(pos) = self.data.completed.iter().position(|r| r.id == id) {
            let mut reminder = self.data.completed.remove(pos);
            reminder.is_completed = false;
            reminder.completed_at = None;

            let actual_count = self
                .data
                .pending
                .iter()
                .filter(|r| r.list_type == ListType::Actual)
                .count();

            if actual_count < MAX_ACTUAL_TASKS {
                reminder.list_type = ListType::Actual;
                for r in self.data.pending.iter_mut() {
                    if r.list_type == ListType::Actual {
                        r.sort_order += 1;
                    }
                }
                reminder.sort_order = 0;
            } else {
                reminder.list_type = ListType::Backlog;
                let min_backlog = self
                    .data
                    .pending
                    .iter()
                    .filter(|r| r.list_type == ListType::Backlog)
                    .map(|r| r.sort_order)
                    .min()
                    .unwrap_or(0);
                reminder.sort_order = min_backlog - 1;
            }

            self.data.pending.push(reminder);
            self.save()?;
        }
        Ok(())
    }

    pub fn refresh_from_cloud(&mut self) -> Result<bool, String> {
        if !self.use_drive {
            return Ok(false);
        }

        if let Err(_) = self.load_from_drive() {
            self.refresh_access_token()?;
            self.load_from_drive()?;
        }

        if let Err(e) = self.save_to_drive() {
            eprintln!("Warning: Failed to sync merged data to cloud: {}", e);
        }
        self.save_local()?;

        Ok(true)
    }

    pub fn get_completion_stats(&self) -> (usize, usize) {
        let now = Utc::now();
        let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let week_start =
            today_start - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);

        let today_count = self
            .data
            .completed
            .iter()
            .filter(|r| {
                if let Some(ref completed_at) = r.completed_at {
                    if let Ok(dt) = DateTime::parse_from_rfc3339(completed_at) {
                        return dt.naive_utc() >= today_start.and_utc().naive_utc();
                    }
                }
                false
            })
            .count();

        let week_count = self
            .data
            .completed
            .iter()
            .filter(|r| {
                if let Some(ref completed_at) = r.completed_at {
                    if let Ok(dt) = DateTime::parse_from_rfc3339(completed_at) {
                        return dt.naive_utc() >= week_start.and_utc().naive_utc();
                    }
                }
                false
            })
            .count();

        (today_count, week_count)
    }

    pub fn get_historical_stats(
        &self,
    ) -> (Vec<(String, usize)>, Vec<usize>, Vec<usize>, usize) {
        let now = Utc::now();

        // Daily completions for past 14 days
        let mut daily_completions: Vec<(String, usize)> = Vec::new();
        for days_ago in (0..14).rev() {
            let date = now.date_naive() - chrono::Duration::days(days_ago);
            let date_str = date.format("%Y-%m-%d").to_string();
            let count = self
                .data
                .completed
                .iter()
                .filter(|r| {
                    if let Some(ref completed_at) = r.completed_at {
                        if let Ok(dt) = DateTime::parse_from_rfc3339(completed_at) {
                            return dt.date_naive() == date;
                        }
                    }
                    false
                })
                .count();
            daily_completions.push((date_str, count));
        }

        // Hourly distribution
        let mut hourly: Vec<usize> = vec![0; 24];
        for r in &self.data.completed {
            if let Some(ref completed_at) = r.completed_at {
                if let Ok(dt) = DateTime::parse_from_rfc3339(completed_at) {
                    hourly[dt.hour() as usize] += 1;
                }
            }
        }

        // Daily distribution (0=Monday, 6=Sunday)
        let mut daily: Vec<usize> = vec![0; 7];
        for r in &self.data.completed {
            if let Some(ref completed_at) = r.completed_at {
                if let Ok(dt) = DateTime::parse_from_rfc3339(completed_at) {
                    daily[dt.weekday().num_days_from_monday() as usize] += 1;
                }
            }
        }

        // Backlog size
        let backlog_size = self
            .data
            .pending
            .iter()
            .filter(|r| r.list_type == ListType::Backlog)
            .count();

        (daily_completions, hourly, daily, backlog_size)
    }

    pub fn reorder_reminders(&mut self, ordered_ids: Vec<i64>) -> Result<(), String> {
        for (index, id) in ordered_ids.iter().enumerate() {
            if let Some(reminder) = self.data.pending.iter_mut().find(|r| r.id == *id) {
                reminder.sort_order = index as i64;
            }
        }
        self.save_local()
    }

    pub fn sync_to_cloud(&mut self) -> Result<(), String> {
        if self.use_drive {
            self.save_to_drive()?;
        }
        Ok(())
    }

    // ============ OAuth Methods ============

    pub fn has_oauth_credentials(&self) -> bool {
        oauth::has_oauth_credentials(&self.app_data_path)
    }

    pub fn is_logged_in(&self) -> bool {
        self.use_drive && self.access_token.is_some()
    }

    pub fn get_oauth_status(&self) -> (bool, bool) {
        (self.has_oauth_credentials(), self.is_logged_in())
    }

    pub fn save_oauth_credentials(&self, credentials: &OAuthCredentials) -> Result<(), String> {
        oauth::save_oauth_credentials(&self.app_data_path, credentials)
    }

    pub fn get_oauth_credentials(&self) -> Option<OAuthCredentials> {
        oauth::load_oauth_credentials(&self.app_data_path).ok()
    }

    pub fn get_app_data_path(&self) -> &std::path::Path {
        &self.app_data_path
    }

    pub fn reload_oauth_state(&mut self) -> Result<(), String> {
        self.init_drive()
    }

    pub fn get_oauth_url(&self) -> Result<String, String> {
        oauth::get_oauth_url(&self.app_data_path)
    }

    pub fn disconnect_drive(&mut self) -> Result<(), String> {
        oauth::disconnect(&self.app_data_path)?;
        self.use_drive = false;
        self.access_token = None;
        self.refresh_token = None;
        self.file_id = None;
        Ok(())
    }
}

/// Complete the entire OAuth flow in a blocking context (for use in a separate thread)
pub fn complete_oauth_flow_blocking(app_data_path: &std::path::Path) -> Result<(), String> {
    oauth::complete_oauth_flow_blocking(app_data_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_reminder(id: i64, list_type: ListType, sort_order: i64) -> Reminder {
        Reminder {
            id,
            message: format!("Task {}", id),
            urgency: Urgency::Today,
            list_type,
            created_at: Utc::now().to_rfc3339(),
            is_completed: false,
            completed_at: None,
            sort_order,
        }
    }

    #[test]
    fn test_promote_from_backlog_when_room() {
        let mut store = ReminderStore::default();

        for i in 0..5 {
            store.pending.push(create_test_reminder(i, ListType::Actual, i));
        }

        store.pending.push(create_test_reminder(100, ListType::Backlog, 0));
        store.pending.push(create_test_reminder(101, ListType::Backlog, 1));

        let mut storage = Storage {
            data: store,
            app_data_path: PathBuf::from("/tmp/test"),
            use_drive: false,
            access_token: None,
            refresh_token: None,
            client_id: None,
            client_secret: None,
            folder_id: None,
            file_id: None,
        };

        storage.promote_from_backlog_if_room();

        let actual_count = storage.data.pending.iter()
            .filter(|r| r.list_type == ListType::Actual)
            .count();

        assert_eq!(actual_count, 6);
    }

    #[test]
    fn test_promoted_task_goes_to_end() {
        let mut store = ReminderStore::default();

        for i in 0..3 {
            store.pending.push(create_test_reminder(i, ListType::Actual, i));
        }

        store.pending.push(create_test_reminder(100, ListType::Backlog, 0));

        let mut storage = Storage {
            data: store,
            app_data_path: PathBuf::from("/tmp/test"),
            use_drive: false,
            access_token: None,
            refresh_token: None,
            client_id: None,
            client_secret: None,
            folder_id: None,
            file_id: None,
        };

        storage.promote_from_backlog_if_room();

        let promoted = storage.data.pending.iter()
            .find(|r| r.id == 100)
            .unwrap();

        assert_eq!(promoted.list_type, ListType::Actual);
        assert_eq!(promoted.sort_order, 3); // After 0, 1, 2
    }
}
