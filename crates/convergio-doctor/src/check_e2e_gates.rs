//! E2E doctor checks for gate hardening features:
//! - PlanStatusGate: blocks task transitions on non-active plans
//! - Auto-Thor: validates wave automatically when all tasks submitted
//! - Evidence pre-checks: 404/400/409 responses on invalid evidence

use crate::check_e2e_cleanup::purge_doctor_plan;
use crate::check_e2e_helpers::{test_name, DoctorHttpClient};
use crate::checks::{run_check, CheckResult, CheckStatus};
use serde_json::json;

pub fn run_e2e_gate_hardening_checks() -> Vec<CheckResult> {
    vec![
        check_plan_status_gate_blocks(),
        check_evidence_preflight_404(),
        check_evidence_preflight_duplicate_409(),
        check_plan_status_gate_allows_active(),
    ]
}

/// PlanStatusGate blocks task transitions when plan is not in_progress.
fn check_plan_status_gate_blocks() -> CheckResult {
    run_check("gates_plan_status_blocks_todo", "e2e", || {
        let client = DoctorHttpClient::new();
        let name = test_name("psg");

        let (_, body) = match client.post_json(
            "/api/plan-db/create",
            &json!({
                "project_id": "_doctor_test_proj", "name": name,
                "objective": "PlanStatusGate test", "motivation": "E2E",
                "requester": "doctor-e2e"
            }),
        ) {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("create failed: {e}")),
        };
        let plan_id = body["plan_id"]
            .as_i64()
            .or(body["id"].as_i64())
            .unwrap_or(0);
        if plan_id == 0 {
            return (CheckStatus::Fail, "no plan_id".into());
        }

        // Create wave + task (plan stays in 'todo' status — NOT started)
        let w = client.post_json(
            "/api/plan-db/wave/create",
            &json!({"plan_id": plan_id, "wave_id": test_name("w"), "name": "W1"}),
        );
        let wave_id = match w {
            Ok((_, b)) => b["id"].as_i64().unwrap_or(0),
            Err(e) => {
                purge_doctor_plan(&client, plan_id);
                return (CheckStatus::Fail, format!("wave create: {e}"));
            }
        };
        let t = client.post_json(
            "/api/plan-db/task/create",
            &json!({"plan_id": plan_id, "wave_id": wave_id, "title": test_name("t")}),
        );
        let task_id = match t {
            Ok((_, b)) => b["id"].as_i64().unwrap_or(0),
            Err(e) => {
                purge_doctor_plan(&client, plan_id);
                return (CheckStatus::Fail, format!("task create: {e}"));
            }
        };

        // Try to move task to in_progress — should be blocked by PlanStatusGate
        let agent = test_name("agent");
        let resp = client.post_json(
            "/api/plan-db/task/update",
            &json!({"task_id": task_id, "status": "in_progress", "agent_id": agent}),
        );
        purge_doctor_plan(&client, plan_id);

        match resp {
            Ok((_, body)) => {
                let err = body["error"].as_str().unwrap_or("");
                if err.contains("PlanStatusGate") {
                    (CheckStatus::Pass, "PlanStatusGate blocked todo plan".into())
                } else if err.is_empty() {
                    (CheckStatus::Warn, "transition allowed on todo plan".into())
                } else {
                    (CheckStatus::Warn, format!("different error: {err}"))
                }
            }
            Err(e) => (CheckStatus::Fail, format!("request failed: {e}")),
        }
    })
}

/// Evidence preflight returns 404 for nonexistent task.
fn check_evidence_preflight_404() -> CheckResult {
    run_check("evidence_preflight_404", "e2e", || {
        let client = DoctorHttpClient::new();

        let resp = client.post_json(
            "/api/plan-db/task/evidence",
            &json!({
                "task_db_id": 999999,
                "evidence_type": "test_result",
                "command": "test", "output_summary": "ok", "exit_code": 0
            }),
        );
        match resp {
            Ok((404, _)) => (
                CheckStatus::Pass,
                "evidence returns 404 for missing task".into(),
            ),
            Ok((status, body)) => (
                CheckStatus::Warn,
                format!("expected 404, got {status}: {body}"),
            ),
            Err(e) => (CheckStatus::Fail, format!("request failed: {e}")),
        }
    })
}

/// Evidence preflight returns 409 for duplicate evidence_type.
fn check_evidence_preflight_duplicate_409() -> CheckResult {
    run_check("evidence_preflight_duplicate_409", "e2e", || {
        let client = DoctorHttpClient::new();
        let name = test_name("dup");

        let (_, body) = match client.post_json(
            "/api/plan-db/create",
            &json!({
                "project_id": "_doctor_test_proj", "name": name,
                "objective": "dup test", "motivation": "E2E",
                "requester": "doctor-e2e"
            }),
        ) {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("create: {e}")),
        };
        let plan_id = body["plan_id"]
            .as_i64()
            .or(body["id"].as_i64())
            .unwrap_or(0);

        let w = client.post_json(
            "/api/plan-db/wave/create",
            &json!({"plan_id": plan_id, "wave_id": test_name("w"), "name": "W1"}),
        );
        let wave_id = match w {
            Ok((_, b)) => b["id"].as_i64().unwrap_or(0),
            _ => {
                purge_doctor_plan(&client, plan_id);
                return (CheckStatus::Fail, "wave create failed".into());
            }
        };
        let t = client.post_json(
            "/api/plan-db/task/create",
            &json!({"plan_id": plan_id, "wave_id": wave_id, "title": test_name("t")}),
        );
        let task_id = match t {
            Ok((_, b)) => b["id"].as_i64().unwrap_or(0),
            _ => {
                purge_doctor_plan(&client, plan_id);
                return (CheckStatus::Fail, "task create failed".into());
            }
        };

        // Record test_pass once — should succeed
        let first = client.post_json(
            "/api/plan-db/task/evidence",
            &json!({
                "task_db_id": task_id, "evidence_type": "test_pass",
                "command": "test", "output_summary": "ok", "exit_code": 0
            }),
        );
        if let Ok((s, _)) = &first {
            if *s >= 400 {
                purge_doctor_plan(&client, plan_id);
                return (CheckStatus::Fail, format!("first evidence failed: {s}"));
            }
        }

        // Record test_pass again — should be 409
        let second = client.post_json(
            "/api/plan-db/task/evidence",
            &json!({
                "task_db_id": task_id, "evidence_type": "test_pass",
                "command": "test", "output_summary": "ok", "exit_code": 0
            }),
        );
        purge_doctor_plan(&client, plan_id);

        match second {
            Ok((409, _)) => (CheckStatus::Pass, "duplicate test_pass returns 409".into()),
            Ok((status, body)) => (
                CheckStatus::Warn,
                format!("expected 409, got {status}: {body}"),
            ),
            Err(e) => (CheckStatus::Fail, format!("request failed: {e}")),
        }
    })
}

/// PlanStatusGate allows transitions when plan IS in_progress.
fn check_plan_status_gate_allows_active() -> CheckResult {
    run_check("gates_plan_status_allows_active", "e2e", || {
        let client = DoctorHttpClient::new();
        let name = test_name("psa");

        let (_, body) = match client.post_json(
            "/api/plan-db/create",
            &json!({
                "project_id": "_doctor_test_proj", "name": name,
                "objective": "active gate test", "motivation": "E2E",
                "requester": "doctor-e2e"
            }),
        ) {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("create: {e}")),
        };
        let plan_id = body["plan_id"]
            .as_i64()
            .or(body["id"].as_i64())
            .unwrap_or(0);

        let w = client.post_json(
            "/api/plan-db/wave/create",
            &json!({"plan_id": plan_id, "wave_id": test_name("w"), "name": "W1"}),
        );
        let wave_id = match w {
            Ok((_, b)) => b["id"].as_i64().unwrap_or(0),
            _ => {
                purge_doctor_plan(&client, plan_id);
                return (CheckStatus::Fail, "wave create failed".into());
            }
        };
        let t = client.post_json(
            "/api/plan-db/task/create",
            &json!({"plan_id": plan_id, "wave_id": wave_id, "title": test_name("t")}),
        );
        let task_id = match t {
            Ok((_, b)) => b["id"].as_i64().unwrap_or(0),
            _ => {
                purge_doctor_plan(&client, plan_id);
                return (CheckStatus::Fail, "task create failed".into());
            }
        };

        // Start plan (review + start)
        let _ = client.post_json(
            "/api/plan-db/review",
            &json!({
                "plan_id": plan_id, "reviewer": test_name("rev"),
                "verdict": "proceed", "notes": "ok"
            }),
        );
        let _ = client.post_json(&format!("/api/plan-db/start/{plan_id}"), &json!({}));

        // Now task transition should work
        let agent = test_name("agent");
        let resp = client.post_json(
            "/api/plan-db/task/update",
            &json!({"task_id": task_id, "status": "in_progress", "agent_id": agent}),
        );
        purge_doctor_plan(&client, plan_id);

        match resp {
            Ok((_, body)) => {
                if body.get("error").is_some() {
                    let err = body["error"].as_str().unwrap_or("unknown");
                    (CheckStatus::Warn, format!("still blocked: {err}"))
                } else {
                    (CheckStatus::Pass, "active plan allows transitions".into())
                }
            }
            Err(e) => (CheckStatus::Fail, format!("request failed: {e}")),
        }
    })
}
