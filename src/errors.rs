use std::fmt;

pub const INVALID_INPUT: &str = "invalid_input";
pub const TOO_LARGE: &str = "too_large";
pub const UNSUPPORTED_FORMAT: &str = "unsupported_format";
pub const ENCRYPTED: &str = "encrypted";
pub const PARSE_FAILED: &str = "parse_failed";
pub const INTERNAL_ERROR: &str = "internal_error";

#[derive(Debug, Clone)]
pub struct AppError {
    pub kind: &'static str,
    pub message: String,
}

impl AppError {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(INVALID_INPUT, message)
    }

    pub fn too_large(message: impl Into<String>) -> Self {
        Self::new(TOO_LARGE, message)
    }

    pub fn unsupported_format(message: impl Into<String>) -> Self {
        Self::new(UNSUPPORTED_FORMAT, message)
    }

    pub fn encrypted(message: impl Into<String>) -> Self {
        Self::new(ENCRYPTED, message)
    }

    pub fn parse_failed(message: impl Into<String>) -> Self {
        Self::new(PARSE_FAILED, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(INTERNAL_ERROR, message)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for AppError {}
