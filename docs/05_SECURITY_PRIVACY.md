# lnwdeck Security and Privacy Specification

## 1. Security objectives

- Sensitive AI content never becomes application data
- Secrets are not persisted in ordinary files
- Provider changes cannot compromise Core
- Browser extension cannot communicate with unknown native host
- Hook installation cannot destroy user config
- Update supply chain is verifiable
- Local-only remains true by default

## 2. Data classification

### Allowed

- Provider
- Tool
- Model
- Token counts
- Request count
- Cost
- Quota percentage/amount
- Reset time
- Timestamp
- Project alias
- Keyed session hash
- Confidence and freshness
- Sanitized error code

### Forbidden

- Prompt
- Response
- Source code
- File content
- File name
- Absolute path
- Browser Cookie
- Authorization header
- API key
- Session token
- Full DOM/HTML
- Clipboard content
- Environment dump

## 3. Privacy guard

All collector output passes a central Privacy guard before storage.

Checks:

- Schema allowlist
- String pattern scan for paths and secret formats
- Maximum string lengths
- No unknown fields
- No nested arbitrary JSON
- Adapter ID and source provenance
- Rejection audit using error code only

Privacy guard failure is fail-closed

## 4. Secret storage

- Use Windows Credential Manager for API keys and local hashing key
- Consider Tauri Stronghold only if it simplifies cross-platform future without weakening Windows integration
- UI receives credential state: missing/configured/expired
- UI never receives secret value after save
- Logs must redact bearer tokens, cookies, query secrets and known key patterns
- Memory buffers should be zeroized where practical

## 5. Browser Helper

Architecture:

```text
Provider page
 -> Content script
 -> Extension service worker
 -> Native Messaging host
 -> Authenticated local IPC
 -> Desktop Core
```

Controls:

- Manifest V3
- No remote code
- Minimal host permissions
- Optional permissions granted per Provider
- Native Messaging `allowed_origins` contains exact extension IDs
- Separate Chrome and Edge registration under HKCU
- Message schema version
- Nonce, timestamp and replay protection
- Size limit substantially below browser maximum
- No arbitrary command execution
- No Cookie API unless a future reviewed design explicitly requires it
- Page extraction returns normalized fields only

## 6. Hook safety

- Passive mode default
- Preview before write
- Backup with timestamp and file hash
- Parse existing config
- Preserve formatting where safe
- Chain existing hook only when semantics are known
- Atomic temp-file write and replace
- Reparse after write
- Functional validation
- Automatic rollback on failure
- One-click restore
- Never edit when file is read-only or merge is ambiguous

## 7. Community adapter sandbox

Recommended v0.1 runtime:

- Wasmtime Component Model
- Explicit WIT interfaces
- No inherited environment
- No ambient filesystem
- No ambient network
- Memory ceiling
- Fuel/epoch timeout
- Output size limit
- Capability handles supplied by Core
- Signed package metadata where available
- Checksum always shown
- Unverified status for unsigned packages

Native DLL plugin loading is forbidden

## 8. Update security

- GitHub Releases as distribution source
- Tauri update signing key
- Private signing key only in protected release environment
- Public key embedded in app
- Signature verification before staging
- Artifact checksum
- GitHub build provenance attestation
- Protected release workflow
- Stable update channel only in v0.1
- Background download; install only after explicit Restart action
- Portable build downloads but does not overwrite itself

## 9. Threat model summary

| Threat                      | Main control                                               |
| --------------------------- | ---------------------------------------------------------- |
| Malicious provider log      | Parser limits, schema validation, no code execution        |
| Malicious community adapter | Wasm sandbox, deny-by-default permissions                  |
| Secret leak in log          | Central redaction and secret-free UI contract              |
| Browser extension spoof     | Exact extension origin, nonce, local IPC auth              |
| Replay of Browser message   | Timestamp and nonce cache                                  |
| Hook config corruption      | Backup, atomic write, validation, rollback                 |
| Duplicate usage inflation   | Stable fingerprint and transaction                         |
| Supply chain tampering      | Signed updater, checksum, provenance                       |
| Database theft              | Metadata-only; optional encrypted DB considered after v0.1 |
| Path disclosure             | Keyed hash and alias before persistence                    |

## 10. Security gates

Release is blocked by:

- Critical/High vulnerability in reachable dependency without mitigation
- Privacy contract test failure
- Unsigned updater artifact
- Extension host wildcard
- Adapter with undeclared permission
- Config write without recoverable backup
- Raw secret appearing in test logs
