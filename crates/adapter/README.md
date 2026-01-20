# unrelated-mcp-adapter

Expose MCP servers over streamable HTTP (`/mcp`).

Supports:

- Stdio-based MCP servers (child processes)
- HTTP APIs via OpenAPI (`type: openapi`)
- Manually defined HTTP tools (`type: http`)

> **IMPORTANT**
>
> The adapter intentionally does **not** implement authn/z or tenancy. Those controls are expected to be provided by the **Gateway** (or your reverse proxy).
>
> **Assumption**: the adapter runs only inside a **private network** (or behind your internal edge) and is **not** exposed directly to the public internet.

## Documentation

- Start here: [`docs/adapter/INDEX.md`](../../docs/adapter/INDEX.md)
- Configuration (field-by-field): [`docs/adapter/CONFIG.md`](../../docs/adapter/CONFIG.md)
- Local running & testing: [`docs/adapter/TESTING.md`](../../docs/adapter/TESTING.md)
- Architecture: [`docs/adapter/ARCHITECTURE.md`](../../docs/adapter/ARCHITECTURE.md)
