//! Beta doctor checks — prompts, routes, agents catalog.

use super::checks::{run_check, CheckResult, CheckStatus};

const MIN_PROMPTS: i64 = 10;
const ROUTE_TABLES: &[(&str, &str)] = &[
    ("prompts", "prompt_templates"),
    ("skills", "prompt_skills"),
    ("agents/catalog", "agent_catalog"),
    ("plans", "plans"),
    ("tasks", "tasks"),
    ("waves", "waves"),
];

/// Run all beta checks: prompts, routes, agents, mesh, skills.
pub fn run_beta_checks(pool: &convergio_db::pool::ConnPool) -> Vec<CheckResult> {
    let mut c = vec![
        check_prompts_populated(pool),
        check_routes_reachable(pool),
        check_agents_catalog_consistent(pool),
        check_mesh_node_sync(pool),
        check_sync_readiness(pool),
        check_cli_version_match(),
        crate::check_beta_mcp::check_mcp_release_sync(),
    ];
    c.push(crate::check_beta_skills::check_skill_prompt_content(pool));
    c
}

/// Verify the prompt_templates table exists and is populated.
fn check_prompts_populated(pool: &convergio_db::pool::ConnPool) -> CheckResult {
    run_check("prompts_populated", "beta", || {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => return (CheckStatus::Fail, format!("DB: {e}")),
        };
        match conn.query_row("SELECT COUNT(*) FROM prompt_templates", [], |r| {
            r.get::<_, i64>(0)
        }) {
            Ok(count) if count >= MIN_PROMPTS => (
                CheckStatus::Pass,
                format!("{count} prompt templates loaded"),
            ),
            Ok(0) => (
                CheckStatus::Warn,
                "prompt_templates table exists but is empty — run seed".into(),
            ),
            Ok(count) => (
                CheckStatus::Warn,
                format!("Only {count} prompts (expected {MIN_PROMPTS}+)"),
            ),
            Err(_) => (
                CheckStatus::Fail,
                "prompt_templates table missing — migrations not applied".into(),
            ),
        }
    })
}

/// Verify that all tables backing API routes are queryable.
fn check_routes_reachable(pool: &convergio_db::pool::ConnPool) -> CheckResult {
    run_check("routes_reachable", "beta", || {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => return (CheckStatus::Fail, format!("DB: {e}")),
        };
        let mut missing = Vec::new();
        for (route, table) in ROUTE_TABLES {
            let sql = format!("SELECT 1 FROM {table} LIMIT 0");
            if conn.execute(&sql, []).is_err() {
                missing.push(*route);
            }
        }
        if missing.is_empty() {
            (
                CheckStatus::Pass,
                format!("All {} route-backing tables accessible", ROUTE_TABLES.len()),
            )
        } else {
            (
                CheckStatus::Fail,
                format!(
                    "Unreachable routes (missing tables): {}",
                    missing.join(", ")
                ),
            )
        }
    })
}

/// Verify agents catalog: table exists, entries valid, prompt_ref not dangling.
fn check_agents_catalog_consistent(pool: &convergio_db::pool::ConnPool) -> CheckResult {
    run_check("agents_catalog_consistent", "beta", || {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => return (CheckStatus::Fail, format!("DB: {e}")),
        };

        let total: i64 =
            match conn.query_row("SELECT COUNT(*) FROM agent_catalog", [], |r| r.get(0)) {
                Ok(n) => n,
                Err(_) => return (CheckStatus::Fail, "agent_catalog table missing".into()),
            };

        if total == 0 {
            return (
                CheckStatus::Warn,
                "agent_catalog is empty — run seed".into(),
            );
        }

        let mut issues: Vec<String> = Vec::new();

        // Check for agents with empty name or role
        let bad: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_catalog WHERE name = '' OR role = ''",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if bad > 0 {
            issues.push(format!("{bad} agents with empty name/role"));
        }

        // Check prompt_ref references (only if prompt_templates table exists)
        let has_prompts = conn
            .execute("SELECT 1 FROM prompt_templates LIMIT 0", [])
            .is_ok();
        if has_prompts {
            let dangling: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM agent_catalog
                     WHERE prompt_ref IS NOT NULL
                       AND prompt_ref != ''
                       AND prompt_ref NOT IN (SELECT name FROM prompt_templates)",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if dangling > 0 {
                issues.push(format!("{dangling} dangling prompt_ref(s)"));
            }
        }

        if issues.is_empty() {
            (CheckStatus::Pass, format!("{total} agents, all consistent"))
        } else {
            (
                CheckStatus::Warn,
                format!("{total} agents, issues: {}", issues.join("; ")),
            )
        }
    })
}

fn check_mesh_node_sync(pool: &convergio_db::pool::ConnPool) -> CheckResult {
    run_check("mesh_node_sync", "beta", || {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => return (CheckStatus::Fail, format!("DB: {e}")),
        };

        // Count total registered peers
        let total: i64 = conn
            .query_row("SELECT count(*) FROM peer_heartbeats", [], |r| r.get(0))
            .unwrap_or(0);

        // Count online peers (seen in last 10 min)
        let online: i64 = conn
            .query_row(
                "SELECT count(*) FROM peer_heartbeats WHERE last_seen > unixepoch() - 600",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if total == 0 {
            return (CheckStatus::Warn, "No mesh peers registered".into());
        }
        if online == 0 {
            return (
                CheckStatus::Fail,
                format!("All {total} peers OFFLINE — heartbeat broken"),
            );
        }

        // Check version alignment
        let daemon_ver = env!("CARGO_PKG_VERSION");
        let mismatched: i64 = conn
            .query_row(
                "SELECT count(*) FROM peer_heartbeats \
                 WHERE last_seen > unixepoch() - 600 AND version != ?1",
                [daemon_ver],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if mismatched > 0 {
            return (
                CheckStatus::Warn,
                format!("{online}/{total} online, {mismatched} with different version"),
            );
        }

        (
            CheckStatus::Pass,
            format!("{online}/{total} peers online, versions aligned"),
        )
    })
}

/// Verify that the installed `cvg` CLI version matches the daemon version.
fn check_cli_version_match() -> CheckResult {
    run_check("cli_version_match", "beta", || {
        let daemon_version = env!("CARGO_PKG_VERSION");
        let cli_output = std::process::Command::new("cvg")
            .args(["--version"])
            .output();
        match cli_output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let cli_version = stdout.split_whitespace().last().unwrap_or("").trim();
                if cli_version == daemon_version {
                    (
                        CheckStatus::Pass,
                        format!("cvg {cli_version} matches daemon {daemon_version}"),
                    )
                } else {
                    (
                        CheckStatus::Fail,
                        format!(
                            "cvg={cli_version} != daemon={daemon_version} — \
                             run: cargo install --path crates/convergio-cli --force"
                        ),
                    )
                }
            }
            Ok(_) => (
                CheckStatus::Fail,
                "cvg command failed — is it installed?".into(),
            ),
            Err(_) => (
                CheckStatus::Fail,
                "cvg not found in PATH — install with: cargo install --path crates/convergio-cli"
                    .into(),
            ),
        }
    })
}

/// Check SYNC_TABLES have updated_at column (required for mesh sync).
fn check_sync_readiness(pool: &convergio_db::pool::ConnPool) -> CheckResult {
    run_check("sync_readiness", "beta", || {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => return (CheckStatus::Fail, format!("DB: {e}")),
        };
        let sync_tables = ["plans", "tasks", "waves", "knowledge_base", "notifications"];
        let mut missing = Vec::new();
        for table in &sync_tables {
            let has_updated_at: bool = conn
                .prepare(&format!("PRAGMA table_info(\"{table}\")"))
                .and_then(|mut stmt| {
                    let cols: Vec<String> = stmt
                        .query_map([], |r| r.get::<_, String>(1))?
                        .filter_map(|r| r.ok())
                        .collect();
                    Ok(cols.iter().any(|c| c == "updated_at"))
                })
                .unwrap_or(false);
            if !has_updated_at {
                // Table might not exist — that's OK (skip)
                let exists: bool = conn
                    .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1")
                    .and_then(|mut s| s.exists(rusqlite::params![table]))
                    .unwrap_or(false);
                if exists {
                    missing.push(*table);
                }
            }
        }
        if missing.is_empty() {
            (
                CheckStatus::Pass,
                format!("All {} sync tables have updated_at", sync_tables.len()),
            )
        } else {
            (
                CheckStatus::Fail,
                format!(
                    "Tables missing updated_at (mesh sync broken): {}",
                    missing.join(", ")
                ),
            )
        }
    })
}
