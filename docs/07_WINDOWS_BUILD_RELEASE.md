# lnwdeck Windows Build and Release

## 1. Supported Windows

- Windows 10 22H2
- Windows 11 versions still supported by Microsoft
- WebView2 Runtime required

Unsupported OS receives a clear message

## 2. Architecture tiers

### Tier 1

- x64
- ARM64

Requirements:

- Build passes
- Automated tests pass
- Installer and Portable generated
- Updater entry generated
- Smoke test documented
- No disabled core feature without Release note

### Compatibility Tier

- x86

Requirements:

- Core Dashboard, SQLite, Tray and supported adapters run
- Feature depending on unsupported native dependency is disabled explicitly
- Release notes list unsupported adapters
- Memory budget is stricter
- x86 issue does not block Tier 1 release unless it affects shared code or data integrity

## 3. Distribution

Artifacts:

```text
lnwdeck_<version>_windows_x64_setup.exe
lnwdeck_<version>_windows_x64_portable.zip
lnwdeck_<version>_windows_arm64_setup.exe
lnwdeck_<version>_windows_arm64_portable.zip
lnwdeck_<version>_windows_x86_setup.exe
lnwdeck_<version>_windows_x86_portable.zip
SHA256SUMS
latest.json
SBOM files
provenance attestations
```

Primary installer: NSIS per-user

Installer behavior:

- No administrator privilege for normal install
- Start Menu entry
- Optional Desktop shortcut
- Optional Start with Windows
- Register Native Messaging host under HKCU
- Register Chrome and Edge separately
- Preserve local data on uninstall by default
- Offer explicit Remove all local data

## 4. Portable mode

- Marker file identifies portable mode
- Data stored under `lnwdeck-data`
- No auto-start registration unless user requests
- Native Messaging registration requires explicit action and must point to current portable path
- Portable updater downloads new ZIP and prompts user to replace manually
- No automatic self-overwrite

## 5. WebView2

- Detect Evergreen WebView2 Runtime
- Installer offers official bootstrapper when missing
- Handle x64, ARM64 and x86 loader/runtime correctly
- No bundling mismatched architecture binary
- CI UI smoke sets `LNWD_E2E_CDP_PORT`; the desktop Tauri setup passes the
  matching remote-debugging argument directly to every WebView2 window.
- The browser-debugging argument is opt-in to the E2E environment and is not
  enabled for normal or release launches.

## 6. Auto-update

Behavior:

1. Check at startup after UI is usable
2. Check periodically with conservative interval
3. Download in background
4. Verify update signature
5. Validate target architecture
6. Show Release notes
7. Display `Restart to update`
8. Install only after user action
9. Preserve existing version when update fails

Stable channel only in v0.1

## 7. CI workflows

- `ci.yml`: format, lint, unit, integration, UI tests
- `extension.yml`: extension build and tests
- `security.yml`: dependency, license, secret and SBOM scans
- `release.yml`: matrix build, sign, checksum, manifest, attest, publish
- `nightly-compat.yml`: architecture compile checks and fixture suites

### CI build performance

- Cargo build concurrency is scoped per Windows job at two workers; it is not
  applied globally, so unrelated jobs do not inherit a serialized build.
- Check, test, and UI jobs share the x64 Rust cache. Architecture compile jobs
  keep target-specific cache keys so x64, ARM64, and x86 artifacts never mix.
- The UI smoke job builds the Native Messaging Host and Tauri app in debug mode
  so both use the same `target/debug` artifact tree. Release packaging continues
  to use release artifacts.

## 8. Release protection

- Tagged release
- Protected environment for signing secrets
- Minimal workflow permissions
- Pin third-party actions by immutable commit
- Generate SBOM
- Generate provenance attestation
- Verify artifacts before publishing
- Release notes list Provider capability changes

## 9. Versioning

- Semantic Versioning
- Adapter contract has independent schema version
- Database schema version is monotonic
- Breaking adapter change requires compatibility layer or major app version
- Price catalog has independent version
