use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("workspace path does not exist: {0}")]
    WorkspaceNotFound(String),

    #[error("workspace has not been initialized for Beadline/Beads")]
    NotInitialized,

    #[error("not found: {0}")]
    NotFound(String),

    #[error("stale revision for {issue_id}: expected rev {expected}, current rev {actual}")]
    StaleRevision {
        issue_id: String,
        expected: u64,
        actual: u64,
    },

    #[error("resource busy, please retry: {0}")]
    Busy(String),

    #[error("workspace mismatch: {0}")]
    WorkspaceMismatch(String),

    #[error("session expired: {0}")]
    SessionExpired(String),

    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    #[error("io error: {0}")]
    Io(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::WorkspaceNotFound(_) => "WORKSPACE_NOT_FOUND",
            Self::NotInitialized => "NOT_INITIALIZED",
            Self::NotFound(_) => "NOT_FOUND",
            Self::StaleRevision { .. } => "STALE_REV",
            Self::Busy(_) => "BUSY",
            Self::WorkspaceMismatch(_) => "WORKSPACE_MISMATCH",
            Self::SessionExpired(_) => "SESSION_EXPIRED",
            Self::InvalidOperation(_) => "INVALID_INPUT",
            Self::Io(_) => "IO_ERROR",
            Self::Internal(_) => "INTERNAL",
        }
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
