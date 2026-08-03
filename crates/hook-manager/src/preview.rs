use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub struct ChangePreview {
    pub target: String,
    pub original_hash: Option<String>,
    pub is_create: bool,
    pub diff_summary: String,
}

pub fn preview_change(target: &str, new_content: &str) -> Result<ChangePreview, crate::HookError> {
    let is_create = !Path::new(target).exists();

    let original_hash = if is_create {
        None
    } else {
        let original = fs::read_to_string(target).map_err(|_| crate::HookError::NotFound)?;
        Some(compute_hash(&original))
    };

    let diff = if is_create {
        format!("create file with {} bytes", new_content.len())
    } else {
        let original = fs::read_to_string(target).unwrap_or_default();
        if original == new_content {
            "no change".to_string()
        } else {
            format!(
                "{} bytes changed",
                (new_content.len() as i64 - original.len() as i64).abs()
            )
        }
    };

    Ok(ChangePreview {
        target: target.to_string(),
        original_hash,
        is_create,
        diff_summary: diff,
    })
}

pub fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}
