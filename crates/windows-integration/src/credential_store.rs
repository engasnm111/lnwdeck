//! Windows Credential Manager storage for provider API keys.
//!
//! Keys the user enters for network providers are written to the Windows
//! Credential Manager under a per-provider target name and are never written
//! to the application database, to logs, or to any exported diagnostics. The
//! rest of the application only ever sees a [`CredentialState`], so a key
//! cannot leak through a read model.
//!
//! On platforms without the Credential Manager the store reports
//! `Unsupported` for every operation rather than silently falling back to an
//! insecure location.

use crate::credential::{CredentialState, CredentialStatus};

/// Prefix for every credential this application owns.
const TARGET_PREFIX: &str = "lnwdeck:provider:";

/// Failure of a credential operation. Carries no secret material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialError {
    /// No credential is stored for this provider.
    NotFound,
    /// The platform has no credential store.
    Unsupported,
    /// The operating system rejected the operation; carries the OS error code.
    Os(u32),
    /// The stored value was not valid UTF-8 text.
    Corrupt,
    /// The caller passed an empty provider id or secret.
    Invalid,
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "CREDENTIAL_NOT_FOUND"),
            Self::Unsupported => write!(f, "CREDENTIAL_STORE_UNSUPPORTED"),
            Self::Os(code) => write!(f, "CREDENTIAL_OS_ERROR_{code}"),
            Self::Corrupt => write!(f, "CREDENTIAL_CORRUPT"),
            Self::Invalid => write!(f, "CREDENTIAL_INVALID_ARGUMENT"),
        }
    }
}

impl std::error::Error for CredentialError {}

/// Full credential target name for a provider id.
pub fn target_name(provider_id: &str) -> String {
    format!("{TARGET_PREFIX}{provider_id}")
}

/// Stores, reads and deletes provider API keys.
pub struct CredentialStore;

impl CredentialStore {
    /// Writes (or replaces) the API key for a provider.
    pub fn set(provider_id: &str, secret: &str) -> Result<(), CredentialError> {
        if provider_id.trim().is_empty() || secret.is_empty() {
            return Err(CredentialError::Invalid);
        }
        platform::write(&target_name(provider_id), secret)
    }

    /// Reads the API key for a provider.
    ///
    /// The returned string is the secret itself: callers must pass it straight
    /// to the provider request and must never log, serialize, or store it.
    pub fn get(provider_id: &str) -> Result<String, CredentialError> {
        if provider_id.trim().is_empty() {
            return Err(CredentialError::Invalid);
        }
        platform::read(&target_name(provider_id))
    }

    /// Deletes the API key for a provider. Deleting a missing credential is
    /// reported as `NotFound` rather than silently succeeding.
    pub fn delete(provider_id: &str) -> Result<(), CredentialError> {
        if provider_id.trim().is_empty() {
            return Err(CredentialError::Invalid);
        }
        platform::delete(&target_name(provider_id))
    }

    /// Whether a credential exists, without returning it.
    pub fn state(provider_id: &str) -> CredentialState {
        match Self::get(provider_id) {
            Ok(secret) if !secret.is_empty() => CredentialState::Configured,
            Ok(_) => CredentialState::Missing,
            Err(CredentialError::NotFound) | Err(CredentialError::Unsupported) => {
                CredentialState::Missing
            }
            Err(CredentialError::Corrupt) => CredentialState::Expired,
            Err(_) => CredentialState::Missing,
        }
    }

    /// Secret-free status suitable for a read model or an export.
    pub fn status(provider_id: &str) -> CredentialStatus {
        CredentialStatus {
            provider: provider_id.to_string(),
            state: Self::state(provider_id),
        }
    }

    /// Reads a credential stored by another application under its own target
    /// name, for example the token the vendor's own CLI saved for this user.
    ///
    /// Used only to reuse a credential the user already granted to that CLI on
    /// this machine; nothing is written and the value is never persisted or
    /// logged by lnwdeck.
    pub fn get_by_target(target: &str) -> Result<String, CredentialError> {
        if target.trim().is_empty() {
            return Err(CredentialError::Invalid);
        }
        platform::read(target)
    }

    /// True when this build can store credentials at all.
    pub fn is_supported() -> bool {
        platform::IS_SUPPORTED
    }
}

#[cfg(windows)]
mod platform {
    use super::CredentialError;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, FILETIME};
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    };

    pub const IS_SUPPORTED: bool = true;

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn last_error() -> u32 {
        // SAFETY: GetLastError has no preconditions.
        unsafe { windows_sys::Win32::Foundation::GetLastError() }
    }

    pub fn write(target: &str, secret: &str) -> Result<(), CredentialError> {
        let mut target_wide = wide(target);
        let mut blob = secret.as_bytes().to_vec();
        let credential = CREDENTIALW {
            Flags: 0,
            Type: CRED_TYPE_GENERIC,
            TargetName: target_wide.as_mut_ptr(),
            Comment: std::ptr::null_mut(),
            LastWritten: FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            },
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: std::ptr::null_mut(),
            UserName: std::ptr::null_mut(),
        };
        // SAFETY: every pointer above refers to a live local buffer that
        // outlives the call, and the sizes match those buffers.
        let ok = unsafe { CredWriteW(&credential, 0) };
        if ok == 0 {
            return Err(CredentialError::Os(last_error()));
        }
        Ok(())
    }

    pub fn read(target: &str) -> Result<String, CredentialError> {
        let target_wide = wide(target);
        let mut raw: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: `target_wide` is a NUL-terminated wide string and `raw` is a
        // valid out-pointer. On success the buffer is freed with CredFree.
        let ok = unsafe { CredReadW(target_wide.as_ptr(), CRED_TYPE_GENERIC, 0, &mut raw) };
        if ok == 0 {
            let code = last_error();
            return Err(if code == ERROR_NOT_FOUND {
                CredentialError::NotFound
            } else {
                CredentialError::Os(code)
            });
        }
        // SAFETY: CredReadW reported success, so `raw` points to a credential
        // whose blob is `CredentialBlobSize` bytes long.
        let result = unsafe {
            let credential = &*raw;
            let bytes = std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            );
            String::from_utf8(bytes.to_vec()).map_err(|_| CredentialError::Corrupt)
        };
        // SAFETY: `raw` came from CredReadW and is freed exactly once.
        unsafe { CredFree(raw as *mut _) };
        result
    }

    pub fn delete(target: &str) -> Result<(), CredentialError> {
        let target_wide = wide(target);
        // SAFETY: `target_wide` is a NUL-terminated wide string.
        let ok = unsafe { CredDeleteW(target_wide.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if ok == 0 {
            let code = last_error();
            return Err(if code == ERROR_NOT_FOUND {
                CredentialError::NotFound
            } else {
                CredentialError::Os(code)
            });
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod platform {
    use super::CredentialError;

    pub const IS_SUPPORTED: bool = false;

    pub fn write(_target: &str, _secret: &str) -> Result<(), CredentialError> {
        Err(CredentialError::Unsupported)
    }

    pub fn read(_target: &str) -> Result<String, CredentialError> {
        Err(CredentialError::Unsupported)
    }

    pub fn delete(_target: &str) -> Result<(), CredentialError> {
        Err(CredentialError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_names_are_namespaced_per_provider() {
        assert_eq!(
            target_name("openrouter_api"),
            "lnwdeck:provider:openrouter_api"
        );
        assert_ne!(target_name("a"), target_name("b"));
    }

    #[test]
    fn empty_arguments_are_rejected() {
        assert_eq!(
            CredentialStore::set("", "secret"),
            Err(CredentialError::Invalid)
        );
        assert_eq!(
            CredentialStore::set("provider", ""),
            Err(CredentialError::Invalid)
        );
        assert_eq!(CredentialStore::get("  "), Err(CredentialError::Invalid));
        assert_eq!(CredentialStore::delete(""), Err(CredentialError::Invalid));
    }

    #[test]
    fn error_display_carries_no_secret_material() {
        for error in [
            CredentialError::NotFound,
            CredentialError::Unsupported,
            CredentialError::Os(1168),
            CredentialError::Corrupt,
            CredentialError::Invalid,
        ] {
            let text = error.to_string();
            assert!(text.is_ascii());
            assert!(
                !text.contains(' '),
                "error codes stay machine-readable: {text}"
            );
        }
    }

    #[test]
    fn missing_credential_reads_as_missing_state() {
        let state = CredentialStore::state("lnwdeck_test_provider_that_does_not_exist");
        assert_eq!(state, CredentialState::Missing);
        let status = CredentialStore::status("lnwdeck_test_provider_that_does_not_exist");
        assert_eq!(status.state, CredentialState::Missing);
        let json = serde_json::to_string(&status).expect("serialize status");
        assert!(!json.contains("secret"));
    }

    #[cfg(windows)]
    #[test]
    fn roundtrip_stores_reads_and_deletes_a_real_credential() {
        let provider = "lnwdeck_selftest_provider";
        // Clean up any leftover from a previous run without asserting on it.
        let _ = CredentialStore::delete(provider);

        // Managed Windows test runners can lack an interactive logon session
        // (ERROR_NO_SUCH_LOGON_SESSION) or deny Credential Manager writes.
        // Keep the real round-trip assertion when the OS permits it, while
        // avoiding a false-negative for that environment-only limitation.
        if let Err(error) = CredentialStore::set(provider, "sk-test-value-123") {
            match error {
                CredentialError::Os(5 | 1312) => {
                    eprintln!("Credential Manager self-test skipped: {error}");
                    return;
                }
                error => panic!("write credential: {error}"),
            }
        }
        assert_eq!(
            CredentialStore::state(provider),
            CredentialState::Configured
        );
        assert_eq!(
            CredentialStore::get(provider).expect("read credential"),
            "sk-test-value-123"
        );

        CredentialStore::set(provider, "sk-replaced").expect("replace credential");
        assert_eq!(
            CredentialStore::get(provider).expect("read replaced"),
            "sk-replaced"
        );

        CredentialStore::delete(provider).expect("delete credential");
        assert_eq!(
            CredentialStore::get(provider),
            Err(CredentialError::NotFound),
            "the credential must be gone after deletion"
        );
        assert_eq!(
            CredentialStore::delete(provider),
            Err(CredentialError::NotFound),
            "deleting twice must report that nothing was there"
        );
    }
}
