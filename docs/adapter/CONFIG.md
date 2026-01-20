# Configuration

The adapter supports two configuration styles:

1) **Unified config file** (recommended): YAML (default) or JSON (when the file extension is `.json`)
2) **Legacy MCP JSON imports** (via `imports:` or `--mcp-config`)

This document is the entry point for configuration docs. For a complete field-by-field reference, follow the links below.

## Top-level structure (unified config)

```yaml
adapter: {}   # process-level settings (bind/log/timeouts/restarts)
imports: []   # load-time includes (e.g. legacy MCP JSON)
servers: {}   # runtime backends (stdio/openapi/http)
```

See:

- [`config/ADAPTER.md`](config/ADAPTER.md)
- [`config/IMPORTS.md`](config/IMPORTS.md)
- [`config/SERVERS_STDIO.md`](config/SERVERS_STDIO.md)
- [`config/SERVERS_HTTP.md`](config/SERVERS_HTTP.md)
- [`config/SERVERS_OPENAPI.md`](config/SERVERS_OPENAPI.md)

## CLI + environment variables

The adapter is also configurable via CLI flags (and environment variables for those flags).

- Run `unrelated-mcp-adapter --help` to see the full list.
- `--print-effective-config` prints the fully resolved configuration and exits.

See: [`config/ENV_AND_PRECEDENCE.md`](config/ENV_AND_PRECEDENCE.md)

## Common topics

- **Auth blocks**: shared `auth:` schema is used by `http` and `openapi` backends.
  - See: [`config/AUTH.md`](config/AUTH.md)
- **Environment expansion**: strings can contain `${VAR}` (missing vars fail startup).
  - See: [`config/ENV_AND_PRECEDENCE.md`](config/ENV_AND_PRECEDENCE.md)

## Source of truth

For implementation details, see:

- [`crates/adapter/src/config.rs`](../../crates/adapter/src/config.rs) (config schema, parsing, env expansion, precedence)
