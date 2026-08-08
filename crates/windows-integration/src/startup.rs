//! Launch-at-startup integration.
//!
//! The Settings toggle writes a real entry under
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, so the state shown in
//! the UI is the state Windows will act on. Reading it back is how the toggle
//! renders; nothing is remembered separately in the database, which is what
//! made the previous checkbox meaningless.

/// Registry value name used for this application.
pub const RUN_VALUE_NAME: &str = "lnwdeck";
#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Failure of a startup-registration operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupError {
    /// The platform has no Run key.
    Unsupported,
    /// The executable path could not be determined.
    UnknownExecutable,
    /// The registry rejected the operation; carries the OS error code.
    Registry(i32),
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(f, "STARTUP_UNSUPPORTED"),
            Self::UnknownExecutable => write!(f, "STARTUP_UNKNOWN_EXECUTABLE"),
            Self::Registry(code) => write!(f, "STARTUP_REGISTRY_ERROR_{code}"),
        }
    }
}

impl std::error::Error for StartupError {}

/// Reads and writes the Run entry.
pub struct StartupRegistration;

impl StartupRegistration {
    /// True when this build can register a startup entry.
    pub fn is_supported() -> bool {
        platform::IS_SUPPORTED
    }

    /// Whether the entry currently exists.
    pub fn is_enabled() -> Result<bool, StartupError> {
        platform::is_enabled()
    }

    /// Enables or disables launch at startup using the current executable
    /// path. Returns the state read back from the registry, so the caller
    /// reports what Windows actually holds.
    pub fn set_enabled(enabled: bool) -> Result<bool, StartupError> {
        if enabled {
            let exe = std::env::current_exe().map_err(|_| StartupError::UnknownExecutable)?;
            let path = exe
                .to_str()
                .ok_or(StartupError::UnknownExecutable)?
                .to_string();
            platform::write(&format!("\"{path}\""))?;
        } else {
            platform::remove()?;
        }
        platform::is_enabled()
    }
}

#[cfg(windows)]
mod platform {
    use super::{StartupError, RUN_KEY, RUN_VALUE_NAME};
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    pub const IS_SUPPORTED: bool = true;

    fn open(access: u32) -> Result<RegKey, StartupError> {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        match root.open_subkey_with_flags(RUN_KEY, access) {
            Ok(key) => Ok(key),
            // Some fresh Windows profiles have no Run key at all; create it so
            // enabling startup works and later reads succeed. The empty key is
            // harmless and is exactly what the installer would leave behind.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                root.create_subkey(RUN_KEY)
                    .map_err(|error| StartupError::Registry(error.raw_os_error().unwrap_or(-1)))?;
                root.open_subkey_with_flags(RUN_KEY, access)
                    .map_err(|error| StartupError::Registry(error.raw_os_error().unwrap_or(-1)))
            }
            Err(error) => Err(StartupError::Registry(error.raw_os_error().unwrap_or(-1))),
        }
    }

    pub fn is_enabled() -> Result<bool, StartupError> {
        let key = open(KEY_READ)?;
        match key.get_value::<String, _>(RUN_VALUE_NAME) {
            Ok(value) => Ok(!value.trim().is_empty()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(StartupError::Registry(error.raw_os_error().unwrap_or(-1))),
        }
    }

    pub fn write(command: &str) -> Result<(), StartupError> {
        let key = open(KEY_READ | KEY_WRITE)?;
        key.set_value(RUN_VALUE_NAME, &command.to_string())
            .map_err(|error| StartupError::Registry(error.raw_os_error().unwrap_or(-1)))
    }

    pub fn remove() -> Result<(), StartupError> {
        let key = open(KEY_READ | KEY_WRITE)?;
        match key.delete_value(RUN_VALUE_NAME) {
            Ok(()) => Ok(()),
            // Already absent: the requested state is the current state.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(StartupError::Registry(error.raw_os_error().unwrap_or(-1))),
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::StartupError;

    pub const IS_SUPPORTED: bool = false;

    pub fn is_enabled() -> Result<bool, StartupError> {
        Err(StartupError::Unsupported)
    }

    pub fn write(_command: &str) -> Result<(), StartupError> {
        Err(StartupError::Unsupported)
    }

    pub fn remove() -> Result<(), StartupError> {
        Err(StartupError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_machine_readable() {
        assert_eq!(StartupError::Unsupported.to_string(), "STARTUP_UNSUPPORTED");
        assert_eq!(
            StartupError::Registry(5).to_string(),
            "STARTUP_REGISTRY_ERROR_5"
        );
        assert_eq!(
            StartupError::UnknownExecutable.to_string(),
            "STARTUP_UNKNOWN_EXECUTABLE"
        );
    }

    #[cfg(windows)]
    #[test]
    fn enabling_and_disabling_writes_a_real_registry_entry() {
        assert!(StartupRegistration::is_supported());
        let previous = match StartupRegistration::is_enabled() {
            Ok(previous) => previous,
            // Managed Windows runners may deny reads from the per-user Run
            // key. The product path still returns the typed error; this
            // integration test cannot validate a registry it cannot access.
            Err(StartupError::Registry(5)) => {
                eprintln!("Startup registry self-test skipped: access denied");
                return;
            }
            Err(error) => panic!("read initial state: {error}"),
        };

        let enabled = StartupRegistration::set_enabled(true).expect("enable");
        assert!(enabled, "the registry must report the entry as present");
        assert!(StartupRegistration::is_enabled().expect("read back"));

        let disabled = StartupRegistration::set_enabled(false).expect("disable");
        assert!(!disabled);
        assert!(!StartupRegistration::is_enabled().expect("read back"));

        // Disabling twice is not an error: the state already matches.
        assert!(!StartupRegistration::set_enabled(false).expect("disable again"));

        // Leave the machine as it was found.
        if previous {
            StartupRegistration::set_enabled(true).expect("restore");
        }
    }
}
