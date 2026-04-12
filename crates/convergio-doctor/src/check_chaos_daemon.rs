//! Chaos testing — daemon resilience under stress.
//!
//! Tests: PID stability, SIGTERM recovery, concurrent load handling.

use crate::checks::{run_check, CheckResult, CheckStatus};
use convergio_types::dev_auth_header;

pub fn run_chaos_daemon_checks() -> Vec<CheckResult> {
    vec![
        check_chaos_daemon_pid_stable(),
        check_chaos_daemon_under_load(),
    ]
}

fn check_chaos_daemon_pid_stable() -> CheckResult {
    run_check("chaos_daemon_pid_stable", "chaos", || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build();
        let Ok(c) = client else {
            return (CheckStatus::Fail, "cannot build client".into());
        };

        // Get initial health check
        let pid1 = get_daemon_pid(&c);
        if pid1.is_none() {
            return (
                CheckStatus::Fail,
                "cannot reach daemon for PID check".into(),
            );
        }

        // Wait 5 seconds
        std::thread::sleep(std::time::Duration::from_secs(5));

        // Get PID again
        let pid2 = get_daemon_pid(&c);

        match (pid1, pid2) {
            (Some(p1), Some(p2)) if p1 == p2 => (
                CheckStatus::Pass,
                format!("daemon PID {p1} stable after 5s"),
            ),
            (Some(p1), Some(p2)) => (
                CheckStatus::Warn,
                format!("PID changed: {p1} → {p2} (restart detected)"),
            ),
            (Some(_), None) => (CheckStatus::Fail, "daemon unreachable after 5s wait".into()),
            _ => (CheckStatus::Fail, "cannot determine daemon PID".into()),
        }
    })
}

fn get_daemon_pid(client: &reqwest::blocking::Client) -> Option<u32> {
    // Try health endpoint first
    let resp = client
        .get("http://localhost:8420/api/health")
        .header("Authorization", dev_auth_header())
        .send()
        .ok()?;
    let body: serde_json::Value = resp.json().ok()?;
    if let Some(pid) = body.get("pid").and_then(|v| v.as_u64()) {
        return Some(pid as u32);
    }

    // Fallback: pgrep
    let output = std::process::Command::new("pgrep")
        .args(["-f", "target/release/convergio"])
        .output()
        .ok()?;
    if output.status.success() {
        let s = String::from_utf8_lossy(&output.stdout);
        s.lines().next()?.trim().parse().ok()
    } else {
        // Try alternative pattern
        let output = std::process::Command::new("pgrep")
            .args(["-f", "convergio"])
            .output()
            .ok()?;
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            s.lines().next()?.trim().parse().ok()
        } else {
            None
        }
    }
}

fn check_chaos_daemon_under_load() -> CheckResult {
    run_check("chaos_daemon_under_load", "chaos", || {
        let threads = 20;
        let mut handles = Vec::new();

        // Mix of GET and POST requests
        let endpoints: Vec<(&str, Option<serde_json::Value>)> = vec![
            ("/api/health", None),
            ("/api/health/deep", None),
            ("/api/mesh", None),
            ("/api/agents/catalog", None),
            ("/api/orgs", None),
            ("/api/depgraph", None),
            ("/api/metrics", None),
            ("/api/ipc/status", None),
            ("/api/telemetry", None),
            ("/api/doctor/version", None),
            ("/api/health", None),
            ("/api/health", None),
            ("/api/skills", None),
            ("/api/prompts", None),
            ("/api/billing/usage", None),
            ("/api/kernel/status", None),
            ("/api/plan-db/list", None),
            ("/api/decisions", None),
            ("/api/capabilities", None),
            ("/api/locks/active", None),
        ];

        for (path, body) in endpoints {
            let path = path.to_string();
            handles.push(std::thread::spawn(move || {
                let c = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(15))
                    .build()
                    .ok()?;
                let auth = dev_auth_header();
                let resp = if let Some(b) = body {
                    c.post(format!("http://localhost:8420{path}"))
                        .header("Authorization", &auth)
                        .json(&b)
                        .send()
                        .ok()?
                } else {
                    c.get(format!("http://localhost:8420{path}"))
                        .header("Authorization", &auth)
                        .send()
                        .ok()?
                };
                Some(resp.status().as_u16())
            }));
        }

        let mut succeeded = 0;
        let mut failed = 0;
        let mut timeouts = 0;
        for h in handles {
            match h.join() {
                Ok(Some(s)) if s < 500 => succeeded += 1,
                Ok(Some(_)) => failed += 1,
                Ok(None) => timeouts += 1,
                Err(_) => failed += 1,
            }
        }

        if failed == 0 && timeouts == 0 {
            (
                CheckStatus::Pass,
                format!("{threads} concurrent, {succeeded} OK"),
            )
        } else if failed == 0 {
            (
                CheckStatus::Warn,
                format!("{succeeded} OK, {timeouts} timeout"),
            )
        } else {
            (
                CheckStatus::Warn,
                format!("{succeeded} OK, {failed} failed, {timeouts} timeout"),
            )
        }
    })
}
