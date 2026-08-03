//! Regression tests for the Tauri v2 capability set.
//!
//! Without capability files the webview receives no permissions, which
//! silently disables every `listen()` call (update notification, quota
//! refresh) and the widget drag region while leaving `invoke` of
//! application commands working. That failure mode is invisible at runtime
//! because the frontend catches listener errors, so it is pinned here.

use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_capability(name: &str) -> serde_json::Value {
    let path = manifest_dir().join("capabilities").join(name);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("capability file {} must exist: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("capability file {} must be valid JSON: {e}", path.display()))
}

fn permissions_of(capability: &serde_json::Value) -> Vec<String> {
    capability["permissions"]
        .as_array()
        .expect("capability must declare a permissions array")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("permission entries must be identifier strings")
                .to_string()
        })
        .collect()
}

fn windows_of(capability: &serde_json::Value) -> Vec<String> {
    capability["windows"]
        .as_array()
        .expect("capability must declare a windows array")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("window entries must be label strings")
                .to_string()
        })
        .collect()
}

/// Window labels created in `windows::setup_windows`.
const APP_WINDOW_LABELS: [&str; 2] = ["main", "widget"];

#[test]
fn capability_directory_exists_and_is_not_empty() {
    let dir = manifest_dir().join("capabilities");
    assert!(
        dir.is_dir(),
        "capabilities directory is required: {}",
        dir.display()
    );
    let files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("read capabilities dir")
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    assert!(
        files.len() >= 2,
        "expected a capability file per window, found {files:?}"
    );
}

#[test]
fn main_window_can_listen_to_backend_events() {
    let capability = read_capability("main.json");
    assert_eq!(windows_of(&capability), vec!["main".to_string()]);
    let permissions = permissions_of(&capability);
    assert!(
        permissions.iter().any(|p| p == "core:default"
            || p == "core:event:default"
            || p == "core:event:allow-listen"),
        "main window must be allowed to listen for events, got {permissions:?}"
    );
}

#[test]
fn widget_window_can_listen_and_drag() {
    let capability = read_capability("widget.json");
    assert_eq!(windows_of(&capability), vec!["widget".to_string()]);
    let permissions = permissions_of(&capability);
    for required in [
        "core:event:allow-listen",
        "core:window:allow-start-dragging",
    ] {
        assert!(
            permissions.iter().any(|p| p == required),
            "widget capability must grant {required}, got {permissions:?}"
        );
    }
}

#[test]
fn capabilities_only_target_windows_the_app_creates() {
    let dir = manifest_dir().join("capabilities");
    for entry in std::fs::read_dir(&dir).expect("read capabilities dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("file name")
            .to_string();
        let capability = read_capability(&name);
        for label in windows_of(&capability) {
            assert!(
                APP_WINDOW_LABELS.contains(&label.as_str()),
                "{name} targets unknown window label {label:?}"
            );
        }
    }
}

#[test]
fn generated_acl_capabilities_are_not_empty_after_a_build() {
    let generated = manifest_dir()
        .join("gen")
        .join("schemas")
        .join("capabilities.json");
    if !Path::new(&generated).exists() {
        // The file only exists after `tauri-build` has run in this checkout.
        return;
    }
    let raw = std::fs::read_to_string(&generated).expect("read generated capabilities");
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).expect("generated capabilities must be valid JSON");
    let map = parsed
        .as_object()
        .expect("generated capabilities must be an object");
    assert!(
        !map.is_empty(),
        "generated ACL is empty: the webview would have no permissions at runtime"
    );
    for identifier in ["main-window", "widget-window"] {
        assert!(
            map.contains_key(identifier),
            "generated ACL is missing capability {identifier}: {:?}",
            map.keys().collect::<Vec<_>>()
        );
    }
}
