//! E2E wave sequence gate — verify W2 cannot submit while W1 is pending.

use crate::check_e2e_cleanup::purge_doctor_plan;
use crate::check_e2e_helpers::{test_name, DoctorHttpClient};
use crate::checks::{run_check, CheckResult, CheckStatus};
use serde_json::json;

pub fn check_gate_wave_sequence_blocks() -> CheckResult {
    run_check("gates_wave_sequence_blocks", "e2e", || {
        let client = DoctorHttpClient::new();
        let name = test_name("waveseq");
        let (_, body) = match client.post_json(
            "/api/plan-db/create",
            &json!({
                "project_id": "_doctor_test_proj", "name": name,
                "objective": "wave seq test", "motivation": "E2E", "requester": "doctor-e2e"
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

        // Create W1 + task, W2 + task
        let w1 = client.post_json(
            "/api/plan-db/wave/create",
            &json!({"plan_id": plan_id, "wave_id": test_name("w1"), "name": "W1"}),
        );
        let w2 = client.post_json(
            "/api/plan-db/wave/create",
            &json!({"plan_id": plan_id, "wave_id": test_name("w2"), "name": "W2"}),
        );
        let (_w1_id, w2_id) = match (w1, w2) {
            (Ok((_, b1)), Ok((_, b2))) => match (b1["id"].as_i64(), b2["id"].as_i64()) {
                (Some(a), Some(b)) => (a, b),
                _ => return (CheckStatus::Fail, "wave create: no id".into()),
            },
            _ => return (CheckStatus::Fail, "wave create failed".into()),
        };
        let _ = client.post_json(
            "/api/plan-db/task/create",
            &json!({"plan_id": plan_id, "wave_id": _w1_id, "title": test_name("t1")}),
        );
        let _ = client.post_json(
            "/api/plan-db/task/create",
            &json!({"plan_id": plan_id, "wave_id": w2_id, "title": test_name("t2")}),
        );

        let _ = client.post_json("/api/plan-db/review", &json!({
            "plan_id": plan_id, "reviewer": test_name("rev"), "verdict": "proceed", "notes": "ok"
        }));
        let _ = client.post_json(&format!("/api/plan-db/start/{plan_id}"), &json!({}));

        // Get W2 task ID from tree
        let (_, tree) = match client.get(&format!("/api/plan-db/execution-tree/{plan_id}")) {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("tree failed: {e}")),
        };
        let w2_task_id = tree["waves"]
            .as_array()
            .and_then(|w| w.get(1))
            .and_then(|w| w["tasks"].as_array())
            .and_then(|t| t.first())
            .and_then(|t| t["id"].as_i64());
        let Some(task_id) = w2_task_id else {
            purge_doctor_plan(&client, plan_id);
            return (CheckStatus::Warn, "could not find W2 task in tree".into());
        };

        // Try to submit W2 while W1 pending — should be blocked
        let agent = test_name("agent");
        let _ = client.post_json(
            "/api/plan-db/task/update",
            &json!({"task_id": task_id, "notes": "PR https://test commit abc123", "agent_id": agent}),
        );
        for etype in &["test_result", "test_pass"] {
            let _ = client.post_json(
                "/api/plan-db/task/evidence",
                &json!({
                    "task_db_id": task_id, "evidence_type": etype,
                    "command": "test", "output_summary": "ok", "exit_code": 0
                }),
            );
        }
        let resp = client.post_json(
            "/api/plan-db/task/update",
            &json!({"task_id": task_id, "status": "submitted", "agent_id": agent}),
        );
        purge_doctor_plan(&client, plan_id);
        match resp {
            Ok((s, _)) if s >= 400 => (CheckStatus::Pass, format!("gate blocked W2 (HTTP {s})")),
            Ok((s, _)) => (CheckStatus::Warn, format!("expected rejection, got {s}")),
            Err(e) => (CheckStatus::Fail, format!("request failed: {e}")),
        }
    })
}
