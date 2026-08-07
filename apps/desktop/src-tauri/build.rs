fn main() {
    // Ship the native messaging host next to the app: Tauri's `externalBin`
    // expects the binary at `binaries/<name>-<target-triple>.exe`. The host is
    // a workspace member built before this crate, so it lives in the workspace
    // target dir; when it is missing (e.g. a fresh `cargo check`), the build
    // still succeeds and the release pipeline builds it explicitly.
    let target = std::env::var("TARGET").unwrap_or_default();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .map(|dir| dir.to_path_buf());
    if let Some(root) = root {
        let mut candidates = vec![root
            .join("target")
            .join("release")
            .join("lnwdeck-browser-host.exe")];
        if !target.is_empty() {
            candidates.insert(
                0,
                root.join("target")
                    .join(&target)
                    .join("release")
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
