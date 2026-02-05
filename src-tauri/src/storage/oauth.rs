use crate::config::{DEFAULT_DRIVE_FOLDER_ID, OAUTH_REDIRECT_PORT, OAUTH_SCOPES};
use crate::urlencoding;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::time::Duration;

/// OAuth credentials for Google Drive API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredentials {
    pub client_id: String,
    pub client_secret: String,
    #[serde(default = "default_folder_id")]
    pub folder_id: String,
}

fn default_folder_id() -> String {
    DEFAULT_DRIVE_FOLDER_ID.to_string()
}

/// Token file structure for persistence
#[derive(Debug, Deserialize)]
pub struct TokenFile {
    pub token: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    #[allow(dead_code)]
    pub token_uri: Option<String>,
}

/// Response from token refresh endpoint
#[derive(Debug, Deserialize)]
pub struct RefreshResponse {
    pub access_token: String,
}

/// Response from OAuth token exchange
#[derive(Debug, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

/// Loaded OAuth state
pub struct OAuthState {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub folder_id: String,
}

/// Load OAuth state from token.json file
pub fn load_oauth_state(app_data_path: &PathBuf) -> Result<OAuthState, String> {
    let token_path = app_data_path.join("token.json");
    if !token_path.exists() {
        return Err("No token.json found".to_string());
    }

    let token_content = fs::read_to_string(&token_path).map_err(|e| e.to_string())?;
    let token: TokenFile = serde_json::from_str(&token_content).map_err(|e| e.to_string())?;

    let access_token = token
        .token
        .or(token.access_token)
        .ok_or("No access token in token.json")?;

    // Load folder_id from credentials (with default fallback)
    let folder_id = load_oauth_credentials(app_data_path)
        .map(|c| c.folder_id)
        .unwrap_or_else(|_| default_folder_id());

    Ok(OAuthState {
        access_token,
        refresh_token: token.refresh_token,
        client_id: token.client_id,
        client_secret: token.client_secret,
        folder_id,
    })
}

/// Check if OAuth credentials are configured
pub fn has_oauth_credentials(app_data_path: &PathBuf) -> bool {
    let creds_path = app_data_path.join("oauth_credentials.json");
    creds_path.exists()
}

/// Save OAuth credentials to disk
pub fn save_oauth_credentials(
    app_data_path: &PathBuf,
    credentials: &OAuthCredentials,
) -> Result<(), String> {
    let creds_path = app_data_path.join("oauth_credentials.json");
    let content = serde_json::to_string_pretty(credentials).map_err(|e| e.to_string())?;
    fs::write(&creds_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Load OAuth credentials from disk
pub fn load_oauth_credentials(app_data_path: &PathBuf) -> Result<OAuthCredentials, String> {
    let creds_path = app_data_path.join("oauth_credentials.json");
    let content = fs::read_to_string(&creds_path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

/// Get the OAuth authorization URL
pub fn get_oauth_url(app_data_path: &PathBuf) -> Result<String, String> {
    let creds = load_oauth_credentials(app_data_path)?;
    let redirect_uri = format!("http://localhost:{}", OAUTH_REDIRECT_PORT);

    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent",
        urlencoding::encode(&creds.client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(OAUTH_SCOPES)
    );

    Ok(url)
}

/// Refresh an access token
pub fn refresh_access_token(
    app_data_path: &PathBuf,
    refresh_token: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String, String> {
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

    // Update token.json with new access token
    save_token_to_file(app_data_path, &refresh_response.access_token)?;

    eprintln!("Token refreshed successfully");
    Ok(refresh_response.access_token)
}

/// Save access token to token.json, preserving other fields
pub fn save_token_to_file(app_data_path: &PathBuf, new_token: &str) -> Result<(), String> {
    let token_path = app_data_path.join("token.json");

    // Read existing file to preserve other fields
    let token_content = fs::read_to_string(&token_path).map_err(|e| e.to_string())?;
    let mut token: serde_json::Value =
        serde_json::from_str(&token_content).map_err(|e| e.to_string())?;

    // Update the token field
    token["token"] = serde_json::Value::String(new_token.to_string());

    // Write back
    let content = serde_json::to_string_pretty(&token).map_err(|e| e.to_string())?;
    fs::write(&token_path, content).map_err(|e| e.to_string())?;

    Ok(())
}

/// Wait for OAuth callback and return the auth code
pub fn wait_for_oauth_callback() -> Result<String, String> {
    // Try to bind with retries (handles TIME_WAIT from previous connections)
    let listener = {
        let addr = format!("127.0.0.1:{}", OAUTH_REDIRECT_PORT);
        let mut attempts = 0;
        loop {
            match TcpListener::bind(&addr) {
                Ok(l) => break l,
                Err(_) if attempts < 5 => {
                    eprintln!(
                        "Port {} busy, retrying in 1s... (attempt {})",
                        OAUTH_REDIRECT_PORT,
                        attempts + 1
                    );
                    std::thread::sleep(Duration::from_secs(1));
                    attempts += 1;
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to start callback server after {} attempts: {}",
                        attempts, e
                    ))
                }
            }
        }
    };

    eprintln!(
        "Waiting for OAuth callback on port {}...",
        OAUTH_REDIRECT_PORT
    );

    // Keep accepting connections until we get one with the OAuth code
    loop {
        let (mut stream, _) = listener
            .accept()
            .map_err(|e| format!("Failed to accept connection: {}", e))?;

        let mut buffer = [0; 4096];
        let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        let request = String::from_utf8_lossy(&buffer[..n]);

        eprintln!(
            "Received request: {}",
            request.lines().next().unwrap_or("")
        );

        // Parse the code from the request
        let code = request.lines().next().and_then(|line| {
            line.split_whitespace().nth(1).and_then(|path| {
                if !path.starts_with("/?") {
                    return None;
                }
                path.split('?').nth(1).and_then(|query| {
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

/// Exchange auth code for tokens
pub fn exchange_code_for_tokens(
    app_data_path: &PathBuf,
    code: &str,
) -> Result<OAuthTokenResponse, String> {
    let creds = load_oauth_credentials(app_data_path)?;
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

    Ok(token_response)
}

/// Save OAuth tokens after successful authentication
pub fn save_oauth_tokens(
    app_data_path: &PathBuf,
    access_token: &str,
    refresh_token: Option<&str>,
) -> Result<(), String> {
    let creds = load_oauth_credentials(app_data_path)?;

    let token_data = serde_json::json!({
        "token": access_token,
        "refresh_token": refresh_token,
        "client_id": creds.client_id,
        "client_secret": creds.client_secret,
    });

    let token_path = app_data_path.join("token.json");
    let content =
        serde_json::to_string_pretty(&token_data).map_err(|e| format!("Failed to serialize token: {}", e))?;
    fs::write(&token_path, content).map_err(|e| format!("Failed to write token: {}", e))?;

    Ok(())
}

/// Remove token file (logout)
pub fn disconnect(app_data_path: &PathBuf) -> Result<(), String> {
    let token_path = app_data_path.join("token.json");
    if token_path.exists() {
        fs::remove_file(&token_path).map_err(|e| e.to_string())?;
    }
    eprintln!("Disconnected from Google Drive");
    Ok(())
}

/// Complete the entire OAuth flow in a blocking context
pub fn complete_oauth_flow_blocking(app_data_path: &std::path::Path) -> Result<(), String> {
    let code = wait_for_oauth_callback()?;
    eprintln!("Got OAuth code, exchanging for tokens...");

    let app_data_path = app_data_path.to_path_buf();
    let token_response = exchange_code_for_tokens(&app_data_path, &code)?;

    save_oauth_tokens(
        &app_data_path,
        &token_response.access_token,
        token_response.refresh_token.as_deref(),
    )?;

    eprintln!("Token saved successfully");
    Ok(())
}
