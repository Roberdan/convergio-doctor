//! Doctor API routes — /api/doctor endpoints.

use crate::check_advanced;
use crate::checks::{self, DoctorReport};
use crate::runtime::DoctorRuntime;
use axum::extract::{Path, Query};
use axum::routing::get;
use axum::{Json, Router};
use convergio_db::pool::ConnPool;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Instant;

#[derive(Debug, Deserialize)]
pub struct FormatQuery {
    pub format: Option<String>,
}

pub fn doctor_routes(pool: ConnPool, runtime: DoctorRuntime) -> Router {
    let (p1, r1) = (pool.clone(), runtime.clone());
    let (p2, r2) = (pool.clone(), runtime.clone());
    let (p3, r3) = (pool.clone(), runtime.clone());
    let p4 = pool.clone();
    let p5 = pool.clone();
    let p6 = pool.clone();
    let p7 = pool.clone();

    Router::new()
        .route("/api/doctor", get(move || run_full_doctor(p1, r1)))
        .route(
            "/api/doctor/full",
            get(move || run_full_doctor_with_e2e(p2, r2)),
        )
        .route(
            "/api/doctor/check/:category",
            get(move |path: Path<String>| run_category(p3, r3, path)),
        )
        .route("/api/doctor/summary", get(move || handle_summary(p4)))
        .route(
            "/api/doctor/issues",
            get(move |q: Query<FormatQuery>| handle_issues(p5, q)),
        )
        .route("/api/doctor/dashboard", get(move || handle_dashboard(p6)))
        .route(
            "/api/doctor/version",
            get(|| async {
                Json(json!({
                    "doctor_version": crate::DOCTOR_VERSION,
                    "daemon_version": env!("CARGO_PKG_VERSION"),
                    "check_categories": ["core", "extensions", "beta", "e2e", "chaos"],
                }))
            }),
        )
        .route("/api/doctor/history", get(move || get_history(p7)))
}

async fn run_full_doctor(pool: ConnPool, runtime: DoctorRuntime) -> Json<Value> {
    let start = Instant::now();
    let mut all = checks::run_core_checks(&pool);
    all.extend(check_advanced::run_advanced_checks(&pool, &runtime));
    all.extend(crate::check_beta::run_beta_checks(&pool));
    all.extend(crate::check_projects::run_project_checks(&pool));
    let duration = start.elapsed().as_millis() as u64;
    let report = DoctorReport::build(all, duration);
    save_report(&pool, &report);
    Json(serde_json::to_value(report).unwrap_or(json!({"error": "serialization failed"})))
}

async fn run_full_doctor_with_e2e(pool: ConnPool, runtime: DoctorRuntime) -> Json<Value> {
    let start = Instant::now();
    // Core checks are sync-safe (SQLite only, no HTTP)
    let mut all = checks::run_core_checks(&pool);
    all.extend(check_advanced::run_advanced_checks(&pool, &runtime));
    all.extend(crate::check_beta::run_beta_checks(&pool));
    all.extend(crate::check_projects::run_project_checks(&pool));

    // E2E + chaos checks use reqwest::blocking — must run in spawn_blocking
    let pool2 = pool.clone();
    let e2e_results = tokio::task::spawn_blocking(move || {
        let mut v = crate::check_e2e_smoke::run_e2e_smoke_checks();
        v.extend(crate::check_e2e_smoke_ext::run_e2e_smoke_ext_checks());
        v.extend(crate::check_e2e_plan::run_e2e_plan_checks(&pool2));
        v.extend(crate::check_e2e_ipc::run_e2e_ipc_checks());
        v.extend(crate::check_e2e_org::run_e2e_org_checks());
        v.extend(crate::check_e2e_org_flow::run_e2e_org_flow_checks());
        v.extend(crate::check_e2e_mesh::run_e2e_mesh_checks(&pool2));
        v.extend(crate::check_e2e_security::run_e2e_security_checks());
        v.extend(crate::check_e2e_gates::run_e2e_gate_hardening_checks());
        v.extend(crate::check_spec_compliance::run_spec_compliance_checks());
        v.extend(crate::check_chaos_db::run_chaos_db_checks(&pool2));
        v.extend(crate::check_chaos_net::run_chaos_net_checks());
        v.extend(crate::check_chaos_daemon::run_chaos_daemon_checks());
        v.extend(crate::check_cleanup::run_cleanup_checks(&pool2));
        v
    })
    .await
    .unwrap_or_default();
    all.extend(e2e_results);

    let duration = start.elapsed().as_millis() as u64;
    let report = DoctorReport::build(all, duration);
    save_report(&pool, &report);
    Json(serde_json::to_value(report).unwrap_or(json!({"error": "serialization failed"})))
}

async fn run_category(
    pool: ConnPool,
    runtime: DoctorRuntime,
    Path(category): Path<String>,
) -> Json<Value> {
    let start = Instant::now();
    let checks = match category.as_str() {
        "core" => checks::run_core_checks(&pool),
        "extensions" => check_advanced::run_advanced_checks(&pool, &runtime),
        "beta" => crate::check_beta::run_beta_checks(&pool),
        "e2e" => {
            let p = pool.clone();
            tokio::task::spawn_blocking(move || {
                let mut v = crate::check_e2e_smoke::run_e2e_smoke_checks();
                v.extend(crate::check_e2e_smoke_ext::run_e2e_smoke_ext_checks());
                v.extend(crate::check_e2e_plan::run_e2e_plan_checks(&p));
                v.extend(crate::check_e2e_ipc::run_e2e_ipc_checks());
                v.extend(crate::check_e2e_org::run_e2e_org_checks());
                v.extend(crate::check_e2e_org_flow::run_e2e_org_flow_checks());
                v.extend(crate::check_e2e_mesh::run_e2e_mesh_checks(&p));
                v.extend(crate::check_e2e_security::run_e2e_security_checks());
                v.extend(crate::check_e2e_gates::run_e2e_gate_hardening_checks());
                v.extend(crate::check_spec_compliance::run_spec_compliance_checks());
                v.extend(crate::check_cleanup::run_cleanup_checks(&p));
                v
            })
            .await
            .unwrap_or_default()
        }
        "chaos" => {
            let p = pool.clone();
            tokio::task::spawn_blocking(move || {
                let mut v = crate::check_chaos_db::run_chaos_db_checks(&p);
                v.extend(crate::check_chaos_net::run_chaos_net_checks());
                v.extend(crate::check_chaos_daemon::run_chaos_daemon_checks());
                v
            })
            .await
            .unwrap_or_default()
        }
        "cleanup" => crate::check_cleanup::run_cleanup_checks(&pool),
        other => return Json(json!({"error": format!("unknown category: {other}")})),
    };
    let duration = start.elapsed().as_millis() as u64;
    let report = DoctorReport::build(checks, duration);
    Json(serde_json::to_value(report).unwrap_or(json!({"error": "serialization failed"})))
}

async fn handle_summary(pool: ConnPool) -> Json<Value> {
    let start = Instant::now();
    let mut all = checks::run_core_checks(&pool);
    all.extend(crate::check_beta::run_beta_checks(&pool));
    let d = start.elapsed().as_millis() as u64;
    let r = DoctorReport::build(all, d);
    Json(json!({
        "status": if r.summary.failed > 0 { "fail" } else if r.summary.warnings > 0 { "warn" } else { "pass" },
        "total": r.summary.total, "passed": r.summary.passed,
        "warnings": r.summary.warnings, "failed": r.summary.failed,
        "duration_ms": r.summary.duration_ms,
    }))
}

async fn handle_issues(pool: ConnPool, Query(q): Query<FormatQuery>) -> Json<Value> {
    let mut all = checks::run_core_checks(&pool);
    all.extend(crate::check_beta::run_beta_checks(&pool));
    let compact = q.format.as_deref() == Some("compact");
    let issues: Vec<Value> = all
        .into_iter()
        .filter(|c| c.status != checks::CheckStatus::Pass)
        .map(|c| {
            if compact {
                json!({"name": c.name, "category": c.category, "status": c.status})
            } else {
                json!({"name": c.name, "category": c.category, "status": c.status, "message": c.message})
            }
        })
        .collect();
    let count = issues.len();
    Json(json!({"issues": issues, "count": count}))
}

async fn handle_dashboard(pool: ConnPool) -> Json<Value> {
    let start = Instant::now();
    let mut all = checks::run_core_checks(&pool);
    all.extend(crate::check_beta::run_beta_checks(&pool));
    let d = start.elapsed().as_millis() as u64;
    let current = DoctorReport::build(all, d);
    let history = get_history_rows(&pool);
    let trend = compute_trend(&history);
    Json(json!({
        "current": current,
        "history": history,
        "trend": trend,
    }))
}

fn compute_trend(history: &[Value]) -> &'static str {
    if history.len() < 2 {
        return "stable";
    }
    let recent = history[0]["failed"].as_i64().unwrap_or(0);
    let prev = history[1]["failed"].as_i64().unwrap_or(0);
    if recent < prev {
        "improving"
    } else if recent > prev {
        "degrading"
    } else {
        "stable"
    }
}

fn get_history_rows(pool: &ConnPool) -> Vec<Value> {
    let Ok(conn) = pool.get() else { return vec![] };
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, timestamp, version, daemon_version, \
         total_checks, passed, warnings, failed, duration_ms \
         FROM doctor_reports ORDER BY id DESC LIMIT 20",
    ) else {
        return vec![];
    };
    stmt.query_map([], |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?, "timestamp": row.get::<_, String>(1)?,
            "version": row.get::<_, String>(2)?, "daemon_version": row.get::<_, String>(3)?,
            "total_checks": row.get::<_, i64>(4)?, "passed": row.get::<_, i64>(5)?,
            "warnings": row.get::<_, i64>(6)?, "failed": row.get::<_, i64>(7)?,
            "duration_ms": row.get::<_, i64>(8)?,
        }))
    })
    .ok()
    .map(|iter| iter.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

async fn get_history(pool: ConnPool) -> Json<Value> {
    Json(json!({"reports": get_history_rows(&pool)}))
}

fn save_report(pool: &ConnPool, report: &DoctorReport) {
    let Ok(conn) = pool.get() else { return };
    let json_str = serde_json::to_string(report).unwrap_or_default();
    let _ = conn.execute(
        "INSERT INTO doctor_reports \
         (version, daemon_version, total_checks, passed, warnings, failed, duration_ms, report_json) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            report.doctor_version, report.daemon_version,
            report.summary.total, report.summary.passed,
            report.summary.warnings, report.summary.failed,
            report.summary.duration_ms, json_str,
        ],
    );
}
