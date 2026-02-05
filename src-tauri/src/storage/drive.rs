use crate::storage::legacy::try_migrate_legacy_data;
use crate::storage::merge::ReminderStore;
use crate::urlencoding;

/// Find or create reminders.json file in Google Drive
pub fn find_or_create_drive_file(
    access_token: &str,
    folder_id: &str,
    initial_data: &ReminderStore,
) -> Result<String, String> {
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
        .set("Authorization", &format!("Bearer {}", access_token))
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
            if let Some(id) = file["id"].as_str() {
                return Ok(id.to_string());
            }
        }
    }

    // Create new file if not found
    create_drive_file(access_token, folder_id, initial_data)
}

/// Create a new reminders.json file in Google Drive
fn create_drive_file(
    access_token: &str,
    folder_id: &str,
    data: &ReminderStore,
) -> Result<String, String> {
    let metadata = serde_json::json!({
        "name": "reminders.json",
        "parents": [folder_id],
        "mimeType": "application/json"
    });

    let content = serde_json::to_string(data).map_err(|e| e.to_string())?;

    // Use multipart upload
    let boundary = "reminder_app_boundary";
    let body = format!(
        "--{}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{}\r\n--{}\r\nContent-Type: application/json\r\n\r\n{}\r\n--{}--",
        boundary, metadata, boundary, content, boundary
    );

    let response =
        ureq::post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id")
            .set("Authorization", &format!("Bearer {}", access_token))
            .set(
                "Content-Type",
                &format!("multipart/related; boundary={}", boundary),
            )
            .send_string(&body);

    let response = match response {
        Ok(r) => r,
        Err(ureq::Error::Status(401, _)) => return Err("Token expired".to_string()),
        Err(ureq::Error::Status(code, _)) => return Err(format!("Drive API error: {}", code)),
        Err(e) => return Err(e.to_string()),
    };

    let json: serde_json::Value = response.into_json().map_err(|e| e.to_string())?;
    json["id"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| "No file ID in response".to_string())
}

/// Load reminders from Google Drive
pub fn load_from_drive(access_token: &str, file_id: &str) -> Result<ReminderStore, String> {
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}?alt=media",
        file_id
    );

    let response = ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", access_token))
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
    if let Ok(data) = serde_json::from_str::<ReminderStore>(&content) {
        eprintln!(
            "Parsed {} pending, {} completed reminders from Drive",
            data.pending.len(),
            data.completed.len()
        );
        return Ok(data);
    }

    // Try migration from legacy format
    if let Some(migrated) = try_migrate_legacy_data(&content, None) {
        eprintln!("Migrated legacy data from Drive");
        return Ok(migrated);
    }

    eprintln!("Failed to parse Drive content, using empty");
    Ok(ReminderStore::default())
}

/// Save reminders to Google Drive
pub fn save_to_drive(
    access_token: &str,
    file_id: &str,
    data: &ReminderStore,
) -> Result<(), String> {
    let url = format!(
        "https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media",
        file_id
    );

    let content = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;

    let response = ureq::request("PATCH", &url)
        .set("Authorization", &format!("Bearer {}", access_token))
        .set("Content-Type", "application/json")
        .send_string(&content);

    match response {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(401, _)) => Err("Token expired".to_string()),
        Err(ureq::Error::Status(code, _)) => Err(format!("Drive API error: {}", code)),
        Err(e) => Err(e.to_string()),
    }
}
