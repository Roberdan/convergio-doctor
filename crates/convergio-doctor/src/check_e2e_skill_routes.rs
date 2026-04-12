//! Doctor check: verify all seeded skill prompt routes are reachable.
//!
//! Queries /api/skills to discover seeded skills, then tests
//! /api/prompts/active/:name for each. Dynamic — no hardcoded names (fixes #526).

use crate::check_e2e_helpers::DoctorHttpClient;
use crate::checks::{run_check, CheckResult, CheckStatus};

pub fn check_skill_prompt_routes() -> CheckResult {
    run_check("skill_prompt_routes", "e2e", || {
        let client = DoctorHttpClient::new();

        // Get seeded skills from prompt_templates via prompts API
        let (status, body) = match client.get("/api/prompts?category=skill&active_only=true") {
            Ok(r) => r,
            Err(e) => return (CheckStatus::Fail, format!("cannot query skills: {e}")),
        };
        if status >= 400 {
            return (
                CheckStatus::Fail,
                format!("skills endpoint returned {status}"),
            );
        }

        // Extract skill names from response
        let names: Vec<String> = body
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v["name"].as_str().map(String::from))
            .collect();

        if names.is_empty() {
            return (CheckStatus::Warn, "no seeded skills found".into());
        }

        let mut ok = 0usize;
        let mut failed = Vec::new();

        for name in &names {
            match client.get(&format!("/api/prompts/active/{name}")) {
                Ok((200, body)) => {
                    // Verify body is not empty/truncated
                    let body_text = body["body"].as_str().unwrap_or("");
                    if body_text.len() < 100 {
                        failed.push(format!("{name}(truncated:{}b)", body_text.len()));
                    } else {
                        ok += 1;
                    }
                }
                Ok((s, _)) => failed.push(format!("{name}(HTTP {s})")),
                Err(e) => failed.push(format!("{name}({e})")),
            }
        }

        if failed.is_empty() {
            (
                CheckStatus::Pass,
                format!("{ok}/{} skill prompt routes OK", names.len()),
            )
        } else {
            (
                CheckStatus::Warn,
                format!("{ok}/{} OK, failed: {}", names.len(), failed.join(", ")),
            )
        }
    })
}
