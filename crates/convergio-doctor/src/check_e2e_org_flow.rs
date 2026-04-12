//! E2E org onboarding flows: onboard -> dispatch -> spawn -> cleanup and ask-with-knowledge.

use std::path::PathBuf;

use crate::check_e2e_cleanup::purge_all_doctor_data;
use crate::check_e2e_helpers::{test_name, DoctorHttpClient};
use crate::checks::{run_check, CheckResult, CheckStatus};
use serde_json::json;

pub fn run_e2e_org_flow_checks() -> Vec<CheckResult> {
    vec![
        check_onboard_dispatch_spawn_cleanup(),
        check_org_ask_with_knowledge(),
    ]
}

fn check_onboard_dispatch_spawn_cleanup() -> CheckResult {
    run_check("org_onboard_dispatch_spawn_cleanup", "e2e", || {
        let client = DoctorHttpClient::new();
        let (repo_path, repo_name) = match create_temp_rust_repo("onboard-flow") {
            Ok(v) => v,
            Err(e) => return (CheckStatus::Fail, e),
        };

        let result = (|| -> Result<String, String> {
            let (status, body) = client.post_json(
                "/api/org/projects/onboard",
                &json!({"repo_path": repo_path.to_string_lossy()}),
            )?;
            if status >= 400 || body["ok"].as_bool() != Some(true) {
                return Err(format!("onboard returned {status}: {body}"));
            }
            let org_id = body["org_id"].as_str().unwrap_or("").to_string();
            if org_id.is_empty() {
                return Err("onboard returned empty org_id".into());
            }

            let (dispatch_status, dispatch_body) = client.post_json(
                &format!("/api/org/{org_id}/dispatch"),
                &json!({
                    "task_description": "Apply a Rust backend fix",
                    "required_capabilities": ["Rust development"]
                }),
            )?;
            if dispatch_status >= 400 {
                return Err(format!(
                    "dispatch returned {dispatch_status}: {dispatch_body}"
                ));
            }
            let assigned = dispatch_body["assigned_agent"]
                .as_str()
                .unwrap_or("")
                .to_string();
            if assigned.is_empty() {
                return Err(format!(
                    "dispatch returned no assigned_agent: {dispatch_body}"
                ));
            }

            let (spawn_status, spawn_body) = client.post_json(
                "/api/agents/spawn",
                &json!({
                    "agent_name": assigned,
                    "org_id": org_id,
                    "instructions": "noop -- doctor E2E flow validation",
                    "tier": "t1",
                    "budget_usd": 0,
                    "dry_run": true
                }),
            )?;
            if spawn_status >= 400 {
                return Err(format!(
                    "spawn dry-run returned {spawn_status}: {spawn_body}"
                ));
            }

            let (purged, _) = purge_all_doctor_data(&client);
            if let Ok((200, orgs)) = client.get("/api/orgs") {
                if orgs.to_string().contains(&repo_name) {
                    return Err("cleanup left doctor-test org rows behind".into());
                }
            }

            Ok(format!(
                "onboard -> dispatch -> spawn OK; cleanup purged {purged} rows"
            ))
        })();

        let _ = purge_all_doctor_data(&client);
        let _ = std::fs::remove_dir_all(&repo_path);
        match result {
            Ok(msg) => (CheckStatus::Pass, msg),
            Err(e) => (CheckStatus::Fail, e),
        }
    })
}

fn check_org_ask_with_knowledge() -> CheckResult {
    run_check("org_ask_with_knowledge", "e2e", || {
        let client = DoctorHttpClient::new();
        let (repo_path, _) = match create_temp_rust_repo("ask-flow") {
            Ok(v) => v,
            Err(e) => return (CheckStatus::Fail, e),
        };

        let result = (|| -> Result<String, String> {
            let (status, body) = client.post_json(
                "/api/org/projects/onboard",
                &json!({"repo_path": repo_path.to_string_lossy()}),
            )?;
            if status >= 400 || body["ok"].as_bool() != Some(true) {
                return Err(format!("onboard returned {status}: {body}"));
            }
            let org_id = body["org_id"].as_str().unwrap_or("").to_string();
            if org_id.is_empty() {
                return Err("onboard returned empty org_id".into());
            }

            let question = "How do I run this project?";
            let (ask_status, ask_body) = client.post_json(
                &format!("/api/orgs/{org_id}/ask"),
                &json!({"question": question}),
            )?;
            if ask_status >= 400 {
                return Err(format!("ask returned {ask_status}: {ask_body}"));
            }

            let intent = ask_body["intent"].as_str().unwrap_or("");
            let escalated = ask_body["escalated"].as_bool().unwrap_or(false);
            let answer = ask_body["answer"].as_str().unwrap_or("").to_lowercase();
            if intent.is_empty() {
                return Err(format!("ask returned no intent: {ask_body}"));
            }
            if answer.is_empty() && !escalated {
                return Err(format!(
                    "ask returned neither answer nor escalation: {ask_body}"
                ));
            }
            if !answer.is_empty()
                && !answer.contains("cargo")
                && !answer.contains("rust")
                && !answer.contains("run")
            {
                return Err(format!(
                    "grounded answer did not reflect repo knowledge: {ask_body}"
                ));
            }

            match client.get(&format!("/api/orgs/{org_id}/ask-log?limit=5")) {
                Ok((200, log)) if log.to_string().contains(question) => {}
                Ok((s, log)) => return Err(format!("ask-log returned {s}: {log}")),
                Err(e) => return Err(format!("ask-log failed: {e}")),
            }

            Ok(format!("ask intent={intent}, escalated={escalated}"))
        })();

        let _ = purge_all_doctor_data(&client);
        let _ = std::fs::remove_dir_all(&repo_path);
        match result {
            Ok(msg) => (CheckStatus::Pass, msg),
            Err(e) => (CheckStatus::Fail, e),
        }
    })
}

fn create_temp_rust_repo(kind: &str) -> Result<(PathBuf, String), String> {
    let repo_name = test_name(kind).replace('_', "-");
    let repo_path = std::env::temp_dir().join(&repo_name);
    if repo_path.exists() {
        std::fs::remove_dir_all(&repo_path).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(repo_path.join("src")).map_err(|e| e.to_string())?;
    std::fs::write(
        repo_path.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{repo_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
             description = \"Doctor flow fixture\"\n"
        ),
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(
        repo_path.join("README.md"),
        "# Doctor Fixture\nA small Rust service used to validate org onboarding.\n",
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(repo_path.join("src/main.rs"), "fn main() {}\n").map_err(|e| e.to_string())?;
    Ok((repo_path, repo_name))
}
