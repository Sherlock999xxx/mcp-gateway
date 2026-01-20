# Workspace crates

This repository is a Cargo workspace.

Optional (local only): you can enable repo githooks to run CI checks before `git push`:

```bash
make hooks-install
git config core.hooksPath .githooks
```

New workspace members (e.g. `cli`, `gateway`, `core`) should live under this directory, for example:

- [`crates/adapter/`](../crates/adapter/) (runtime adapter binary)
- [`crates/gateway/`](../crates/gateway/) (gateway binary; MCP proxy + upstream aggregation + admin API)
- [`crates/gateway-cli/`](../crates/gateway-cli/) (gateway admin CLI binary)

Other top-level components:

- [`ui/`](../ui/) (Web UI, Next.js)
