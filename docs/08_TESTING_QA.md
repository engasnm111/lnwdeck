# lnwdeck Testing and Quality Plan

## 1. Test pyramid

### Unit

- Domain invariants
- Parser functions
- Price calculations
- Forecast
- Redaction
- Permission matching
- Scheduler/backoff
- UI reducers/selectors

### Integration

- SQLite migrations
- Repository transactions
- Deduplication
- Hook backup/apply/rollback
- Credential abstraction
- Native Messaging framing
- Tauri commands
- Adapter runtime

### Contract

Every Provider adapter runs the same contract suite against synthetic fixtures

### End-to-end

- First run
- Provider detection
- Passive collection
- Hook consent
- Analytics
- Export
- Tray actions
- Floating widget persistence
- Update-ready flow
- Portable mode behavior

## 2. Required commands

Expected workspace commands:

```bash
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

The exact script names may be introduced during Foundation task but must remain stable afterward

## 3. Provider fixtures

- Synthetic only
- Small positive fixture
- Negative fixture
- Malformed fixture
- Large fixture
- Incremental append fixture
- Truncated file fixture
- Rotated file fixture
- Version-change fixture
- Forbidden-content fixture

Fixture license and origin documented

## 4. Privacy tests

Automated checks:

- Serialize every persisted model and inspect keys
- Attempt adapter output containing prompt/response/path
- Scan SQLite dump for forbidden markers
- Scan Export output
- Scan Log output
- Verify raw identifiers are not stored
- Verify keyed hashes differ with different local keys

Any failure blocks release

## 5. Hook tests

- Existing hook absent
- Existing compatible hook
- Existing incompatible hook
- Read-only config
- Invalid config
- Concurrent modification
- Backup creation
- Atomic replace
- Validation failure rollback
- Restore original
- Unicode path
- Windows path length

Tests must not touch real user config

## 6. Browser Helper tests

- Manifest permissions
- Content extraction with fixtures
- Service worker message validation
- Exact native host name
- Origin allowlist
- Length prefix framing
- Oversized message rejection
- Replay rejection
- Unknown provider rejection
- Secret-like field rejection
- Chrome and Edge registration generation

## 7. Performance tests

Benchmarks:

- Parse representative logs
- Incremental scan
- Insert 1 million events
- 30-day overview query
- Rollup rebuild
- Tray read model query
- Cold/warm Dashboard load
- Idle collector CPU
- Adapter timeout recovery

Performance regression threshold is recorded in CI for reference environment; noisy measurements are reported rather than silently ignored

## 8. Architecture matrix

| Test             |      x64 |            ARM64 |                          x86 |
| ---------------- | -------: | ---------------: | ---------------------------: |
| Compile          | Required |         Required |                     Required |
| Unit tests       | Required |         Required | Required where runner exists |
| Installer build  | Required |         Required |                     Required |
| Portable build   | Required |         Required |                     Required |
| Full E2E         | Required | Scheduled/manual |          Compatibility smoke |
| Updater manifest | Required |         Required |                     Required |

## 9. Quality gates

Pull request cannot merge when:

- Format/lint/typecheck fails
- Unit/integration test fails
- Privacy test fails
- License policy fails
- Generated contracts are stale
- Migration test fails
- Adapter contract fails
- Critical security scan finding is unresolved

## 10. Definition of done

A Task is complete only when:

- Acceptance criteria met
- Tests added and passing
- Documentation updated
- Error states implemented
- Accessibility considered
- Security/privacy impact reported
- No unrelated file changes
- Commit is reviewable
