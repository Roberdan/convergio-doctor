//! Check definitions, runner, and result types.

use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub category: String,
    pub status: CheckStatus,
    pub message: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub doctor_version: String,
    pub daemon_version: String,
    pub timestamp: String,
    pub checks: Vec<CheckResult>,
    pub summary: DoctorSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorSummary {
    pub total: usize,
    pub passed: usize,
    pub warnings: usize,
    pub failed: usize,
    pub duration_ms: u64,
}

impl DoctorReport {
    pub fn build(checks: Vec<CheckResult>, duration_ms: u64) -> Self {
        let passed = checks
            .iter()
            .filter(|c| c.status == CheckStatus::Pass)
            .count();
        let warnings = checks
            .iter()
            .filter(|c| c.status == CheckStatus::Warn)
            .count();
        let failed = checks
            .iter()
            .filter(|c| c.status == CheckStatus::Fail)
            .count();
        // `env!("CARGO_PKG_VERSION")` resolves to the doctor crate, not the
        // daemon it's linked into. The daemon exports its real version via
        // the `CONVERGIO_DAEMON_VERSION` env var on startup; fall back to
        // the doctor version string if the var is missing (e.g. in tests).
        let daemon_version = std::env::var("CONVERGIO_DAEMON_VERSION")
            .unwrap_or_else(|_| crate::DOCTOR_VERSION.to_string());
        Self {
            doctor_version: crate::DOCTOR_VERSION.into(),
            daemon_version,
            timestamp: chrono::Utc::now().to_rfc3339(),
            summary: DoctorSummary {
                total: checks.len(),
                passed,
                warnings,
                failed,
                duration_ms,
            },
            checks,
        }
    }
}

/// Run a named check, measuring its duration.
pub fn run_check(
    name: &str,
    category: &str,
    f: impl FnOnce() -> (CheckStatus, String),
) -> CheckResult {
    let start = Instant::now();
    let (status, message) = f();
    CheckResult {
        name: name.into(),
        category: category.into(),
        status,
        message,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

/// Run all core checks against the connection pool.
pub fn run_core_checks(pool: &convergio_db::pool::ConnPool) -> Vec<CheckResult> {
    let mut results = Vec::new();

    results.push(run_check("db_connectivity", "core", || match pool.get() {
        Ok(_) => (CheckStatus::Pass, "Connection pool healthy".into()),
        Err(e) => (CheckStatus::Fail, format!("Cannot get connection: {e}")),
    }));

    results.push(run_check("migrations_applied", "core", || {
        match pool.get() {
            Ok(conn) => {
                // After schema consolidation (PR #185), count tables instead of migration rows
                let tables: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if tables >= 20 {
                    (
                        CheckStatus::Pass,
                        format!("{tables} tables present (schema OK)"),
                    )
                } else if tables > 0 {
                    (
                        CheckStatus::Warn,
                        format!("Only {tables} tables (expected 20+)"),
                    )
                } else {
                    (CheckStatus::Fail, "No tables found".into())
                }
            }
            Err(e) => (CheckStatus::Fail, format!("DB error: {e}")),
        }
    }));

    results.push(run_check("extensions_registered", "core", || {
        match pool.get() {
            Ok(conn) => {
                // Count distinct table prefixes as proxy for extension modules
                let count: i64 = conn
                    .query_row(
                        "SELECT COUNT(DISTINCT SUBSTR(name, 1, INSTR(name||'_', '_')-1)) \
                         FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if count >= 15 {
                    (
                        CheckStatus::Pass,
                        format!("{count} extension modules detected"),
                    )
                } else {
                    (
                        CheckStatus::Warn,
                        format!("Only {count} modules detected (expected 15+)"),
                    )
                }
            }
            Err(e) => (CheckStatus::Fail, format!("DB error: {e}")),
        }
    }));

    results.push(run_check("tables_integrity", "core", || match pool.get() {
        Ok(conn) => {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if count >= 30 {
                (CheckStatus::Pass, format!("{count} tables present"))
            } else {
                (CheckStatus::Warn, format!("Only {count} tables"))
            }
        }
        Err(e) => (CheckStatus::Fail, format!("DB error: {e}")),
    }));

    results.push(run_check("schema_columns", "core", || {
        crate::check_integrity::check_required_columns(pool)
    }));

    results.push(run_check("disk_log_dir", "core", || {
        let dir = convergio_types::platform_paths::convergio_data_dir().join("logs");
        if dir.exists() {
            (
                CheckStatus::Pass,
                format!("Log dir exists: {}", dir.display()),
            )
        } else {
            match std::fs::create_dir_all(&dir) {
                Ok(_) => (CheckStatus::Pass, "Log dir created".into()),
                Err(e) => (CheckStatus::Fail, format!("Cannot create log dir: {e}")),
            }
        }
    }));

    results.push(run_check("wal_mode", "core", || match pool.get() {
        Ok(conn) => {
            let mode: String = conn
                .query_row("PRAGMA journal_mode", [], |r| r.get(0))
                .unwrap_or_default();
            if mode == "wal" {
                (CheckStatus::Pass, "WAL mode active".into())
            } else {
                (
                    CheckStatus::Warn,
                    format!("Journal mode: {mode} (expected wal)"),
                )
            }
        }
        Err(e) => (CheckStatus::Fail, format!("DB error: {e}")),
    }));

    results.push(run_check("self_contained", "core", || {
        crate::check_integrity::check_self_containment()
    }));

    // Real-world checks — production failure modes the legacy suite did not
    // cover. See `check_real_world` module docs for the 2026-04-20 incident
    // that motivated them.
    results.extend(crate::check_real_world::run_real_world_checks(pool));

    results
}
