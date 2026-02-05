use serde::Serialize;
use std::fmt;

/// Application error types for better error handling and user feedback.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "message")]
pub enum AppError {
    /// Errors related to local file storage
    Storage(String),
    /// Errors related to Google Drive operations
    Drive(String),
    /// Errors related to OAuth authentication
    OAuth(String),
    /// Errors related to window management
    Window(String),
    /// Errors related to data validation
    Validation(String),
    /// Errors related to network operations
    Network(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Storage(msg) => write!(f, "Storage error: {}", msg),
            AppError::Drive(msg) => write!(f, "Drive error: {}", msg),
            AppError::OAuth(msg) => write!(f, "OAuth error: {}", msg),
            AppError::Window(msg) => write!(f, "Window error: {}", msg),
            AppError::Validation(msg) => write!(f, "Validation error: {}", msg),
            AppError::Network(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

// Conversion to String for Tauri command return types
impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.to_string()
    }
}

// Convenience constructors
impl AppError {
    pub fn storage<S: Into<String>>(msg: S) -> Self {
        AppError::Storage(msg.into())
    }

    pub fn drive<S: Into<String>>(msg: S) -> Self {
        AppError::Drive(msg.into())
    }

    pub fn oauth<S: Into<String>>(msg: S) -> Self {
        AppError::OAuth(msg.into())
    }

    pub fn window<S: Into<String>>(msg: S) -> Self {
        AppError::Window(msg.into())
    }

    pub fn validation<S: Into<String>>(msg: S) -> Self {
        AppError::Validation(msg.into())
    }

    pub fn network<S: Into<String>>(msg: S) -> Self {
        AppError::Network(msg.into())
    }
}

/// Result type alias for commands
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AppError::storage("file not found");
        assert_eq!(err.to_string(), "Storage error: file not found");
    }

    #[test]
    fn test_error_conversion_to_string() {
        let err = AppError::oauth("token expired");
        let s: String = err.into();
        assert!(s.contains("OAuth error"));
    }

    #[test]
    fn test_error_constructors() {
        let storage_err = AppError::storage("test");
        assert!(matches!(storage_err, AppError::Storage(_)));

        let drive_err = AppError::drive("test");
        assert!(matches!(drive_err, AppError::Drive(_)));
    }

    #[test]
    fn test_error_serialization() {
        let err = AppError::validation("invalid input");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("Validation"));
        assert!(json.contains("invalid input"));
    }
}
