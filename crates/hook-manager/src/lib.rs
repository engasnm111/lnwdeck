pub mod apply;
pub mod backup;
pub mod preview;
pub mod rollback;

pub use apply::apply_change;
pub use preview::{preview_change, ChangePreview};
pub use rollback::{restore_backup, RestoreResult};

#[derive(Debug)]
pub enum HookError {
    FileNotFound,
    HashMismatch,
    ReadOnly,
    BackupFailed,
    RestoreFailed,
    ApplyFailed(String),
    NotFound,
}

impl std::fmt::Display for HookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookError::HashMismatch => write!(f, "hash mismatch"),
            HookError::NotFound => write!(f, "not found"),
            _ => write!(f, "hook error"),
        }
    }
}

impl std::error::Error for HookError {}
