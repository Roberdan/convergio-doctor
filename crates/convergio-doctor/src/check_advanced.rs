//! Advanced doctor checks — dependency graph, migrations, scheduled tasks.
//!
//! Each check returns (CheckStatus, String) for use with `run_check()`.
//! Config validation lives in check_config.rs.

use crate::checks::{run_check, CheckResult, CheckStatus};
use crate::runtime::DoctorRuntime;
use convergio_db::pool::ConnPool;

/// Run all advanced checks and return results.
pub fn run_advanced_checks(pool: &ConnPool, runtime: &DoctorRuntime) -> Vec<CheckResult> {
    vec![
        run_check("dep_graph_valid", "extensions", || check_dep_graph(runtime)),
        run_check("migrations_consistent", "core", || {
            check_migrations_consistent(pool)
        }),
        run_check("scheduled_tasks_registered", "extensions", || {
            check_scheduled_tasks(runtime)
        }),
        run_check("config_valid", "core", || {
            crate::check_config::check_config_valid()
        }),
    ]
}

/// Validate the dependency graph: no missing deps, no cycles, semver ok.
fn check_dep_graph(runtime: &DoctorRuntime) -> (CheckStatus, String) {
    let manifests = runtime.manifests();
    if manifests.is_empty() {
        return (
            CheckStatus::Warn,
            "No manifests available (runtime not populated)".into(),
        );
    }

    match convergio_depgraph::DepGraph::validate(&manifests) {
        Ok(()) => {
            let cap_count: usize = manifests.iter().map(|m| m.provides.len()).sum();
            let dep_count: usize = manifests.iter().map(|m| m.requires.len()).sum();
            (
                CheckStatus::Pass,
                format!(
                    "{} modules, {cap_count} capabilities, {dep_count} deps — no issues",
                    manifests.len()
                ),
            )
        }
        Err(errors) => {
            let msgs: Vec<String> = errors.iter().map(format_graph_error).collect();
            (CheckStatus::Fail, msgs.join("; "))
        }
    }
}

fn format_graph_error(e: &convergio_depgraph::graph::GraphError) -> String {
    use convergio_depgraph::graph::GraphError;
    match e {
        GraphError::MissingDependency {
            module, capability, ..
        } => format!("{module} requires missing '{capability}'"),
        GraphError::CircularDependency { cycle } => {
            format!("cycle: {}", cycle.join(" → "))
        }
        GraphError::SemVerMismatch {
            module,
            capability,
            required,
            provided,
        } => format!("{module}: '{capability}' needs {required}, got {provided}"),
    }
}

/// Verify migration version sequences have no gaps per module.
fn check_migrations_consistent(pool: &ConnPool) -> (CheckStatus, String) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => return (CheckStatus::Fail, format!("DB: {e}")),
    };

    // Post-PR #185: applied_migrations was removed during schema consolidation.
    // Verify schema health by checking critical tables exist instead.
    let critical = [
        "plans",
        "tasks",
        "waves",
        "projects",
        "ipc_orgs",
        "agent_catalog",
        "notifications",
    ];
    let mut found = 0;
    let mut missing = Vec::new();
    for table in &critical {
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if exists {
            found += 1;
        } else {
            missing.push(*table);
        }
    }

    if missing.is_empty() {
        (
            CheckStatus::Pass,
            format!("{found}/{} critical tables present", critical.len()),
        )
    } else {
        (
            CheckStatus::Fail,
            format!("missing tables: {}", missing.join(", ")),
        )
    }
}

/// Check that extensions declaring scheduled tasks are present.
fn check_scheduled_tasks(runtime: &DoctorRuntime) -> (CheckStatus, String) {
    let tasks = runtime.scheduled_tasks();
    if tasks.is_empty() {
        return (
            CheckStatus::Warn,
            "No scheduled task info (runtime not populated)".into(),
        );
    }

    let total: usize = tasks.iter().map(|(_, t)| t.len()).sum();
    if total == 0 {
        return (
            CheckStatus::Warn,
            "No scheduled tasks declared by any extension".into(),
        );
    }

    let providers: Vec<String> = tasks
        .iter()
        .map(|(id, t)| format!("{}({})", id, t.join(",")))
        .collect();

    (
        CheckStatus::Pass,
        format!(
            "{total} tasks from {} providers: {}",
            providers.len(),
            providers.join(", ")
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_graph_empty_manifests_warns() {
        let rt = DoctorRuntime::new();
        let (status, _) = check_dep_graph(&rt);
        assert_eq!(status, CheckStatus::Warn);
    }

    #[test]
    fn scheduled_tasks_empty_warns() {
        let rt = DoctorRuntime::new();
        let (status, _) = check_scheduled_tasks(&rt);
        assert_eq!(status, CheckStatus::Warn);
    }

    #[test]
    fn scheduled_tasks_populated_passes() {
        let rt = DoctorRuntime::new();
        rt.set_scheduled_tasks(vec![
            ("kernel".into(), vec!["monitor".into()]),
            ("orchestrator".into(), vec!["reaper".into()]),
        ]);
        let (status, msg) = check_scheduled_tasks(&rt);
        assert_eq!(status, CheckStatus::Pass);
        assert!(msg.contains("2 tasks"));
    }
}
