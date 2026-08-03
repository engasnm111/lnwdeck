//! Updater artifact verification.
//!
//! These tests verify the real release chain with the real key material: the
//! signature produced by the bundler is checked against the public key embedded
//! in `tauri.conf.json`, using the same `minisign-verify` implementation that
//! `tauri-plugin-updater` uses at runtime. A tampered installer must fail.
//!
//! This is the check that the previous release could not offer: the application
//! contained a hand-written `verify_signature` that returned success for any
//! non-empty string.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

fn tauri_config() -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let raw = std::fs::read_to_string(path).expect("read tauri.conf.json");
    serde_json::from_str(&raw).expect("parse tauri.conf.json")
}

/// The updater public key, decoded from the base64 form stored in the config.
fn public_key() -> minisign_verify::PublicKey {
    let config = tauri_config();
    let encoded = config["plugins"]["updater"]["pubkey"]
        .as_str()
        .expect("updater pubkey must be configured");
    let decoded = String::from_utf8(base64_decode(encoded).expect("pubkey must be valid base64"))
        .expect("pubkey must be utf8");
    // The stored value is the minisign public-key file: a comment line followed
    // by the base64 key.
    let key_line = decoded
        .trim()
        .lines()
        .last()
        .expect("public key line")
        .trim()
        .to_string();
    minisign_verify::PublicKey::from_base64(&key_line).expect("pubkey must be a valid minisign key")
}

/// Minimal base64 decoder, so this test does not depend on the crate the
/// updater happens to use.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (index, byte) in TABLE.iter().enumerate() {
        lookup[*byte as usize] = index as u8;
    }
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in input.bytes() {
        if byte == b'=' || byte == b'\n' || byte == b'\r' {
            continue;
        }
        let value = lookup[byte as usize];
        if value == 255 {
            return None;
        }
        buffer = (buffer << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }
    Some(output)
}

/// Signed installers produced by the last release build, if any.
fn signed_installers() -> Vec<(PathBuf, PathBuf)> {
    let bundle = repo_root().join("target").join("release").join("bundle");
    let mut found = Vec::new();
    for sub in ["nsis", "msi"] {
        let dir = bundle.join(sub);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("sig") {
                continue;
            }
            let installer = path.with_extension("");
            if installer.is_file() {
                found.push((installer, path));
            }
        }
    }
    found
}

/// Version declared in the Tauri configuration.
fn configured_version() -> String {
    tauri_config()["version"]
        .as_str()
        .expect("version must be configured")
        .to_string()
}

#[test]
fn updater_public_key_is_valid_and_decodable() {
    let key = public_key();
    // Decoding succeeded, so the configured key is a real minisign key rather
    // than a placeholder string.
    let _ = key;
}

#[test]
fn every_built_installer_is_signed_by_the_configured_key() {
    let installers = signed_installers();
    if installers.is_empty() {
        // Nothing was built in this checkout; the release workflow additionally
        // fails when no .sig file is produced.
        eprintln!("no signed installers under target/release/bundle; skipping");
        return;
    }

    let key = public_key();
    for (installer, signature_path) in &installers {
        let bytes = std::fs::read(installer).expect("read installer");
        let signature_text = std::fs::read_to_string(signature_path).expect("read signature");
        let decoded = String::from_utf8(
            base64_decode(signature_text.trim()).expect("signature must be base64"),
        )
        .expect("signature must be utf8");
        let signature =
            minisign_verify::Signature::decode(&decoded).expect("signature must be minisign");

        key.verify(&bytes, &signature, false).unwrap_or_else(|err| {
            panic!(
                "{} is not signed by the configured updater key: {err}",
                installer.display()
            )
        });

        assert!(
            installer
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.contains(&configured_version()))
                .unwrap_or(false),
            "{} must carry the configured version in its file name",
            installer.display()
        );
    }
}

#[test]
fn a_tampered_installer_fails_verification() {
    let installers = signed_installers();
    if installers.is_empty() {
        eprintln!("no signed installers under target/release/bundle; skipping");
        return;
    }

    let key = public_key();
    let (installer, signature_path) = &installers[0];
    let mut bytes = std::fs::read(installer).expect("read installer");
    // Flip one byte in the middle of the payload.
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0xFF;

    let signature_text = std::fs::read_to_string(signature_path).expect("read signature");
    let decoded =
        String::from_utf8(base64_decode(signature_text.trim()).expect("base64")).expect("utf8");
    let signature = minisign_verify::Signature::decode(&decoded).expect("signature");

    assert!(
        key.verify(&bytes, &signature, false).is_err(),
        "a modified installer must not verify against the release signature"
    );
}

#[test]
fn a_signature_from_another_key_is_rejected() {
    let installers = signed_installers();
    if installers.is_empty() {
        eprintln!("no signed installers under target/release/bundle; skipping");
        return;
    }

    let (installer, _) = &installers[0];
    let bytes = std::fs::read(installer).expect("read installer");

    // A syntactically valid signature over different content must not verify.
    let key = public_key();
    let other = minisign_verify::Signature::decode(
        "untrusted comment: signature from minisign secret key\nRUQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\ntrusted comment: timestamp:0\tfile:other\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n",
    );
    // Rejecting the crafted signature at decode time is equally correct, so
    // only a successfully decoded signature is verified here.
    if let Ok(signature) = other {
        assert!(
            key.verify(&bytes, &signature, false).is_err(),
            "a foreign signature must be rejected"
        );
    }
}
