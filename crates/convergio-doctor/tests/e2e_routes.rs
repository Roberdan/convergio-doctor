//! E2E tests for convergio-doctor API routes.
//!
//! Verifies /api/doctor endpoints return valid JSON,
//! reports are saved to history, and error cases are handled.

mod helpers;

use axum::http::StatusCode;
use tower::ServiceExt;

#[tokio::test]
async fn get_doctor_returns_valid_report() {
    let pool = helpers::setup_pool();
    helpers::seed_migrations(&pool, 25);
    let app = helpers::build_router(&pool);

    let resp = app.oneshot(helpers::get_req("/api/doctor")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = helpers::body_json(resp).await;
    assert!(json["doctor_version"].is_string());
    assert_eq!(json["doctor_version"], env!("CARGO_PKG_VERSION"));
    assert!(json["daemon_version"].is_string());
    assert!(json["timestamp"].is_string());
    assert!(json["checks"].is_array());
    assert!(json["summary"].is_object());

    let checks = json["checks"].as_array().unwrap();
    assert!(!checks.is_empty(), "doctor must return at least one check");

    for check in checks {
        assert!(check["name"].is_string());
        assert!(check["category"].is_string());
        assert!(check["status"].is_string());
        assert!(check["message"].is_string());
        assert!(check["duration_ms"].is_number());
        let status = check["status"].as_str().unwrap();
        assert!(
            ["pass", "warn", "fail"].contains(&status),
            "Invalid status: {status}"
        );
    }

    let summary = &json["summary"];
    let total = summary["total"].as_u64().unwrap();
    let passed = summary["passed"].as_u64().unwrap();
    let warnings = summary["warnings"].as_u64().unwrap();
    let failed = summary["failed"].as_u64().unwrap();
    assert!(total > 0, "summary total must be > 0");
    assert_eq!(passed + warnings + failed, total);
}

#[tokio::test]
async fn get_doctor_version_endpoint() {
    let pool = helpers::setup_pool();
    let app = helpers::build_router(&pool);

    let resp = app
        .oneshot(helpers::get_req("/api/doctor/version"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = helpers::body_json(resp).await;
    assert_eq!(json["doctor_version"], env!("CARGO_PKG_VERSION"));
    assert!(json["daemon_version"].is_string());
    assert!(json["check_categories"].is_array());
}

#[tokio::test]
async fn get_doctor_check_core_category() {
    let pool = helpers::setup_pool();
    helpers::seed_migrations(&pool, 25);
    let app = helpers::build_router(&pool);

    let resp = app
        .oneshot(helpers::get_req("/api/doctor/check/core"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = helpers::body_json(resp).await;
    assert!(json["checks"].is_array());
    let core_count = json["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["category"] == "core")
        .count();
    assert_eq!(core_count, 8);
    assert!(json["summary"].is_object());
}

#[tokio::test]
async fn get_doctor_check_unknown_category_returns_error() {
    let pool = helpers::setup_pool();
    let app = helpers::build_router(&pool);

    let resp = app
        .oneshot(helpers::get_req("/api/doctor/check/nonexistent"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = helpers::body_json(resp).await;
    assert!(json["error"].is_string());
    assert!(json["error"].as_str().unwrap().contains("unknown category"));
}

#[tokio::test]
async fn get_doctor_history_empty() {
    let pool = helpers::setup_pool();
    let app = helpers::build_router(&pool);

    let resp = app
        .oneshot(helpers::get_req("/api/doctor/history"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = helpers::body_json(resp).await;
    assert!(json["reports"].is_array());
    assert_eq!(json["reports"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn doctor_run_saves_report_to_history() {
    let pool = helpers::setup_pool();
    helpers::seed_migrations(&pool, 25);

    // Run doctor — this should save a report
    let app1 = helpers::build_router(&pool);
    let resp1 = app1.oneshot(helpers::get_req("/api/doctor")).await.unwrap();
    assert_eq!(resp1.status(), StatusCode::OK);

    // Check history — should have 1 report
    let app2 = helpers::build_router(&pool);
    let resp2 = app2
        .oneshot(helpers::get_req("/api/doctor/history"))
        .await
        .unwrap();
    let json = helpers::body_json(resp2).await;
    let reports = json["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 1, "Expected 1 saved report");

    let report = &reports[0];
    assert!(report["id"].is_number());
    assert!(report["timestamp"].is_string());
    assert_eq!(report["version"], env!("CARGO_PKG_VERSION"));
    assert!(report["total_checks"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn multiple_doctor_runs_accumulate_history() {
    let pool = helpers::setup_pool();
    helpers::seed_migrations(&pool, 25);

    for _ in 0..3 {
        let app = helpers::build_router(&pool);
        let resp = app.oneshot(helpers::get_req("/api/doctor")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    let app = helpers::build_router(&pool);
    let resp = app
        .oneshot(helpers::get_req("/api/doctor/history"))
        .await
        .unwrap();
    let json = helpers::body_json(resp).await;
    let reports = json["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 3, "Expected 3 saved reports");
}

#[tokio::test]
async fn doctor_check_statuses_reflect_db_state() {
    let pool = helpers::setup_pool();
    // No migrations seeded — expect warnings
    let app = helpers::build_router(&pool);
    let resp = app.oneshot(helpers::get_req("/api/doctor")).await.unwrap();
    let json = helpers::body_json(resp).await;

    let summary = &json["summary"];
    let warnings = summary["warnings"].as_u64().unwrap();
    assert!(
        warnings > 0,
        "Expected warnings with empty DB, got {warnings}"
    );
}
