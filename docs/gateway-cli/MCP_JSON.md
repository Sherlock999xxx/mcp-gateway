# `mcp.json` (MCP client config)

Many MCP clients (and the MCP Inspector) use a JSON file commonly named **`mcp.json`** with a top-level `mcpServers` object.

For Gateway profiles, the transport is **streamable HTTP**, and the URL is:

- `http://<data-base>/<profile_id>/mcp`

where `data-base` defaults to `http://127.0.0.1:27100`.

## CLI output

### Full file (servers file)

- `unrelated-gateway-admin mcp-json servers-file --profile-id <uuid>`

Example output:

```json
{
  "mcpServers": {
    "unrelated-gateway-<uuid>": {
      "type": "streamable-http",
      "url": "http://127.0.0.1:27100/<uuid>/mcp",
      "note": "Unrelated MCP Gateway profile (streamable HTTP)"
    }
  }
}
```

### Single entry (server entry)

- `unrelated-gateway-admin mcp-json server-entry --profile-id <uuid>`

Example output:

```json
{
  "type": "streamable-http",
  "url": "http://127.0.0.1:27100/<uuid>/mcp",
  "note": "Unrelated MCP Gateway profile (streamable HTTP)"
}
```

## Writing to disk

The CLI **does not write files**. Use redirection:

```bash
unrelated-gateway-admin mcp-json servers-file --profile-id <uuid> > mcp.json
```
