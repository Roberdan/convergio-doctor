---
version: "1.0"
last_updated: "2026-04-07"
author: "convergio-team"
tags: ["adr"]
---

# ADR-025: Doctor System + Stress Test Suite

**Status**: Accepted  
**Date**: 2026-04-06  
**Context**: Convergio v1.2.0 maintenance/hardening phase

## Decision

Introduce a **Doctor system** (`convergio-doctor`) and a **stress test suite** (`tests/stress/`) to provide:

1. **Runtime diagnostics** (`/api/doctor`) — checks DB, migrations, extensions, config, disk, WAL mode, and **self-containment** (no hardcoded paths or legacy references)
2. **CLI access** (`cvg doctor run`) — formatted output like `flutter doctor` + JSON mode for automation
3. **Stress tests** — 20 scenarios covering every subsystem with realistic workflows and concurrent operations, feature-gated behind `stress-tests`
4. **Self-containment enforcement** — automated check that scans source for hardcoded user paths and legacy ConvergioPlatform references

## Context

- All existing tests were **router-level** (in-process) with no daemon process testing
- No load/stress/concurrency tests existed
- No runtime health diagnostics beyond `/api/health`
- Hardcoded paths to `/Users/Roberdan/GitHub/convergio` found in 5 source files
- ConvergioPlatform still listed as "ACTIVE" in capabilities reference
- 107 missing routes in CLI contract test baseline

## Architecture

```
convergio-doctor (Extension crate)
├── checks.rs        — CheckResult/DoctorReport types + core checks
├── ext.rs           — Extension trait impl (manifest, migrations, routes)
├── routes.rs        — /api/doctor, /api/doctor/version, /api/doctor/check/:cat, /api/doctor/history
└── lib.rs           — DOCTOR_VERSION constant (independent of daemon version)

daemon/tests/stress/ (integration test binary, feature-gated)
├── harness.rs       — StressDaemon with concurrent request helpers
├── fixtures.rs      — Reusable JSON templates
├── assertions.rs    — Status/shape/timing assertion helpers
└── scenarios/       — 20 test scenarios (S01-S20)

CLI
└── cvg doctor run [--fast] [--details] [--json]
    cvg doctor check <category>
    cvg doctor issues [--json]
    cvg doctor summary
```

## Doctor checks (v2.0)

Categories: core, extensions, beta, e2e, chaos, cleanup

Default `cvg doctor run` runs ALL categories and shows summary + issues.
Use `--fast` for core-only (~1s), `--details` for verbose output.

| Check | Category | What it verifies |
|-------|----------|------------------|
| db_connectivity | core | Pool returns a connection |
| migrations_applied | core | applied_migrations table has entries |
| extensions_registered | core | 20+ distinct modules have migrations |
| tables_integrity | core | 30+ tables in sqlite_master |
| disk_log_dir | core | Log directory exists or can be created |
| wal_mode | core | SQLite journal_mode = wal |
| self_contained | core | No hardcoded paths in source files |

## Stress test scenarios

S01-S09: Individual subsystem tests (org, plan, agents, delegation, mesh, governance, billing, observatory, IPC)
S10: **Concurrent plans** (5 plans with parallel task updates)
S11-S16: Infrastructure tests (backup, security, longrunning, scheduler, evidence, kernel)
S17-S18: External interface tests (MCP tools coverage, depgraph/openapi)
S19: **Full day simulation** (realistic end-to-end workflow)
S20: **Chaos parallel** (all operations simultaneously — stress maximum)

## Self-containment rule

Convergio MUST be fully portable. The doctor's `self_contained` check scans `daemon/crates/`, `scripts/`, and `claude-config/` for:
- Hardcoded user paths (`/Users/Roberdan`, `/home/roberdan`)
- Legacy references (`ConvergioPlatform`)
- Skips `docs/history/` (historical documentation is exempt)

This check runs in CI and will fail the build if violations are found.

## Consequences

- Every release runs doctor checks + stress tests (CI gate)
- Doctor reports are persisted in DB for trend analysis
- New features must add corresponding stress test scenarios
- Self-containment is enforced automatically — no more hardcoded path drift

## Alternatives considered

- **External test framework** (rejected: adds dependency, loses router-level speed)
- **Docker-based testing** (rejected: too heavy for CI, macOS dev machines)
- **Separate test repo** (rejected: maintenance burden, loses co-evolution with code)
