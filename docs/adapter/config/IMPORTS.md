# `imports:` (load-time includes)

Imports are applied **at config load time**. They let you include other config sources (currently: legacy MCP JSON).

Source of truth: [`crates/adapter/src/config.rs`](../../../crates/adapter/src/config.rs) (`ImportConfig`, `McpJsonImportConfig`).

## Example

```yaml
imports:
  - type: mcp-json
    path: /path/to/claude_desktop_config.json
    prefix: legacy
    conflict: skip
```

## Supported import types

### `type: mcp-json`

Imports a legacy MCP JSON file (`mcpServers` format) into the unified `servers:` map as `type: stdio`.

#### Fields

- `path`
  - **Type**: string (file path)
  - **Required**: yes
- `prefix`
  - **Type**: string
  - **Required**: no
  - **Meaning**: optional name prefix applied to imported server names.
- `conflict`
  - **Type**: enum: `error` | `skip` | `overwrite`
  - **Default**: `error`
  - **Meaning**: what to do if an imported server name collides with an existing `servers:` entry.

## Notes

- Imports are applied after reading the config file and expanding env vars inside it.
- CLI `--mcp-config` is treated as an implicit `mcp-json` import (see [`ENV_AND_PRECEDENCE.md`](ENV_AND_PRECEDENCE.md)).
