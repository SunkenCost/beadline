use beadline_core::error::CoreError;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("bd executable not found")]
    BdNotFound,

    #[error("workspace path does not exist: {0}")]
    WorkspaceNotFound(String),

    #[error("workspace has not initialized Beads")]
    BeadsNotInitialized,

    #[error("bd command failed ({exit_code}): {stderr}")]
    BdCommandFailed { exit_code: i32, stderr: String },

    #[error("bd command timed out after {seconds}s")]
    BdTimeout { seconds: u64 },

    #[error("invalid theme file: {0}")]
    InvalidTheme(String),

    #[error("disallowed external URL scheme: {0}")]
    DisallowedScheme(String),

    #[error("config storage error: {0}")]
    ConfigIo(String),

    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    #[error("resource busy: {0}")]
    Busy(String),

    #[error("stale revision: {0}")]
    StaleRevision(String),

    #[error("managed bd installation failed: {0}")]
    InstallFailed(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl From<CoreError> for AppError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::WorkspaceNotFound(p) => Self::WorkspaceNotFound(p),
            CoreError::NotInitialized => Self::BeadsNotInitialized,
            CoreError::NotFound(m) | CoreError::InvalidOperation(m) | CoreError::WorkspaceMismatch(m) => {
                Self::InvalidOperation(m)
            }
            CoreError::Busy(m) => Self::Busy(m),
            CoreError::StaleRevision { .. } => Self::StaleRevision(err.to_string()),
            CoreError::SessionExpired(m) => Self::InvalidOperation(m),
            CoreError::Io(m) | CoreError::Internal(m) => Self::Internal(m),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub code: &'static str,
    pub message: String,
    pub detail: Option<String>,
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::BdNotFound => "BD_NOT_FOUND",
            Self::WorkspaceNotFound(_) => "WORKSPACE_NOT_FOUND",
            Self::BeadsNotInitialized => "BEADS_NOT_INITIALIZED",
            Self::BdCommandFailed { .. } => "BD_COMMAND_FAILED",
            Self::BdTimeout { .. } => "BD_TIMEOUT",
            Self::InvalidTheme(_) => "INVALID_THEME",
            Self::DisallowedScheme(_) => "DISALLOWED_SCHEME",
            Self::ConfigIo(_) => "CONFIG_IO",
            Self::InvalidOperation(_) => "INVALID_OPERATION",
            Self::Busy(_) => "BUSY",
            Self::StaleRevision(_) => "STALE_REV",
            Self::InstallFailed(_) => "INSTALL_FAILED",
            Self::Internal(_) => "INTERNAL",
        }
    }

    pub fn to_dto(&self) -> AppErrorDto {
        let detail = match self {
            Self::BdCommandFailed { stderr, .. } if !stderr.is_empty() => Some(stderr.clone()),
            _ => None,
        };
        AppErrorDto {
            code: self.code(),
            message: self.to_string(),
            detail,
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_dto().serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;
