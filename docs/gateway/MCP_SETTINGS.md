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

## `mcp.security` (upstream trust + proxy hardening)

These settings control how the Gateway behaves when interacting with **upstream MCP servers** and
how it hardens certain proxy mechanics against malicious clients.

Notes:

- This is **per-profile**, but supports **per-upstream overrides** (because not all upstreams are equally trusted).
- Defaults are intentionally **non-breaking** (preserve current behavior), but enable operators to tighten policy.

### `mcp.security.signedProxiedRequestIds`

If `true` (default), the Gateway will sign proxied upstream server→client request IDs with a
per-session key and reject downstream responses whose IDs do not verify (mitigates forged responses
from malicious downstream clients).

### `mcp.security.upstreamDefault` / `mcp.security.upstreamOverrides`

Shape:

- `mcp.security.upstreamDefault`: default policy applied to all upstreams unless overridden
- `mcp.security.upstreamOverrides`: object keyed by upstream id → policy override

Each upstream policy supports:

- `clientCapabilitiesMode`: how the Gateway advertises **client capabilities** upstream during `initialize`
  - `passthrough` (default): forward downstream client capabilities unchanged
  - `strip`: send empty client capabilities upstream
  - `allowlist`: forward only the keys in `clientCapabilitiesAllow`
- `clientCapabilitiesAllow`: list of top-level capability keys (e.g. `sampling`, `roots`, `elicitation`)
- `rewriteClientInfo`: if `true`, replace downstream `clientInfo` before sending `initialize` upstream (privacy)
- `serverRequests`: filter for upstream **server→client JSON-RPC requests** forwarded over SSE
  - `defaultAction`: `allow` (default) or `deny`
  - `allow`: allowlist of method strings
  - `deny`: denylist of method strings

Examples of request methods:

- `sampling/createMessage`
- `roots/list`
- `elicitation/create`

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
      security:
        signedProxiedRequestIds: true
        upstreamDefault:
          clientCapabilitiesMode: passthrough
          rewriteClientInfo: false
          serverRequests:
            defaultAction: allow
            allow: []
            deny: []
        upstreamOverrides:
          untrusted-upstream-1:
            clientCapabilitiesMode: strip
            rewriteClientInfo: true
            serverRequests:
              defaultAction: deny
              allow: []
              deny: []
```
