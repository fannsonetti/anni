use thiserror::Error;

#[derive(Debug, Error)]
pub enum InnaError {
    #[error("not authenticated")]
    NotAuthenticated,

    #[error("INNA session expired")]
    SessionExpired,

    #[error("network unavailable")]
    Network,

    #[error("HTTP status {0}")]
    HttpStatus(u16),

    #[error("failed to parse INNA response")]
    Parse,

    #[error("database error")]
    Database,

    #[error("invalid XSRF state")]
    MissingXsrf,

    #[error("unexpected INNA response")]
    UnexpectedResponse,

    #[error("{0}")]
    Other(String),
}

impl From<reqwest::Error> for InnaError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_connect() || e.is_timeout() {
            InnaError::Network
        } else if let Some(status) = e.status() {
            match status.as_u16() {
                401 | 403 => InnaError::SessionExpired,
                code => InnaError::HttpStatus(code),
            }
        } else {
            InnaError::Other(e.to_string())
        }
    }
}

impl From<serde_json::Error> for InnaError {
    fn from(_: serde_json::Error) -> Self {
        InnaError::Parse
    }
}
