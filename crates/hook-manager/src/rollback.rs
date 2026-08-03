use std::fs;
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub enum RestoreResult {
    Restored,
    NoBackupFound,
}

pub fn restore_backup(target: &str) -> Result<RestoreResult, crate::HookError> {
    let backup_path = format!("{}.lnwdeck_backup", target);
    if !Path::new(&backup_path).exists() {
        return Ok(RestoreResult::NoBackupFound);
    }

    fs::copy(&backup_path, target).map_err(|_| crate::HookError::RestoreFailed)?;
    fs::remove_file(&backup_path).map_err(|_| crate::HookError::RestoreFailed)?;

    Ok(RestoreResult::Restored)
}
