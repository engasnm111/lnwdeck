//! Tauri commands and the local asset protocol for the widget pet.
//!
//! The heavy lifting lives in [`crate::pets`]; this module is the thin IPC
//! layer: it resolves the pet store directory, exposes import/select/remove
//! commands, and serves installed pet assets to the widget over
//! `petlocal://pets/<id>/...` so the webview never loads a remote asset.

use crate::state::AppState;
use lnwdeck_application::settings::SettingsService;
use std::path::{Path, PathBuf};
use tauri::http::Response;
use tauri::Manager;

/// Directory holding installed community pets, next to the database.
pub fn pet_store_dir(app: &tauri::AppHandle) -> PathBuf {
    let state = app.state::<AppState>();
    state
        .db_path
        .parent()
        .map(|parent| parent.join("pets"))
        .unwrap_or_else(|| PathBuf::from("pets"))
}

/// Imports a community pet from a codex-pets.net URL or a bare pet id.
///
/// Downloads happen only here, on an explicit user action, over HTTPS, and
/// only against `codex-pets.net`; the package is fully validated before it is
/// stored locally.
#[tauri::command]
pub fn import_widget_pet(
    app: tauri::AppHandle,
    input: String,
) -> Result<crate::pets::PetManifest, String> {
    let id = crate::pets::pet_id_from_codex_pets_url(&input)?;
    let package = crate::pets::download_pet_package(&id)?;
    crate::pets::install_pet(&pet_store_dir(&app), &package)
}

/// Imports a community pet from a local `.codex-pet.zip` file path.
#[tauri::command]
pub fn import_widget_pet_file(
    app: tauri::AppHandle,
    path: String,
) -> Result<crate::pets::PetManifest, String> {
    let bytes = std::fs::read(&path).map_err(|error| format!("could not read {path}: {error}"))?;
    crate::pets::install_pet(&pet_store_dir(&app), &bytes)
}

/// Lists installed community pets, sorted by display name.
#[tauri::command]
pub fn list_widget_pets(app: tauri::AppHandle) -> Vec<crate::pets::PetManifest> {
    crate::pets::list_installed_pets(&pet_store_dir(&app))
}

/// The community pet selected for the widget, when one is selected.
#[tauri::command]
pub fn get_widget_pet(app: tauri::AppHandle) -> Option<crate::pets::PetManifest> {
    let settings = crate::windows::widget_settings(&app);
    if settings.pet_id.is_empty() {
        return None;
    }
    crate::pets::load_pet_dir(
        &pet_store_dir(&app).join(&settings.pet_id),
        Some(&settings.pet_id),
    )
    .ok()
}

/// Selects the widget's community pet. An empty id restores the built-in robot.
#[tauri::command]
pub fn set_widget_pet(app: tauri::AppHandle, pet_id: String) -> Result<String, String> {
    let trimmed = pet_id.trim();
    if !trimmed.is_empty() && !pet_store_dir(&app).join(trimmed).is_dir() {
        return Err(format!("pet {trimmed} is not installed"));
    }
    let state = app.state::<AppState>();
    let stored = {
        let guard = state.ensure_storage()?;
        let storage = guard.as_ref().ok_or("storage not initialized")?;
        SettingsService::set_widget_pet_id(&storage.conn, trimmed).map_err(|e| e.to_string())?
    };
    // Emit only after the storage guard is dropped: emitting reads the
    // settings back and would deadlock on the storage mutex.
    crate::windows::emit_widget_settings(&app);
    Ok(stored)
}

/// Removes an installed pet; when it was the active one the widget falls back
/// to the built-in robot.
#[tauri::command]
pub fn remove_widget_pet(app: tauri::AppHandle, pet_id: String) -> Result<(), String> {
    crate::pets::remove_installed_pet(&pet_store_dir(&app), &pet_id)?;
    let settings = crate::windows::widget_settings(&app);
    if settings.pet_id == pet_id {
        let state = app.state::<AppState>();
        {
            let guard = state.ensure_storage()?;
            let storage = guard.as_ref().ok_or("storage not initialized")?;
            SettingsService::set_widget_pet_id(&storage.conn, "").map_err(|e| e.to_string())?;
        }
        crate::windows::emit_widget_settings(&app);
    }
    Ok(())
}

/// Serves an installed pet asset for `petlocal://pets/<id>/<file>`.
///
/// Only the two well-known file names under a validated pet id are served;
/// anything else is a 404, so the protocol cannot be used to read arbitrary
/// files.
pub fn serve_pet_asset(store: &Path, path: &str) -> Response<Vec<u8>> {
    let response = |status: u16, content_type: &str, body: Vec<u8>| {
        Response::builder()
            .status(status)
            .header("content-type", content_type)
            .body(body)
            .expect("valid response")
    };
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if segments.len() != 3 || segments[0] != "pets" {
        return response(404, "text/plain", b"not found".to_vec());
    }
    let id = segments[1];
    let file = segments[2];
    if !crate::pets::is_pet_id(id)
        || (file != crate::pets::PET_MANIFEST && file != crate::pets::PET_SPRITESHEET)
    {
        return response(404, "text/plain", b"not found".to_vec());
    }
    let Ok(bytes) = std::fs::read(store.join(id).join(file)) else {
        return response(404, "text/plain", b"not found".to_vec());
    };
    let content_type = if file == crate::pets::PET_SPRITESHEET {
        "image/webp"
    } else {
        "application/json"
    };
    response(200, content_type, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn fake_webp(width: u32, height: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(b"VP8X");
        body.extend_from_slice(&10u32.to_le_bytes());
        body.extend_from_slice(&[0u8; 4]);
        body.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
        body.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&(4 + body.len() as u32).to_le_bytes());
        file.extend_from_slice(b"WEBP");
        file.extend_from_slice(&body);
        file
    }

    fn install_fixture(store: &Path, id: &str) {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        let manifest = format!(
            r#"{{"id":"{id}","displayName":"Test Pet","description":"Fixture","spritesheetPath":"spritesheet.webp"}}"#
        );
        zip.start_file(crate::pets::PET_MANIFEST, options).unwrap();
        zip.write_all(manifest.as_bytes()).unwrap();
        zip.start_file(crate::pets::PET_SPRITESHEET, options)
            .unwrap();
        zip.write_all(&fake_webp(1536, 1872)).unwrap();
        let cursor = zip.finish().unwrap();
        crate::pets::install_pet(store, &cursor.into_inner()).unwrap();
    }

    #[test]
    fn pet_assets_are_served_with_the_right_content_types() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("pets");
        install_fixture(&store, "sprout");

        let sheet = serve_pet_asset(&store, "/pets/sprout/spritesheet.webp");
        assert_eq!(sheet.status().as_u16(), 200);
        assert_eq!(
            sheet
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "image/webp"
        );
        assert_eq!(sheet.body().len(), fake_webp(1536, 1872).len());

        let manifest = serve_pet_asset(&store, "/pets/sprout/pet.json");
        assert_eq!(manifest.status().as_u16(), 200);
        assert_eq!(
            manifest
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "application/json"
        );
    }

    #[test]
    fn pet_protocol_refuses_unknown_paths_and_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("pets");
        install_fixture(&store, "sprout");

        for path in [
            "/",
            "/pets",
            "/pets/sprout",
            "/pets/sprout/evil.png",
            "/pets/../secret.txt",
            "/pets/sprout/../../Cargo.toml",
            "/other/sprout/spritesheet.webp",
        ] {
            assert_eq!(
                serve_pet_asset(&store, path).status().as_u16(),
                404,
                "{path} must be refused"
            );
        }
    }

    #[test]
    fn pet_protocol_returns_404_for_missing_pets() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("pets");
        assert_eq!(
            serve_pet_asset(&store, "/pets/ghost/spritesheet.webp")
                .status()
                .as_u16(),
            404
        );
    }
}
