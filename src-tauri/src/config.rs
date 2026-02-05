/// Application configuration constants
///
/// Centralized configuration for the reminder app.

/// Maximum number of tasks allowed in the Actual list
pub const MAX_ACTUAL_TASKS: usize = 6;

/// OAuth redirect port for Google Drive authentication
pub const OAUTH_REDIRECT_PORT: u16 = 8085;

/// Google Drive OAuth scopes
pub const OAUTH_SCOPES: &str = "https://www.googleapis.com/auth/drive";

/// Default Google Drive folder ID for syncing reminders
pub const DEFAULT_DRIVE_FOLDER_ID: &str = "1F0qYeAVU_7H73kX9uz-1ZF3i2KS_V-mk";

/// Bar dimensions
pub const BAR_HEIGHT: i32 = 60;

/// Organization prompt trigger times (hours in 24h format)
pub const ORGANIZE_PROMPT_HOURS: [u32; 3] = [8, 13, 18];

/// Organization prompt window in minutes (triggers within first N minutes of hour)
pub const ORGANIZE_PROMPT_WINDOW_MINUTES: u32 = 5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_actual_tasks_is_reasonable() {
        assert!(MAX_ACTUAL_TASKS > 0);
        assert!(MAX_ACTUAL_TASKS <= 10);
    }

    #[test]
    fn test_oauth_port_is_valid() {
        assert!(OAUTH_REDIRECT_PORT > 1024);
        assert!(OAUTH_REDIRECT_PORT < 65535);
    }

    #[test]
    fn test_organize_prompt_hours_are_valid() {
        for hour in ORGANIZE_PROMPT_HOURS {
            assert!(hour < 24);
        }
    }
}
