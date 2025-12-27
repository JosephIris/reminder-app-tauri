# Reminder App - Tauri Migration Progress

## Session 2 Updates (Dec 26, 2024)

### Fixed Issues:
1. **Google Drive Storage** - Added OAuth token refresh to handle expired tokens
2. **Tauri v2 Capabilities** - Added `src-tauri/capabilities/default.json` with permissions for:
   - Window operations (show, hide, minimize, drag)
   - Events (emit, listen)
   - Notifications
   - Autostart
   - Global shortcuts
3. **Notification Popups** - Added `NotificationPopup.tsx` component that shows when reminders are due
4. **Scheduler** - Added 10-second interval check for due reminders in App.tsx
5. **Launch Arguments** - Added `--show`, `--quick-add`, `--startup` support in Rust

### Remaining Items:
- Custom hotkey editing in settings (currently shows fixed Ctrl+Shift+R)
- Recurrence handling when completing reminders
- Test Drive sync thoroughly

---

## What We Did (Dec 25, 2024)

### 1. Original Python App Improvements
- Added Google Drive sync (folder ID: `1oGm0zY87yCDRIYAcoWCWXbGEiy3vY8kf`)
- Added completed reminders section (collapsible)
- Added loading animation for save operations
- Fixed quick reminder focus
- Made popups more transparent (72%)
- Fixed edit dialog height issue

### 2. Decision to Migrate to Tauri
- User felt CustomTkinter limited the UI aesthetics
- Chose Tauri over Electron for smaller app size (~10MB vs ~150MB)

### 3. Created New Tauri + React Project
Location: `C:\Users\irisy\dev-projects\reminder-app-tauri`

**Frontend (React + TypeScript + Tailwind):**
- `src/App.tsx` - Main app component
- `src/components/TitleBar.tsx` - Custom window title bar
- `src/components/ReminderInput.tsx` - Natural language input
- `src/components/ReminderItem.tsx` - Reminder card with actions
- `src/components/CompletedSection.tsx` - Collapsible completed section
- `src/components/EditDialog.tsx` - Edit reminder modal
- `src/hooks/useReminders.ts` - Reminder state management
- `src/utils/time.ts` - Natural language time parsing

**Backend (Rust):**
- `src-tauri/src/lib.rs` - Tauri commands & setup (tray, hotkeys)
- `src-tauri/src/storage.rs` - Google Drive sync (same folder ID)
- `src-tauri/src/reminder.rs` - Reminder data structure

### 4. Environment Setup Issues Encountered
- Node.js installed but not in PATH (Conda was interfering)
- Rust installed but needed PATH fix
- **Solution:** Use plain `cmd` instead of Conda terminal

### 5. Current Blocker
- Visual Studio Build Tools installed but MSVC linker (`link.exe`) not found
- "Desktop development with C++" workload is checked but may need reinstall/restart

---

## What To Do Next

### Step 1: Restart Computer
Full restart to load MSVC environment variables.

### Step 2: Open Plain CMD (not Anaconda)
Press `Win + R`, type `cmd`, press Enter.

### Step 3: Run the App
```
cd C:\Users\irisy\dev-projects\reminder-app-tauri
npm run tauri dev
```

### If Still Fails
Open Visual Studio Installer and verify these are installed:
- MSVC v143 - VS 2022 C++ x64/x86 build tools
- Windows 11 SDK (or Windows 10 SDK)

Click "Modify" to reinstall if needed, then restart again.

---

## Features Ready in New App
- Modern dark UI with Tailwind CSS
- System tray with left-click show, right-click menu
- Global hotkey (Ctrl+Shift+R) for quick add
- Natural language time parsing ("in 2 hours", "at 3pm")
- Google Drive sync (same folder as Python app)
- Completed reminders section
- Snooze/edit/delete actions
- Custom frameless window

## Files to Copy
If Drive sync needs setup, copy `token.json` from:
`%LOCALAPPDATA%\ReminderApp\token.json`
