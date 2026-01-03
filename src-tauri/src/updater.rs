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
        // Find the Windows exe asset
        let asset_name = format!("reminder-app_{}_x64.exe", latest.version);
        let download_url = latest
            .assets
            .iter()
            .find(|a| a.name == asset_name || a.name == "reminder-app.exe")
            .map(|a| a.download_url.clone())
            .unwrap_or_else(|| {
                format!(
                    "https://github.com/{}/{}/releases/download/{}/reminder-app.exe",
                    REPO_OWNER, REPO_NAME, latest.version
                )
            });

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
    let script_content = format!(
        r#"
# Wait for the old process to exit
Start-Sleep -Milliseconds 500
$maxRetries = 20
$retryCount = 0
while ($retryCount -lt $maxRetries) {{
    try {{
        # Try to replace the executable
        Copy-Item -Path "{}" -Destination "{}" -Force -ErrorAction Stop
        break
    }} catch {{
        $retryCount++
        Start-Sleep -Milliseconds 250
    }}
}}
# Start the new version
Start-Process -FilePath "{}"
# Clean up
Remove-Item -Path "{}" -Force -ErrorAction SilentlyContinue
Remove-Item -Path $MyInvocation.MyCommand.Path -Force -ErrorAction SilentlyContinue
"#,
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
