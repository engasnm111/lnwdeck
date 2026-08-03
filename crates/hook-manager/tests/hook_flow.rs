use lnwdeck_hook_manager::{apply_change, preview_change, restore_backup, RestoreResult};
use std::fs;
use tempfile::TempDir;

fn write_config(dir: &TempDir, filename: &str, content: &str) -> String {
    let path = dir.path().join(filename);
    fs::write(&path, content).unwrap();
    path.to_string_lossy().to_string()
}

fn hash_content(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

// ── absent hook ──

#[test]
fn preview_absent_file_shows_create() {
    let dir = TempDir::new().unwrap();
    let path = dir
        .path()
        .join("missing.json")
        .to_string_lossy()
        .to_string();

    let new_content = r#"{"enabled": true}"#.to_string();
    let result = preview_change(&path, &new_content).unwrap();

    assert!(result.original_hash.is_none());
    assert_eq!(result.target, path);
}

#[test]
fn apply_to_absent_file_creates_it() {
    let dir = TempDir::new().unwrap();
    let path = dir
        .path()
        .join("new_config.json")
        .to_string_lossy()
        .to_string();
    let new_content = r#"{"enabled": true}"#.to_string();

    apply_change(&path, &new_content, None).unwrap();

    assert!(std::path::Path::new(&path).exists());
    let written = fs::read_to_string(&path).unwrap();
    assert_eq!(written, new_content);
}

// ── compatible hook ──

#[test]
fn preview_compatible_change_shows_diff() {
    let dir = TempDir::new().unwrap();
    let original = r#"{"enabled": false}"#;
    let path = write_config(&dir, "config.json", original);
    let original_hash = hash_content(original);

    let new_content = r#"{"enabled": true}"#.to_string();
    let result = preview_change(&path, &new_content).unwrap();

    assert_eq!(result.original_hash, Some(original_hash));
    assert!(
        !result.is_create,
        "existing file should not be a create operation"
    );
}

#[test]
fn apply_compatible_change_succeeds() {
    let dir = TempDir::new().unwrap();
    let original = r#"{"enabled": false}"#;
    let path = write_config(&dir, "config.json", original);
    let new_content = r#"{"enabled": true}"#.to_string();
    let original_hash = hash_content(original);

    apply_change(&path, &new_content, Some(&original_hash)).unwrap();

    let written = fs::read_to_string(&path).unwrap();
    assert_eq!(written, new_content);
}

// ── incompatible hook ──

#[test]
fn apply_with_hash_mismatch_fails() {
    let dir = TempDir::new().unwrap();
    let original = r#"{"enabled": false}"#;
    let path = write_config(&dir, "config.json", original);
    let new_content = r#"{"enabled": true}"#.to_string();
    let wrong_hash = hash_content("different content");

    let result = apply_change(&path, &new_content, Some(&wrong_hash));
    assert!(result.is_err());

    let written = fs::read_to_string(&path).unwrap();
    assert_eq!(
        written, original,
        "original must not be modified on hash mismatch"
    );
}

// ── rollback ──

#[test]
fn restore_backup_after_apply() {
    let dir = TempDir::new().unwrap();
    let original = r#"{"enabled": false}"#;
    let path = write_config(&dir, "config.json", original);
    let new_content = r#"{"enabled": true}"#.to_string();
    let original_hash = hash_content(original);

    apply_change(&path, &new_content, Some(&original_hash)).unwrap();

    let written = fs::read_to_string(&path).unwrap();
    assert_eq!(written, new_content);

    let result = restore_backup(&path).unwrap();
    assert_eq!(result, RestoreResult::Restored);

    let restored = fs::read_to_string(&path).unwrap();
    assert_eq!(restored, original);
}

#[test]
fn restore_without_backup_fails() {
    let dir = TempDir::new().unwrap();
    let original = r#"{"enabled": false}"#;
    let path = write_config(&dir, "config.json", original);

    let result = restore_backup(&path).unwrap();
    assert_eq!(result, RestoreResult::NoBackupFound);

    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original, "original must be intact");
}

// ── invalid JSON ──

#[test]
fn apply_invalid_json_is_written_as_is() {
    let dir = TempDir::new().unwrap();
    let original = r#"{"enabled": false}"#;
    let path = write_config(&dir, "config.json", original);
    let invalid_content = "{invalid json}".to_string();

    // Hook manager does NOT validate JSON — it manages raw file content
    let result = apply_change(&path, &invalid_content, None);
    assert!(result.is_ok());

    let written = fs::read_to_string(&path).unwrap();
    assert_eq!(written, invalid_content);
}

// ── backup cleanup after rollback ──

#[test]
fn backup_file_removed_after_successful_rollback() {
    let dir = TempDir::new().unwrap();
    let original = r#"{"enabled": false}"#;
    let path = write_config(&dir, "config.json", original);
    let new_content = r#"{"enabled": true}"#.to_string();
    let original_hash = hash_content(original);

    apply_change(&path, &new_content, Some(&original_hash)).unwrap();
    restore_backup(&path).unwrap();

    let backup_path = format!("{}.lnwdeck_backup", path);
    assert!(
        !std::path::Path::new(&backup_path).exists(),
        "backup file must be removed after rollback"
    );
}
