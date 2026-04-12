//! Doctor project checks — verify onboarded orgs are healthy.

use super::checks::{run_check, CheckResult, CheckStatus};
use convergio_db::pool::ConnPool;

const EXCLUDED_ORGS: &[&str] = &["convergio-io", "audit-test-org"];

/// Run all project health checks.
pub fn run_project_checks(pool: &ConnPool) -> Vec<CheckResult> {
    vec![
        check_orgs_have_members(pool),
        check_orgs_have_real_mission(pool),
        check_orgs_have_knowledge(pool),
    ]
}

/// Every onboarded org (excluding system orgs) must have at least 1 member.
fn check_orgs_have_members(pool: &ConnPool) -> CheckResult {
    run_check("project_orgs_have_members", "projects", || {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => return (CheckStatus::Fail, format!("DB: {e}")),
        };
        let orgs = match list_org_ids(&conn) {
            Ok(o) => o,
            Err(e) => return (CheckStatus::Fail, e),
        };
        if orgs.is_empty() {
            return (CheckStatus::Warn, "no onboarded orgs found".into());
        }
        let mut empty = Vec::new();
        for org_id in &orgs {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM ipc_org_members WHERE org_id = ?1",
                    [org_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if count == 0 {
                empty.push(org_id.as_str());
            }
        }
        if empty.is_empty() {
            (
                CheckStatus::Pass,
                format!("{} orgs all have members", orgs.len()),
            )
        } else {
            (
                CheckStatus::Fail,
                format!(
                    "{} org(s) have 0 members: {}",
                    empty.len(),
                    empty.join(", ")
                ),
            )
        }
    })
}

/// Orgs should not have the generic "Maintain and evolve" mission.
fn check_orgs_have_real_mission(pool: &ConnPool) -> CheckResult {
    run_check("project_orgs_real_mission", "projects", || {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => return (CheckStatus::Fail, format!("DB: {e}")),
        };
        let orgs = match list_org_ids(&conn) {
            Ok(o) => o,
            Err(e) => return (CheckStatus::Fail, e),
        };
        if orgs.is_empty() {
            return (CheckStatus::Warn, "no onboarded orgs found".into());
        }
        let mut generic = Vec::new();
        for org_id in &orgs {
            let mission: String = conn
                .query_row(
                    "SELECT mission FROM ipc_orgs WHERE id = ?1",
                    [org_id],
                    |r| r.get(0),
                )
                .unwrap_or_default();
            if mission.starts_with("Maintain and evolve") {
                generic.push(org_id.as_str());
            }
        }
        if generic.is_empty() {
            (
                CheckStatus::Pass,
                format!("{} orgs all have real missions", orgs.len()),
            )
        } else {
            (
                CheckStatus::Warn,
                format!(
                    "{} org(s) still have generic mission: {}",
                    generic.len(),
                    generic.join(", ")
                ),
            )
        }
    })
}

/// Orgs should have at least 1 knowledge base entry.
fn check_orgs_have_knowledge(pool: &ConnPool) -> CheckResult {
    run_check("project_orgs_have_knowledge", "projects", || {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => return (CheckStatus::Fail, format!("DB: {e}")),
        };
        let orgs = match list_org_ids(&conn) {
            Ok(o) => o,
            Err(e) => return (CheckStatus::Fail, e),
        };
        if orgs.is_empty() {
            return (CheckStatus::Warn, "no onboarded orgs found".into());
        }
        let mut no_kb = Vec::new();
        for org_id in &orgs {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM knowledge_base WHERE domain = ?1",
                    [org_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if count == 0 {
                no_kb.push(org_id.as_str());
            }
        }
        if no_kb.is_empty() {
            (
                CheckStatus::Pass,
                format!("{} orgs all have knowledge base entries", orgs.len()),
            )
        } else {
            (
                CheckStatus::Warn,
                format!(
                    "{} org(s) have empty knowledge base: {}",
                    no_kb.len(),
                    no_kb.join(", ")
                ),
            )
        }
    })
}

fn list_org_ids(conn: &rusqlite::Connection) -> Result<Vec<String>, String> {
    let excluded = EXCLUDED_ORGS
        .iter()
        .map(|o| format!("'{o}'"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id FROM ipc_orgs WHERE id NOT IN ({excluded}) \
         AND id NOT LIKE '_doctor_test_%' ORDER BY id"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("query: {e}"))?;
    let rows: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| format!("exec: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}
