# ADR-037: Security Audit and Hardening Fixes

**Status:** Accepted  
**Date:** 2025-07-24  
**Author:** Security Audit (Copilot)

## Context

convergio-doctor runs system diagnostics, health checks, and stress tests.
As a diagnostic tool it has privileged access to the daemon, database, mesh peers,
and filesystem. A security audit identified several classes of vulnerabilities.

## Findings and Fixes

### CRITICAL — Command Injection (`check_e2e_mesh.rs`)

**Before:** Peer IP addresses from the `/api/mesh/peers` API response were passed
directly to `std::process::Command::new("ssh")` as arguments. A malicious peer
entry (e.g., `"-oProxyCommand=..."`) could execute arbitrary commands.

**Fix:** Added `is_safe_peer_addr()` validation — only alphanumeric, `.`, `:`, `-`
characters allowed, max 253 chars, must not start with `-`. Invalid addresses are
skipped with a warning.

### HIGH — SSRF (`check_e2e_helpers.rs`, `check_e2e_cleanup.rs`)

**Before:** `DoctorHttpClient::with_base()` and `check_peer_online()` accepted
arbitrary URLs. If peer data contained `file://` or other scheme URLs, the client
would follow them.

**Fix:**
- Scheme validation: only `http://` and `https://` allowed.
- Redirect policy set to `Policy::none()` to prevent open-redirect SSRF.

### HIGH — Path Traversal (`check_spec_compliance.rs`)

**Before:** Task descriptions containing `create file ../../../etc/passwd` would
cause `root.join(candidate)` to resolve outside the workspace.

**Fix:** Reject candidates containing `..`. Canonicalize paths and verify they
remain within the workspace root before checking existence.

### MEDIUM — Path Traversal (`check_command_sync.rs`)

**Before:** The `source:` YAML frontmatter field was used as a filesystem path
without validation.

**Fix:** Reject source paths containing `..` sequences.

### MEDIUM — Secret Exposure (`routes.rs`)

**Before:** Full diagnostic report JSON was stored in `doctor_reports` table. If
any check message contained tokens, passwords, or API keys, they would be persisted
and exposed via the `/api/doctor/history` endpoint.

**Fix:** Added `sanitize_report()` that scans check messages for secret patterns
(`token=`, `password=`, `bearer`, etc.) and redacts matching messages before storage.

### MEDIUM — SQL Identifier Validation (`check_e2e_cleanup.rs`)

**Before:** Table and column names from `sqlite_master` and `PRAGMA table_info`
were interpolated into SQL without validation.

**Fix:** Added alphanumeric/underscore validation for table and column names.

### LOW — UTF-8 Panic (`check_e2e_mesh.rs`, `check_chaos_net.rs`)

**Before:** `trunc()` sliced strings by byte index (`&s[..max]`), which panics
if the boundary falls inside a multi-byte UTF-8 character.

**Fix:** Use `char_indices().nth(max)` for safe character-boundary truncation.

### LOW — Config Path Canonicalization (`check_config.rs`)

**Before:** `CONVERGIO_CONFIG` env var was used as-is without resolving symlinks.

**Fix:** Canonicalize the path to resolve symlinks and prevent path ambiguity.

## Decision

All findings were fixed in a single PR. No behavioral changes to passing checks.

## Consequences

- Mesh SSH checks skip peers with suspicious addresses (minor coverage reduction).
- Redirect-following disabled for peer HTTP clients (intentional).
- Report messages containing potential secrets are redacted in storage.
