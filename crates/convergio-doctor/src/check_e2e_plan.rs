//! E2E plan lifecycle + gate testing.

use crate::check_e2e_cleanup::purge_doctor_plan;
use crate::check_e2e_helpers::{test_name, DoctorHttpClient};
use crate::checks::{run_check, CheckResult, CheckStatus};
use convergio_db::pool::ConnPool;
use serde_json::json;

pub fn run_e2e_plan_checks(_pool: &ConnPool) -> Vec<CheckResult> {
    vec![
        check_plan_lifecycle(),
        check_gate_evidence_blocks(),
        check_gate_test_pass_blocks(),
        check_gate_pr_commit_blocks(),
        crate::check_e2e_wave_gate::check_gate_wave_sequence_blocks(),
    ]
}

fn check_plan_lifecycle() -> CheckResult {
    run_check("plan_lifecycle_e2e", "e2e", || {
        let client = DoctorHttpClient::new();
        let plan_name = test_name("plan");

        // Create plan
        let create = json!({
            "project_id": "_doctor_test_proj",
            "name": plan_name,
            "objective": "Doctor E2E test plan",
            "motivation": "Automated health check",
            "requester": "doctor-e2e"
        });
        let (status, body) = match client.post_json("/api/plan-db/create", &create) {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("create failed: {e}")),
        };
        if status >= 400 {
            return (CheckStatus::Fail, format!("create returned {status}"));
        }
        let plan_id = body["plan_id"]
            .as_i64()
            .or(body["id"].as_i64())
            .unwrap_or(0);
        if plan_id == 0 {
            return (CheckStatus::Fail, "no plan_id in response".into());
        }

        // Review
        let review = json!({"plan_id": plan_id, "reviewer": test_name("reviewer"), "verdict": "proceed", "notes": "doctor test"});
        if let Err(e) = client.post_json("/api/plan-db/review", &review) {
            return (CheckStatus::Fail, format!("review failed: {e}"));
        }

        // Start
        if let Err(e) = client.post_json(&format!("/api/plan-db/start/{plan_id}"), &json!({})) {
            return (CheckStatus::Fail, format!("start failed: {e}"));
        }

        // Verify execution tree
        match client.get(&format!("/api/plan-db/execution-tree/{plan_id}")) {
            Ok((200, tree)) if tree.get("waves").is_some() || tree.get("plan").is_some() => {}
            Ok((200, tree)) => {
                purge_doctor_plan(&client, plan_id);
                return (CheckStatus::Fail, format!("tree missing waves: {tree}"));
            }
            Ok((s, _)) => {
                purge_doctor_plan(&client, plan_id);
                return (CheckStatus::Fail, format!("tree returned {s}"));
            }
            Err(e) => {
                purge_doctor_plan(&client, plan_id);
                return (CheckStatus::Fail, format!("tree failed: {e}"));
            }
        }

        purge_doctor_plan(&client, plan_id);
        (CheckStatus::Pass, format!("plan {plan_id} lifecycle OK"))
    })
}

fn create_test_plan(client: &DoctorHttpClient) -> Result<(i64, i64), String> {
    let name = test_name("gate");
    let (_, body) = client.post_json(
        "/api/plan-db/create",
        &json!({
            "project_id": "_doctor_test_proj", "name": name,
            "objective": "Doctor gate test", "motivation": "E2E check", "requester": "doctor-e2e"
        }),
    )?;
    let plan_id = body["plan_id"]
        .as_i64()
        .or(body["id"].as_i64())
        .ok_or("no plan_id")?;

    // Create a wave + task so the execution tree is not empty
    let (_, wave_body) = client.post_json(
        "/api/plan-db/wave/create",
        &json!({ "plan_id": plan_id, "wave_id": test_name("w1"), "name": "W1" }),
    )?;
    let wave_id = wave_body["id"].as_i64().ok_or("no wave_id")?;
    client.post_json(
        "/api/plan-db/task/create",
        &json!({
            "plan_id": plan_id, "wave_id": wave_id,
            "title": test_name("task"),
            "task_id": test_name("tid"),
        }),
    )?;

    client.post_json("/api/plan-db/review", &json!({
        "plan_id": plan_id, "reviewer": test_name("rev"), "verdict": "proceed", "notes": "gate test"
    }))?;
    client.post_json(&format!("/api/plan-db/start/{plan_id}"), &json!({}))?;
    let (_, tree) = client.get(&format!("/api/plan-db/execution-tree/{plan_id}"))?;
    let task_id = tree["waves"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|wave| {
            wave["tasks"]
                .as_array()
                .and_then(|tasks| tasks.iter().find_map(|task| task["id"].as_i64()))
        })
        .ok_or("no task id in tree")?;
    Ok((plan_id, task_id))
}

fn check_gate_evidence_blocks() -> CheckResult {
    run_check("gates_evidence_blocks", "e2e", || {
        let client = DoctorHttpClient::new();
        let (plan_id, task_id) = match create_test_plan(&client) {
            Ok(ids) => ids,
            Err(e) => return (CheckStatus::Fail, format!("setup failed: {e}")),
        };
        // Try to submit without evidence → should be rejected
        let agent = test_name("agent");
        let resp = client.post_json(
            "/api/plan-db/task/update",
            &json!({
                "task_id": task_id, "status": "submitted", "agent_id": agent
            }),
        );
        purge_doctor_plan(&client, plan_id);
        match resp {
            Ok((status, _body)) if status >= 400 => (
                CheckStatus::Pass,
                format!("gate blocked submit without evidence (HTTP {status})"),
            ),
            Ok((status, _)) => (
                CheckStatus::Warn,
                format!("expected rejection, got {status} (gate may not be active)"),
            ),
            Err(e) => (CheckStatus::Fail, format!("request failed: {e}")),
        }
    })
}

fn check_gate_test_pass_blocks() -> CheckResult {
    run_check("gates_test_pass_blocks", "e2e", || {
        let client = DoctorHttpClient::new();
        let (plan_id, task_id) = match create_test_plan(&client) {
            Ok(ids) => ids,
            Err(e) => return (CheckStatus::Fail, format!("setup failed: {e}")),
        };
        let agent = test_name("agent");
        // Add test_result but NOT test_pass
        let _ = client.post_json(
            "/api/plan-db/task/evidence",
            &json!({
                "task_db_id": task_id, "evidence_type": "test_result",
                "command": "cargo test", "output_summary": "1 passed", "exit_code": 0
            }),
        );
        // Set notes
        let _ = client.post_json(
            "/api/plan-db/task/update",
            &json!({
                "task_id": task_id, "notes": "PR https://test commit abc123", "agent_id": agent
            }),
        );
        // Try submit
        let resp = client.post_json(
            "/api/plan-db/task/update",
            &json!({
                "task_id": task_id, "status": "submitted", "agent_id": agent
            }),
        );
        purge_doctor_plan(&client, plan_id);
        match resp {
            Ok((status, _)) if status >= 400 => (
                CheckStatus::Pass,
                format!("gate blocked: no test_pass (HTTP {status})"),
            ),
            Ok((status, _)) => (
                CheckStatus::Warn,
                format!("expected rejection, got {status}"),
            ),
            Err(e) => (CheckStatus::Fail, format!("request failed: {e}")),
        }
    })
}

fn check_gate_pr_commit_blocks() -> CheckResult {
    run_check("gates_pr_commit_blocks", "e2e", || {
        let client = DoctorHttpClient::new();
        let (plan_id, task_id) = match create_test_plan(&client) {
            Ok(ids) => ids,
            Err(e) => return (CheckStatus::Fail, format!("setup failed: {e}")),
        };
        let agent = test_name("agent");
        // Add evidence but no notes with PR
        let _ = client.post_json(
            "/api/plan-db/task/evidence",
            &json!({
                "task_db_id": task_id, "evidence_type": "test_result",
                "command": "cargo test", "output_summary": "ok", "exit_code": 0
            }),
        );
        let _ = client.post_json(
            "/api/plan-db/task/evidence",
            &json!({
                "task_db_id": task_id, "evidence_type": "test_pass",
                "command": "cargo test", "output_summary": "ok", "exit_code": 0
            }),
        );
        // Submit WITHOUT setting notes with PR URL
        let resp = client.post_json(
            "/api/plan-db/task/update",
            &json!({
                "task_id": task_id, "status": "submitted", "agent_id": agent
            }),
        );
        purge_doctor_plan(&client, plan_id);
        match resp {
            Ok((status, _)) if status >= 400 => (
                CheckStatus::Pass,
                format!("gate blocked: no PR in notes (HTTP {status})"),
            ),
            Ok((status, _)) => (
                CheckStatus::Warn,
                format!("expected rejection, got {status}"),
            ),
            Err(e) => (CheckStatus::Fail, format!("request failed: {e}")),
        }
    })
}
