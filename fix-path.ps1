# Fix PATH for Node.js and Rust
# Run this script as Administrator

$nodePath = "C:\Program Files\nodejs"
$rustPath = "C:\Program Files\Rust stable MSVC 1.91\bin"

# Get current User PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")

# Check and add Node.js
if ($userPath -notlike "*$nodePath*") {
    $userPath = "$userPath;$nodePath"
    Write-Host "Added Node.js to PATH" -ForegroundColor Green
} else {
    Write-Host "Node.js already in PATH" -ForegroundColor Yellow
}

# Check and add Rust
if ($userPath -notlike "*$rustPath*") {
    $userPath = "$userPath;$rustPath"
    Write-Host "Added Rust to PATH" -ForegroundColor Green
} else {
    Write-Host "Rust already in PATH" -ForegroundColor Yellow
}

# Save the updated PATH
[Environment]::SetEnvironmentVariable("Path", $userPath, "User")

Write-Host ""
Write-Host "PATH updated! Close this terminal and open a new one for changes to take effect." -ForegroundColor Cyan
Write-Host ""
Write-Host "Then run:" -ForegroundColor White
Write-Host "  cd C:\Users\irisy\dev-projects\reminder-app-tauri" -ForegroundColor Gray
Write-Host "  npm install" -ForegroundColor Gray
Write-Host "  npm run tauri dev" -ForegroundColor Gray
