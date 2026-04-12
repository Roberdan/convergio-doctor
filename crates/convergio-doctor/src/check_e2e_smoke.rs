//! E2E smoke tests — table-driven GET endpoint reachability.
//!
//! Tests all safe GET endpoints grouped by family. One CheckResult per family,
//! plus an aggregate "smoke_all" and auto-discovery "smoke_untested_routes".

use crate::check_e2e_helpers::DoctorHttpClient;
use crate::checks::{run_check, CheckResult, CheckStatus};
use std::collections::HashMap;

/// (family, path) — safe GET endpoints that don't require path parameters.
const SMOKE_ROUTES: &[(&str, &str)] = &[
    ("health", "/api/health"),
    ("health", "/api/health/deep"),
    ("mesh", "/api/mesh"),
    ("mesh", "/api/mesh/peers"),
    ("mesh", "/api/mesh/capabilities"),
    ("mesh", "/api/mesh/config/roles"),
    ("mesh", "/api/mesh/config/topology"),
    ("sync", "/api/sync/status"),
    ("agents", "/api/agents/catalog"),
    ("agents", "/api/agents/runtime"),
    ("orgs", "/api/orgs"),
    ("prompts", "/api/prompts"),
    ("skills", "/api/skills"),
    ("skills", "/api/skills/search"),
    ("skills", "/api/prompts/active/skill-solve"),
    ("billing", "/api/billing/alerts"),
    ("billing", "/api/billing/invoices"),
    ("billing", "/api/billing/rates"),
    ("billing", "/api/billing/usage"),
    ("observatory", "/api/observatory/anomalies"),
    ("observatory", "/api/observatory/dashboard"),
    ("observatory", "/api/observatory/metrics"),
    ("observatory", "/api/observatory/timeline"),
    ("ipc", "/api/ipc/agents"),
    ("ipc", "/api/ipc/channels"),
    ("ipc", "/api/ipc/context"),
    ("ipc", "/api/ipc/messages"),
    ("ipc", "/api/ipc/status"),
    ("scheduler", "/api/scheduler/history"),
    ("scheduler", "/api/scheduler/policy"),
    ("deploy", "/api/deploy/diagnostics"),
    ("deploy", "/api/deploy/history"),
    ("deploy", "/api/deploy/status"),
    ("doctor", "/api/doctor/history"),
    ("doctor", "/api/doctor/version"),
    ("depgraph", "/api/depgraph"),
    ("depgraph", "/api/depgraph/validate"),
    ("backup", "/api/backup/snapshots"),
    ("backup", "/api/backup/retention/rules"),
    ("tenancy", "/api/tenancy/audit"),
    ("tenancy", "/api/tenancy/peers"),
    ("tenancy", "/api/tenancy/resources"),
    ("tenancy", "/api/tenancy/secrets"),
    ("evidence", "/api/evidence"),
    ("longrunning", "/api/longrunning/heartbeat/stale"),
    ("night_agents", "/api/night-agents"),
    ("workspace", "/api/workspace/list-owned"),
    ("codegraph", "/api/codegraph/package-deps"),
    ("workspace_context", "/api/plan-db/tasks/in-progress"),
    ("kernel", "/api/kernel/config"),
    ("kernel", "/api/kernel/events"),
    ("kernel", "/api/kernel/status"),
    ("kernel", "/api/kernel/watchdog"),
    ("voice", "/api/voice/status"),
    ("inference", "/api/inference/costs"),
    ("autoresearch", "/api/autoresearch/experiments"),
    ("autoresearch", "/api/autoresearch/metrics"),
    ("autoresearch", "/api/autoresearch/results"),
    ("reports", "/api/reports"),
    ("build", "/api/build/history"),
    ("build", "/api/build/self"),
    ("provision", "/api/provision/runs"),
    ("file_transport", "/api/file-transport/transfers"),
    ("delegate", "/api/delegate/list"),
    ("notifications", "/api/notify/queue"),
    ("decisions", "/api/decisions"),
    ("metrics", "/api/metrics"),
    ("metrics", "/api/metrics/cost"),
    ("metrics", "/api/metrics/summary"),
    ("telemetry", "/api/telemetry"),
    ("security", "/api/security/trust"),
    ("plan", "/api/plan-db/list"),
    ("plan", "/api/plan-db/metadata"),
    ("projects", "/api/dashboard/projects"),
    ("openapi", "/api/openapi"),
    ("capabilities", "/api/capabilities"),
    ("locks", "/api/locks/active"),
    ("merge", "/api/merge/queue"),
    ("merge", "/api/merge/dependency-queue"),
    ("pm", "/api/pm/digest"),
    ("pm", "/api/pm/learnings"),
    ("pm", "/api/pm/cost-forecast"),
    ("evaluations", "/api/evaluations/list"),
    ("evaluations", "/api/evaluations/planner-rate"),
    ("evaluations", "/api/evaluations/thor-accuracy"),
    ("learnings", "/api/learnings"),
    ("org_packages", "/api/org-packages"),
    ("approvals", "/api/approvals/pending"),
    ("approvals", "/api/approvals/threshold"),
    ("tracking", "/api/tracking/agent-activity"),
    ("tracking", "/api/tracking/tokens"),
    // CEO pattern
    ("ceo", "/api/ceo/log"),
];

pub fn run_e2e_smoke_checks() -> Vec<CheckResult> {
    let client = DoctorHttpClient::new();
    let mut results = Vec::new();

    // Group routes by family and test each
    let mut families: HashMap<&str, Vec<(&str, bool)>> = HashMap::new();
    for &(family, path) in SMOKE_ROUTES {
        let ok = client.get(path).map(|(s, _)| s < 500).unwrap_or(false);
        families.entry(family).or_default().push((path, ok));
    }

    let mut total_ok = 0usize;
    let mut total_routes = 0usize;
    let mut sorted_families: Vec<_> = families.into_iter().collect();
    sorted_families.sort_by_key(|(k, _)| *k);

    for (family, routes) in &sorted_families {
        let ok_count = routes.iter().filter(|(_, ok)| *ok).count();
        total_ok += ok_count;
        total_routes += routes.len();
        let name = format!("smoke_{family}");
        let failed: Vec<_> = routes
            .iter()
            .filter(|(_, ok)| !ok)
            .map(|(p, _)| *p)
            .collect();
        results.push(run_check(&name, "e2e", || {
            if failed.is_empty() {
                (
                    CheckStatus::Pass,
                    format!("{ok_count}/{} endpoints OK", routes.len()),
                )
            } else {
                (CheckStatus::Fail, format!("failed: {}", failed.join(", ")))
            }
        }));
    }

    results.push(run_check("smoke_all", "e2e", || {
        if total_ok == total_routes {
            (
                CheckStatus::Pass,
                format!("{total_ok}/{total_routes} endpoints OK"),
            )
        } else {
            let diff = total_routes - total_ok;
            (
                CheckStatus::Fail,
                format!("{total_ok}/{total_routes} OK ({diff} failed)"),
            )
        }
    }));

    results.push(check_untested_routes(&client));
    results
}

fn check_untested_routes(client: &DoctorHttpClient) -> CheckResult {
    run_check("smoke_untested_routes", "e2e", || {
        let known: std::collections::HashSet<&str> = SMOKE_ROUTES.iter().map(|(_, p)| *p).collect();
        let ext_known: std::collections::HashSet<&str> =
            crate::check_e2e_smoke_ext::SMOKE_ROUTES_EXT
                .iter()
                .map(|(_, p)| *p)
                .collect();
        let all_known: std::collections::HashSet<&str> = known.union(&ext_known).copied().collect();

        match client.get("/api/openapi") {
            Ok((200, body)) => {
                let paths = extract_openapi_paths(&body);
                let untested: Vec<_> = paths
                    .iter()
                    .filter(|p| !all_known.contains(p.as_str()))
                    .filter(|p| !p.contains(':') && !p.contains('{'))
                    .collect();
                if untested.is_empty() {
                    (
                        CheckStatus::Pass,
                        format!("{} routes all covered", all_known.len()),
                    )
                } else {
                    let list = untested
                        .iter()
                        .take(10)
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    (
                        CheckStatus::Warn,
                        format!("{} untested routes: {list}", untested.len()),
                    )
                }
            }
            _ => (CheckStatus::Warn, "OpenAPI endpoint unavailable".into()),
        }
    })
}

fn extract_openapi_paths(body: &serde_json::Value) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(obj) = body.get("paths").and_then(|p| p.as_object()) {
        for key in obj.keys() {
            paths.push(key.clone());
        }
    }
    paths
}
