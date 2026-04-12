//! Doctor check: validate skill prompt content has required adversarial phases.
//!
//! ADR-039 mandates Devil's Advocate in solve, Stress Test in planner,
//! and Parallel Thinking Advisors in execute. This check verifies the
//! seeded prompts contain these sections.

use super::checks::{run_check, CheckResult, CheckStatus};

/// Required sections per workflow skill (name_suffix, required_phrases).
const SKILL_REQUIREMENTS: &[(&str, &[&str])] = &[
    (
        "skill-solve",
        &["Devil's Advocate", "Phase 5c", "challenge"],
    ),
    ("skill-planner", &["Stress Test", "Phase 2b"]),
    ("skill-execute", &["Parallel Thinking Advisors", "Phase 2b"]),
];

pub fn check_skill_prompt_content(pool: &convergio_db::pool::ConnPool) -> CheckResult {
    run_check("skill_prompt_content", "beta", || {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => return (CheckStatus::Fail, format!("DB: {e}")),
        };
        let mut missing: Vec<String> = Vec::new();

        for &(skill_name, required) in SKILL_REQUIREMENTS {
            let body: Option<String> = conn
                .query_row(
                    "SELECT body FROM prompt_templates WHERE name = ?1 AND active = 1",
                    [skill_name],
                    |r| r.get(0),
                )
                .ok();

            match body {
                None => missing.push(format!("{skill_name}: not found in DB")),
                Some(b) => {
                    for phrase in required {
                        if !b.contains(phrase) {
                            missing.push(format!("{skill_name}: missing '{phrase}'"));
                        }
                    }
                }
            }
        }

        if missing.is_empty() {
            (
                CheckStatus::Pass,
                "All workflow skills have required adversarial phases (ADR-039)".into(),
            )
        } else {
            (CheckStatus::Warn, missing.join("; "))
        }
    })
}
