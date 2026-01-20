# `auth:` blocks

Both `type: http` and `type: openapi` servers can attach authentication to outgoing HTTP requests.

Source of truth: [`crates/adapter/src/config.rs`](../../../crates/adapter/src/config.rs) (`AuthConfig`).

## Example

```yaml
servers:
  internal_api:
    type: http
    baseUrl: http://internal-api:8080
    auth:
      type: header
      name: X-Api-Key
      value: ${INTERNAL_API_KEY}
```

## Supported auth types

### `type: none`

- Sends no auth.

### `type: bearer`

```yaml
auth:
  type: bearer
  token: ${TOKEN}
```

Adds `Authorization: Bearer <token>`.

### `type: basic`

```yaml
auth:
  type: basic
  username: ${USER}
  password: ${PASS}
```

Adds HTTP Basic auth.

### `type: header`

```yaml
auth:
  type: header
  name: X-Api-Key
  value: ${KEY}
```

Adds a custom header.

### `type: query`

```yaml
auth:
  type: query
  name: api_key
  value: ${KEY}
```

Appends a query parameter to outgoing requests.

## Notes

- The adapter does **not** implement inbound authn/z (Gateway/reverse-proxy responsibility).
- **No passthrough**: the adapter does not forward an inbound MCP client `Authorization` header to HTTP/OpenAPI backends. Outbound credentials must be configured explicitly via `auth:` (and/or explicit default headers).