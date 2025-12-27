# Reminder App Tauri Setup Script
# Run this from PowerShell in the reminder-app-tauri directory

Write-Host "Installing npm dependencies..." -ForegroundColor Cyan
npm install

if ($LASTEXITCODE -ne 0) {
    Write-Host "npm install failed. Make sure Node.js is in your PATH." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Setup complete!" -ForegroundColor Green
Write-Host ""
Write-Host "To run the app in development mode:" -ForegroundColor Yellow
Write-Host "  npm run tauri dev"
Write-Host ""
Write-Host "To build for production:" -ForegroundColor Yellow
Write-Host "  npm run tauri build"
Write-Host ""
