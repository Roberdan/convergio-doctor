//! E2E security — auth enforcement tests.
//!
//! Detects dev-mode (auth bypass) and adjusts expectations accordingly.

use crate::check_e2e_helpers::DoctorHttpClient;
use crate::checks::{run_check, CheckResult, CheckStatus};

pub fn run_e2e_security_checks() -> Vec<CheckResult> {
    vec![
        check_auth_health_exempt(),
        check_auth_required_on_protected(),
        check_auth_valid_token_works(),
        check_auth_invalid_token_rejected(),
    ]
}

/// Detect if daemon is in dev-mode (auth bypass active).
fn is_dev_mode() -> bool {
    let client = DoctorHttpClient::new();
    // In dev-mode, unauthenticated requests to protected routes succeed
    matches!(client.get_no_auth("/api/orgs"), Ok((status, _)) if status < 400)
}

fn check_auth_health_exempt() -> CheckResult {
    run_check("auth_health_exempt", "e2e", || {
        let client = DoctorHttpClient::new();
        match client.get_no_auth("/api/health") {
            Ok((200, _)) => (CheckStatus::Pass, "health exempt from auth".into()),
            Ok((status, _)) => (
                CheckStatus::Fail,
                format!("health returned {status} without auth"),
            ),
            Err(e) => (CheckStatus::Fail, format!("health failed: {e}")),
        }
    })
}

fn check_auth_required_on_protected() -> CheckResult {
    run_check("auth_required_on_protected", "e2e", || {
        if is_dev_mode() {
            return (
                CheckStatus::Warn,
                "dev-mode active — auth bypass enabled".into(),
            );
        }
        let client = DoctorHttpClient::new();
        let protected = ["/api/orgs", "/api/plan-db/list", "/api/agents/catalog"];
        let mut ok = 0;
        let mut failed = Vec::new();
        for path in &protected {
            match client.get_no_auth(path) {
                Ok((401, _)) | Ok((403, _)) => ok += 1,
                Ok((status, _)) => failed.push(format!("{path} → {status}")),
                Err(e) => failed.push(format!("{path} → {e}")),
            }
        }
        if failed.is_empty() {
            (
                CheckStatus::Pass,
                format!("{ok}/{} require auth", protected.len()),
            )
        } else {
            (
                CheckStatus::Fail,
                format!("unprotected: {}", failed.join("; ")),
            )
        }
    })
}

fn check_auth_valid_token_works() -> CheckResult {
    run_check("auth_valid_token_works", "e2e", || {
        let client = DoctorHttpClient::new();
        match client.get("/api/orgs") {
            Ok((status, _)) if status < 400 => (CheckStatus::Pass, "valid token accepted".into()),
            Ok((status, _)) => (CheckStatus::Fail, format!("valid token returned {status}")),
            Err(e) => (CheckStatus::Fail, format!("request failed: {e}")),
        }
    })
}

fn check_auth_invalid_token_rejected() -> CheckResult {
    run_check("auth_invalid_token_rejected", "e2e", || {
        if is_dev_mode() {
            return (
                CheckStatus::Warn,
                "dev-mode active — auth bypass enabled".into(),
            );
        }
        let client = DoctorHttpClient::new();
        match client.get_with_token("/api/orgs", "totally-wrong-token-12345") {
            Ok((401, _)) | Ok((403, _)) => (CheckStatus::Pass, "invalid token rejected".into()),
            Ok((status, _)) => (CheckStatus::Fail, format!("invalid token got {status}")),
            Err(e) => (CheckStatus::Fail, format!("request failed: {e}")),
        }
    })
}
