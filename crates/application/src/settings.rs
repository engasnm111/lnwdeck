//! Application settings.
//!
//! Every control on the Settings page maps to a key in `app_settings`. Values
//! are validated on write and read back from storage, so a control can only
//! show a state the backend actually holds. Unset keys fall back to a
//! documented default rather than to whatever the UI happened to render.

use lnwdeck_storage::repositories::AppSettingsRepository;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub const KEY_LAUNCH_AT_STARTUP: &str = "launch_at_startup";
pub const KEY_THEME: &str = "theme";
pub const KEY_REFRESH_INTERVAL: &str = "refresh_interval_seconds";
pub const KEY_AUTO_UPDATE: &str = "auto_update_check";
pub const KEY_WIDGET_OPACITY: &str = "widget_opacity";
pub const KEY_WIDGET_LOCKED: &str = "widget_locked";
pub const KEY_WIDGET_VISIBLE: &str = "widget_visible";
pub const KEY_WIDGET_PROVIDERS: &str = "widget_providers";
pub const KEY_WIDGET_VIEW: &str = "widget_view";
pub const KEY_WIDGET_PET: &str = "widget_pet_id";
pub const KEY_WIDGET_SIZE: &str = "widget_size_preset";
pub const KEY_RETENTION_DAYS: &str = "retention_days";
pub const KEY_PET_VISIBLE: &str = "pet_visible";
pub const KEY_PET_CHARACTER: &str = "pet_character";
pub const KEY_PET_SPEED: &str = "pet_speed";
pub const KEY_PET_OPACITY: &str = "pet_opacity";
pub const KEY_PET_AUTO_SLEEP: &str = "pet_auto_sleep";
pub const KEY_PET_SIZE: &str = "pet_size_preset";

/// Refresh intervals the UI offers. Zero disables the background loop.
pub const ALLOWED_REFRESH_INTERVALS: &[u64] = &[0, 30, 60, 300, 900, 3600];
/// Themes the UI offers.
pub const ALLOWED_THEMES: &[&str] = &["dark", "light", "system"];
/// Retention windows the UI offers, in days.
pub const ALLOWED_RETENTION_DAYS: &[u64] = &[7, 30, 90, 365, 0];
/// Widget layouts: horizontal bars, compact rings, or the animated pet.
pub const ALLOWED_WIDGET_VIEWS: &[&str] = &["bars", "rings", "pet"];
/// Widget size presets: the widget window is fixed-size, never user-resized.
pub const ALLOWED_WIDGET_SIZES: &[&str] = &["small", "medium", "large"];
/// Desktop pet walk speeds.
pub const ALLOWED_PET_SPEEDS: &[&str] = &["slow", "normal", "fast"];
/// Desktop pet size presets: the pet window is fixed-size, never resized by
/// the user; the preset scales both the window and the sprite.
pub const ALLOWED_PET_SIZES: &[&str] = &["small", "medium", "large"];
/// Built-in desktop pet characters.
pub const BUILTIN_PET_CHARACTERS: &[&str] = &["robot", "cat", "ghost", "dragon", "crab", "blob"];

/// Defaults used when a key has never been written.
const DEFAULT_REFRESH_INTERVAL: u64 = 300;
const DEFAULT_RETENTION_DAYS: u64 = 90;
const DEFAULT_WIDGET_OPACITY: f64 = 1.0;
const DEFAULT_WIDGET_SIZE: &str = "medium";
const DEFAULT_PET_OPACITY: f64 = 1.0;
const DEFAULT_PET_CHARACTER: &str = "robot";
const DEFAULT_PET_SPEED: &str = "normal";
const DEFAULT_PET_SIZE: &str = "medium";
/// Upper bound on pinned widget providers, matching the built-in adapter count
/// with room to spare.
const MAX_WIDGET_PROVIDERS: usize = 32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub launch_at_startup: bool,
    pub theme: String,
    pub refresh_interval_seconds: u64,
    pub auto_update_check: bool,
    pub widget_opacity: f64,
    pub widget_locked: bool,
    pub widget_visible: bool,
    pub widget_size: String,
    pub retention_days: u64,
    pub pet_visible: bool,
    pub pet_character: String,
    pub pet_speed: String,
    pub pet_opacity: f64,
    pub pet_auto_sleep: bool,
    pub pet_size: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            launch_at_startup: false,
            theme: "system".to_string(),
            refresh_interval_seconds: DEFAULT_REFRESH_INTERVAL,
            auto_update_check: true,
            widget_opacity: DEFAULT_WIDGET_OPACITY,
            widget_locked: false,
            widget_visible: false,
            widget_size: DEFAULT_WIDGET_SIZE.to_string(),
            retention_days: DEFAULT_RETENTION_DAYS,
            pet_visible: false,
            pet_character: DEFAULT_PET_CHARACTER.to_string(),
            pet_speed: DEFAULT_PET_SPEED.to_string(),
            pet_opacity: DEFAULT_PET_OPACITY,
            pet_auto_sleep: true,
            pet_size: DEFAULT_PET_SIZE.to_string(),
        }
    }
}

/// Why a settings write was refused. Shown to the user; the stored value is
/// left untouched.
#[derive(Debug, PartialEq)]
pub enum SettingsError {
    InvalidTheme(String),
    InvalidInterval(u64),
    InvalidOpacity(f64),
    InvalidRetention(u64),
    InvalidProviderId,
    InvalidWidgetView(String),
    InvalidPetId(String),
    TooManyProviders(usize),
    Storage(String),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTheme(value) => write!(f, "unknown theme: {value}"),
            Self::InvalidInterval(value) => {
                write!(f, "unsupported refresh interval: {value} seconds")
            }
            Self::InvalidOpacity(value) => {
                write!(f, "widget opacity must be between 0.1 and 1.0, got {value}")
            }
            Self::InvalidRetention(value) => {
                write!(f, "unsupported retention window: {value} days")
            }
            Self::InvalidProviderId => write!(f, "a provider id must not be empty"),
            Self::InvalidWidgetView(value) => write!(f, "unknown widget layout: {value}"),
            Self::InvalidPetId(value) => write!(f, "unknown widget pet id: {value}"),
            Self::TooManyProviders(count) => {
                write!(f, "too many pinned providers: {count}")
            }
            Self::Storage(error) => write!(f, "storage error: {error}"),
        }
    }
}

impl std::error::Error for SettingsError {}

pub struct SettingsService;

impl SettingsService {
    /// Reads the current settings, falling back to documented defaults.
    pub fn load(conn: &Connection) -> Result<AppSettings, SettingsError> {
        let repo = AppSettingsRepository::new(conn);
        let read = |key: &str| -> Result<Option<String>, SettingsError> {
            repo.get(key)
                .map_err(|e| SettingsError::Storage(e.to_string()))
        };
        let defaults = AppSettings::default();
        Ok(AppSettings {
            launch_at_startup: read(KEY_LAUNCH_AT_STARTUP)?
                .map(|value| value == "true")
                .unwrap_or(defaults.launch_at_startup),
            theme: read(KEY_THEME)?
                .filter(|value| ALLOWED_THEMES.contains(&value.as_str()))
                .unwrap_or(defaults.theme),
            refresh_interval_seconds: read(KEY_REFRESH_INTERVAL)?
                .and_then(|value| value.parse().ok())
                .filter(|value| ALLOWED_REFRESH_INTERVALS.contains(value))
                .unwrap_or(defaults.refresh_interval_seconds),
            auto_update_check: read(KEY_AUTO_UPDATE)?
                .map(|value| value == "true")
                .unwrap_or(defaults.auto_update_check),
            widget_opacity: read(KEY_WIDGET_OPACITY)?
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| (0.1..=1.0).contains(value))
                .unwrap_or(defaults.widget_opacity),
            widget_locked: read(KEY_WIDGET_LOCKED)?
                .map(|value| value == "true")
                .unwrap_or(defaults.widget_locked),
            widget_visible: read(KEY_WIDGET_VISIBLE)?
                .map(|value| value == "true")
                .unwrap_or(defaults.widget_visible),
            widget_size: read(KEY_WIDGET_SIZE)?
                .filter(|value| ALLOWED_WIDGET_SIZES.contains(&value.as_str()))
                .unwrap_or_else(|| defaults.widget_size.clone()),
            retention_days: read(KEY_RETENTION_DAYS)?
                .and_then(|value| value.parse().ok())
                .filter(|value| ALLOWED_RETENTION_DAYS.contains(value))
                .unwrap_or(defaults.retention_days),
            pet_visible: read(KEY_PET_VISIBLE)?
                .map(|value| value == "true")
                .unwrap_or(defaults.pet_visible),
            pet_character: read(KEY_PET_CHARACTER)?
                .filter(|value| {
                    BUILTIN_PET_CHARACTERS.contains(&value.as_str()) || is_pet_id_slug(value)
                })
                .unwrap_or_else(|| defaults.pet_character.clone()),
            pet_speed: read(KEY_PET_SPEED)?
                .filter(|value| ALLOWED_PET_SPEEDS.contains(&value.as_str()))
                .unwrap_or_else(|| defaults.pet_speed.clone()),
            pet_opacity: read(KEY_PET_OPACITY)?
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| (0.1..=1.0).contains(value))
                .unwrap_or(defaults.pet_opacity),
            pet_auto_sleep: read(KEY_PET_AUTO_SLEEP)?
                .map(|value| value == "true")
                .unwrap_or(defaults.pet_auto_sleep),
            pet_size: read(KEY_PET_SIZE)?
                .filter(|value| ALLOWED_PET_SIZES.contains(&value.as_str()))
                .unwrap_or_else(|| defaults.pet_size.clone()),
        })
    }

    /// Validates and persists a full settings object, then returns what was
    /// actually stored. A rejected value aborts the write: the caller reports
    /// the error and the previous settings stay in place.
    pub fn save(conn: &Connection, settings: &AppSettings) -> Result<AppSettings, SettingsError> {
        if !ALLOWED_THEMES.contains(&settings.theme.as_str()) {
            return Err(SettingsError::InvalidTheme(settings.theme.clone()));
        }
        if !ALLOWED_REFRESH_INTERVALS.contains(&settings.refresh_interval_seconds) {
            return Err(SettingsError::InvalidInterval(
                settings.refresh_interval_seconds,
            ));
        }
        if !(0.1..=1.0).contains(&settings.widget_opacity) {
            return Err(SettingsError::InvalidOpacity(settings.widget_opacity));
        }
        if !ALLOWED_RETENTION_DAYS.contains(&settings.retention_days) {
            return Err(SettingsError::InvalidRetention(settings.retention_days));
        }

        let repo = AppSettingsRepository::new(conn);
        let write = |key: &str, value: String| -> Result<(), SettingsError> {
            repo.set(key, &value)
                .map_err(|e| SettingsError::Storage(e.to_string()))
        };
        write(
            KEY_LAUNCH_AT_STARTUP,
            settings.launch_at_startup.to_string(),
        )?;
        write(KEY_THEME, settings.theme.clone())?;
        write(
            KEY_REFRESH_INTERVAL,
            settings.refresh_interval_seconds.to_string(),
        )?;
        write(KEY_AUTO_UPDATE, settings.auto_update_check.to_string())?;
        write(
            KEY_WIDGET_OPACITY,
            format!("{:.2}", settings.widget_opacity),
        )?;
        write(KEY_WIDGET_LOCKED, settings.widget_locked.to_string())?;
        write(KEY_WIDGET_VISIBLE, settings.widget_visible.to_string())?;
        write(KEY_WIDGET_SIZE, settings.widget_size.clone())?;
        write(KEY_RETENTION_DAYS, settings.retention_days.to_string())?;
        write(KEY_PET_VISIBLE, settings.pet_visible.to_string())?;
        write(KEY_PET_CHARACTER, settings.pet_character.clone())?;
        write(KEY_PET_SPEED, settings.pet_speed.clone())?;
        write(KEY_PET_OPACITY, format!("{:.2}", settings.pet_opacity))?;
        write(KEY_PET_AUTO_SLEEP, settings.pet_auto_sleep.to_string())?;
        write(KEY_PET_SIZE, settings.pet_size.clone())?;

        // Read back so the caller reports stored state, not the request.
        Self::load(conn)
    }

    /// Persists one widget setting without touching the rest.
    pub fn set_widget_opacity(conn: &Connection, opacity: f64) -> Result<f64, SettingsError> {
        if !(0.1..=1.0).contains(&opacity) {
            return Err(SettingsError::InvalidOpacity(opacity));
        }
        AppSettingsRepository::new(conn)
            .set(KEY_WIDGET_OPACITY, &format!("{opacity:.2}"))
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(Self::load(conn)?.widget_opacity)
    }

    pub fn set_widget_locked(conn: &Connection, locked: bool) -> Result<bool, SettingsError> {
        AppSettingsRepository::new(conn)
            .set(KEY_WIDGET_LOCKED, &locked.to_string())
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(Self::load(conn)?.widget_locked)
    }

    pub fn set_widget_visible(conn: &Connection, visible: bool) -> Result<bool, SettingsError> {
        AppSettingsRepository::new(conn)
            .set(KEY_WIDGET_VISIBLE, &visible.to_string())
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(Self::load(conn)?.widget_visible)
    }

    /// The widget size preset, defaulting to `medium`.
    pub fn widget_size_preset(conn: &Connection) -> Result<String, SettingsError> {
        let stored = AppSettingsRepository::new(conn)
            .get(KEY_WIDGET_SIZE)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(stored
            .filter(|value| ALLOWED_WIDGET_SIZES.contains(&value.as_str()))
            .unwrap_or_else(|| DEFAULT_WIDGET_SIZE.to_string()))
    }

    /// Stores the widget size preset and returns what was stored.
    pub fn set_widget_size_preset(
        conn: &Connection,
        preset: &str,
    ) -> Result<String, SettingsError> {
        if !ALLOWED_WIDGET_SIZES.contains(&preset) {
            return Err(SettingsError::InvalidWidgetView(preset.to_string()));
        }
        AppSettingsRepository::new(conn)
            .set(KEY_WIDGET_SIZE, preset)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Self::widget_size_preset(conn)
    }

    /// Provider ids the widget is pinned to.
    ///
    /// An empty list means "every provider that reported data"; it is not a
    /// filter that hides everything, because a widget showing nothing would be
    /// indistinguishable from a widget with no data.
    pub fn widget_providers(conn: &Connection) -> Result<Vec<String>, SettingsError> {
        let raw = AppSettingsRepository::new(conn)
            .get(KEY_WIDGET_PROVIDERS)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        let Some(raw) = raw else {
            return Ok(Vec::new());
        };
        // A corrupt value is treated as "no selection" rather than as an error:
        // the widget then shows every provider, which is the safe default.
        Ok(serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default())
    }

    /// Widget layout, defaulting to bars.
    pub fn widget_view(conn: &Connection) -> Result<String, SettingsError> {
        let stored = AppSettingsRepository::new(conn)
            .get(KEY_WIDGET_VIEW)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(stored
            .filter(|value| ALLOWED_WIDGET_VIEWS.contains(&value.as_str()))
            .unwrap_or_else(|| "bars".to_string()))
    }

    /// Stores the widget layout and returns what was stored.
    pub fn set_widget_view(conn: &Connection, view: &str) -> Result<String, SettingsError> {
        if !ALLOWED_WIDGET_VIEWS.contains(&view) {
            return Err(SettingsError::InvalidWidgetView(view.to_string()));
        }
        AppSettingsRepository::new(conn)
            .set(KEY_WIDGET_VIEW, view)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Self::widget_view(conn)
    }

    /// Community pet selected for the widget pet layout.
    ///
    /// Empty means the built-in robot. Stored ids are validated as lowercase
    /// URL-safe slugs; whether the id is actually installed is checked by the
    /// pet store command layer, which owns the pets directory.
    pub fn widget_pet_id(conn: &Connection) -> Result<String, SettingsError> {
        let stored = AppSettingsRepository::new(conn)
            .get(KEY_WIDGET_PET)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(stored
            .filter(|value| is_pet_id_slug(value))
            .unwrap_or_default())
    }

    /// Stores the selected community pet id and returns what was stored.
    pub fn set_widget_pet_id(conn: &Connection, pet_id: &str) -> Result<String, SettingsError> {
        let trimmed = pet_id.trim();
        if !trimmed.is_empty() && !is_pet_id_slug(trimmed) {
            return Err(SettingsError::InvalidPetId(trimmed.to_string()));
        }
        AppSettingsRepository::new(conn)
            .set(KEY_WIDGET_PET, trimmed)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Self::widget_pet_id(conn)
    }

    /// Stores the provider selection and returns what was stored.
    ///
    /// Ids are trimmed and deduplicated; an entry that is empty after trimming
    /// is rejected so a blank checkbox cannot silently hide a provider.
    pub fn set_widget_providers(
        conn: &Connection,
        providers: &[String],
    ) -> Result<Vec<String>, SettingsError> {
        if providers.len() > MAX_WIDGET_PROVIDERS {
            return Err(SettingsError::TooManyProviders(providers.len()));
        }
        let mut cleaned: Vec<String> = Vec::with_capacity(providers.len());
        for provider in providers {
            let trimmed = provider.trim();
            if trimmed.is_empty() {
                return Err(SettingsError::InvalidProviderId);
            }
            if !cleaned.iter().any(|existing| existing == trimmed) {
                cleaned.push(trimmed.to_string());
            }
        }
        let encoded =
            serde_json::to_string(&cleaned).map_err(|e| SettingsError::Storage(e.to_string()))?;
        AppSettingsRepository::new(conn)
            .set(KEY_WIDGET_PROVIDERS, &encoded)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Self::widget_providers(conn)
    }

    // ── Desktop pet settings ──────────────────────────────────────────────

    pub fn set_pet_visible(conn: &Connection, visible: bool) -> Result<bool, SettingsError> {
        AppSettingsRepository::new(conn)
            .set(KEY_PET_VISIBLE, &visible.to_string())
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(Self::load(conn)?.pet_visible)
    }

    pub fn set_pet_character(conn: &Connection, character: &str) -> Result<String, SettingsError> {
        let trimmed = character.trim();
        if trimmed.is_empty()
            || !(BUILTIN_PET_CHARACTERS.contains(&trimmed) || is_pet_id_slug(trimmed))
        {
            return Err(SettingsError::InvalidPetId(trimmed.to_string()));
        }
        AppSettingsRepository::new(conn)
            .set(KEY_PET_CHARACTER, trimmed)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(Self::load(conn)?.pet_character)
    }

    pub fn set_pet_speed(conn: &Connection, speed: &str) -> Result<String, SettingsError> {
        if !ALLOWED_PET_SPEEDS.contains(&speed) {
            return Err(SettingsError::InvalidWidgetView(speed.to_string()));
        }
        AppSettingsRepository::new(conn)
            .set(KEY_PET_SPEED, speed)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(Self::load(conn)?.pet_speed)
    }

    pub fn set_pet_opacity(conn: &Connection, opacity: f64) -> Result<f64, SettingsError> {
        if !(0.1..=1.0).contains(&opacity) {
            return Err(SettingsError::InvalidOpacity(opacity));
        }
        AppSettingsRepository::new(conn)
            .set(KEY_PET_OPACITY, &format!("{opacity:.2}"))
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(Self::load(conn)?.pet_opacity)
    }

    pub fn set_pet_auto_sleep(conn: &Connection, auto_sleep: bool) -> Result<bool, SettingsError> {
        AppSettingsRepository::new(conn)
            .set(KEY_PET_AUTO_SLEEP, &auto_sleep.to_string())
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(Self::load(conn)?.pet_auto_sleep)
    }

    /// The pet size preset, defaulting to `medium`.
    pub fn pet_size_preset(conn: &Connection) -> Result<String, SettingsError> {
        let stored = AppSettingsRepository::new(conn)
            .get(KEY_PET_SIZE)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Ok(stored
            .filter(|value| ALLOWED_PET_SIZES.contains(&value.as_str()))
            .unwrap_or_else(|| DEFAULT_PET_SIZE.to_string()))
    }

    /// Stores the pet size preset and returns what was stored.
    pub fn set_pet_size_preset(conn: &Connection, preset: &str) -> Result<String, SettingsError> {
        if !ALLOWED_PET_SIZES.contains(&preset) {
            return Err(SettingsError::InvalidWidgetView(preset.to_string()));
        }
        AppSettingsRepository::new(conn)
            .set(KEY_PET_SIZE, preset)
            .map_err(|e| SettingsError::Storage(e.to_string()))?;
        Self::pet_size_preset(conn)
    }
}

/// A lowercase URL-safe slug, mirroring the codex-pets.net pet id format.
fn is_pet_id_slug(value: &str) -> bool {
    if value.is_empty() || value.len() > 64 {
        return false;
    }
    let bytes = value.as_bytes();
    let alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    if !alnum(bytes[0]) || !alnum(bytes[bytes.len() - 1]) {
        return false;
    }
    bytes.iter().all(|b| alnum(*b) || *b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use lnwdeck_storage::{migrations::apply_all, Storage};
    use tempfile::tempdir;

    fn open_db() -> Storage {
        let dir = tempdir().expect("temp dir");
        let dir = std::mem::ManuallyDrop::new(dir);
        let storage = Storage::open(&dir.path().join("test.db")).expect("open");
        apply_all(&storage.conn).expect("migrate");
        storage
    }

    #[test]
    fn unset_settings_report_documented_defaults() {
        let storage = open_db();
        let settings = SettingsService::load(&storage.conn).expect("load");
        assert_eq!(settings, AppSettings::default());
        assert_eq!(settings.refresh_interval_seconds, 300);
        assert_eq!(settings.theme, "system");
        assert!(!settings.launch_at_startup);
        assert!(settings.auto_update_check);
    }

    #[test]
    fn save_then_load_roundtrips_every_field() {
        let storage = open_db();
        let desired = AppSettings {
            launch_at_startup: true,
            theme: "light".to_string(),
            refresh_interval_seconds: 60,
            auto_update_check: false,
            widget_opacity: 0.7,
            widget_locked: true,
            widget_visible: true,
            widget_size: "large".to_string(),
            retention_days: 30,
            pet_visible: true,
            pet_character: "cat".to_string(),
            pet_speed: "fast".to_string(),
            pet_opacity: 0.8,
            pet_auto_sleep: false,
            pet_size: "large".to_string(),
        };

        let stored = SettingsService::save(&storage.conn, &desired).expect("save");
        assert_eq!(stored, desired, "save must report what was stored");
        assert_eq!(
            SettingsService::load(&storage.conn).expect("load"),
            desired,
            "a reload must show the same state"
        );
    }

    #[test]
    fn invalid_values_are_refused_and_leave_storage_untouched() {
        let storage = open_db();
        let good = AppSettings {
            theme: "dark".to_string(),
            refresh_interval_seconds: 30,
            ..AppSettings::default()
        };
        SettingsService::save(&storage.conn, &good).expect("save");

        let bad_theme = SettingsService::save(
            &storage.conn,
            &AppSettings {
                theme: "neon".to_string(),
                ..good.clone()
            },
        );
        assert_eq!(
            bad_theme,
            Err(SettingsError::InvalidTheme("neon".to_string()))
        );

        let bad_interval = SettingsService::save(
            &storage.conn,
            &AppSettings {
                refresh_interval_seconds: 7,
                ..good.clone()
            },
        );
        assert_eq!(bad_interval, Err(SettingsError::InvalidInterval(7)));

        let bad_opacity = SettingsService::save(
            &storage.conn,
            &AppSettings {
                widget_opacity: 2.0,
                ..good.clone()
            },
        );
        assert_eq!(bad_opacity, Err(SettingsError::InvalidOpacity(2.0)));

        let bad_retention = SettingsService::save(
            &storage.conn,
            &AppSettings {
                retention_days: 5,
                ..good.clone()
            },
        );
        assert_eq!(bad_retention, Err(SettingsError::InvalidRetention(5)));

        assert_eq!(
            SettingsService::load(&storage.conn).expect("load"),
            good,
            "a refused write must not change stored settings"
        );
    }

    #[test]
    fn corrupt_stored_values_fall_back_to_defaults() {
        let storage = open_db();
        let repo = AppSettingsRepository::new(&storage.conn);
        repo.set(KEY_THEME, "chartreuse").expect("write");
        repo.set(KEY_REFRESH_INTERVAL, "not-a-number")
            .expect("write");
        repo.set(KEY_WIDGET_OPACITY, "9.9").expect("write");

        let settings = SettingsService::load(&storage.conn).expect("load");
        assert_eq!(settings.theme, "system");
        assert_eq!(settings.refresh_interval_seconds, 300);
        assert_eq!(settings.widget_opacity, 1.0);
    }

    #[test]
    fn widget_setters_persist_independently() {
        let storage = open_db();
        assert_eq!(
            SettingsService::set_widget_opacity(&storage.conn, 0.4).expect("opacity"),
            0.4
        );
        assert!(SettingsService::set_widget_locked(&storage.conn, true).expect("locked"));
        assert!(SettingsService::set_widget_visible(&storage.conn, true).expect("visible"));

        let settings = SettingsService::load(&storage.conn).expect("load");
        assert_eq!(settings.widget_opacity, 0.4);
        assert!(settings.widget_locked);
        assert!(settings.widget_visible);
        assert_eq!(
            settings.refresh_interval_seconds, 300,
            "unrelated settings keep their value"
        );

        assert_eq!(
            SettingsService::set_widget_opacity(&storage.conn, 0.0),
            Err(SettingsError::InvalidOpacity(0.0))
        );
        assert_eq!(
            SettingsService::load(&storage.conn)
                .expect("load")
                .widget_opacity,
            0.4,
            "a refused opacity leaves the stored value alone"
        );
    }

    #[test]
    fn no_provider_selection_means_every_provider() {
        let storage = open_db();
        assert!(
            SettingsService::widget_providers(&storage.conn)
                .expect("read")
                .is_empty(),
            "an empty selection is the documented default"
        );
    }

    #[test]
    fn provider_selection_roundtrips_and_is_deduplicated() {
        let storage = open_db();
        let stored = SettingsService::set_widget_providers(
            &storage.conn,
            &[
                "anthropic_claude".to_string(),
                " openai_codex ".to_string(),
                "anthropic_claude".to_string(),
            ],
        )
        .expect("store");
        assert_eq!(
            stored,
            vec!["anthropic_claude".to_string(), "openai_codex".to_string()],
            "ids are trimmed and duplicates collapse"
        );
        assert_eq!(
            SettingsService::widget_providers(&storage.conn).expect("read"),
            stored,
            "a reload returns the stored selection"
        );
    }

    #[test]
    fn an_empty_provider_id_is_refused_and_changes_nothing() {
        let storage = open_db();
        SettingsService::set_widget_providers(&storage.conn, &["opencode".to_string()])
            .expect("store");

        assert_eq!(
            SettingsService::set_widget_providers(
                &storage.conn,
                &["opencode".to_string(), "   ".to_string()]
            ),
            Err(SettingsError::InvalidProviderId)
        );
        assert_eq!(
            SettingsService::widget_providers(&storage.conn).expect("read"),
            vec!["opencode".to_string()],
            "a refused write leaves the previous selection in place"
        );
    }

    #[test]
    fn an_absurd_selection_size_is_refused() {
        let storage = open_db();
        let many: Vec<String> = (0..64).map(|index| format!("provider_{index}")).collect();
        assert_eq!(
            SettingsService::set_widget_providers(&storage.conn, &many),
            Err(SettingsError::TooManyProviders(64))
        );
    }

    #[test]
    fn clearing_the_selection_restores_every_provider() {
        let storage = open_db();
        SettingsService::set_widget_providers(&storage.conn, &["opencode".to_string()])
            .expect("store");
        assert!(SettingsService::set_widget_providers(&storage.conn, &[])
            .expect("clear")
            .is_empty());
        assert!(SettingsService::widget_providers(&storage.conn)
            .expect("read")
            .is_empty());
    }

    #[test]
    fn a_corrupt_selection_falls_back_to_every_provider() {
        let storage = open_db();
        AppSettingsRepository::new(&storage.conn)
            .set(KEY_WIDGET_PROVIDERS, "not json")
            .expect("write");
        assert!(
            SettingsService::widget_providers(&storage.conn)
                .expect("read")
                .is_empty(),
            "a corrupt value must not hide every provider"
        );
    }

    #[test]
    fn widget_view_defaults_to_bars_and_rejects_anything_else() {
        let storage = open_db();
        assert_eq!(
            SettingsService::widget_view(&storage.conn).expect("read"),
            "bars"
        );
        assert_eq!(
            SettingsService::set_widget_view(&storage.conn, "rings").expect("store"),
            "rings"
        );
        assert_eq!(
            SettingsService::widget_view(&storage.conn).expect("read"),
            "rings"
        );
        assert_eq!(
            SettingsService::set_widget_view(&storage.conn, "hexagons"),
            Err(SettingsError::InvalidWidgetView("hexagons".to_string()))
        );
        assert_eq!(
            SettingsService::widget_view(&storage.conn).expect("read"),
            "rings",
            "a refused layout leaves the stored one in place"
        );
    }

    #[test]
    fn widget_view_accepts_the_pet_layout_and_roundtrips() {
        let storage = open_db();
        assert_eq!(
            SettingsService::set_widget_view(&storage.conn, "pet").expect("store"),
            "pet"
        );
        assert_eq!(
            SettingsService::widget_view(&storage.conn).expect("read"),
            "pet",
            "a reload returns the stored pet layout"
        );
        assert_eq!(
            SettingsService::set_widget_view(&storage.conn, "rings").expect("store"),
            "rings",
            "pet is one option among the allowed layouts, not a replacement"
        );
    }

    #[test]
    fn widget_pet_id_defaults_empty_and_roundtrips_slugs() {
        let storage = open_db();
        assert_eq!(
            SettingsService::widget_pet_id(&storage.conn).expect("read"),
            "",
            "no pet selected means the built-in robot"
        );
        assert_eq!(
            SettingsService::set_widget_pet_id(&storage.conn, "sprout").expect("store"),
            "sprout"
        );
        assert_eq!(
            SettingsService::widget_pet_id(&storage.conn).expect("read"),
            "sprout"
        );
        assert_eq!(
            SettingsService::set_widget_pet_id(&storage.conn, "").expect("clear"),
            "",
            "clearing restores the built-in robot"
        );
    }

    #[test]
    fn an_invalid_widget_pet_id_is_refused_and_changes_nothing() {
        let storage = open_db();
        SettingsService::set_widget_pet_id(&storage.conn, "sprout").expect("store");
        assert_eq!(
            SettingsService::set_widget_pet_id(&storage.conn, "Bad Pet!").unwrap_err(),
            SettingsError::InvalidPetId("Bad Pet!".to_string())
        );
        assert_eq!(
            SettingsService::widget_pet_id(&storage.conn).expect("read"),
            "sprout",
            "a refused pet id leaves the stored one in place"
        );
    }

    #[test]
    fn a_corrupt_widget_pet_id_falls_back_to_the_builtin() {
        let storage = open_db();
        AppSettingsRepository::new(&storage.conn)
            .set(KEY_WIDGET_PET, "not a pet id")
            .expect("write");
        assert_eq!(
            SettingsService::widget_pet_id(&storage.conn).expect("read"),
            ""
        );
    }

    #[test]
    fn a_corrupt_widget_view_falls_back_to_bars() {
        let storage = open_db();
        AppSettingsRepository::new(&storage.conn)
            .set(KEY_WIDGET_VIEW, "spirals")
            .expect("write");
        assert_eq!(
            SettingsService::widget_view(&storage.conn).expect("read"),
            "bars"
        );
    }

    #[test]
    fn zero_interval_disables_the_loop_and_is_allowed() {
        let storage = open_db();
        let stored = SettingsService::save(
            &storage.conn,
            &AppSettings {
                refresh_interval_seconds: 0,
                ..AppSettings::default()
            },
        )
        .expect("save");
        assert_eq!(stored.refresh_interval_seconds, 0);
    }
}
