use crate::reminder::{ListType, Reminder, Urgency};
use crate::urlencoding;
use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read as IoRead, Write as IoWrite};
use std::net::TcpListener;
use std::path::PathBuf;

const OAUTH_REDIRECT_PORT: u16 = 8085;
// Use drive scope to access all files, not just app-created ones
const OAUTH_SCOPES: &str = "https://www.googleapis.com/auth/drive";
const MAX_ACTUAL_TASKS: usize = 6;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ReminderStore {
    pending: Vec<Reminder>,
    completed: Vec<Reminder>,
}

/// Merge two ReminderStores, keeping all unique tasks and preferring newer versions for conflicts
fn merge_stores(local: &ReminderStore, cloud: &ReminderStore) -> ReminderStore {
    use std::collections::HashMap;

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
        if let Some(_) = cloud.pending.iter().find(|cr| cr.id == r.id) {
            // Local has it as completed, cloud has as pending - keep as completed
            pending_map.remove(&r.id);
        }
    }

    ReminderStore {
        pending: pending_map.into_values().collect(),
        completed: completed_map.into_values().collect(),
    }
}

// Legacy data structures for migration
#[derive(Debug, Clone, Deserialize)]
struct LegacyReminder {
    id: i64,
    message: String,
    due_time: String,
    created_at: String,
    #[serde(default)]
    recurrence: String,
    is_completed: bool,
    #[serde(default)]
    is_snoozed: bool,
    original_due_time: Option<String>,
    completed_at: Option<String>,
    #[serde(default)]
    sort_order: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyReminderStore {
    pending: Vec<LegacyReminder>,
    completed: Vec<LegacyReminder>,
}

#[derive(Debug, Deserialize)]
struct TokenFile {
    token: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    #[allow(dead_code)]
    token_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RefreshResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredentials {
    pub client_id: String,
    pub client_secret: String,
    #[serde(default = "default_folder_id")]
    pub folder_id: String,
}

fn default_folder_id() -> String {
    "1F0qYeAVU_7H73kX9uz-1ZF3i2KS_V-mk".to_string()
}

/// Migrate a legacy reminder to the new format
fn migrate_legacy_reminder(legacy: LegacyReminder) -> Reminder {
    // Determine urgency based on due_time
    let urgency = if let Ok(due) = DateTime::parse_from_rfc3339(&legacy.due_time) {
        let now = Utc::now();
        let due_utc = due.with_timezone(&Utc);
        let hours_until = (due_utc - now).num_hours();

        if hours_until <= 1 {
            Urgency::Now
        } else if hours_until <= 24 {
            Urgency::Today
        } else if hours_until <= 168 { // 7 days
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
fn try_migrate_legacy_data(content: &str, backup_path: Option<&PathBuf>) -> Option<ReminderStore> {
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

        eprintln!("Migrated {} pending, {} completed reminders", pending.len(), completed.len());

        return Some(ReminderStore { pending, completed });
    }

    None
}

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
            storage.load_local()?;
        }

        Ok(storage)
    }

    fn init_drive(&mut self) -> Result<(), String> {
        // IMPORTANT: Load local data first so we can merge with cloud
        // This prevents data loss if cloud has stale data
        if let Err(e) = self.load_local() {
            eprintln!("No local data to load ({}), will use cloud data only", e);
        } else {
            eprintln!("Loaded {} local pending, {} local completed",
                self.data.pending.len(), self.data.completed.len());
        }

        // Check for token.json in app data
        let token_path = self.app_data_path.join("token.json");
        if !token_path.exists() {
            return Err("No token.json found".to_string());
        }

        // Read and parse token
        let token_content = fs::read_to_string(&token_path).map_err(|e| e.to_string())?;
        let token: TokenFile = serde_json::from_str(&token_content).map_err(|e| e.to_string())?;

        self.access_token = token.token.or(token.access_token);
        self.refresh_token = token.refresh_token;
        self.client_id = token.client_id;
        self.client_secret = token.client_secret;

        if self.access_token.is_none() {
            return Err("No access token in token.json".to_string());
        }

        // Load folder_id from credentials (with default fallback)
        if let Ok(creds) = self.load_oauth_credentials() {
            self.folder_id = Some(creds.folder_id);
        } else {
            self.folder_id = Some(default_folder_id());
        }

        self.use_drive = true;

        // Find or create reminders.json in Drive
        if let Err(e) = self.find_or_create_drive_file() {
            // Token might be expired, try to refresh
            eprintln!("Drive file search failed: {}, trying token refresh...", e);
            self.refresh_access_token()?;
            self.find_or_create_drive_file()?;
        }

        // Try to load from Drive (this will merge with local data)
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

        eprintln!("Drive sync initialized successfully. Found {} pending, {} completed reminders.",
            self.data.pending.len(), self.data.completed.len());

        Ok(())
    }

    fn refresh_access_token(&mut self) -> Result<(), String> {
        let refresh_token = self.refresh_token.as_ref().ok_or("No refresh token")?;
        let client_id = self.client_id.as_ref().ok_or("No client ID")?;
        let client_secret = self.client_secret.as_ref().ok_or("No client secret")?;

        let form_body = format!(
            "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
            urlencoding::encode(client_id),
            urlencoding::encode(client_secret),
            urlencoding::encode(refresh_token)
        );

        let response = ureq::post("https://oauth2.googleapis.com/token")
            .set("Content-Type", "application/x-www-form-urlencoded")
            .send_string(&form_body)
            .map_err(|e| format!("Token refresh request failed: {}", e))?;

        let refresh_response: RefreshResponse = response
            .into_json()
            .map_err(|e| format!("Failed to parse refresh response: {}", e))?;

        self.access_token = Some(refresh_response.access_token.clone());

        // Update token.json with new access token
        self.save_token_file(&refresh_response.access_token)?;

        eprintln!("Token refreshed successfully");
        Ok(())
    }

    fn save_token_file(&self, new_token: &str) -> Result<(), String> {
        let token_path = self.app_data_path.join("token.json");

        // Read existing file to preserve other fields
        let token_content = fs::read_to_string(&token_path).map_err(|e| e.to_string())?;
        let mut token: serde_json::Value = serde_json::from_str(&token_content).map_err(|e| e.to_string())?;

        // Update the token field
        token["token"] = serde_json::Value::String(new_token.to_string());

        // Write back
        let content = serde_json::to_string_pretty(&token).map_err(|e| e.to_string())?;
        fs::write(&token_path, content).map_err(|e| e.to_string())?;

        Ok(())
    }

    fn find_or_create_drive_file(&mut self) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("No access token")?;
        let folder_id = self.folder_id.as_ref().ok_or("No folder ID configured")?;

        // Search for existing file in the specific folder
        let query = format!(
            "name='reminders.json' and '{}' in parents and trashed=false",
            folder_id
        );
        let url = format!(
            "https://www.googleapis.com/drive/v3/files?q={}&fields=files(id)",
            urlencoding::encode(&query)
        );

        eprintln!("Searching for reminders.json in folder {}...", folder_id);

        let response = ureq::get(&url)
            .set("Authorization", &format!("Bearer {}", token))
            .call();

        let response = match response {
            Ok(r) => r,
            Err(ureq::Error::Status(401, _)) => return Err("Token expired".to_string()),
            Err(ureq::Error::Status(code, _)) => return Err(format!("Drive API error: {}", code)),
            Err(e) => return Err(e.to_string()),
        };

        let json: serde_json::Value = response.into_json().map_err(|e| e.to_string())?;

        if let Some(files) = json["files"].as_array() {
            if let Some(file) = files.first() {
                self.file_id = file["id"].as_str().map(String::from);
                return Ok(());
            }
        }

        // Create new file if not found
        self.create_drive_file()
    }

    fn create_drive_file(&mut self) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("No access token")?;
        let folder_id = self.folder_id.as_ref().ok_or("No folder ID configured")?;

        let metadata = serde_json::json!({
            "name": "reminders.json",
            "parents": [folder_id],
            "mimeType": "application/json"
        });

        let content = serde_json::to_string(&self.data).map_err(|e| e.to_string())?;

        // Use multipart upload
        let boundary = "reminder_app_boundary";
        let body = format!(
            "--{}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{}\r\n--{}\r\nContent-Type: application/json\r\n\r\n{}\r\n--{}--",
            boundary,
            metadata,
            boundary,
            content,
            boundary
        );

        let response = ureq::post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id")
            .set("Authorization", &format!("Bearer {}", token))
            .set("Content-Type", &format!("multipart/related; boundary={}", boundary))
            .send_string(&body);

        let response = match response {
            Ok(r) => r,
            Err(ureq::Error::Status(401, _)) => return Err("Token expired".to_string()),
            Err(ureq::Error::Status(code, _)) => return Err(format!("Drive API error: {}", code)),
            Err(e) => return Err(e.to_string()),
        };

        let json: serde_json::Value = response.into_json().map_err(|e| e.to_string())?;
        self.file_id = json["id"].as_str().map(String::from);

        Ok(())
    }

    fn load_from_drive(&mut self) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("No access token")?;
        let file_id = self.file_id.as_ref().ok_or("No file ID")?;

        let url = format!(
            "https://www.googleapis.com/drive/v3/files/{}?alt=media",
            file_id
        );

        let response = ureq::get(&url)
            .set("Authorization", &format!("Bearer {}", token))
            .call();

        let response = match response {
            Ok(r) => r,
            Err(ureq::Error::Status(401, _)) => return Err("Token expired".to_string()),
            Err(ureq::Error::Status(code, _)) => return Err(format!("Drive API error: {}", code)),
            Err(e) => return Err(e.to_string()),
        };

        let content = response.into_string().map_err(|e| e.to_string())?;
        eprintln!("Drive content received: {} bytes", content.len());

        // Try to parse as new format first
        let cloud_data = if let Ok(data) = serde_json::from_str::<ReminderStore>(&content) {
            eprintln!("Parsed {} pending, {} completed reminders from Drive",
                data.pending.len(), data.completed.len());
            data
        } else {
            // Try migration from legacy format
            if let Some(migrated) = try_migrate_legacy_data(&content, None) {
                eprintln!("Migrated legacy data from Drive");
                migrated
            } else {
                eprintln!("Failed to parse Drive content, using empty");
                ReminderStore::default()
            }
        };

        // Merge cloud data with local data instead of overwriting
        let local_count = self.data.pending.len() + self.data.completed.len();
        let cloud_count = cloud_data.pending.len() + cloud_data.completed.len();

        if local_count > 0 && cloud_count > 0 {
            eprintln!("Merging {} local items with {} cloud items", local_count, cloud_count);
            self.data = merge_stores(&self.data, &cloud_data);
            eprintln!("After merge: {} pending, {} completed",
                self.data.pending.len(), self.data.completed.len());
        } else if cloud_count > 0 {
            // No local data, use cloud
            self.data = cloud_data;
        }
        // If only local data exists, keep it

        Ok(())
    }

    fn save_to_drive(&mut self) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("No access token")?.clone();
        let file_id = self.file_id.as_ref().ok_or("No file ID")?.clone();

        let url = format!(
            "https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media",
            file_id
        );

        let content = serde_json::to_string_pretty(&self.data).map_err(|e| e.to_string())?;

        let response = ureq::request("PATCH", &url)
            .set("Authorization", &format!("Bearer {}", token))
            .set("Content-Type", "application/json")
            .send_string(&content);

        match response {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(401, _)) => {
                // Token expired, try to refresh and retry
                self.refresh_access_token()?;
                let new_token = self.access_token.as_ref().ok_or("No access token after refresh")?;
                let content = serde_json::to_string_pretty(&self.data).map_err(|e| e.to_string())?;

                ureq::request("PATCH", &url)
                    .set("Authorization", &format!("Bearer {}", new_token))
                    .set("Content-Type", "application/json")
                    .send_string(&content)
                    .map_err(|e| format!("Drive API error after refresh: {}", e))?;
                Ok(())
            }
            Err(ureq::Error::Status(code, _)) => Err(format!("Drive API error: {}", code)),
            Err(e) => Err(e.to_string()),
        }
    }

    fn load_local(&mut self) -> Result<(), String> {
        let path = self.app_data_path.join("reminders.json");
        if path.exists() {
            let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

            // Try to parse as new format first
            if let Ok(data) = serde_json::from_str::<ReminderStore>(&content) {
                self.data = data;
            } else {
                // Try migration from legacy format
                let backup_path = self.app_data_path.join("reminders_backup_v1.json");
                if let Some(migrated) = try_migrate_legacy_data(&content, Some(&backup_path)) {
                    self.data = migrated;
                    // Save migrated data immediately
                    self.save_local()?;
                } else {
                    self.data = ReminderStore::default();
                }
            }
        }
        Ok(())
    }

    fn save_local(&self) -> Result<(), String> {
        let path = self.app_data_path.join("reminders.json");
        let content = serde_json::to_string_pretty(&self.data).map_err(|e| e.to_string())?;
        fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn save(&mut self) -> Result<(), String> {
        // Always save locally as backup
        self.save_local()?;

        // Also save to Drive if enabled - propagate error so UI can show it
        if self.use_drive {
            if let Err(e) = self.save_to_drive() {
                eprintln!("Failed to save to Drive: {}", e);
                // Return error so the UI can notify the user, but data is still saved locally
                return Err(format!("Saved locally but cloud sync failed: {}", e));
            }
        }

        Ok(())
    }

    fn next_id(&self) -> i64 {
        let max_pending = self.data.pending.iter().map(|r| r.id).max().unwrap_or(0);
        let max_completed = self.data.completed.iter().map(|r| r.id).max().unwrap_or(0);
        max_pending.max(max_completed) + 1
    }

    pub fn get_pending_reminders(&self) -> Vec<Reminder> {
        let mut reminders = self.data.pending.clone();
        // Sort by sort_order (lower = higher priority)
        reminders.sort_by(|a, b| a.sort_order.cmp(&b.sort_order));
        reminders
    }

    /// Get only actual (non-backlog) pending reminders
    pub fn get_actual_reminders(&self) -> Vec<Reminder> {
        let mut reminders: Vec<Reminder> = self.data.pending
            .iter()
            .filter(|r| r.list_type == ListType::Actual)
            .cloned()
            .collect();
        reminders.sort_by(|a, b| a.sort_order.cmp(&b.sort_order));
        reminders
    }

    /// Get only backlog pending reminders
    pub fn get_backlog_reminders(&self) -> Vec<Reminder> {
        let mut reminders: Vec<Reminder> = self.data.pending
            .iter()
            .filter(|r| r.list_type == ListType::Backlog)
            .cloned()
            .collect();
        reminders.sort_by(|a, b| a.sort_order.cmp(&b.sort_order));
        reminders
    }

    pub fn get_completed_reminders(&self) -> Vec<Reminder> {
        let mut reminders = self.data.completed.clone();
        // Sort by completion time (newest first)
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

        // If adding to actual list, handle the 6-task limit
        if reminder.list_type == ListType::Actual {
            let actual_count = self.data.pending
                .iter()
                .filter(|r| r.list_type == ListType::Actual)
                .count();

            if actual_count >= MAX_ACTUAL_TASKS {
                // Bump the least important actual task to backlog
                self.bump_least_important_to_backlog();
            }

            // Shift all actual tasks' sort_order to make room at the top
            for r in self.data.pending.iter_mut() {
                if r.list_type == ListType::Actual {
                    r.sort_order += 1;
                }
            }
            reminder.sort_order = 0; // New task at top
        } else {
            // For backlog, add to top of backlog
            let min_backlog_order = self.data.pending
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

    /// Bump the least important actual task to the top of backlog
    fn bump_least_important_to_backlog(&mut self) {
        // Find the actual task with highest sort_order (least important)
        if let Some(idx) = self.data.pending
            .iter()
            .enumerate()
            .filter(|(_, r)| r.list_type == ListType::Actual)
            .max_by_key(|(_, r)| r.sort_order)
            .map(|(i, _)| i)
        {
            // Move to backlog at top
            self.data.pending[idx].list_type = ListType::Backlog;
            let min_backlog_order = self.data.pending
                .iter()
                .filter(|r| r.list_type == ListType::Backlog)
                .map(|r| r.sort_order)
                .min()
                .unwrap_or(0);
            self.data.pending[idx].sort_order = min_backlog_order - 1;
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

    /// Move a reminder between actual and backlog lists
    pub fn move_reminder(&mut self, id: i64, to_list: ListType) -> Result<(), String> {
        // First, check if the reminder exists and get its current list type
        let current_list = self.data.pending
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.list_type.clone());

        let current_list = match current_list {
            Some(list) if list == to_list => return Ok(()), // Already in target list
            Some(list) => list,
            None => return Ok(()), // Reminder not found
        };

        if to_list == ListType::Actual {
            // Moving to actual - check limit
            let actual_count = self.data.pending
                .iter()
                .filter(|r| r.list_type == ListType::Actual && r.id != id)
                .count();

            if actual_count >= MAX_ACTUAL_TASKS {
                // Bump least important to backlog first
                self.bump_least_important_to_backlog();
            }

            // Shift all actual tasks to make room at top
            for r in self.data.pending.iter_mut() {
                if r.list_type == ListType::Actual {
                    r.sort_order += 1;
                }
            }

            // Now update our reminder
            if let Some(r) = self.data.pending.iter_mut().find(|r| r.id == id) {
                r.list_type = ListType::Actual;
                r.sort_order = 0; // Top of actual
            }
        } else {
            // Moving to backlog - calculate min order first
            let min_backlog_order = self.data.pending
                .iter()
                .filter(|r| r.list_type == ListType::Backlog)
                .map(|r| r.sort_order)
                .min()
                .unwrap_or(0);

            // Then update the reminder
            if let Some(r) = self.data.pending.iter_mut().find(|r| r.id == id) {
                r.list_type = ListType::Backlog;
                r.sort_order = min_backlog_order - 1;
            }
        }

        self.save()?;
        Ok(())
    }

    /// Update urgency of a reminder
    pub fn set_urgency(&mut self, id: i64, urgency: Urgency) -> Result<(), String> {
        if let Some(reminder) = self.data.pending.iter_mut().find(|r| r.id == id) {
            reminder.urgency = urgency;
            self.save()?;
        }
        Ok(())
    }

    pub fn delete_reminder(&mut self, id: i64) -> Result<(), String> {
        self.data.pending.retain(|r| r.id != id);
        self.data.completed.retain(|r| r.id != id);
        self.save()?;
        Ok(())
    }

    pub fn complete_reminder(&mut self, id: i64) -> Result<(), String> {
        if let Some(pos) = self.data.pending.iter().position(|r| r.id == id) {
            let mut reminder = self.data.pending.remove(pos);
            reminder.is_completed = true;
            reminder.completed_at = Some(Utc::now().to_rfc3339());
            self.data.completed.push(reminder);
            self.save()?;
        }
        Ok(())
    }

    pub fn uncomplete_reminder(&mut self, id: i64) -> Result<(), String> {
        if let Some(pos) = self.data.completed.iter().position(|r| r.id == id) {
            let mut reminder = self.data.completed.remove(pos);
            reminder.is_completed = false;
            reminder.completed_at = None;

            // Add to actual if there's room, otherwise backlog
            let actual_count = self.data.pending
                .iter()
                .filter(|r| r.list_type == ListType::Actual)
                .count();

            if actual_count < MAX_ACTUAL_TASKS {
                reminder.list_type = ListType::Actual;
                // Add to top of actual
                for r in self.data.pending.iter_mut() {
                    if r.list_type == ListType::Actual {
                        r.sort_order += 1;
                    }
                }
                reminder.sort_order = 0;
            } else {
                reminder.list_type = ListType::Backlog;
                let min_backlog = self.data.pending
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

        // Try to reload from Drive (this merges with local data)
        if let Err(_) = self.load_from_drive() {
            // Token might be expired, try refresh
            self.refresh_access_token()?;
            self.load_from_drive()?;
        }

        // Push merged data back to cloud and local
        if let Err(e) = self.save_to_drive() {
            eprintln!("Warning: Failed to sync merged data to cloud: {}", e);
        }
        self.save_local()?;

        Ok(true)
    }

    /// Get completion stats
    pub fn get_completion_stats(&self) -> (usize, usize) {
        let now = Utc::now();
        let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let week_start = today_start - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);

        let today_count = self.data.completed
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

        let week_count = self.data.completed
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

    /// Get historical stats for reports
    /// Returns: (daily_completions, hourly_distribution, daily_distribution, backlog_size)
    /// - daily_completions: Vec of (date_string, count) for past 14 days
    /// - hourly_distribution: Vec of 24 counts (0-23)
    /// - daily_distribution: Vec of 7 counts (Mon-Sun)
    /// - backlog_size: current backlog task count
    pub fn get_historical_stats(&self) -> (Vec<(String, usize)>, Vec<usize>, Vec<usize>, usize) {
        let now = Utc::now();

        // Daily completions for past 14 days
        let mut daily_completions: Vec<(String, usize)> = Vec::new();
        for days_ago in (0..14).rev() {
            let date = now.date_naive() - chrono::Duration::days(days_ago);
            let date_str = date.format("%Y-%m-%d").to_string();
            let count = self.data.completed
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

        // Hourly distribution (all time)
        let mut hourly: Vec<usize> = vec![0; 24];
        for r in &self.data.completed {
            if let Some(ref completed_at) = r.completed_at {
                if let Ok(dt) = DateTime::parse_from_rfc3339(completed_at) {
                    hourly[dt.hour() as usize] += 1;
                }
            }
        }

        // Daily distribution (all time, 0=Monday, 6=Sunday)
        let mut daily: Vec<usize> = vec![0; 7];
        for r in &self.data.completed {
            if let Some(ref completed_at) = r.completed_at {
                if let Ok(dt) = DateTime::parse_from_rfc3339(completed_at) {
                    daily[dt.weekday().num_days_from_monday() as usize] += 1;
                }
            }
        }

        // Backlog size
        let backlog_size = self.data.pending
            .iter()
            .filter(|r| r.list_type == ListType::Backlog)
            .count();

        (daily_completions, hourly, daily, backlog_size)
    }

    /// Reorder pending reminders based on the given order of IDs
    pub fn reorder_reminders(&mut self, ordered_ids: Vec<i64>) -> Result<(), String> {
        // Update sort_order based on position in ordered_ids
        for (index, id) in ordered_ids.iter().enumerate() {
            if let Some(reminder) = self.data.pending.iter_mut().find(|r| r.id == *id) {
                reminder.sort_order = index as i64;
            }
        }
        // Only save locally for instant response - cloud sync happens in background
        self.save_local()
    }

    /// Sync current state to cloud (call after local operations that should sync)
    pub fn sync_to_cloud(&mut self) -> Result<(), String> {
        if self.use_drive {
            self.save_to_drive()?;
        }
        Ok(())
    }

    /// Check if OAuth credentials are configured
    pub fn has_oauth_credentials(&self) -> bool {
        let creds_path = self.app_data_path.join("oauth_credentials.json");
        let exists = creds_path.exists();
        eprintln!("has_oauth_credentials: path={:?}, exists={}", creds_path, exists);
        exists
    }

    /// Check if we have valid tokens (user is logged in)
    pub fn is_logged_in(&self) -> bool {
        let result = self.use_drive && self.access_token.is_some();
        eprintln!("is_logged_in: use_drive={}, has_token={}, result={}",
            self.use_drive, self.access_token.is_some(), result);
        result
    }

    /// Get the OAuth status
    pub fn get_oauth_status(&self) -> (bool, bool) {
        (self.has_oauth_credentials(), self.is_logged_in())
    }

    /// Save OAuth credentials (client_id and client_secret from GCP)
    pub fn save_oauth_credentials(&self, credentials: &OAuthCredentials) -> Result<(), String> {
        let creds_path = self.app_data_path.join("oauth_credentials.json");
        let content = serde_json::to_string_pretty(credentials).map_err(|e| e.to_string())?;
        fs::write(&creds_path, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Load OAuth credentials
    fn load_oauth_credentials(&self) -> Result<OAuthCredentials, String> {
        let creds_path = self.app_data_path.join("oauth_credentials.json");
        let content = fs::read_to_string(&creds_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    /// Get OAuth credentials (public accessor)
    pub fn get_oauth_credentials(&self) -> Option<OAuthCredentials> {
        self.load_oauth_credentials().ok()
    }

    /// Get the app data path
    pub fn get_app_data_path(&self) -> &std::path::Path {
        &self.app_data_path
    }

    /// Reload OAuth state from disk (after external OAuth completion)
    pub fn reload_oauth_state(&mut self) -> Result<(), String> {
        self.init_drive()
    }

    /// Get the OAuth authorization URL to open in browser
    pub fn get_oauth_url(&self) -> Result<String, String> {
        let creds = self.load_oauth_credentials()?;
        let redirect_uri = format!("http://localhost:{}", OAUTH_REDIRECT_PORT);

        let url = format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent",
            urlencoding::encode(&creds.client_id),
            urlencoding::encode(&redirect_uri),
            urlencoding::encode(OAUTH_SCOPES)
        );

        Ok(url)
    }

    /// Start local server to receive OAuth callback and return the auth code
    pub fn wait_for_oauth_callback(&self) -> Result<String, String> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", OAUTH_REDIRECT_PORT))
            .map_err(|e| format!("Failed to start callback server: {}", e))?;

        eprintln!("Waiting for OAuth callback on port {}...", OAUTH_REDIRECT_PORT);

        // Accept one connection
        let (mut stream, _) = listener
            .accept()
            .map_err(|e| format!("Failed to accept connection: {}", e))?;

        let mut buffer = [0; 4096];
        let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        let request = String::from_utf8_lossy(&buffer[..n]);

        // Parse the code from the request
        // GET /?code=AUTH_CODE&scope=... HTTP/1.1
        let code = request
            .lines()
            .next()
            .and_then(|line| {
                line.split_whitespace()
                    .nth(1)
                    .and_then(|path| {
                        path.split('?')
                            .nth(1)
                            .and_then(|query| {
                                query.split('&').find_map(|param| {
                                    let mut parts = param.split('=');
                                    if parts.next() == Some("code") {
                                        parts.next().map(String::from)
                                    } else {
                                        None
                                    }
                                })
                            })
                    })
            })
            .ok_or("Failed to parse auth code from callback")?;

        // Send success response
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Success!</h1><p>You can close this window and return to the app.</p><script>window.close();</script></body></html>";
        stream.write_all(response.as_bytes()).ok();

        eprintln!("Received OAuth code");
        Ok(code)
    }

    /// Exchange auth code for tokens
    pub fn exchange_oauth_code(&mut self, code: &str) -> Result<(), String> {
        let creds = self.load_oauth_credentials()?;
        let redirect_uri = format!("http://localhost:{}", OAUTH_REDIRECT_PORT);

        let form_body = format!(
            "client_id={}&client_secret={}&code={}&grant_type=authorization_code&redirect_uri={}",
            urlencoding::encode(&creds.client_id),
            urlencoding::encode(&creds.client_secret),
            urlencoding::encode(code),
            urlencoding::encode(&redirect_uri)
        );

        let response = ureq::post("https://oauth2.googleapis.com/token")
            .set("Content-Type", "application/x-www-form-urlencoded")
            .send_string(&form_body)
            .map_err(|e| format!("Token exchange failed: {}", e))?;

        let token_response: OAuthTokenResponse = response
            .into_json()
            .map_err(|e| format!("Failed to parse token response: {}", e))?;

        // Save tokens
        self.access_token = Some(token_response.access_token.clone());
        self.refresh_token = token_response.refresh_token.clone();
        self.client_id = Some(creds.client_id.clone());
        self.client_secret = Some(creds.client_secret.clone());

        // Save to token.json for persistence
        let token_data = serde_json::json!({
            "token": token_response.access_token,
            "refresh_token": token_response.refresh_token,
            "client_id": creds.client_id,
            "client_secret": creds.client_secret,
        });

        let token_path = self.app_data_path.join("token.json");
        let content = serde_json::to_string_pretty(&token_data).map_err(|e| e.to_string())?;
        fs::write(&token_path, content).map_err(|e| e.to_string())?;

        // Now initialize Drive
        self.use_drive = true;
        self.find_or_create_drive_file()?;
        self.load_from_drive()?;

        eprintln!("OAuth completed successfully. Drive sync enabled.");
        Ok(())
    }

    /// Disconnect from Google Drive (logout)
    pub fn disconnect_drive(&mut self) -> Result<(), String> {
        // Remove token file
        let token_path = self.app_data_path.join("token.json");
        if token_path.exists() {
            fs::remove_file(&token_path).map_err(|e| e.to_string())?;
        }

        // Clear state
        self.use_drive = false;
        self.access_token = None;
        self.refresh_token = None;
        self.file_id = None;

        eprintln!("Disconnected from Google Drive");
        Ok(())
    }
}

/// Standalone function to wait for OAuth callback (can be called from spawn_blocking)
pub fn wait_for_oauth_callback_standalone() -> Result<String, String> {
    use std::io::{Read, Write};
    use std::time::Duration;

    // Try to bind with retries (handles TIME_WAIT from previous connections)
    let listener = {
        let addr = format!("127.0.0.1:{}", OAUTH_REDIRECT_PORT);
        let mut attempts = 0;
        loop {
            match TcpListener::bind(&addr) {
                Ok(l) => break l,
                Err(_) if attempts < 5 => {
                    eprintln!("Port {} busy, retrying in 1s... (attempt {})", OAUTH_REDIRECT_PORT, attempts + 1);
                    std::thread::sleep(Duration::from_secs(1));
                    attempts += 1;
                }
                Err(e) => return Err(format!("Failed to start callback server after {} attempts: {}", attempts, e)),
            }
        }
    };

    eprintln!("Waiting for OAuth callback on port {}...", OAUTH_REDIRECT_PORT);

    // Keep accepting connections until we get one with the OAuth code
    // (browser may send favicon.ico or other requests first)
    loop {
        let (mut stream, _) = listener
            .accept()
            .map_err(|e| format!("Failed to accept connection: {}", e))?;

        let mut buffer = [0; 4096];
        let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        let request = String::from_utf8_lossy(&buffer[..n]);

        eprintln!("Received request: {}", request.lines().next().unwrap_or(""));

        // Parse the code from the request
        // GET /?code=AUTH_CODE&scope=... HTTP/1.1
        let code = request
            .lines()
            .next()
            .and_then(|line| {
                line.split_whitespace()
                    .nth(1)
                    .and_then(|path| {
                        // Only process requests to the root path with query params
                        if !path.starts_with("/?") {
                            return None;
                        }
                        path.split('?')
                            .nth(1)
                            .and_then(|query| {
                                query.split('&').find_map(|param| {
                                    let mut parts = param.split('=');
                                    if parts.next() == Some("code") {
                                        parts.next().map(String::from)
                                    } else {
                                        None
                                    }
                                })
                            })
                    })
            });

        if let Some(code) = code {
            // Send success response
            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body><h1>Success!</h1><p>You can close this window and return to the app.</p><script>window.close();</script></body></html>";
            stream.write_all(response.as_bytes()).ok();
            eprintln!("Received OAuth code");
            return Ok(code);
        } else {
            // Send 404 for other requests (favicon.ico, etc.)
            let response = "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n";
            stream.write_all(response.as_bytes()).ok();
        }
    }
}

/// Complete the entire OAuth flow in a blocking context (for use in a separate thread)
/// This does: wait for callback -> exchange code for tokens -> save tokens
pub fn complete_oauth_flow_blocking(app_data_path: &std::path::Path) -> Result<(), String> {
    // Wait for the OAuth callback
    let code = wait_for_oauth_callback_standalone()?;
    eprintln!("Got OAuth code, exchanging for tokens...");

    // Load credentials
    let creds_path = app_data_path.join("oauth_credentials.json");
    let creds_content = fs::read_to_string(&creds_path)
        .map_err(|e| format!("Failed to read credentials: {}", e))?;
    let creds: OAuthCredentials = serde_json::from_str(&creds_content)
        .map_err(|e| format!("Failed to parse credentials: {}", e))?;

    // Exchange code for tokens
    let redirect_uri = format!("http://localhost:{}", OAUTH_REDIRECT_PORT);
    let form_body = format!(
        "client_id={}&client_secret={}&code={}&grant_type=authorization_code&redirect_uri={}",
        urlencoding::encode(&creds.client_id),
        urlencoding::encode(&creds.client_secret),
        urlencoding::encode(&code),
        urlencoding::encode(&redirect_uri)
    );

    let response = ureq::post("https://oauth2.googleapis.com/token")
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&form_body)
        .map_err(|e| format!("Token exchange failed: {}", e))?;

    let token_response: OAuthTokenResponse = response
        .into_json()
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    // Save tokens to token.json
    let token_data = serde_json::json!({
        "token": token_response.access_token,
        "refresh_token": token_response.refresh_token,
        "client_id": creds.client_id,
        "client_secret": creds.client_secret,
    });

    let token_path = app_data_path.join("token.json");
    let content = serde_json::to_string_pretty(&token_data)
        .map_err(|e| format!("Failed to serialize token: {}", e))?;
    fs::write(&token_path, content)
        .map_err(|e| format!("Failed to write token: {}", e))?;

    eprintln!("Token saved successfully");
    Ok(())
}
