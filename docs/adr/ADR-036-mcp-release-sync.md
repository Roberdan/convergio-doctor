# ADR-036: MCP Release Sync

## Status
Accepted.

## Context
Every Convergio release ships three surfaces in lockstep: **API, CLI, and MCP**.
Drift between the three causes doctor checks, planner flows, and IDE integrations
to fail silently. The `doctor` `mcp_release_sync` check enforces parity.

## Decision
The release workflow MUST:

1. Run `cargo test -p convergio-mcp --tests` and `cargo test -p convergio-doctor --lib --tests`.
2. Publish the `convergio-mcp-server` binary as a release artifact.
3. Expose every plan/task/execution-tree feature through API, CLI, and MCP tool
   (e.g. `cvg_get_execution_tree`, `cvg_update_task`, `cvg_complete_task`,
   `cvg_validate_plan`).

The `doctor` crate contains `check_mcp_release_sync` which statically verifies:

- required tools are registered in `convergio-mcp::registry_defs`,
- release workflow runs the MCP + doctor test gates and ships `convergio-mcp-server`,
- this ADR mentions `doctor`, `convergio-mcp-server`, and `API, CLI, and MCP`,
- the getting-started guide references `convergio-mcp-server`,
- each feature has matching API route, CLI handler, and MCP tool.

## Consequences
Missing any of the above fails `cvg doctor run` and blocks release. See the
`mcp_release_sync` check in `crates/convergio-doctor/src/check_beta_mcp.rs` for
the authoritative list of required strings.

This file is the in-repo stub; the canonical ADR lives in the Convergio monorepo
`docs/adr/`.
