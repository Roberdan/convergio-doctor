# ADR-036: MCP Release Sync

## Status
Accepted.

## Context
Every Convergio release ships three surfaces in lockstep: **API, CLI, and MCP**.
Drift between the three causes doctor checks, planner flows, and IDE integrations
to fail silently. The `doctor` `mcp_release_sync` check enforces parity.

## Decision
Every release of the Convergio stack MUST:

1. Run the local crate's test gate (in this repo: `cargo test -p convergio-doctor --lib --tests`).
2. Expose every plan/task/execution-tree feature through API, CLI, and MCP tool
   (e.g. `cvg_get_execution_tree`, `cvg_update_task`, `cvg_complete_task`,
   `cvg_validate_plan`).

Post-extraction (33 separate repos) the cross-crate test matrix and the
`convergio-mcp-server` artifact are owned by the `convergio-mcp` repo; they
are not re-run here. Parity is enforced statically via the `REQUIRED_TOOLS`
list, which reads `convergio-mcp::registry_defs::all_defs()` — the same
symbol table the running daemon uses.

The `doctor` crate contains `check_mcp_release_sync` which statically verifies:

- required tools are registered in `convergio-mcp::registry_defs`,
- this repo's release workflow runs the convergio-doctor test gate,
- this ADR mentions `doctor` and `API, CLI, and MCP`,
- the getting-started guide references `convergio-mcp-server`,
- each feature has matching API route, CLI handler, and MCP tool.

## Consequences
Missing any of the above fails `cvg doctor run` and blocks release. See the
`mcp_release_sync` check in `crates/convergio-doctor/src/check_beta_mcp.rs` for
the authoritative list of required strings.

This file is the in-repo stub; the canonical ADR lives in the Convergio monorepo
`docs/adr/`.
