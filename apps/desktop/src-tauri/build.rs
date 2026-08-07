fn main() {
    // Ship the native messaging host next to the app: Tauri's `externalBin`
    // expects the binary at `binaries/<name>-<target-triple>.exe`. The host is
    // a workspace member built before this crate, so it lives in the workspace
    // target dir; CI builds the matching debug artifact before checks and the
    // matching release artifact before packaging.
    let target = std::env::var("TARGET").unwrap_or_default();
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
    let profiles: [&str; 2] = if profile == "debug" {
        ["debug", "release"]
    } else {
        ["release", "debug"]
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .map(|dir| dir.to_path_buf());
    if let Some(root) = root {
        let mut candidates = Vec::with_capacity(profiles.len() * 2);
        for candidate_profile in profiles {
            if !target.is_empty() {
                candidates.push(
                    root.join("target")
                        .join(&target)
                        .join(candidate_profile)
                        .join("lnwdeck-browser-host.exe"),
                );
            }
            candidates.push(
                root.join("target")
                    .join(candidate_profile)
                    .join("lnwdeck-browser-host.exe"),
            );
        }
        for src in candidates {
            if src.exists() {
                let dest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
                let _ = std::fs::create_dir_all(&dest_dir);
                let dest = dest_dir.join(format!("lnwdeck-browser-host-{target}.exe"));
                if !dest.exists() {
                    let _ = std::fs::copy(&src, &dest);
                }
                break;
            }
        }
    }
    tauri_build::build()
}
