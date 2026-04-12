//! E2E tests for beta doctor checks: prompts, routes, agents catalog.

mod helpers;

use convergio_doctor::check_beta::run_beta_checks;
use convergio_doctor::checks::CheckStatus;

#[test]
fn beta_checks_return_expected_results() {
    let pool = helpers::setup_pool();
    let results = run_beta_checks(&pool);
    assert!(!results.is_empty(), "beta checks must return results");
    let names: Vec<&str> = results.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"prompts_populated"));
    assert!(names.contains(&"routes_reachable"));
    assert!(names.contains(&"agents_catalog_consistent"));
    assert!(names.contains(&"mcp_release_sync"));
    for c in &results {
        assert_eq!(c.category, "beta");
    }
}

#[test]
fn prompts_populated_warns_when_empty() {
    let pool = helpers::setup_pool();
    let results = run_beta_checks(&pool);
    let c = results
        .iter()
        .find(|c| c.name == "prompts_populated")
        .unwrap();
    assert_eq!(c.status, CheckStatus::Warn);
    assert!(c.message.contains("empty"));
}

#[test]
fn prompts_populated_passes_when_seeded() {
    let pool = helpers::setup_pool();
    let conn = pool.get().unwrap();
    for i in 0..15 {
        conn.execute(
            "INSERT INTO prompt_templates (id, name, body) VALUES (?1, ?2, 'body')",
            rusqlite::params![format!("p-{i}"), format!("prompt-{i}")],
        )
        .unwrap();
    }
    drop(conn);
    let results = run_beta_checks(&pool);
    let c = results
        .iter()
        .find(|c| c.name == "prompts_populated")
        .unwrap();
    assert_eq!(c.status, CheckStatus::Pass);
    assert!(c.message.contains("15"));
}

#[test]
fn routes_reachable_passes_with_all_tables() {
    let pool = helpers::setup_pool();
    let results = run_beta_checks(&pool);
    let c = results
        .iter()
        .find(|c| c.name == "routes_reachable")
        .unwrap();
    assert_eq!(c.status, CheckStatus::Pass);
    assert!(c.message.contains("accessible"));
}

#[test]
fn agents_catalog_warns_when_empty() {
    let pool = helpers::setup_pool();
    let results = run_beta_checks(&pool);
    let c = results
        .iter()
        .find(|c| c.name == "agents_catalog_consistent")
        .unwrap();
    assert_eq!(c.status, CheckStatus::Warn);
    assert!(c.message.contains("empty"));
}

#[test]
fn agents_catalog_passes_when_consistent() {
    let pool = helpers::setup_pool();
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO agent_catalog (id, name, role, category) \
         VALUES ('a1', 'test-agent', 'tester', 'core')",
        [],
    )
    .unwrap();
    drop(conn);
    let results = run_beta_checks(&pool);
    let c = results
        .iter()
        .find(|c| c.name == "agents_catalog_consistent")
        .unwrap();
    assert_eq!(c.status, CheckStatus::Pass);
    assert!(c.message.contains("1 agents"));
}

#[test]
fn agents_catalog_warns_on_dangling_prompt_ref() {
    let pool = helpers::setup_pool();
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO agent_catalog (id, name, role, category, prompt_ref) \
         VALUES ('a1', 'test-agent', 'tester', 'core', 'nonexistent')",
        [],
    )
    .unwrap();
    drop(conn);
    let results = run_beta_checks(&pool);
    let c = results
        .iter()
        .find(|c| c.name == "agents_catalog_consistent")
        .unwrap();
    assert_eq!(c.status, CheckStatus::Warn);
    assert!(c.message.contains("dangling"));
}

#[test]
fn agents_catalog_warns_on_empty_name() {
    let pool = helpers::setup_pool();
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO agent_catalog (id, name, role, category) \
         VALUES ('a1', '', 'tester', 'core')",
        [],
    )
    .unwrap();
    drop(conn);
    let results = run_beta_checks(&pool);
    let c = results
        .iter()
        .find(|c| c.name == "agents_catalog_consistent")
        .unwrap();
    assert_eq!(c.status, CheckStatus::Warn);
    assert!(c.message.contains("empty name/role"));
}
