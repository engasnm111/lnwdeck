//! ZCode 3.14+ credential store.
//!
//! ZCode 3.14 stopped writing `v2/config.json`; the same login now lives in
//! `v2/credentials.json` as `enc:v1:<iv>.<tag>.<ciphertext>` values. The key
//! is SHA-256 over a secret the desktop app derives from the OS user, so the
//! file is only readable by the account that created it.
//!
//! This module only decrypts values in memory: no plaintext credential is ever
//! written back, logged or returned through a public diagnostic surface.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use aes_gcm::aead::{generic_array::GenericArray, Aead, KeyInit};
use aes_gcm::Aes256Gcm;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use sha2::{Digest, Sha256};

const ENCRYPTED_PREFIX: &str = "enc:v1:";
const SECRET_ENV: &str = "ZCODE_CREDENTIAL_SECRET";

/// Team scope attached to a coding-plan credential. Organization and project
/// ids select the subscription at the provider endpoint; they are not secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamContext {
    /// Organization the coding plan belongs to.
    pub organization_id: String,
    /// Project the coding plan belongs to.
    pub project_id: String,
}

/// One per-account coding-plan API key recovered from `credentials.json`.
#[derive(Clone, PartialEq, Eq)]
pub struct AccountCredential {
    /// Built-in provider key the credential serves, e.g. `builtin:zai-coding-plan`.
    pub provider_key: String,
    /// ZCode account id the key was issued to.
    pub account_id: String,
    /// Team scope when the key belongs to a team subscription.
    pub team: Option<TeamContext>,
    /// The API key itself. Never printed by `Debug`.
    pub api_key: String,
}

impl fmt::Debug for AccountCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountCredential")
            .field("provider_key", &self.provider_key)
            .field("account_id", &self.account_id)
            .field("team", &self.team)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

/// Decrypted contents of `v2/credentials.json`.
#[derive(Clone, Default)]
pub struct CredentialStore {
    entries: BTreeMap<String, String>,
}

impl fmt::Debug for CredentialStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialStore")
            .field(
                "entries",
                &format_args!("<{} redacted>", self.entries.len()),
            )
            .finish()
    }
}

impl CredentialStore {
    /// Loads and decrypts the credential file under `home` (the ZCode home).
    /// A missing, unreadable or malformed file yields an empty store.
    pub fn load(home: &Path) -> Self {
        let user_home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();
        let username = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_default();
        let env_secret = std::env::var(SECRET_ENV).ok();
        let secret = credential_secret(
            env_secret.as_deref(),
            &user_home,
            node_platform(),
            &username,
        );
        Self::load_with_secret(home, &secret)
    }

    /// Loads with an explicit secret instead of the OS-derived one. Only tests
    /// use this; production always derives the secret from the current user.
    pub fn load_with_secret(home: &Path, secret: &str) -> Self {
        let path = home.join("v2").join("credentials.json");
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let Ok(serde_json::Value::Object(object)) = serde_json::from_str(&raw) else {
            return Self::default();
        };
        let mut entries = BTreeMap::new();
        for (name, value) in object {
            let Some(value) = value.as_str() else {
                continue;
            };
            if let Some(plaintext) = decrypt_credential_value(value, secret) {
                if !plaintext.trim().is_empty() {
                    entries.insert(name, plaintext);
                }
            }
        }
        Self { entries }
    }

    /// Plaintext value for one credential name, if it decrypted.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries.get(name).map(String::as_str)
    }

    /// Per-account coding-plan keys, in file order. Start-plan balances use the
    /// shared `zcodejwttoken` instead and are not returned here.
    pub fn account_credentials(&self) -> Vec<AccountCredential> {
        let mut accounts = Vec::new();
        for (name, api_key) in &self.entries {
            let Some((provider_key, account_id, team)) = parse_account_provider_credential(name)
            else {
                continue;
            };
            accounts.push(AccountCredential {
                provider_key,
                account_id,
                team,
                api_key: api_key.clone(),
            });
        }
        accounts
    }
}

/// Secret ZCode derives its credential key from. `env_secret` mirrors
/// `ZCODE_CREDENTIAL_SECRET`, which the desktop app honors for portable
/// installs; otherwise the fallback is the OS user identity.
pub fn credential_secret(
    env_secret: Option<&str>,
    user_home: &str,
    platform: &str,
    username: &str,
) -> String {
    match env_secret.map(str::trim) {
        Some(value) if !value.is_empty() => value.to_string(),
        _ => format!("zcode-credential-fallback:{platform}:{user_home}:{username}"),
    }
}

/// Node's `process.platform` spelling for the current target, so the derived
/// secret matches the value ZCode used when it encrypted the file.
pub fn node_platform() -> &'static str {
    if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}

/// Decrypts one `enc:v1:` credential value. Plaintext values are returned
/// unchanged; malformed or undecryptable values return `None`.
pub fn decrypt_credential_value(value: &str, secret: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.starts_with(ENCRYPTED_PREFIX) {
        return Some(trimmed.to_string());
    }
    let encoded = &trimmed[ENCRYPTED_PREFIX.len()..];
    let mut parts = encoded.split('.');
    let (Some(iv_part), Some(tag_part), Some(cipher_part), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let iv = URL_SAFE_NO_PAD.decode(iv_part).ok()?;
    let tag = URL_SAFE_NO_PAD.decode(tag_part).ok()?;
    let ciphertext = URL_SAFE_NO_PAD.decode(cipher_part).ok()?;
    if iv.len() != 12 || tag.len() != 16 {
        return None;
    }
    let key = Sha256::digest(secret.as_bytes());
    let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
    let mut payload = ciphertext;
    payload.extend_from_slice(&tag);
    let plaintext = cipher
        .decrypt(GenericArray::from_slice(&iv), payload.as_slice())
        .ok()?;
    String::from_utf8(plaintext).ok()
}

/// Maps a ZCode 3.14+ per-account credential name onto the provider key it
/// serves and its optional team scope.
pub fn parse_account_provider_credential(
    name: &str,
) -> Option<(String, String, Option<TeamContext>)> {
    let rest = name.strip_prefix("account-provider:")?;
    let scope = rest.strip_suffix(":api-key")?;
    let split_at = scope.rfind(":account:")?;
    let credential_scope = &scope[..split_at];
    let account_id = scope[split_at + ":account:".len()..].trim();
    if account_id.is_empty() {
        return None;
    }

    if let Some(individual) = credential_scope.strip_prefix("coding-plan:account:") {
        let family = individual
            .strip_suffix("-individual-coding-plan")
            .filter(|family| *family == "bigmodel" || *family == "zai")?;
        return Some((
            format!("builtin:{family}-coding-plan"),
            account_id.to_string(),
            None,
        ));
    }

    let team_scope = credential_scope.strip_prefix("team:")?;
    let parts: Vec<&str> = team_scope.split(':').collect();
    if parts.len() != 4 {
        return None;
    }
    let provider_id = percent_decode(parts[0]);
    let product_id = percent_decode(parts[1]);
    let organization_id = percent_decode(parts[2]);
    let project_id = percent_decode(parts[3]);
    let family = provider_id
        .strip_prefix("account:")
        .and_then(|value| value.strip_suffix("-team-coding-plan"))
        .filter(|family| *family == "bigmodel" || *family == "zai")?;
    if product_id.trim().is_empty()
        || organization_id.trim().is_empty()
        || project_id.trim().is_empty()
    {
        return None;
    }
    Some((
        format!("builtin:{family}-coding-plan"),
        account_id.to_string(),
        Some(TeamContext {
            organization_id,
            project_id,
        }),
    ))
}

/// Decodes `%XX` escapes the way `decodeURIComponent` does; `+` stays literal.
pub(crate) fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                out.push((high * 16 + low) as u8);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VECTOR: &str = "enc:v1:AAECAwQFBgcICQoL.YvjzoOrcSzdrRVYgmQRkEg.NPLn7_S0QvHoyjqjaaQIBgQ";

    #[test]
    fn decrypts_known_aes_gcm_vector() {
        assert_eq!(
            decrypt_credential_value(VECTOR, "test-secret").as_deref(),
            Some("sk-zcode-test-key")
        );
    }

    #[test]
    fn wrong_secret_yields_nothing() {
        assert_eq!(decrypt_credential_value(VECTOR, "other-secret"), None);
    }

    #[test]
    fn plaintext_value_is_returned_unchanged() {
        assert_eq!(
            decrypt_credential_value("sk-plain", "test-secret").as_deref(),
            Some("sk-plain")
        );
    }

    #[test]
    fn malformed_value_is_rejected() {
        assert_eq!(decrypt_credential_value("enc:v1:only.two", "s"), None);
    }

    #[test]
    fn env_secret_wins_over_the_fallback() {
        assert_eq!(
            credential_secret(Some("custom"), r"C:\home\T", "win32", "T"),
            "custom"
        );
    }

    #[test]
    fn fallback_secret_matches_the_desktop_format() {
        assert_eq!(
            credential_secret(None, r"C:\home\T", "win32", "T"),
            r"zcode-credential-fallback:win32:C:\home\T:T"
        );
    }

    #[test]
    fn platform_spelling_matches_node() {
        let expected = if cfg!(windows) {
            "win32"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        };
        assert_eq!(node_platform(), expected);
    }

    #[test]
    fn individual_coding_plan_key_maps_to_the_region_provider() {
        let parsed = parse_account_provider_credential(
            "account-provider:coding-plan:account:zai-individual-coding-plan:account:acct-1:api-key",
        )
        .expect("individual credential");
        assert_eq!(parsed.0, "builtin:zai-coding-plan");
        assert_eq!(parsed.1, "acct-1");
        assert_eq!(parsed.2, None);
    }

    #[test]
    fn team_coding_plan_key_keeps_its_scope() {
        let parsed = parse_account_provider_credential(
            "account-provider:team:account%3Abigmodel-team-coding-plan:prod-1:org%3A9:proj%2F2:account:acct-2:api-key",
        )
        .expect("team credential");
        assert_eq!(parsed.0, "builtin:bigmodel-coding-plan");
        assert_eq!(parsed.1, "acct-2");
        assert_eq!(
            parsed.2,
            Some(TeamContext {
                organization_id: "org:9".to_string(),
                project_id: "proj/2".to_string(),
            })
        );
    }

    #[test]
    fn unrelated_credential_names_are_ignored() {
        assert!(parse_account_provider_credential("oauth:active_provider").is_none());
        assert!(parse_account_provider_credential(
            "account-provider:team:account:zai-team-coding-plan:only-two:parts:account:a:api-key"
        )
        .is_none());
    }

    #[test]
    fn store_decrypts_entries_and_lists_account_keys() {
        let dir = tempfile::tempdir().expect("temp dir");
        let home = dir.path();
        std::fs::create_dir_all(home.join("v2")).expect("v2 dir");
        let store = serde_json::json!({
            "account-provider:coding-plan:account:zai-individual-coding-plan:account:acct-9:api-key": VECTOR,
            "oauth:active_provider": "zai",
        });
        std::fs::write(
            home.join("v2").join("credentials.json"),
            serde_json::to_vec(&store).expect("json"),
        )
        .expect("write credentials");
        // The vector was encrypted with "test-secret"; point the store at it.
        let loaded = CredentialStore::load_with_secret(home, "test-secret");
        assert_eq!(loaded.get("oauth:active_provider"), Some("zai"));
        let accounts = loaded.account_credentials();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].provider_key, "builtin:zai-coding-plan");
        assert_eq!(accounts[0].account_id, "acct-9");
        assert_eq!(accounts[0].api_key, "sk-zcode-test-key");
    }

    #[test]
    fn missing_credentials_file_is_an_empty_store() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = CredentialStore::load(dir.path());
        assert!(store.get("oauth:active_provider").is_none());
        assert!(store.account_credentials().is_empty());
    }
}
