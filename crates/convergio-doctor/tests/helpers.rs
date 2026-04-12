//! Shared test helpers for convergio-doctor E2E tests.
#![allow(dead_code)]

use axum::body::Body;
use axum::http::Request;
use convergio_db::pool::ConnPool;

/// Create an in-memory pool and apply required schemas for doctor checks.
pub fn setup_pool() -> ConnPool {
    let pool = convergio_db::pool::create_memory_pool().unwrap();
    let conn = pool.get().unwrap();

    // Doctor extension migration
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS doctor_reports (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            version TEXT NOT NULL,
            daemon_version TEXT NOT NULL,
            total_checks INTEGER NOT NULL DEFAULT 0,
            passed INTEGER NOT NULL DEFAULT 0,
            warnings INTEGER NOT NULL DEFAULT 0,
            failed INTEGER NOT NULL DEFAULT 0,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            report_json TEXT NOT NULL DEFAULT '{}'
        );",
    )
    .unwrap();

    // Tables required by doctor core checks
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS applied_migrations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            module TEXT NOT NULL,
            version INTEGER NOT NULL,
            applied_at TEXT DEFAULT (datetime('now'))
        );",
    )
    .unwrap();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL,
            plan_id INTEGER NOT NULL,
            wave_id INTEGER NOT NULL DEFAULT 0,
            title TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'pending',
            metadata TEXT NOT NULL DEFAULT '{}'
        );",
    )
    .unwrap();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS plans (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id TEXT NOT NULL DEFAULT '',
            name TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'pending',
            tasks_done INTEGER NOT NULL DEFAULT 0,
            tasks_total INTEGER NOT NULL DEFAULT 0
        );",
    )
    .unwrap();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS waves (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            wave_id INTEGER NOT NULL DEFAULT 0,
            plan_id INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'pending'
        );",
    )
    .unwrap();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS plan_metadata (
            plan_id INTEGER PRIMARY KEY,
            objective TEXT NOT NULL DEFAULT '',
            motivation TEXT NOT NULL DEFAULT '',
            requester TEXT NOT NULL DEFAULT '',
            worktree_path TEXT NOT NULL DEFAULT ''
        );",
    )
    .unwrap();

    // Tables required by beta doctor checks
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS prompt_templates (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, version INTEGER DEFAULT 1,
            body TEXT NOT NULL, variables TEXT DEFAULT '[]', category TEXT,
            active INTEGER DEFAULT 1, created_at TEXT DEFAULT '', updated_at TEXT DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS prompt_skills (
            id TEXT PRIMARY KEY, agent TEXT NOT NULL, host TEXT NOT NULL,
            capability TEXT NOT NULL, confidence REAL DEFAULT 0.5,
            description TEXT DEFAULT '', last_used TEXT, registered_at TEXT DEFAULT ''
        );
        CREATE TABLE IF NOT EXISTS agent_catalog (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, role TEXT NOT NULL,
            org_id TEXT DEFAULT 'convergio', category TEXT NOT NULL,
            model_tier TEXT DEFAULT 't2', max_tokens INTEGER DEFAULT 200000,
            hourly_budget REAL DEFAULT 0.0, capabilities_json TEXT DEFAULT '[]',
            prompt_ref TEXT, escalation_target TEXT, status TEXT DEFAULT 'active',
            created_at TEXT DEFAULT '', updated_at TEXT DEFAULT ''
        );",
    )
    .unwrap();

    drop(conn);
    pool
}

/// Seed the pool with sample migrations to make checks pass.
pub fn seed_migrations(pool: &ConnPool, module_count: usize) {
    let conn = pool.get().unwrap();
    for i in 0..module_count {
        conn.execute(
            "INSERT INTO applied_migrations (module, version) VALUES (?1, 1)",
            rusqlite::params![format!("module-{i}")],
        )
        .unwrap();
    }
}

/// Build doctor router from the pool.
pub fn build_router(pool: &ConnPool) -> axum::Router {
    let runtime = convergio_doctor::DoctorRuntime::new();
    convergio_doctor::routes::doctor_routes(pool.clone(), runtime)
}

/// Parse response body as JSON.
pub async fn body_json(resp: axum::http::Response<Body>) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Build a GET request.
pub fn get_req(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}
