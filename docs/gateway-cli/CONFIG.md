# CLI config + auth

The CLI supports **flags**, **env**, and a **local config file** (AWS-style).

## Config file

Default location:

- `$XDG_CONFIG_HOME/unrelated/gateway-admin.json`
- or `~/.config/unrelated/gateway-admin.json`

Fields:

- `adminBase`: string
- `dataBase`: string
- `adminToken`: string

## Precedence

Highest → lowest:

1. CLI flags (`--admin-base`, `--data-base`, `--token*`)
2. Env (`UNRELATED_GATEWAY_ADMIN_BASE`, `UNRELATED_GATEWAY_DATA_BASE`, `UNRELATED_GATEWAY_ADMIN_TOKEN`, `UNRELATED_GATEWAY_ADMIN_TOKEN_FILE`)
3. Config file (`gateway-admin.json`)
4. Built-in defaults:
   - admin base: `http://127.0.0.1:27101`
   - data base: `http://127.0.0.1:27100`

## Token sources

One of:

- `--token <value>`
- `--token-file <path>` (trimmed)
- `--token-stdin` (trimmed)

Persist (write to config file):

- `config set --token ...`
- `config set --token-file ...`
- `config set --token-stdin`
