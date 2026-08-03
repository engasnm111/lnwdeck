# Community Adapter Example

This directory demonstrates the structure of a sandboxed community adapter for inwdeck.

## Structure

```
community-adapter/
  manifest.json       — declares identity, capabilities, and runtime limits
  example_adapter.wasm — compiled Wasm module
```

## Manifest

The `manifest.json` declares:
- `id`: unique adapter identifier
- `name`: display name
- `capabilities`: explicit list of granted permissions (deny-by-default)
- `runtime_limits`: memory, fuel, output, and timeout bounds

## Capabilities

Community adapters run in a sandbox with deny-by-default permissions:
- `filesystem:read` — read access to declared files
- `filesystem:write` — write access to declared paths
- `network:http` — HTTP requests to declared domains
- `env:read` — read environment variables

## Security

Adapters run in a Wasm sandbox with:
- Memory limits (default 64 MiB)
- Execution fuel metering
- Output size bounds
- Hard timeout enforcement
- Deny-by-default capability gating
