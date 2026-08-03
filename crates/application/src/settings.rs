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
pub const KEY_RETENTION_DAYS: &str = "retention_days";

/// Refresh intervals the UI offers. Zero disables the background loop.
pub const ALLOWED_REFRESH_INTERVALS: &[u64] = &[0, 30, 60, 300, 900, 3600];
/// Themes the UI offers.
pub const ALLOWED_THEMES: &[&str] = &["dark", "light", "system"];
/// Retention windows the UI offers, in days.
pub const ALLOWED_RETENTION_DAYS: &[u64] = &[7, 30, 90, 365, 0];

/// Defaults used when a key has never been written.
const DEFAULT_REFRESH_INTERVAL: u64 = 300;
const DEFAULT_RETENTION_DAYS: u64 = 90;
const DEFAULT_WIDGET_OPACITY: f64 = 1.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub launch_at_startup: bool,
    pub theme: String,
    pub refresh_interval_seconds: u64,
    pub auto_update_check: bool,
    pub widget_opacity: f64,
    pub widget_locked: bool,
    pub widget_visible: bool,
    pub retention_days: u64,
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
            retention_days: DEFAULT_RETENTION_DAYS,
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
            retention_days: read(KEY_RETENTION_DAYS)?
                .and_then(|value| value.parse().ok())
                .filter(|value| ALLOWED_RETENTION_DAYS.contains(value))
                .unwrap_or(defaults.retention_days),
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
        write(KEY_RETENTION_DAYS, settings.retention_days.to_string())?;

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
            retention_days: 30,
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
