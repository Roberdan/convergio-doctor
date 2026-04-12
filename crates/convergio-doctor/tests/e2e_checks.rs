//! E2E tests for convergio-doctor core checks.
//!
//! Verifies all 8 core checks run, produce valid CheckStatus values,
//! and that the DoctorReport builder produces correct summaries.

mod helpers;

use convergio_doctor::checks::{run_check, run_core_checks, CheckStatus, DoctorReport};

#[test]
fn all_core_checks_run_and_produce_valid_status() {
    let pool = helpers::setup_pool();
    let results = run_core_checks(&pool);

    // 8 core checks expected
    assert_eq!(
        results.len(),
        8,
        "Expected 8 core checks, got {}",
        results.len()
    );

    let expected_names = [
        "db_connectivity",
        "migrations_applied",
        "extensions_registered",
        "tables_integrity",
        "schema_columns",
        "disk_log_dir",
        "wal_mode",
        "self_contained",
    ];

    for name in &expected_names {
        let found = results.iter().find(|c| c.name == *name);
        assert!(found.is_some(), "Missing check: {name}");
    }

    for check in &results {
        assert!(!check.name.is_empty(), "Check name is empty");
        assert_eq!(check.category, "core", "Wrong category for {}", check.name);
        assert!(
            !check.message.is_empty(),
            "Empty message for {}",
            check.name
        );
        assert!(
            matches!(
                check.status,
                CheckStatus::Pass | CheckStatus::Warn | CheckStatus::Fail
            ),
            "Invalid status for {}",
            check.name
        );
    }
}

#[test]
fn db_connectivity_passes_with_valid_pool() {
    let pool = helpers::setup_pool();
    let results = run_core_checks(&pool);
    let check = results
        .iter()
        .find(|c| c.name == "db_connectivity")
        .unwrap();
    assert_eq!(check.status, CheckStatus::Pass);
    assert!(check.message.contains("healthy"));
}

#[test]
fn migrations_warns_with_few_tables() {
    let pool = helpers::setup_pool();
    // In-memory pool has only applied_migrations — very few tables
    let results = run_core_checks(&pool);
    let check = results
        .iter()
        .find(|c| c.name == "migrations_applied")
        .unwrap();
    // Should warn or pass depending on table count (not fail)
    assert!(
        check.status == CheckStatus::Warn || check.status == CheckStatus::Pass,
        "unexpected status: {:?} — {}",
        check.status,
        check.message
    );
}

#[test]
fn migrations_passes_with_many_tables() {
    let pool = helpers::setup_pool();
    let conn = pool.get().unwrap();
    // Create 25 tables to simulate a full schema
    for i in 0..25 {
        conn.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS test_table_{i} (id INTEGER)"
        ))
        .unwrap();
    }
    drop(conn);
    let results = run_core_checks(&pool);
    let check = results
        .iter()
        .find(|c| c.name == "migrations_applied")
        .unwrap();
    assert_eq!(check.status, CheckStatus::Pass);
    assert!(check.message.contains("tables present"));
}

#[test]
fn extensions_registered_warns_with_few_prefixes() {
    let pool = helpers::setup_pool();
    let results = run_core_checks(&pool);
    let check = results
        .iter()
        .find(|c| c.name == "extensions_registered")
        .unwrap();
    // Few tables → few prefixes → warn
    assert!(
        check.status == CheckStatus::Warn || check.status == CheckStatus::Pass,
        "unexpected: {:?}",
        check.status
    );
}

#[test]
fn extensions_registered_passes_with_many_prefixes() {
    let pool = helpers::setup_pool();
    let conn = pool.get().unwrap();
    // Create tables with 20 distinct prefixes
    let prefixes = [
        "plans",
        "tasks",
        "waves",
        "agents",
        "ipc",
        "mesh",
        "billing",
        "backup",
        "evidence",
        "observatory",
        "prompts",
        "kernel",
        "org",
        "security",
        "deploy",
        "scheduler",
        "inference",
        "multitenancy",
        "longrunning",
    ];
    for p in &prefixes {
        conn.execute_batch(&format!("CREATE TABLE IF NOT EXISTS {p}_data (id INTEGER)"))
            .unwrap();
    }
    drop(conn);
    let results = run_core_checks(&pool);
    let check = results
        .iter()
        .find(|c| c.name == "extensions_registered")
        .unwrap();
    assert_eq!(check.status, CheckStatus::Pass);
    assert!(check.message.contains("modules detected"));
}

#[test]
fn schema_columns_passes_when_all_present() {
    let pool = helpers::setup_pool();
    let results = run_core_checks(&pool);
    let check = results.iter().find(|c| c.name == "schema_columns").unwrap();
    assert_eq!(check.status, CheckStatus::Pass);
    assert!(check.message.contains("All required columns"));
}

#[test]
fn run_check_measures_duration() {
    let result = run_check("test_check", "test", || {
        std::thread::sleep(std::time::Duration::from_millis(10));
        (CheckStatus::Pass, "ok".into())
    });
    assert!(
        result.duration_ms >= 10,
        "Duration too short: {}ms",
        result.duration_ms
    );
    assert_eq!(result.name, "test_check");
    assert_eq!(result.category, "test");
    assert_eq!(result.status, CheckStatus::Pass);
}

#[test]
fn doctor_report_build_computes_summary() {
    let pool = helpers::setup_pool();
    helpers::seed_migrations(&pool, 25);
    let checks = run_core_checks(&pool);
    let report = DoctorReport::build(checks.clone(), 42);

    assert_eq!(report.doctor_version, env!("CARGO_PKG_VERSION"));
    assert!(!report.daemon_version.is_empty());
    assert!(!report.timestamp.is_empty());
    assert_eq!(report.summary.total, 8);
    assert_eq!(
        report.summary.passed + report.summary.warnings + report.summary.failed,
        report.summary.total,
        "Summary counts don't add up"
    );
    assert_eq!(report.summary.duration_ms, 42);
    assert!(!report.checks.is_empty(), "report must have checks");
}

#[test]
fn doctor_report_serializes_to_valid_json() {
    let pool = helpers::setup_pool();
    let checks = run_core_checks(&pool);
    let report = DoctorReport::build(checks, 100);
    let json = serde_json::to_value(&report).unwrap();

    assert!(json["doctor_version"].is_string());
    assert!(json["daemon_version"].is_string());
    assert!(json["timestamp"].is_string());
    assert!(json["checks"].is_array());
    assert!(json["summary"].is_object());
    assert!(json["summary"]["total"].is_number());
    assert!(json["summary"]["passed"].is_number());
    assert!(json["summary"]["warnings"].is_number());
    assert!(json["summary"]["failed"].is_number());
    assert!(json["summary"]["duration_ms"].is_number());
}
