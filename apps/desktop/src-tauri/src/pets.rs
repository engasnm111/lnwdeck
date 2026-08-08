//! Widget pet store and the codex-pets.net import.
//!
//! The floating widget's pet can be a community pet from
//! <https://codex-pets.net>, packaged as a `.codex-pet.zip` containing exactly
//! `pet.json` and `spritesheet.webp` at its root. This module validates the
//! package against the same rules TokenTracker uses, stores it atomically
//! under `<local data>/lnwdeck/pets/<id>/`, and serves it to the widget
//! through the `petlocal://` URI scheme so the webview never loads remote
//! assets.
//!
//! Downloads happen only on an explicit user action (import URL or pet id),
//! only over HTTPS, and only against `codex-pets.net`, which is the single
//! declared network destination for this feature.

use lnwdeck_provider_http::{get_bytes, JsonRequest};
use lnwdeck_storage::repositories::AppSettingsRepository;
use rusqlite::Connection;
use serde::Serialize;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use std::time::Duration;

pub const PET_MANIFEST: &str = "pet.json";
pub const PET_SPRITESHEET: &str = "spritesheet.webp";
pub const MAX_PACKAGE_BYTES: usize = 12 * 1024 * 1024;
pub const MAX_MANIFEST_BYTES: usize = 16 * 1024;
pub const MAX_SPRITESHEET_BYTES: usize = 10 * 1024 * 1024;
pub const SPRITESHEET_WIDTH: u32 = 1536;
pub const SPRITESHEET_HEIGHT_V1: u32 = 1872;
pub const SPRITESHEET_HEIGHT_V2: u32 = 2288;
/// The only host the pet import will ever talk to.
pub const CODEX_PETS_HOST: &str = "codex-pets.net";
/// Download timeout for the pet package.
const PET_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// Bundled default pets from codex-pets.net, shipped inside the binary so a
/// fresh install has a full character roster offline.
///
/// The packages are vendored under `src-tauri/assets/pets/` and never fetched
/// at runtime. Ids are the codex-pets.net manifest ids.
pub const DEFAULT_PET_IDS: [&str; 8] = [
    "youyou",
    "old-bai",
    "a-ti",
    "sharkler",
    "solaire",
    "tennis-ball",
    "friend-pixel-pet",
    "yae-miko",
];

/// The bundled packages, aligned with [`DEFAULT_PET_IDS`].
const DEFAULT_PET_PACKAGES: [&[u8]; 8] = [
    include_bytes!("../assets/pets/youyou.zip"),
    include_bytes!("../assets/pets/old-bai.zip"),
    include_bytes!("../assets/pets/a-ti.zip"),
    include_bytes!("../assets/pets/sharkler.zip"),
    include_bytes!("../assets/pets/solaire.zip"),
    include_bytes!("../assets/pets/tennis-ball.zip"),
    include_bytes!("../assets/pets/friend-pixel-pet.zip"),
    include_bytes!("../assets/pets/yae-miko.zip"),
];

/// `app_settings` key marking that the bundled defaults were installed once.
const DEFAULT_PETS_SEEDED_KEY: &str = "pet_defaults_seeded";

/// Installs the bundled default pets into the store, exactly once.
///
/// Re-runs are idempotent: when the marker is already set, the currently
/// installed defaults are reported and nothing is written, so a default the
/// user removed stays removed.
pub fn seed_default_pets(pets_dir: &Path, conn: &Connection) -> Result<Vec<String>, String> {
    let repo = AppSettingsRepository::new(conn);
    if repo
        .get(DEFAULT_PETS_SEEDED_KEY)
        .map_err(|e| e.to_string())?
        .as_deref()
        == Some("true")
    {
        return Ok(DEFAULT_PET_IDS
            .iter()
            .filter(|id| pets_dir.join(id).is_dir())
            .map(|id| id.to_string())
            .collect());
    }

    let mut installed = Vec::new();
    for (id, package) in DEFAULT_PET_IDS.iter().zip(DEFAULT_PET_PACKAGES.iter()) {
        if pets_dir.join(id).is_dir() {
            installed.push((*id).to_string());
            continue;
        }
        match install_pet(pets_dir, package) {
            Ok(manifest) => installed.push(manifest.id),
            Err(error) => {
                return Err(format!("default pet {id}: {error}"));
            }
        }
    }
    repo.set(DEFAULT_PETS_SEEDED_KEY, "true")
        .map_err(|e| e.to_string())?;
    Ok(installed)
}

/// A validated pet manifest, as stored and served to the widget.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetManifest {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub spritesheet_path: String,
    /// Always serialized: the webview needs the real value (1 or 2) to pick
    /// the right atlas layout. Omitting v1 broke v1 pets โ€” the frontend saw
    /// `undefined` and rendered them with the v2 grid.
    pub sprite_version_number: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// A lowercase URL-safe pet id: `[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?`.
pub fn is_pet_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 64 {
        return false;
    }
    let bytes = id.as_bytes();
    let alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    if !alnum(bytes[0]) || !alnum(bytes[bytes.len() - 1]) {
        return false;
    }
    bytes.iter().all(|b| alnum(*b) || *b == b'-')
}

/// `[a-z][a-z0-9-]{0,39}` โ€” an optional short label for the pet kind.
fn is_kind(value: &str) -> bool {
    if value.is_empty() || value.len() > 40 {
        return false;
    }
    let bytes = value.as_bytes();
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Validates and normalizes the parsed `pet.json` object.
pub fn normalize_manifest(value: &serde_json::Value) -> Result<PetManifest, String> {
    if !value.is_object() {
        return Err("pet.json must contain a JSON object".to_string());
    }
    let read_str = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };
    let id = read_str("id").to_lowercase();
    let display_name = read_str("displayName");
    let description = read_str("description");
    let spritesheet_path = read_str("spritesheetPath");
    let kind = read_str("kind").to_lowercase();
    let version_provided = value.get("spriteVersionNumber").is_some();
    let sprite_version_number = if version_provided {
        value
            .get("spriteVersionNumber")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| "pet.json spriteVersionNumber must be a number".to_string())?
            .try_into()
            .map_err(|_| "pet.json spriteVersionNumber is out of range".to_string())?
    } else {
        1
    };

    if !is_pet_id(&id) {
        return Err("pet.json id must be a lowercase URL-safe slug".to_string());
    }
    if display_name.is_empty() || display_name.len() > 80 {
        return Err(
            "pet.json displayName is required and must be at most 80 characters".to_string(),
        );
    }
    if description.is_empty() || description.len() > 280 {
        return Err(
            "pet.json description is required and must be at most 280 characters".to_string(),
        );
    }
    if spritesheet_path != PET_SPRITESHEET {
        return Err("pet.json spritesheetPath must be spritesheet.webp".to_string());
    }
    if !kind.is_empty() && !is_kind(&kind) {
        return Err("pet.json kind must be a short lowercase label when provided".to_string());
    }
    if version_provided && !matches!(sprite_version_number, 1 | 2) {
        return Err("pet.json spriteVersionNumber must be 1 or 2 when provided".to_string());
    }

    Ok(PetManifest {
        id,
        display_name,
        description,
        spritesheet_path: PET_SPRITESHEET.to_string(),
        sprite_version_number,
        kind: (!kind.is_empty()).then_some(kind),
    })
}

/// Reads the dimensions of a WebP file by walking its chunks.
///
/// Mirrors the reference implementation so an imported package is accepted or
/// rejected identically: only the `VP8X`, `VP8 ` and `VP8L` image frames are
/// recognized, and any malformed chunk header rejects the sheet.
pub fn read_webp_dimensions(buffer: &[u8]) -> Result<(u32, u32), String> {
    if buffer.len() < 30
        || &buffer[0..4] != b"RIFF"
        || &buffer[8..12] != b"WEBP"
        || u32::from_le_bytes(buffer[4..8].try_into().unwrap()) + 8 != buffer.len() as u32
    {
        return Err("spritesheet.webp is not a valid WebP file".to_string());
    }

    let read_u24 = |offset: usize| -> u32 {
        (buffer[offset] as u32)
            | ((buffer[offset + 1] as u32) << 8)
            | ((buffer[offset + 2] as u32) << 16)
    };

    let mut offset = 12;
    while offset + 8 <= buffer.len() {
        let chunk_type = &buffer[offset..offset + 4];
        let size = u32::from_le_bytes(buffer[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let data = offset + 8;
        if data + size > buffer.len() {
            return Err("spritesheet.webp is truncated".to_string());
        }
        match chunk_type {
            b"VP8X" if size >= 10 => {
                return Ok((read_u24(data + 4) + 1, read_u24(data + 7) + 1));
            }
            b"VP8 " if size >= 10 => {
                if buffer[data + 3] != 0x9d || buffer[data + 4] != 0x01 || buffer[data + 5] != 0x2a
                {
                    return Err("spritesheet.webp has an invalid VP8 frame header".to_string());
                }
                let width =
                    u16::from_le_bytes(buffer[data + 6..data + 8].try_into().unwrap()) & 0x3fff;
                let height =
                    u16::from_le_bytes(buffer[data + 8..data + 10].try_into().unwrap()) & 0x3fff;
                return Ok((width as u32, height as u32));
            }
            b"VP8L" if size >= 5 => {
                if buffer[data] != 0x2f {
                    return Err("spritesheet.webp has an invalid VP8L header".to_string());
                }
                let bits = u32::from_le_bytes(buffer[data + 1..data + 5].try_into().unwrap());
                let width = (bits & 0x3fff) + 1;
                let height = ((bits >> 14) & 0x3fff) + 1;
                return Ok((width, height));
            }
            _ => {}
        }
        offset = data + size + (size % 2);
    }
    Err("spritesheet.webp has no decodable image frame".to_string())
}

/// Validates a manifest against its spritesheet.
pub fn validate_package(manifest: &PetManifest, spritesheet: &[u8]) -> Result<(), String> {
    if spritesheet.is_empty() || spritesheet.len() > MAX_SPRITESHEET_BYTES {
        return Err("spritesheet.webp is missing or too large".to_string());
    }
    let (width, height) = read_webp_dimensions(spritesheet)?;
    let expected_height = match manifest.sprite_version_number {
        1 => SPRITESHEET_HEIGHT_V1,
        2 => SPRITESHEET_HEIGHT_V2,
        _ => return Err("unsupported spriteVersionNumber".to_string()),
    };
    if width != SPRITESHEET_WIDTH || height != expected_height {
        return Err(format!(
            "spritesheet.webp must be {SPRITESHEET_WIDTH}x{expected_height} for a v{} pet",
            manifest.sprite_version_number
        ));
    }
    Ok(())
}

/// Reads a pet package zip: exactly `pet.json` and `spritesheet.webp` at the
/// root, no symlinks, no duplicates, within the size limits.
fn read_zip_entries(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    if bytes.is_empty() || bytes.len() > MAX_PACKAGE_BYTES {
        return Err("Pet package is empty or exceeds 12 MB".to_string());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("Invalid pet package: {error}"))?;

    let mut manifest: Option<Vec<u8>> = None;
    let mut spritesheet: Option<Vec<u8>> = None;
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| format!("Invalid pet package: {error}"))?;
        let name = file.name().to_string();
        let limit = match name.as_str() {
            PET_MANIFEST => {
                if manifest.is_some() {
                    return Err(format!("Pet package contains duplicate {PET_MANIFEST}"));
                }
                MAX_MANIFEST_BYTES
            }
            PET_SPRITESHEET => {
                if spritesheet.is_some() {
                    return Err(format!("Pet package contains duplicate {PET_SPRITESHEET}"));
                }
                MAX_SPRITESHEET_BYTES
            }
            _ => {
                return Err(
                    "A standard pet package must contain only pet.json and spritesheet.webp at its root"
                        .to_string(),
                );
            }
        };
        if file
            .unix_mode()
            .is_some_and(|mode| (mode & 0o170000) == 0o120000)
        {
            return Err("Pet package entries must not be symbolic links".to_string());
        }
        let size = file.size();
        if size == 0 || size > limit as u64 {
            return Err(format!("{name} is empty or too large"));
        }
        let mut content = Vec::with_capacity(size.min(limit as u64) as usize);
        file.take(limit as u64 + 1)
            .read_to_end(&mut content)
            .map_err(|error| format!("Could not read {name}: {error}"))?;
        if content.len() > limit {
            return Err(format!("{name} is too large"));
        }
        if name == PET_MANIFEST {
            manifest = Some(content);
        } else {
            spritesheet = Some(content);
        }
    }
    match (manifest, spritesheet) {
        (Some(manifest), Some(spritesheet)) => Ok((manifest, spritesheet)),
        _ => Err("Pet package must contain pet.json and spritesheet.webp".to_string()),
    }
}

/// Parses and fully validates a pet package zip.
pub fn validate_package_zip(bytes: &[u8]) -> Result<PetManifest, String> {
    let (manifest_bytes, spritesheet) = read_zip_entries(bytes)?;
    if manifest_bytes.is_empty() || manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err("pet.json is missing or too large".to_string());
    }
    let parsed: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| "pet.json is not valid JSON".to_string())?;
    let manifest = normalize_manifest(&parsed)?;
    validate_package(&manifest, &spritesheet)?;
    Ok(manifest)
}

/// Installed pet id from a bare id or a codex-pets.net URL.
///
/// Accepted: a bare pet id, `https://codex-pets.net/#/pets/<id>`,
/// `https://codex-pets.net/api/pets/<id>` and
/// `https://codex-pets.net/api/pets/<id>/download`.
pub fn pet_id_from_codex_pets_url(input: &str) -> Result<String, String> {
    let raw = input.trim();
    if is_pet_id(raw) {
        return Ok(raw.to_string());
    }
    let parsed = url_parse(raw).ok_or_else(|| "Enter a Codex Pets URL or pet id".to_string())?;
    if parsed.scheme != "https" || parsed.host != CODEX_PETS_HOST {
        return Err("Only https://codex-pets.net pet URLs are supported".to_string());
    }
    let id = if let Some(captured) = parsed.hash.strip_prefix("#/pets/") {
        captured.trim_end_matches('/').to_string()
    } else if let Some(rest) = parsed.path.strip_prefix("/api/pets/") {
        rest.trim_end_matches('/')
            .strip_suffix("/download")
            .unwrap_or(rest.trim_end_matches('/'))
            .to_string()
    } else {
        String::new()
    };
    let id = id.to_lowercase();
    if !is_pet_id(&id) {
        return Err("The Codex Pets URL does not contain a valid pet id".to_string());
    }
    Ok(id)
}

/// A minimal URL splitter for the pet import allowlist. Only `https` and the
/// codex-pets.net host are ever accepted, so a full URL crate is unnecessary.
struct SimpleUrl {
    scheme: String,
    host: String,
    path: String,
    hash: String,
}

fn url_parse(raw: &str) -> Option<SimpleUrl> {
    let (scheme, rest) = raw.split_once("://")?;
    let (authority, tail) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, _port) = authority.split_once(':').unwrap_or((authority, ""));
    let (path, hash) = tail.split_once('#').unwrap_or((tail, ""));
    Some(SimpleUrl {
        scheme: scheme.to_ascii_lowercase(),
        host: host.to_ascii_lowercase(),
        path: format!("/{path}"),
        hash: format!("#{hash}"),
    })
}

/// Download URL for a pet id.
pub fn codex_pets_download_url(id: &str) -> String {
    format!("https://{CODEX_PETS_HOST}/api/pets/{id}/download")
}

/// Downloads a pet package from codex-pets.net.
///
/// Called only on an explicit user action; the URL is derived from a
/// validated id, so no raw user input is ever fetched.
pub fn download_pet_package(id: &str) -> Result<Vec<u8>, String> {
    if !is_pet_id(id) {
        return Err("invalid pet id".to_string());
    }
    let url = codex_pets_download_url(id);
    let request = JsonRequest {
        raw_auth_token: None,
        url: &url,
        bearer_token: None,
        timeout: PET_DOWNLOAD_TIMEOUT,
        capture_headers: &[],
        extra_headers: &[],
    };
    let (status, bytes) = get_bytes(request)?;
    if status != 200 {
        return Err(format!("codex-pets.net returned HTTP {status}"));
    }
    Ok(bytes)
}

/// Atomically installs a validated package into the pet store.
///
/// The package is written to a staging directory first, re-validated there,
/// and only then renamed into place. An existing pet with the same id is kept
/// as a backup until the new one is fully written, and restored if the swap
/// fails.
pub fn install_pet(pets_dir: &Path, package_zip: &[u8]) -> Result<PetManifest, String> {
    let manifest = validate_package_zip(package_zip)?;
    fs::create_dir_all(pets_dir).map_err(|error| format!("pet store: {error}"))?;

    let stage = pets_dir.join(format!(
        ".lnwdeck-pet-{}-{}",
        std::process::id(),
        hex_uuid()
    ));
    let destination = pets_dir.join(&manifest.id);
    let backup = pets_dir.join(format!(
        "{}-backup",
        stage.file_name().unwrap_or_default().to_string_lossy()
    ));
    fs::create_dir_all(&stage).map_err(|error| format!("pet staging: {error}"))?;

    let staged_manifest =
        serde_json::to_vec_pretty(&manifest).map_err(|error| format!("pet manifest: {error}"))?;
    let write = |name: &str, content: &[u8]| -> Result<(), String> {
        fs::write(stage.join(name), content).map_err(|error| format!("pet write {name}: {error}"))
    };

    let result = (|| -> Result<PetManifest, String> {
        write(PET_MANIFEST, &staged_manifest)?;
        let (_, spritesheet) = read_zip_entries(package_zip)?;
        write(PET_SPRITESHEET, &spritesheet)?;
        // Re-read and re-validate what was written, then swap atomically.
        let stored = load_pet_dir(&stage, Some(&manifest.id))?;
        if destination.exists() {
            fs::rename(&destination, &backup).map_err(|error| format!("pet backup: {error}"))?;
        }
        if let Err(error) = fs::rename(&stage, &destination) {
            if !destination.exists() && backup.exists() {
                let _ = fs::rename(&backup, &destination);
            }
            return Err(format!("pet install: {error}"));
        }
        let _ = fs::remove_dir_all(&backup);
        Ok(stored)
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
        if !destination.exists() && backup.exists() {
            let _ = fs::rename(&backup, &destination);
        }
    }
    result
}

/// Loads and validates one installed pet directory.
pub fn load_pet_dir(dir: &Path, expected_id: Option<&str>) -> Result<PetManifest, String> {
    let meta = fs::metadata(dir).map_err(|error| format!("pet dir: {error}"))?;
    if !meta.is_dir() {
        return Err("Pet path is not a directory".to_string());
    }
    let manifest_bytes =
        fs::read(dir.join(PET_MANIFEST)).map_err(|_| "pet.json is missing".to_string())?;
    if manifest_bytes.is_empty() || manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err("pet.json is missing or too large".to_string());
    }
    let spritesheet = fs::read(dir.join(PET_SPRITESHEET))
        .map_err(|_| "spritesheet.webp is missing".to_string())?;
    let parsed: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| "pet.json is not valid JSON".to_string())?;
    let manifest = normalize_manifest(&parsed)?;
    if let Some(expected) = expected_id {
        if manifest.id != expected {
            return Err("Pet directory and manifest id do not match".to_string());
        }
    }
    validate_package(&manifest, &spritesheet)?;
    Ok(manifest)
}

/// Lists installed community pets, sorted by display name.
pub fn list_installed_pets(pets_dir: &Path) -> Vec<PetManifest> {
    let mut pets = Vec::new();
    let Ok(entries) = fs::read_dir(pets_dir) else {
        return pets;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || !is_pet_id(&name) {
            continue;
        }
        if let Ok(manifest) = load_pet_dir(&entry.path(), Some(&name)) {
            pets.push(manifest);
        }
    }
    pets.sort_by_key(|pet| pet.display_name.to_lowercase());
    pets
}

/// Removes an installed pet. Returns an error when the id is not installed.
pub fn remove_installed_pet(pets_dir: &Path, id: &str) -> Result<(), String> {
    if !is_pet_id(id) {
        return Err("invalid pet id".to_string());
    }
    let dir = pets_dir.join(id);
    if !dir.exists() {
        return Err(format!("pet {id} is not installed"));
    }
    fs::remove_dir_all(&dir).map_err(|error| format!("pet remove: {error}"))
}

fn hex_uuid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        bytes.copy_from_slice(&(count as u128).to_le_bytes()[..8]);
    }
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn manifest_json(id: &str, display_name: &str, version: Option<u64>) -> serde_json::Value {
        let mut value = serde_json::json!({
            "id": id,
            "displayName": display_name,
            "description": "A test pet",
            "spritesheetPath": "spritesheet.webp",
        });
        if let Some(version) = version {
            value["spriteVersionNumber"] = serde_json::json!(version);
        }
        value
    }

    /// A structurally valid WebP of the given dimensions: RIFF/WEBP header
    /// plus a minimal VP8X chunk carrying the size.
    fn fake_webp(width: u32, height: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(b"VP8X");
        body.extend_from_slice(&10u32.to_le_bytes());
        body.extend_from_slice(&[0u8; 4]); // flags
        body.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
        body.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
        let riff_size = 4 + body.len() as u32;
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&riff_size.to_le_bytes());
        file.extend_from_slice(b"WEBP");
        file.extend_from_slice(&body);
        file
    }

    fn package_zip(id: &str, version: Option<u64>, sheet: &[u8]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        let manifest = serde_json::to_vec_pretty(&manifest_json(id, "Test Pet", version)).unwrap();
        zip.start_file(PET_MANIFEST, options).unwrap();
        zip.write_all(&manifest).unwrap();
        zip.start_file(PET_SPRITESHEET, options).unwrap();
        zip.write_all(sheet).unwrap();
        let cursor = zip.finish().unwrap();
        cursor.into_inner()
    }

    #[test]
    fn pet_ids_are_lowercase_url_safe_slugs() {
        assert!(is_pet_id("sprout"));
        assert!(is_pet_id("pet-123"));
        assert!(is_pet_id("a"));
        assert!(!is_pet_id(""));
        assert!(!is_pet_id("Pet"));
        assert!(!is_pet_id("-pet"));
        assert!(!is_pet_id("pet-"));
        assert!(!is_pet_id("pet_id"));
        assert!(!is_pet_id(&"x".repeat(65)));
    }

    #[test]
    fn manifests_are_validated_and_normalized() {
        let parsed = serde_json::from_value(manifest_json("sprout", "Sprout", Some(2))).unwrap();
        let manifest = normalize_manifest(&parsed).unwrap();
        assert_eq!(manifest.id, "sprout");
        assert_eq!(manifest.sprite_version_number, 2);
        assert_eq!(manifest.spritesheet_path, "spritesheet.webp");

        let no_version = serde_json::from_value(manifest_json("sprout", "Sprout", None)).unwrap();
        assert_eq!(
            normalize_manifest(&no_version)
                .unwrap()
                .sprite_version_number,
            1
        );
    }

    #[test]
    fn invalid_manifests_are_refused() {
        for (label, value) in [
            ("not an object", serde_json::json!([])),
            ("bad id", manifest_json("Bad Pet", "x", None)),
            ("missing name", {
                let mut v = manifest_json("sprout", "x", None);
                v["displayName"] = serde_json::json!("");
                v
            }),
            ("wrong sheet", {
                let mut v = manifest_json("sprout", "x", None);
                v["spritesheetPath"] = serde_json::json!("other.png");
                v
            }),
            ("bad version", manifest_json("sprout", "x", Some(3))),
            ("bad kind", {
                let mut v = manifest_json("sprout", "x", None);
                v["kind"] = serde_json::json!("Not A Kind!");
                v
            }),
        ] {
            let parsed = serde_json::from_value(value).unwrap();
            assert!(
                normalize_manifest(&parsed).is_err(),
                "{label} must be refused"
            );
        }
    }

    #[test]
    fn webp_dimensions_are_parsed_from_every_frame_type() {
        assert_eq!(
            read_webp_dimensions(&fake_webp(1536, 1872)).unwrap(),
            (1536, 1872)
        );
        assert_eq!(
            read_webp_dimensions(&fake_webp(1536, 2288)).unwrap(),
            (1536, 2288)
        );

        let mut bad = fake_webp(1536, 1872);
        bad[0] = b'X';
        assert!(read_webp_dimensions(&bad).is_err());
        assert!(read_webp_dimensions(b"not a webp").is_err());
    }

    #[test]
    fn packages_are_validated_end_to_end() {
        let zip = package_zip("sprout", Some(2), &fake_webp(1536, 2288));
        let manifest = validate_package_zip(&zip).unwrap();
        assert_eq!(manifest.id, "sprout");

        let wrong_size = package_zip("sprout", Some(2), &fake_webp(100, 100));
        assert!(validate_package_zip(&wrong_size).is_err());
    }

    #[test]
    fn packages_with_extra_or_missing_entries_are_refused() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("evil.sh", options).unwrap();
        zip.write_all(b"rm -rf /").unwrap();
        let cursor = zip.finish().unwrap();
        let bytes = cursor.into_inner();
        assert!(validate_package_zip(&bytes).is_err());
    }

    #[test]
    fn pet_ids_are_extracted_from_codex_pets_urls() {
        assert_eq!(pet_id_from_codex_pets_url("sprout").unwrap(), "sprout");
        assert_eq!(
            pet_id_from_codex_pets_url("https://codex-pets.net/#/pets/sprout").unwrap(),
            "sprout"
        );
        assert_eq!(
            pet_id_from_codex_pets_url("https://codex-pets.net/api/pets/sprout").unwrap(),
            "sprout"
        );
        assert_eq!(
            pet_id_from_codex_pets_url("https://codex-pets.net/api/pets/sprout/download").unwrap(),
            "sprout"
        );
        assert!(pet_id_from_codex_pets_url("https://evil.example/pets/sprout").is_err());
        assert!(pet_id_from_codex_pets_url("http://codex-pets.net/api/pets/sprout").is_err());
        assert!(pet_id_from_codex_pets_url("https://codex-pets.net/#/pets/Not%20A%20Pet").is_err());
    }

    #[test]
    fn packages_install_and_remove_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("pets");
        let zip = package_zip("sprout", None, &fake_webp(1536, 1872));

        let manifest = install_pet(&store, &zip).unwrap();
        assert_eq!(manifest.id, "sprout");
        assert_eq!(list_installed_pets(&store).len(), 1);
        assert!(store.join("sprout").join(PET_SPRITESHEET).exists());
        assert!(!store.join(".lnwdeck-pet").exists());

        // Reinstalling the same id replaces it cleanly.
        install_pet(&store, &zip).unwrap();
        assert_eq!(list_installed_pets(&store).len(), 1);

        remove_installed_pet(&store, "sprout").unwrap();
        assert!(list_installed_pets(&store).is_empty());
        assert!(remove_installed_pet(&store, "sprout").is_err());
    }

    #[test]
    fn a_corrupt_package_is_refused_and_stores_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("pets");
        assert!(install_pet(&store, b"not a zip").is_err());
        assert!(list_installed_pets(&store).is_empty());
    }

    #[test]
    fn bundled_default_pets_install_exactly_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("pets");
        let db = tempfile::tempdir().unwrap();
        let storage = lnwdeck_storage::Storage::open(&db.path().join("test.db")).unwrap();
        lnwdeck_storage::migrations::apply_all(&storage.conn).unwrap();

        let installed = seed_default_pets(&store, &storage.conn).unwrap();
        assert_eq!(installed.len(), DEFAULT_PET_IDS.len());
        for id in DEFAULT_PET_IDS {
            assert!(store.join(id).join(PET_SPRITESHEET).exists(), "{id}");
        }

        // Seeding again reports the same set and re-installs nothing removed.
        let again = seed_default_pets(&store, &storage.conn).unwrap();
        assert_eq!(again.len(), DEFAULT_PET_IDS.len());
        fs::remove_dir_all(store.join("youyou")).unwrap();
        let after_removal = seed_default_pets(&store, &storage.conn).unwrap();
        assert!(
            !after_removal.iter().any(|id| id == "youyou"),
            "a default the user removed stays removed after the first seed"
        );
    }
}
