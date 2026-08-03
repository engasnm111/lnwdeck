use crate::preview::compute_hash;
use std::fs;
use std::path::Path;

pub fn apply_change(
    target: &str,
    new_content: &str,
    expected_hash: Option<&str>,
) -> Result<(), crate::HookError> {
    if Path::new(target).exists() {
        if let Some(expected) = expected_hash {
            let original = fs::read_to_string(target).map_err(|_| crate::HookError::NotFound)?;
            let actual_hash = compute_hash(&original);
            if actual_hash != expected {
                return Err(crate::HookError::HashMismatch);
            }
        }
        backup_file(target)?;
    }

    write_atomic(target, new_content).map_err(|e| crate::HookError::ApplyFailed(e.to_string()))?;

    Ok(())
}

fn write_atomic(target: &str, content: &str) -> std::io::Result<()> {
    let tmp_path = format!("{}.tmp", target);
    fs::write(&tmp_path, content)?;
    fs::rename(&tmp_path, target)?;
    Ok(())
}

fn backup_file(target: &str) -> Result<(), crate::HookError> {
    let backup_path = format!("{}.inwdeck_backup", target);
    if Path::new(&backup_path).exists() {
        fs::remove_file(&backup_path).map_err(|_| crate::HookError::BackupFailed)?;
    }
    fs::copy(target, &backup_path).map_err(|_| crate::HookError::BackupFailed)?;
    Ok(())
}
