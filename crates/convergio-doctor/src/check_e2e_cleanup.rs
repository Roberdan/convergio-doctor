//! Cleanup and reachability helpers shared by doctor E2E checks.

use crate::check_e2e_helpers::{
    matches_test_marker, DoctorHttpClient, TEST_PREFIX, TEST_SLUG_PREFIX,
};

/// Cancel and purge a doctor test plan — leaves zero trace in the DB.
/// Must be called after every E2E check that creates a plan.
pub fn purge_doctor_plan(client: &DoctorHttpClient, plan_id: i64) {
    let _ = client.post_json(
        &format!("/api/plan-db/cancel/{plan_id}"),
        &serde_json::json!({}),
    );
    for status in &[
        "cancelled",
        "in_progress",
        "paused",
        "todo",
        "done",
        "pending",
        "failed",
        "stale",
    ] {
        let _ = client.post_json(
            "/api/plan-db/purge",
            &serde_json::json!({"status": status, "name_prefix": TEST_PREFIX}),
        );
    }
}

/// Nuclear cleanup: delete ALL doctor test data from the entire system.
/// Covers both raw `_doctor_test_` markers and slugified `doctor-test-` org IDs.
pub fn purge_all_doctor_data(client: &DoctorHttpClient) -> (usize, Vec<String>) {
    let mut total = 0usize;
    let mut cleaned = Vec::new();

    for status in &[
        "cancelled",
        "in_progress",
        "paused",
        "todo",
        "done",
        "pending",
        "failed",
        "stale",
        "reviewed",
        "approved",
    ] {
        if let Ok((_, body)) = client.post_json(
            "/api/plan-db/purge",
            &serde_json::json!({"status": status, "name_prefix": TEST_PREFIX}),
        ) {
            let n = body["purged"].as_u64().unwrap_or(0) as usize;
            if n > 0 {
                total += n;
                cleaned.push(format!("plans({status}):{n}"));
            }
        }
    }

    if let Ok((_, body)) = client.get("/api/orgs") {
        if let Some(orgs) = body["orgs"].as_array() {
            for org in orgs {
                let oid = org["id"].as_str().unwrap_or("");
                if matches_test_marker(oid) || oid == "audit-test-org" {
                    if let Ok((_, deleted)) = client.delete(&format!("/api/orgs/{oid}")) {
                        total += 1;
                        cleaned.push(format!("org:{oid}"));
                        if let Some(map) = deleted["deleted"].as_object() {
                            for (label, value) in map {
                                let count = value.as_u64().unwrap_or(0) as usize;
                                if count > 0 {
                                    total += count;
                                    cleaned.push(format!("{label}:{oid}:{count}"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Ok((200, body)) = client.get("/api/agents/catalog") {
        if let Some(agents) = body.as_array() {
            for agent in agents {
                let name = agent["name"].as_str().unwrap_or("");
                let org = agent["org"]
                    .as_str()
                    .or_else(|| agent["org_id"].as_str())
                    .unwrap_or("");
                if (matches_test_marker(name) || matches_test_marker(org))
                    && client
                        .delete(&format!("/api/agents/catalog/{name}"))
                        .is_ok()
                {
                    total += 1;
                    cleaned.push(format!("catalog:{name}"));
                }
            }
        }
    }

    if let Ok((_, body)) = client.get("/api/agents/runtime") {
        if let Some(agents) = body["active_agents"].as_array() {
            for agent in agents {
                let name = agent["agent_name"].as_str().unwrap_or("");
                let org = agent["org_id"].as_str().unwrap_or("");
                if matches_test_marker(name) || matches_test_marker(org) {
                    total += 1;
                    cleaned.push(format!("agent:{name}"));
                }
            }
        }
    }

    (total, cleaned)
}

/// Check if a peer is reachable via HTTP health endpoint.
pub fn check_peer_online(base_url: &str) -> bool {
    // SSRF mitigation: only allow http(s)
    if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
        return false;
    }
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build();
    let Ok(client) = client else { return false };
    let url = format!("{base_url}/api/health");
    client
        .get(&url)
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Delete all test data from all tables. Returns (total_deleted, table_names).
pub fn cleanup_test_data(pool: &convergio_db::pool::ConnPool) -> (usize, Vec<String>) {
    let Ok(conn) = pool.get() else {
        return (0, vec![]);
    };
    let mut total = 0usize;
    let mut tables = Vec::new();

    let test_plan_ids: Vec<i64> = conn
        .prepare(
            "SELECT id FROM plans WHERE project_id = '_doctor_test_proj' \
             OR name LIKE '%\\_doctor\\_test\\_%' ESCAPE '\\' \
             OR name LIKE '%doctor-test-%'",
        )
        .and_then(|mut s| {
            s.query_map([], |r| r.get(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    if !test_plan_ids.is_empty() {
        let ids_csv = test_plan_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        for (table, col) in &[
            ("tasks", "plan_id"),
            ("waves", "plan_id"),
            ("task_evidence", "task_db_id"),
            ("task_status_log", "task_db_id"),
        ] {
            let sql = if *table == "task_evidence" || *table == "task_status_log" {
                format!(
                    "DELETE FROM \"{table}\" WHERE \"{col}\" IN \
                     (SELECT id FROM tasks WHERE plan_id IN ({ids_csv}))"
                )
            } else {
                format!("DELETE FROM \"{table}\" WHERE \"{col}\" IN ({ids_csv})")
            };
            if let Ok(deleted) = conn.execute(&sql, []) {
                if deleted > 0 {
                    total += deleted;
                    tables.push((*table).to_string());
                }
            }
        }
    }

    let Ok(mut stmt) = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
    else {
        return (total, tables);
    };
    let table_names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    for table in &table_names {
        // Sanitize: SQLite identifiers must be alphanumeric/underscore only
        if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        let Ok(mut info) = conn.prepare(&format!("PRAGMA table_info(\"{table}\")")) else {
            continue;
        };
        let text_cols: Vec<String> = info
            .query_map([], |r| {
                let name: String = r.get(1)?;
                let typ: String = r.get(2)?;
                Ok((name, typ))
            })
            .ok()
            .map(|rows| {
                rows.filter_map(|r| r.ok())
                    .filter(|(_, t)| t.to_uppercase().contains("TEXT"))
                    .map(|(n, _)| n)
                    .collect()
            })
            .unwrap_or_default();

        for col in &text_cols {
            // Sanitize column names
            if !col.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            let sql = format!(
                "DELETE FROM \"{table}\" WHERE \"{col}\" LIKE '%{TEST_PREFIX}%' \
                 OR \"{col}\" LIKE '%{TEST_SLUG_PREFIX}%'"
            );
            if let Ok(deleted) = conn.execute(&sql, []) {
                if deleted > 0 {
                    total += deleted;
                    if !tables.contains(table) {
                        tables.push(table.clone());
                    }
                }
            }
        }
    }

    (total, tables)
}
