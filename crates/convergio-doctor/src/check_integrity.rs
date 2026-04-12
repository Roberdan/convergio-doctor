//! Integrity checks — schema columns, self-containment, source scanning.
//!
//! Split from checks.rs to stay under 250 lines.

use super::checks::CheckStatus;

/// Verify that critical table columns actually exist in the schema.
/// Catches bugs like the tasks.metadata missing column incident.
pub fn check_required_columns(pool: &convergio_db::pool::ConnPool) -> (CheckStatus, String) {
    let required: &[(&str, &[&str])] = &[
        (
            "tasks",
            &[
                "id", "task_id", "plan_id", "wave_id", "title", "status", "metadata",
            ],
        ),
        (
            "plans",
            &[
                "id",
                "project_id",
                "name",
                "status",
                "tasks_done",
                "tasks_total",
            ],
        ),
        ("waves", &["id", "wave_id", "plan_id", "status"]),
        (
            "plan_metadata",
            &[
                "plan_id",
                "objective",
                "motivation",
                "requester",
                "worktree_path",
            ],
        ),
    ];
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => return (CheckStatus::Fail, format!("DB: {e}")),
    };
    let mut missing = Vec::new();
    for (table, columns) in required {
        for col in *columns {
            let sql = format!("SELECT {col} FROM {table} LIMIT 0");
            if conn.execute(&sql, []).is_err() {
                missing.push(format!("{table}.{col}"));
            }
        }
    }
    if missing.is_empty() {
        (CheckStatus::Pass, "All required columns present".into())
    } else {
        (
            CheckStatus::Fail,
            format!("Missing columns: {}", missing.join(", ")),
        )
    }
}

/// Scan source files for hardcoded user-specific paths.
/// Convergio must be fully self-contained and portable.
pub fn check_self_containment() -> (CheckStatus, String) {
    // Build patterns at runtime to avoid this file flagging itself.
    let home_mac = format!("/Users/{}", "Roberdan");
    let home_linux = format!("/home/{}", "roberdan");
    let legacy_repo = format!("Convergio{}", "Platform");
    let patterns = [home_mac.as_str(), home_linux.as_str(), legacy_repo.as_str()];
    let scan_dirs = ["daemon/crates", "scripts", "claude-config"];
    let skip_dirs = ["target", ".git", "docs/history", "claude-config/reference"];
    let mut violations = Vec::new();

    for dir in &scan_dirs {
        let path = std::path::Path::new(dir);
        if !path.exists() {
            continue;
        }
        scan_dir_for_patterns(path, &patterns, &skip_dirs, &mut violations);
    }

    if violations.is_empty() {
        (
            CheckStatus::Pass,
            "No hardcoded paths or legacy references".into(),
        )
    } else {
        let msg = format!("{} violations: {}", violations.len(), violations.join(", "));
        (CheckStatus::Fail, msg)
    }
}

fn scan_dir_for_patterns(
    dir: &std::path::Path,
    patterns: &[&str],
    skip: &[&str],
    violations: &mut Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let path_str = path.display().to_string();
        if skip.iter().any(|s| path_str.contains(s)) {
            continue;
        }
        if path.is_dir() {
            scan_dir_for_patterns(&path, patterns, skip, violations);
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(
                ext,
                "rs" | "sh" | "toml" | "yml" | "yaml" | "json" | "plist" | "md"
            ) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                for pat in patterns {
                    if content.contains(pat) {
                        violations.push(format!("{}:{}", path.display(), pat));
                    }
                }
            }
        }
    }
}
