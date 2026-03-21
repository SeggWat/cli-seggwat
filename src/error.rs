use thiserror::Error;

/// Errors that can occur during CLI operations.
#[derive(Debug, Error)]
pub enum CliError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Authentication failed: {message}")]
    Unauthorized { message: String },

    #[error("Access denied: {message}")]
    Forbidden { message: String },

    #[error("Not found: {message}")]
    NotFound { message: String },

    #[error("Bad request: {message}")]
    BadRequest { message: String },

    #[error("Server error ({status}): {message}")]
    ServerError { status: u16, message: String },

    #[error("Failed to parse API response: {0}")]
    ParseError(String),

    #[error("Failed to build HTTP client: {0}")]
    ClientBuild(String),

    #[error("Token refresh failed: {message}")]
    TokenRefreshFailed { message: String },

    #[error("Login failed: {message}")]
    LoginFailed { message: String },

    #[error("Login timed out. Please try again.")]
    LoginTimeout,

    #[error("Token storage error: {message}")]
    TokenStorage { message: String },
}
