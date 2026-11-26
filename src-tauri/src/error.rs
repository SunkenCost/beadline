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

    #[error("internal error: {0}")]
    Internal(String),
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
