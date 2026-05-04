use crate::CtfEmbed;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CtfError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Rate limited, retry after {retry_after:?}")]
    RateLimit {
        retry_after: Option<std::time::Duration>,
    },

    #[error("Request timed out")]
    Timeout,

    #[error("External API error {status}: {message}")]
    ExternalApi { status: u16, message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl CtfError {
    pub fn to_embed(&self) -> CtfEmbed {
        match self {
            CtfError::NotFound(m) => CtfEmbed::error("Not Found").description(m),
            CtfError::PermissionDenied(m) => CtfEmbed::error("Permission Denied").description(m),
            CtfError::InvalidInput(m) => CtfEmbed::error("Invalid Input").description(m),
            CtfError::Database(_) => CtfEmbed::error("Service Unavailable")
                .description("A database error occurred. Please try again later."),
            CtfError::RateLimit { retry_after } => {
                let msg = if let Some(d) = retry_after {
                    format!("Please try again in {d:?}.")
                } else {
                    "Please try again later.".to_string()
                };
                CtfEmbed::error("Rate Limited").description(msg)
            }
            CtfError::Timeout => CtfEmbed::error("Request Timeout")
                .description("The request took too long. Please try again."),
            CtfError::ExternalApi { status, message } => {
                CtfEmbed::error("Upstream Error").description(format!("Error {status}: {message}"))
            }
            CtfError::Serialization(_) => CtfEmbed::error("Data Error")
                .description("Failed to process data from external service."),
            CtfError::Internal(m) => CtfEmbed::error("Internal Error").description(m),
        }
    }
}

pub trait CtfErrorContext<T> {
    fn ctf_context(self, msg: &str) -> Result<T, CtfError>;
}

impl<T, E> CtfErrorContext<T> for Result<T, E>
where
    E: std::fmt::Display,
{
    fn ctf_context(self, msg: &str) -> Result<T, CtfError> {
        self.map_err(|e| CtfError::Internal(format!("{msg}: {e}")))
    }
}

pub type CtfResult<T> = std::result::Result<T, CtfError>;
