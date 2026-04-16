# Getting Started with Convergio Doctor

`convergio-doctor` runs diagnostic checks against a live Convergio daemon.

## Install

```bash
cargo install --path crates/convergio-doctor
```

## Running

Start the daemon and the `convergio-mcp-server` (needed for MCP-parity checks),
then:

```bash
cvg doctor run
```

The doctor checks:

- CLI ↔ daemon version parity
- MCP release sync (ADR-036) — validates `convergio-mcp-server` exposes every
  API/CLI feature via an MCP tool
- Mesh node synchronization
- E2E smoke routes, agent/skill CRUD, authentication gates
- Test data cleanup

## See also

- ADR-036: MCP release sync (`docs/adr/ADR-036-mcp-release-sync.md`)
- ADR-025: Doctor stress tests
- ADR-037: Audit security fixes

This file is the standalone-repo stub; the canonical guide lives in the
Convergio monorepo `docs/guides/`.
