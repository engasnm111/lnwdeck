# inwdeck Open Source Governance

## 1. License

- Project license: MIT
- Repository includes `LICENSE`
- Third-party code and adapted parsers require attribution
- Repository includes `THIRD_PARTY_LICENSES.md`
- Dependency license scan runs in CI

## 2. Required community files

```text
LICENSE
SECURITY.md
CONTRIBUTING.md
CODE_OF_CONDUCT.md
THIRD_PARTY_LICENSES.md
CHANGELOG.md
.github/
├─ ISSUE_TEMPLATE/
│  ├─ bug.yml
│  ├─ provider-request.yml
│  ├─ adapter-bug.yml
│  └─ security-config.yml
├─ PULL_REQUEST_TEMPLATE.md
└─ workflows/
```

## 3. Contribution rules

- One purpose per Pull request
- Tests required
- Provider fixture must be sanitized
- New permission requires security explanation
- New dependency requires license and size explanation
- UI change includes screenshot
- Provider change includes capability matrix update
- Breaking schema change includes migration
- Contributor confirms code can be distributed under MIT

## 4. Adapter review levels

### Built-in

- Maintainer reviewed
- Full contract tests
- Included in release
- Permission surface documented

### Verified community

- Source repository public
- Package checksum
- Review completed
- Contract tests pass
- Publisher identity documented

### Unverified community

- Explicit warning
- Manual installation
- Permissions shown
- No official support guarantee

## 5. Security reports

- `SECURITY.md` defines private disclosure path
- Do not request secrets or real logs in public issue
- Maintainers provide sanitized fixture instructions
- Security advisory used for confirmed vulnerabilities
- Revoked adapter versions can be blocked by signed local registry update after design review

## 6. Release cadence

- Alpha: architecture and adapters evolve
- Beta: database and adapter schemas stabilized
- Stable: compatibility guarantees documented
- Emergency patch: security, data corruption or provider-wide breakage

## 7. Attribution to references

TokenTracker and other Open Source projects may be studied for collection techniques. Code reuse requires:

1. Confirm license
2. Record source file and commit
3. Preserve required notices
4. Adapt to inwdeck privacy and architecture rules
5. Add tests
6. Avoid copying branding or proprietary assets
