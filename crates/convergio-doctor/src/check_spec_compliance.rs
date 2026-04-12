//! Spec compliance check — verifies agent deliverables match the plan spec.

use crate::check_e2e_helpers::DoctorHttpClient;
use crate::checks::{run_check, CheckResult, CheckStatus};
use std::path::Path;

#[rustfmt::skip]
const DEP_KEYWORDS: &[&str] = &[
    "lancedb", "tantivy", "meilisearch", "qdrant", "redis", "postgres", "sqlite",
    "surrealdb", "clickhouse", "nats", "kafka", "rabbitmq", "wasm", "grpc", "tonic", "prost",
];
const REQ_SIGNALS: &[&str] = &["must", "required", "shall", "mandatory"];

pub fn run_spec_compliance_checks() -> Vec<CheckResult> {
    vec![check_completed_tasks_match_spec()]
}

fn check_completed_tasks_match_spec() -> CheckResult {
    run_check("spec_compliance", "beta", || {
        let client = DoctorHttpClient::new();
        let root = find_workspace_root();
        let plans = match fetch_plans(&client) {
            Ok(p) => p,
            Err(e) => return (CheckStatus::Warn, format!("cannot fetch plans: {e}")),
        };
        if plans.is_empty() {
            return (CheckStatus::Pass, "no active/done plans to check".into());
        }
        let mut total = 0usize;
        let mut violations: Vec<String> = Vec::new();
        for (plan_id, _) in &plans {
            let tasks = match fetch_done_tasks(&client, *plan_id) {
                Ok(t) => t,
                Err(_) => continue,
            };
            for task in &tasks {
                total += 1;
                violations.extend(verify_task(task, &root));
            }
        }
        if total == 0 {
            return (CheckStatus::Pass, "no completed tasks to verify".into());
        }
        if violations.is_empty() {
            return (CheckStatus::Pass, format!("{total} tasks match specs"));
        }
        let critical = violations.iter().any(|v| v.starts_with("[FAIL]"));
        let status = if critical {
            CheckStatus::Fail
        } else {
            CheckStatus::Warn
        };
        let msg: Vec<_> = violations.iter().take(5).cloned().collect();
        let extra = if violations.len() > 5 {
            format!(" (+{})", violations.len() - 5)
        } else {
            String::new()
        };
        (status, format!("{}{extra}", msg.join("; ")))
    })
}

struct TaskInfo {
    title: String,
    description: String,
    plan_name: String,
}

fn fetch_plans(client: &DoctorHttpClient) -> Result<Vec<(i64, String)>, String> {
    let (status, body) = client.get("/api/plan-db/list")?;
    if status >= 400 {
        return Err(format!("HTTP {status}"));
    }
    let plans = body["plans"]
        .as_array()
        .or(body.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(plans
        .iter()
        .filter_map(|p| {
            let id = p["id"].as_i64().unwrap_or(0);
            let st = p["status"].as_str().unwrap_or("");
            let pid = p["project_id"].as_str().unwrap_or("");
            if pid.contains("_doctor_test") {
                return None;
            }
            if st == "done" || st == "in_progress" {
                Some((id, p["name"].as_str().unwrap_or("").into()))
            } else {
                None
            }
        })
        .collect())
}

fn fetch_done_tasks(client: &DoctorHttpClient, plan_id: i64) -> Result<Vec<TaskInfo>, String> {
    let (status, body) = client.get(&format!("/api/plan-db/execution-tree/{plan_id}"))?;
    if status >= 400 {
        return Err(format!("HTTP {status}"));
    }
    let plan_name = body["plan"]["name"]
        .as_str()
        .or(body["name"].as_str())
        .unwrap_or("unknown")
        .to_string();
    let waves = body["waves"].as_array().cloned().unwrap_or_default();
    let mut tasks = Vec::new();
    for wave in &waves {
        for t in wave["tasks"].as_array().cloned().unwrap_or_default() {
            let st = t["status"].as_str().unwrap_or("");
            if st == "done" || st == "submitted" {
                tasks.push(TaskInfo {
                    title: t["title"].as_str().unwrap_or("").into(),
                    description: t["description"]
                        .as_str()
                        .or(t["notes"].as_str())
                        .unwrap_or("")
                        .into(),
                    plan_name: plan_name.clone(),
                });
            }
        }
    }
    Ok(tasks)
}

fn verify_task(task: &TaskInfo, root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let text = format!("{} {}", task.title, task.description).to_lowercase();
    check_crate_exists(task, &text, root, &mut out);
    check_dep_declared(task, &text, root, &mut out);
    check_file_created(task, &text, root, &mut out);
    check_route_present(task, &text, root, &mut out);
    out
}

fn severity(text: &str) -> &'static str {
    if REQ_SIGNALS.iter().any(|s| text.contains(s)) {
        "[FAIL]"
    } else {
        "[WARN]"
    }
}

fn check_crate_exists(task: &TaskInfo, text: &str, root: &Path, out: &mut Vec<String>) {
    for word in text.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
        if clean.starts_with("convergio-") && clean.len() > 10 {
            let toml = root.join("daemon/crates").join(clean).join("Cargo.toml");
            if !toml.exists() {
                out.push(format!(
                    "{} task '{}' (plan '{}') mentions crate `{clean}` but {} missing",
                    severity(text),
                    task.title,
                    task.plan_name,
                    toml.display(),
                ));
            }
        }
    }
}

fn check_dep_declared(task: &TaskInfo, text: &str, root: &Path, out: &mut Vec<String>) {
    for &dep in DEP_KEYWORDS {
        if !text.contains(dep) {
            continue;
        }
        let crate_name = extract_crate_name(text);
        let toml_path = match &crate_name {
            Some(n) => root.join("daemon/crates").join(n).join("Cargo.toml"),
            None => root.join("daemon/Cargo.toml"),
        };
        if !toml_path.exists() {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&toml_path) else {
            continue;
        };
        if !contents.to_lowercase().contains(dep) {
            let target = crate_name.as_deref().unwrap_or("workspace");
            out.push(format!(
                "{} task '{}' (plan '{}') requires `{dep}` but not in {target}/Cargo.toml",
                severity(text),
                task.title,
                task.plan_name,
            ));
        }
    }
}

fn check_file_created(task: &TaskInfo, text: &str, root: &Path, out: &mut Vec<String>) {
    for pat in ["create file ", "add file ", "new file "] {
        for seg in text.split(pat).skip(1) {
            let candidate = seg
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches(|c: char| c == '`' || c == '\'' || c == '"');
            // Path traversal mitigation: reject candidates that escape the workspace
            if candidate.contains("..") {
                continue;
            }
            let full = root.join(candidate);
            let Ok(canonical_root) = root.canonicalize() else {
                continue;
            };
            // Only check existence if the resolved path stays within the workspace root
            if candidate.contains('/') && candidate.contains('.') {
                if full
                    .canonicalize()
                    .map(|p| !p.starts_with(&canonical_root))
                    .unwrap_or(false)
                {
                    continue;
                }
                if !full.exists() {
                    out.push(format!(
                        "[WARN] task '{}' (plan '{}') mentions creating `{candidate}` but not found",
                        task.title, task.plan_name,
                    ));
                }
            }
        }
    }
}

fn check_route_present(task: &TaskInfo, text: &str, root: &Path, out: &mut Vec<String>) {
    for seg in text.split("route /api/").skip(1) {
        let rp = seg
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches([',', '.', ')']);
        if rp.is_empty() {
            continue;
        }
        let full = format!("/api/{rp}");
        let Some(crate_name) = extract_crate_name(text) else {
            continue;
        };
        let rf = root
            .join("daemon/crates")
            .join(&crate_name)
            .join("src/routes.rs");
        if !rf.exists() {
            continue;
        }
        let Ok(c) = std::fs::read_to_string(&rf) else {
            continue;
        };
        if !c.contains(&full) && !c.contains(rp) {
            out.push(format!(
                "[WARN] task '{}' (plan '{}') mentions `{full}` but not in {}",
                task.title,
                task.plan_name,
                rf.display(),
            ));
        }
    }
}

fn extract_crate_name(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '-'))
        .find(|w| w.starts_with("convergio-") && w.len() > 10)
        .map(|s| s.to_string())
}

fn find_workspace_root() -> std::path::PathBuf {
    let start = std::env::var("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
    let mut c = start.clone();
    for _ in 0..8 {
        if c.join("daemon").is_dir() {
            return c;
        }
        if !c.pop() {
            break;
        }
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_crate_name_works() {
        assert_eq!(
            extract_crate_name("implement convergio-vector store with lancedb"),
            Some("convergio-vector".into()),
        );
        assert_eq!(extract_crate_name("add redis support"), None);
    }

    #[test]
    fn verify_task_empty_title_no_findings() {
        let task = TaskInfo {
            title: String::new(),
            description: String::new(),
            plan_name: "t".into(),
        };
        assert!(verify_task(&task, Path::new("/nonexistent")).is_empty());
    }

    #[test]
    fn severity_and_keywords() {
        assert_eq!(severity("must implement lancedb"), "[FAIL]");
        assert_eq!(severity("implement lancedb"), "[WARN]");
        for kw in DEP_KEYWORDS {
            assert_eq!(*kw, kw.to_lowercase());
        }
    }
}
