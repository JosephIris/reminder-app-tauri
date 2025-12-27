use crate::reminder::Reminder;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const FOLDER_ID: &str = "1oGm0zY87yCDRIYAcoWCWXbGEiy3vY8kf";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ReminderStore {
    pending: Vec<Reminder>,
    completed: Vec<Reminder>,
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

pub struct Storage {
    data: ReminderStore,
    app_data_path: PathBuf,
    use_drive: bool,
    access_token: Option<String>,
    refresh_token: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
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

        self.use_drive = true;

        // Find or create reminders.json in Drive
        if let Err(e) = self.find_or_create_drive_file() {
            // Token might be expired, try to refresh
            eprintln!("Drive file search failed: {}, trying token refresh...", e);
            self.refresh_access_token()?;
            self.find_or_create_drive_file()?;
        }

        // Try to load from Drive, refresh token if needed
        if let Err(e) = self.load_from_drive() {
            eprintln!("Drive load failed: {}, trying token refresh...", e);
            self.refresh_access_token()?;
            self.load_from_drive()?;
        }

        eprintln!("Drive sync initialized successfully. Found {} pending, {} completed reminders.",
            self.data.pending.len(), self.data.completed.len());

        Ok(())
    }

    fn refresh_access_token(&mut self) -> Result<(), String> {
        let refresh_token = self.refresh_token.as_ref().ok_or("No refresh token")?;
        let client_id = self.client_id.as_ref().ok_or("No client ID")?;
        let client_secret = self.client_secret.as_ref().ok_or("No client secret")?;

        let client = reqwest::blocking::Client::new();
        let params = [
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ];

        let response = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .map_err(|e| format!("Token refresh request failed: {}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(format!("Token refresh failed: {}", error_text));
        }

        let refresh_response: RefreshResponse = response
            .json()
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

        // Search for existing file
        let client = reqwest::blocking::Client::new();
        let query = format!(
            "name='reminders.json' and '{}' in parents and trashed=false",
            FOLDER_ID
        );
        let url = format!(
            "https://www.googleapis.com/drive/v3/files?q={}&fields=files(id)",
            urlencoding::encode(&query)
        );

        let response = client
            .get(&url)
            .bearer_auth(token)
            .send()
            .map_err(|e| e.to_string())?;

        // Check for auth errors
        if response.status() == 401 {
            return Err("Token expired".to_string());
        }
        if !response.status().is_success() {
            return Err(format!("Drive API error: {}", response.status()));
        }

        let json: serde_json::Value = response.json().map_err(|e| e.to_string())?;

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
        let client = reqwest::blocking::Client::new();

        let metadata = serde_json::json!({
            "name": "reminders.json",
            "parents": [FOLDER_ID],
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

        let response = client
            .post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id")
            .bearer_auth(token)
            .header(
                "Content-Type",
                format!("multipart/related; boundary={}", boundary),
            )
            .body(body)
            .send()
            .map_err(|e| e.to_string())?;

        if response.status() == 401 {
            return Err("Token expired".to_string());
        }
        if !response.status().is_success() {
            return Err(format!("Drive API error: {}", response.status()));
        }

        let json: serde_json::Value = response.json().map_err(|e| e.to_string())?;
        self.file_id = json["id"].as_str().map(String::from);

        Ok(())
    }

    fn load_from_drive(&mut self) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("No access token")?;
        let file_id = self.file_id.as_ref().ok_or("No file ID")?;

        let client = reqwest::blocking::Client::new();
        let url = format!(
            "https://www.googleapis.com/drive/v3/files/{}?alt=media",
            file_id
        );

        let response = client
            .get(&url)
            .bearer_auth(token)
            .send()
            .map_err(|e| e.to_string())?;

        if response.status() == 401 {
            return Err("Token expired".to_string());
        }
        if !response.status().is_success() {
            return Err(format!("Drive API error: {}", response.status()));
        }

        let content = response.text().map_err(|e| e.to_string())?;
        self.data = serde_json::from_str(&content).unwrap_or_default();

        Ok(())
    }

    fn save_to_drive(&mut self) -> Result<(), String> {
        let token = self.access_token.as_ref().ok_or("No access token")?.clone();
        let file_id = self.file_id.as_ref().ok_or("No file ID")?.clone();

        let client = reqwest::blocking::Client::new();
        let url = format!(
            "https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media",
            file_id
        );

        let content = serde_json::to_string_pretty(&self.data).map_err(|e| e.to_string())?;

        let response = client
            .patch(&url)
            .bearer_auth(&token)
            .header("Content-Type", "application/json")
            .body(content)
            .send()
            .map_err(|e| e.to_string())?;

        if response.status() == 401 {
            // Token expired, try to refresh and retry
            self.refresh_access_token()?;
            let new_token = self.access_token.as_ref().ok_or("No access token after refresh")?;
            let content = serde_json::to_string_pretty(&self.data).map_err(|e| e.to_string())?;
            let retry_response = client
                .patch(&url)
                .bearer_auth(new_token)
                .header("Content-Type", "application/json")
                .body(content)
                .send()
                .map_err(|e| e.to_string())?;

            if !retry_response.status().is_success() {
                return Err(format!("Drive API error after refresh: {}", retry_response.status()));
            }
        } else if !response.status().is_success() {
            return Err(format!("Drive API error: {}", response.status()));
        }

        Ok(())
    }

    fn load_local(&mut self) -> Result<(), String> {
        let path = self.app_data_path.join("reminders.json");
        if path.exists() {
            let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            self.data = serde_json::from_str(&content).unwrap_or_default();
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

        // Also save to Drive if enabled
        if self.use_drive {
            if let Err(e) = self.save_to_drive() {
                eprintln!("Failed to save to Drive: {}", e);
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
        reminders.sort_by(|a, b| a.due_time.cmp(&b.due_time));
        reminders
    }

    pub fn get_completed_reminders(&self) -> Vec<Reminder> {
        let mut reminders = self.data.completed.clone();
        reminders.sort_by(|a, b| b.due_time.cmp(&a.due_time));
        reminders
    }

    pub fn add_reminder(&mut self, mut reminder: Reminder) -> Result<i64, String> {
        reminder.id = self.next_id();
        let id = reminder.id;
        self.data.pending.push(reminder);
        self.save()?;
        Ok(id)
    }

    pub fn update_reminder(
        &mut self,
        id: i64,
        message: String,
        due_time: String,
        recurrence: String,
    ) -> Result<(), String> {
        if let Some(reminder) = self.data.pending.iter_mut().find(|r| r.id == id) {
            reminder.message = message;
            reminder.due_time = due_time;
            reminder.recurrence = recurrence;
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
            let reminder = self.data.pending.remove(pos);

            // Handle recurring reminders - create next occurrence
            if reminder.recurrence == "daily" || reminder.recurrence == "weekly" {
                if let Ok(due) = chrono::DateTime::parse_from_rfc3339(&reminder.due_time) {
                    let next_due = if reminder.recurrence == "daily" {
                        due + Duration::days(1)
                    } else {
                        due + Duration::weeks(1)
                    };

                    let mut new_reminder = Reminder::new(
                        reminder.message.clone(),
                        next_due.to_rfc3339(),
                        reminder.recurrence.clone(),
                    );
                    new_reminder.id = self.next_id();
                    self.data.pending.push(new_reminder);
                }
            }

            // Mark original as completed
            let mut completed_reminder = reminder;
            completed_reminder.is_completed = true;
            completed_reminder.completed_at = Some(Utc::now().to_rfc3339());
            self.data.completed.push(completed_reminder);
            self.save()?;
        }
        Ok(())
    }

    pub fn refresh_from_cloud(&mut self) -> Result<bool, String> {
        if !self.use_drive {
            return Ok(false);
        }

        // Try to reload from Drive
        if let Err(_) = self.load_from_drive() {
            // Token might be expired, try refresh
            self.refresh_access_token()?;
            self.load_from_drive()?;
        }

        // Also save locally as backup
        self.save_local()?;

        Ok(true)
    }

    pub fn snooze_reminder(&mut self, id: i64, minutes: i64) -> Result<(), String> {
        if let Some(reminder) = self.data.pending.iter_mut().find(|r| r.id == id) {
            if reminder.original_due_time.is_none() {
                reminder.original_due_time = Some(reminder.due_time.clone());
            }
            let new_time = Utc::now() + Duration::minutes(minutes);
            reminder.due_time = new_time.to_rfc3339();
            reminder.is_snoozed = true;
            self.save()?;
        }
        Ok(())
    }
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::new();
        for c in s.chars() {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
                ' ' => result.push_str("%20"),
                '\'' => result.push_str("%27"),
                _ => {
                    for b in c.to_string().as_bytes() {
                        result.push_str(&format!("%{:02X}", b));
                    }
                }
            }
        }
        result
    }
}
