# Changelog

This repository is newly public. Earlier internal iteration notes are intentionally omitted.

## 2026-01-24

- Tenant-level Web UI: `0.4.1`

### Web UI

- Fix Profile auth editor to use a draft + explicit Apply (discard on Cancel/outside-click).
- Fix Profile metadata editor stacking/duplication by making it a modal (Cancel now closes + discards).
- OpenAPI source editor: standardize the Discovery enable/disable control to match the app’s toggle style.
- UI Docker image: remove bundled `npm`/`npx` from runtime to reduce vulnerability surface (Trivy tar CVEs).

## Initial public release

- Adapter: `0.9.0`
- Gateway: `0.8.0`
- Gateway admin CLI: `0.8.0`
- Tenant-level Web UI: `0.4.0`
