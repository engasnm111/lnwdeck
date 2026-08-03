# Security Policy

`lnwdeck` operates under a local-only, metadata-only data policy by default. If you discover a security vulnerability, please report it privately. Do not open public issues for security vulnerabilities.

## Reporting a Vulnerability

- **Do not open public GitHub issues** for security vulnerabilities.
- Email the security team with:
  - A brief summary of the issue.
  - Steps to reproduce using **synthetic test fixtures only** (do not include real user logs or sensitive data).
  - Version and environment details (Windows OS version, CPU architecture).
- Maintainers will acknowledge receipt within 48 hours and coordinate patch development and disclosure.

## Report Checklist

Please include the following in your report where applicable:

- Estimated attack vector and mechanism.
- Impact on privacy or risk of secret exposure.
- Impact on user configuration or settings integrity.
- Supply chain impact regarding update verification.

## Security Posture & Safeguards

- Secrets are never persisted in plain text files.
- Secrets are stored exclusively using Windows Credential Manager or Tauri Stronghold.
- Browser Helper utilizes Chromium Manifest V3, strict origin allowlists, and validated Native Messaging protocols.
- Community Adapters run inside Wasm Sandboxes with deny-by-default permission policies.
- Releases are strictly blocked if unmitigated Critical or High severity findings exist.
- Update artifact signatures must be cryptographically verified prior to installation.

## Disclosure and Attribution

- Maintainers publish GitHub Security Advisories for confirmed vulnerabilities.
- Reporters receive public credit according to prior agreement before advisory disclosure.

## Supported Versions

- **v0.1**: Currently supported stable version; receives urgent security patches.
- **Alpha / Beta releases**: Database schemas and adapter interfaces are subject to change; fixes are applied directly to `main`.
