use crate::reminder::Reminder;
use crate::urlencoding;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read as IoRead, Write as IoWrite};
use std::net::TcpListener;
use std::path::PathBuf;

const OAUTH_REDIRECT_PORT: u16 = 8085;
// Use drive scope to access all files, not just app-created ones
const OAUTH_SCOPES: &str = "https://www.googleapis.com/auth/drive";

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

        match serde_json::from_str::<ReminderStore>(&content) {
            Ok(data) => {
                eprintln!("Parsed {} pending, {} completed reminders from Drive",
                    data.pending.len(), data.completed.len());
                self.data = data;
            }
            Err(e) => {
                eprintln!("Failed to parse Drive content: {}. Content: {}", e, &content[..content.len().min(500)]);
                self.data = ReminderStore::default();
            }
        }

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

    pub fn uncomplete_reminder(&mut self, id: i64) -> Result<(), String> {
        if let Some(pos) = self.data.completed.iter().position(|r| r.id == id) {
            let mut reminder = self.data.completed.remove(pos);
            reminder.is_completed = false;
            reminder.completed_at = None;
            self.data.pending.push(reminder);
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
