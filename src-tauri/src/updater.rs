use serde::Serialize;
use std::env;

const REPO_OWNER: &str = "JosephIris";
const REPO_NAME: &str = "reminder-app-tauri";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub download_url: String,
}

/// Check GitHub releases for a newer version
pub fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    use self_update::backends::github::Update;

    let updater = Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("reminder-app")
        .current_version(CURRENT_VERSION)
        .build()
        .map_err(|e| format!("Failed to configure updater: {}", e))?;

    // Get the latest release info
    let latest = updater
        .get_latest_release()
        .map_err(|e| format!("Failed to fetch latest release: {}", e))?;

    let latest_version = latest.version.trim_start_matches('v');
    let current = CURRENT_VERSION.trim_start_matches('v');

    // Compare versions
    if version_is_newer(latest_version, current) {
        // Use direct browser download URL (not API URL which requires Accept header)
        let download_url = format!(
            "https://github.com/{}/{}/releases/download/{}/reminder-app.exe",
            REPO_OWNER, REPO_NAME, latest.version
        );

        Ok(Some(UpdateInfo {
            version: latest.version,
            current_version: CURRENT_VERSION.to_string(),
            download_url,
        }))
    } else {
        Ok(None)
    }
}

/// Download and install the update, replacing the current executable
pub fn install_update(download_url: &str) -> Result<(), String> {
    use std::fs;
    use std::io::Write;
    use std::process::Command;

    // Log to file for debugging
    let log_path = env::temp_dir().join("reminder-app-update.log");
    let log = |msg: &str| {
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&log_path) {
            let _ = writeln!(f, "[{}] {}", chrono::Local::now().format("%H:%M:%S"), msg);
        }
        println!("{}", msg);
    };

    // Download to temp file
    let temp_dir = env::temp_dir();
    let temp_exe = temp_dir.join("reminder-app-update.exe");

    log(&format!("Downloading update from: {}", download_url));

    // Use ureq to download (we already have it as a dependency)
    log("Starting HTTP request...");
    let response = ureq::get(download_url)
        .call()
        .map_err(|e| {
            log(&format!("Download failed: {}", e));
            format!("Failed to download update: {}", e)
        })?;

    log(&format!("Response status: {}", response.status()));

    // Handle redirects (GitHub uses them for asset downloads)
    let mut reader = response.into_reader();
    let mut bytes = Vec::new();
    log("Reading response body...");
    std::io::Read::read_to_end(&mut reader, &mut bytes)
        .map_err(|e| {
            log(&format!("Read failed: {}", e));
            format!("Failed to read update data: {}", e)
        })?;

    log(&format!("Downloaded {} bytes", bytes.len()));

    // Validate we got an actual executable (PE files start with MZ)
    if bytes.len() < 1_000_000 || !bytes.starts_with(b"MZ") {
        let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]);
        log(&format!("Invalid download - not a PE executable. Preview: {}", preview));
        return Err("Downloaded file is not a valid Windows executable".to_string());
    }

    // Write to temp file
    let mut file = fs::File::create(&temp_exe)
        .map_err(|e| {
            log(&format!("Create temp file failed: {}", e));
            format!("Failed to create temp file: {}", e)
        })?;
    file.write_all(&bytes)
        .map_err(|e| {
            log(&format!("Write failed: {}", e));
            format!("Failed to write update: {}", e)
        })?;
    drop(file);

    log(&format!("Wrote to {:?}", temp_exe));

    // Get the current executable path
    let current_exe = env::current_exe()
        .map_err(|e| format!("Failed to get current exe path: {}", e))?;

    log(&format!("Current exe: {:?}", current_exe));

    // Create a PowerShell script to replace the exe after this process exits
    let update_script = temp_dir.join("reminder-app-updater.ps1");
    let ps_log = temp_dir.join("reminder-app-updater.log");
    let script_content = format!(
        r#"
$logFile = "{}"
function Log($msg) {{ Add-Content -Path $logFile -Value "[$(Get-Date -Format 'HH:mm:ss')] $msg" }}

Log "Updater script started"
Log "Waiting for app to exit..."
Start-Sleep -Milliseconds 1000

$maxRetries = 30
$retryCount = 0
$success = $false

while ($retryCount -lt $maxRetries) {{
    try {{
        Log "Attempt $($retryCount + 1): Copying update..."
        Copy-Item -Path "{}" -Destination "{}" -Force -ErrorAction Stop
        $success = $true
        Log "Copy succeeded!"
        break
    }} catch {{
        Log "Copy failed: $_"
        $retryCount++
        Start-Sleep -Milliseconds 500
    }}
}}

if ($success) {{
    Log "Starting new version..."
    Start-Process -FilePath "{}"
    Log "App started"
}} else {{
    Log "ERROR: Failed to copy after $maxRetries attempts"
}}

# Clean up temp exe
Remove-Item -Path "{}" -Force -ErrorAction SilentlyContinue
Log "Cleanup complete"
"#,
        ps_log.display(),
        temp_exe.display(),
        current_exe.display(),
        current_exe.display(),
        temp_exe.display()
    );

    fs::write(&update_script, &script_content)
        .map_err(|e| format!("Failed to write update script: {}", e))?;

    log(&format!("Created update script: {:?}", update_script));

    // Launch the PowerShell script detached
    Command::new("powershell")
        .args([
            "-ExecutionPolicy", "Bypass",
            "-WindowStyle", "Hidden",
            "-File", &update_script.to_string_lossy(),
        ])
        .spawn()
        .map_err(|e| format!("Failed to launch update script: {}", e))?;

    log("Update script launched, app will restart shortly");
    Ok(())
}

/// Compare semver versions, returns true if `new` is newer than `current`
fn version_is_newer(new: &str, current: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<u32> = v
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect();
        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };

    let new_v = parse(new);
    let current_v = parse(current);

    new_v > current_v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(version_is_newer("1.2.0", "1.1.0"));
        assert!(version_is_newer("1.1.14", "1.1.13"));
        assert!(version_is_newer("2.0.0", "1.9.9"));
        assert!(!version_is_newer("1.1.13", "1.1.13"));
        assert!(!version_is_newer("1.1.12", "1.1.13"));
    }
}
