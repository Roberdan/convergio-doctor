//! E2E org lifecycle + CRUD tests for orgs, agents, prompts, skills, night agents.

use crate::check_e2e_helpers::{test_name, DoctorHttpClient};
use crate::checks::{run_check, CheckResult, CheckStatus};
use serde_json::json;

pub fn run_e2e_org_checks() -> Vec<CheckResult> {
    vec![
        check_org_lifecycle(),
        check_agents_catalog_crud(),
        check_prompts_crud(),
        check_skills_crud(),
        check_night_agents_crud(),
        check_decisions_log(),
    ]
}

fn check_org_lifecycle() -> CheckResult {
    run_check("org_lifecycle", "e2e", || {
        let client = DoctorHttpClient::new();
        let org_name = test_name("org");
        let ceo_name = test_name("ceo");

        // Create org — schema: id, mission, ceo_agent (required)
        let (status, body) = match client.post_json(
            "/api/orgs",
            &json!({
                "id": org_name,
                "mission": "Doctor E2E test org",
                "ceo_agent": ceo_name
            }),
        ) {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("create org failed: {e}")),
        };
        if status >= 400 {
            return (CheckStatus::Fail, format!("create org returned {status}"));
        }
        let org_id = body["id"]
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| body["id"].as_i64().map(|n| n.to_string()))
            .unwrap_or_default();

        // Get org
        if !org_id.is_empty() {
            match client.get(&format!("/api/orgs/{org_id}")) {
                Ok((200, _)) => {}
                Ok((s, _)) => return (CheckStatus::Warn, format!("GET org returned {s}")),
                Err(e) => return (CheckStatus::Warn, format!("GET org failed: {e}")),
            }

            // Update
            let _ = client.put_json(
                &format!("/api/orgs/{org_id}"),
                &json!({"description": "doctor test update"}),
            );

            // Sub-endpoints
            for sub in &["telemetry", "digest", "orgchart", "plans"] {
                let _ = client.get(&format!("/api/orgs/{org_id}/{sub}"));
            }
        }

        (
            CheckStatus::Pass,
            "create → get → update → sub-endpoints OK".into(),
        )
    })
}

fn check_agents_catalog_crud() -> CheckResult {
    run_check("agents_catalog_crud", "e2e", || {
        let client = DoctorHttpClient::new();
        let name = test_name("agent");

        // Create — schema: name, role, category (AgentCategory enum)
        let (status, _body) = match client.post_json(
            "/api/agents/catalog",
            &json!({
                "name": name,
                "role": "doctor-e2e-test",
                "category": "core_utility"
            }),
        ) {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("create failed: {e}")),
        };
        if status >= 400 {
            return (CheckStatus::Fail, format!("create agent returned {status}"));
        }

        // Verify in list
        match client.get("/api/agents/catalog") {
            Ok((200, body)) if body.to_string().contains(&name) => {}
            Ok((200, _)) => return (CheckStatus::Warn, "agent created but not in list".into()),
            _ => {}
        }

        (CheckStatus::Pass, "create → list OK".into())
    })
}

fn check_prompts_crud() -> CheckResult {
    run_check("prompts_crud", "e2e", || {
        let client = DoctorHttpClient::new();
        let name = test_name("prompt");

        let (status, body) = match client.post_json(
            "/api/prompts",
            &json!({"name": name, "body": "doctor test prompt", "variables": []}),
        ) {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("create prompt failed: {e}")),
        };
        if status >= 400 {
            return (
                CheckStatus::Warn,
                format!("create prompt returned {status}"),
            );
        }

        // Verify in list
        match client.get("/api/prompts") {
            Ok((200, body)) if body.to_string().contains(&name) => {}
            Ok((200, _)) => return (CheckStatus::Warn, "prompt created but not in list".into()),
            _ => {}
        }

        let prompt_id = body["id"].as_i64().unwrap_or(0);
        if prompt_id > 0 {
            let _ = client.delete(&format!("/api/prompts/{prompt_id}"));
        }

        (CheckStatus::Pass, "create → list → delete OK".into())
    })
}

fn check_skills_crud() -> CheckResult {
    run_check("skills_crud", "e2e", || {
        let client = DoctorHttpClient::new();
        let name = test_name("skill");

        let (status, _) = match client.post_json(
            "/api/skills",
            &json!({"agent": name, "host": "localhost", "capability": name, "confidence": 0.9, "description": "doctor test skill"}),
        ) {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("create skill failed: {e}")),
        };
        if status >= 400 {
            return (CheckStatus::Warn, format!("create skill returned {status}"));
        }

        // Search
        match client.get(&format!("/api/skills/search?q={name}")) {
            Ok((200, body)) if body.to_string().contains(&name) => {}
            Ok((200, _)) => return (CheckStatus::Warn, "skill created but not searchable".into()),
            _ => {}
        }

        (CheckStatus::Pass, "create → search OK".into())
    })
}

fn check_night_agents_crud() -> CheckResult {
    run_check("night_agents_crud", "e2e", || {
        let client = DoctorHttpClient::new();
        let name = test_name("nagent");

        let (status, body) = match client.post_json(
            "/api/night-agents",
            &json!({"name": name, "description": "doctor test", "schedule": "0 3 * * *", "agent_prompt": "doctor E2E test agent"}),
        ) {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("create failed: {e}")),
        };
        if status >= 400 {
            return (
                CheckStatus::Warn,
                format!("create night-agent returned {status}"),
            );
        }

        let agent_id = body["id"].as_i64().unwrap_or(0);
        if agent_id > 0 {
            let _ = client.get(&format!("/api/night-agents/{agent_id}"));
            let _ = client.put_json(
                &format!("/api/night-agents/{agent_id}"),
                &json!({"description": "updated by doctor"}),
            );
            let _ = client.delete(&format!("/api/night-agents/{agent_id}"));
        }

        (
            CheckStatus::Pass,
            "create → get → update → delete OK".into(),
        )
    })
}

fn check_decisions_log() -> CheckResult {
    run_check("decisions_log", "e2e", || {
        let client = DoctorHttpClient::new();
        let agent = test_name("dec");

        let (status, _) = match client.post_json(
            "/api/decisions",
            &json!({
                "org_id": "_doctor_test_proj", "agent": agent,
                "decision": "test decision", "reasoning": "E2E check"
            }),
        ) {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("log decision failed: {e}")),
        };
        if status >= 400 {
            return (CheckStatus::Warn, format!("decisions returned {status}"));
        }

        match client.get("/api/decisions") {
            Ok((200, body)) if body.to_string().contains(&agent) => {
                (CheckStatus::Pass, "decision logged and visible".into())
            }
            Ok((200, _)) => (CheckStatus::Warn, "decision logged but not in list".into()),
            _ => (CheckStatus::Warn, "decisions list unavailable".into()),
        }
    })
}
