# Gateway MCP settings (per-profile)

Profiles can tune how the Gateway behaves as an MCP server **to downstream clients**, especially for aggregated upstreams.

These settings are supported in:

- **Mode 1** (config file): under `profiles.<id>.mcp`
- **Mode 3** (Postgres): via Admin/Tenant profile APIs (`mcp` field), stored in `profiles.mcp_settings`
  - CLI: `unrelated-gateway-admin profiles create|put --mcp-json ...` (or `--mcp-file ...`)

## `mcp.capabilities` (allow/deny)

Controls which MCP **server** capabilities the Gateway advertises (and enforces for the corresponding methods/notifications).

Shape:

- `mcp.capabilities.allow`: list of capability keys (non-empty ⇒ acts as an allowlist overriding defaults)
- `mcp.capabilities.deny`: list of capability keys (applied after defaults/allowlist)

Supported keys:

- `logging`
- `completions`
- `resources-subscribe`
- `tools-list-changed`
- `resources-list-changed`
- `prompts-list-changed`

Defaults: all of the above are enabled.

## `mcp.notifications` (filtering)

Allows users to tune noisy upstream servers by filtering server→client notifications in the merged SSE stream.

Shape:

- `mcp.notifications.allow`: list of notification method strings (non-empty ⇒ allowlist)
- `mcp.notifications.deny`: list of notification method strings (denylist)

Examples:

- `notifications/message`
- `notifications/progress`
- `notifications/resources/updated`
- `notifications/cancelled`
- `notifications/tools/list_changed`
- `notifications/resources/list_changed`
- `notifications/prompts/list_changed`

Defaults: allow everything.

Note: disabling the `logging` capability also suppresses `notifications/message` (even if not explicitly filtered).

## `mcp.namespacing` (IDs in the merged SSE stream)

Controls how the Gateway namespaces IDs so aggregated upstream streams don’t collide.

### `mcp.namespacing.requestId`

- `opaque` (default): `unrelated.proxy.<b64(upstream_id)>.<b64(json(request_id))>`
- `readable`: `unrelated.proxy.r.<upstream_id>.<b64(json(request_id))>`

### `mcp.namespacing.sseEventId`

- `upstream-slash` (default): `{upstream_id}/{upstream_event_id}`
- `none`: do not prefix upstream SSE event IDs (may break per-upstream resume via `Last-Event-ID`)

## Mode 1 example

```yaml
profiles:
  my-profile:
    tenantId: t1
    upstreams: ["u1", "u2"]
    mcp:
      capabilities:
        deny: ["logging"]
      notifications:
        deny: ["notifications/progress"]
      namespacing:
        requestId: opaque
        sseEventId: upstream-slash
```
