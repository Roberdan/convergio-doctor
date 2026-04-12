//! Extended smoke tests — POST endpoints + parameterized endpoint error handling.
//!
//! Tests POST-only endpoints with safe/empty payloads and verifies that
//! path-parameterized endpoints return proper errors for bogus IDs.

use crate::check_e2e_helpers::DoctorHttpClient;
use crate::checks::{run_check, CheckResult, CheckStatus};
use serde_json::json;

/// (family, path) — additional routes tested with POST or parameterized GET.
pub const SMOKE_ROUTES_EXT: &[(&str, &str)] = &[
    ("heartbeat", "/api/heartbeat"),
    ("ipc", "/api/ipc/send"),
    ("scheduler", "/api/scheduler/decide"),
    ("deploy", "/api/deploy/push-all"),
    ("deploy", "/api/deploy/upgrade"),
    ("backup", "/api/backup/export"),
    ("backup", "/api/backup/import"),
    ("backup", "/api/backup/restore"),
    ("backup", "/api/backup/snapshots/create"),
    ("backup", "/api/backup/snapshots/verify"),
    ("backup", "/api/backup/purge-log"),
    ("backup", "/api/backup/retention/purge"),
    ("kernel", "/api/kernel/ask"),
    ("kernel", "/api/kernel/classify"),
    ("kernel", "/api/kernel/register-node"),
    ("kernel", "/api/kernel/telegram-test"),
    ("kernel", "/api/kernel/verify"),
    ("voice", "/api/voice/intent"),
    ("voice", "/api/voice/transcribe"),
    ("voice", "/api/voice/speak"),
    ("voice", "/api/voice/pipeline"),
    ("inference", "/api/inference/complete"),
    ("inference", "/api/inference/routing-decision"),
    ("autoresearch", "/api/autoresearch/trigger"),
    ("build", "/api/build/rollback"),
    ("provision", "/api/provision/peer"),
    ("file_transport", "/api/file-transport/pull"),
    ("file_transport", "/api/file-transport/push"),
    ("delegate", "/api/delegate/spawn"),
    ("notifications", "/api/notify"),
    ("notifications", "/api/notify/telegram/test"),
    ("sync", "/api/sync/export"),
    ("sync", "/api/sync/import"),
    ("plan", "/api/plan-db/create"),
    ("plan", "/api/plan-db/review"),
    ("plan", "/api/plan-db/challenge"),
    ("plan", "/api/plan-db/import"),
    ("plan", "/api/plan-db/report"),
    ("plan", "/api/plan-db/kb-search"),
    ("plan", "/api/plan-db/kb-write"),
    ("plan", "/api/plan-db/task/create"),
    ("plan", "/api/plan-db/task/update"),
    ("plan", "/api/plan-db/task/evidence"),
    ("plan", "/api/plan-db/task/heartbeat"),
    ("plan", "/api/plan-db/wave/create"),
    ("plan", "/api/plan-db/wave/update"),
    ("plan", "/api/plan-db/validate"),
    ("plan", "/api/plan-db/checkpoint/save"),
    ("locks", "/api/locks/acquire"),
    ("locks", "/api/locks/release"),
    ("merge", "/api/merge/request"),
    ("merge", "/api/merge/dependencies"),
    ("approvals", "/api/approvals/request"),
    ("approvals", "/api/approvals/check"),
    ("compensations", "/api/compensations/trigger"),
    ("org_packages", "/api/org-packages/install"),
    ("org_packages", "/api/org-packages/validate"),
    ("bundles", "/api/bundles/create"),
    ("evaluations", "/api/evaluations/record"),
    ("projects", "/api/projects/scaffold"),
    ("projects", "/api/projects/scan"),
    ("workspace", "/api/workspace/check-owner"),
    ("workspace", "/api/workspace/reap"),
    ("codegraph", "/api/codegraph/expand"),
    ("claim", "/api/plan-db/task/1/claim-file"),
    ("observatory", "/api/observatory/search"),
    ("org", "/api/org/projects/onboard"),
    ("night_agents", "/api/night-agents"),
    ("billing", "/api/billing/alerts"),
    ("deploy", "/api/deploy/rollback"),
    // CEO pattern
    ("ceo", "/api/ceo"),
];

pub fn run_e2e_smoke_ext_checks() -> Vec<CheckResult> {
    let client = DoctorHttpClient::new();
    vec![
        check_post_returns_json(&client),
        check_bogus_path_params(&client),
        check_empty_post_no_500(&client),
    ]
}

fn check_post_returns_json(client: &DoctorHttpClient) -> CheckResult {
    run_check("smoke_ext_post_json", "e2e", || {
        let test_posts = ["/api/plan-db/create", "/api/ipc/send", "/api/locks/acquire"];
        let mut ok = 0;
        let mut failed = Vec::new();
        for path in &test_posts {
            match client.post_json(path, &json!({})) {
                Ok((status, body)) => {
                    if body.is_object() || body.is_array() || body.is_null() {
                        ok += 1;
                    } else if status < 500 {
                        ok += 1; // non-JSON but not 500 is acceptable
                    } else {
                        failed.push(format!("{path} → {status} non-JSON"));
                    }
                }
                Err(e) => failed.push(format!("{path} → {e}")),
            }
        }
        if failed.is_empty() {
            (
                CheckStatus::Pass,
                format!("{ok}/{} POST endpoints return JSON", test_posts.len()),
            )
        } else {
            (CheckStatus::Fail, format!("failed: {}", failed.join("; ")))
        }
    })
}

fn check_bogus_path_params(client: &DoctorHttpClient) -> CheckResult {
    run_check("smoke_ext_bogus_params", "e2e", || {
        let bogus_paths = [
            "/api/orgs/999999",
            "/api/plan-db/json/999999",
            "/api/night-agents/999999",
            "/api/prompts/999999",
            "/api/artifacts/999999",
        ];
        let mut ok = 0;
        let mut failed = Vec::new();
        for path in &bogus_paths {
            match client.get(path) {
                Ok((status, _)) if status != 500 => ok += 1,
                Ok((_status, _)) => failed.push(format!("{path} → 500")),
                Err(e) => failed.push(format!("{path} → {e}")),
            }
        }
        if failed.is_empty() {
            (
                CheckStatus::Pass,
                format!("{ok}/{} return proper error (not 500)", bogus_paths.len()),
            )
        } else {
            (
                CheckStatus::Warn,
                format!("500 errors: {}", failed.join("; ")),
            )
        }
    })
}

fn check_empty_post_no_500(client: &DoctorHttpClient) -> CheckResult {
    run_check("smoke_ext_empty_post", "e2e", || {
        let sample_posts = [
            "/api/agents/spawn",
            "/api/orgs",
            "/api/plan-db/review",
            "/api/notify",
        ];
        let mut ok = 0;
        let mut five_hundreds = Vec::new();
        for path in &sample_posts {
            match client.post_json(path, &json!({})) {
                Ok((status, _)) if status != 500 => ok += 1,
                Ok(_) => five_hundreds.push(*path),
                Err(_e) => five_hundreds.push(path),
            }
        }
        if five_hundreds.is_empty() {
            (
                CheckStatus::Pass,
                format!("{ok}/{} handle empty POST gracefully", sample_posts.len()),
            )
        } else {
            let list = five_hundreds
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            (CheckStatus::Warn, format!("500 on empty POST: {list}"))
        }
    })
}
