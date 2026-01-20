# Contributing

Thanks for contributing!

## Development setup

- Rust: `1.92.0` (see `rust-toolchain.toml`)
- Docker: required for integration tests (Testcontainers)

## Common commands

```bash
make ci
make test-integration
```

Optional local hooks:

```bash
make hooks-install
git config core.hooksPath .githooks
```

## PRs

- Keep PRs focused and small when possible
- Add tests for bug fixes and new behavior
- Run `make ci` before pushing
