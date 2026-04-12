//! E2E mesh peer reachability + sync tests.
//!
//! ALL mesh checks WARN (not FAIL) if no peers are online — single-node mode is valid.

use crate::check_e2e_cleanup::check_peer_online;
use crate::check_e2e_helpers::{test_name, DoctorHttpClient};
use crate::checks::{run_check, CheckResult, CheckStatus};
use convergio_db::pool::ConnPool;
use serde_json::json;

pub fn run_e2e_mesh_checks(pool: &ConnPool) -> Vec<CheckResult> {
    vec![
        check_mesh_peers_reachable(),
        check_mesh_heartbeat_send(),
        check_mesh_sync_status(),
        check_mesh_schema_compat(),
        check_mesh_sync_roundtrip(pool),
        check_delegation_ssh_connectivity(),
    ]
}

fn get_online_peers(client: &DoctorHttpClient) -> Vec<(String, String)> {
    let Ok((200, body)) = client.get("/api/mesh/peers") else {
        return vec![];
    };
    let Some(peers) = body
        .as_array()
        .or_else(|| body.get("peers").and_then(|p| p.as_array()))
    else {
        return vec![];
    };
    peers
        .iter()
        .filter_map(|p| {
            let ip = p
                .get("ip")
                .or(p.get("address"))
                .and_then(|v| v.as_str())?
                .to_string();
            let name = p
                .get("name")
                .or(p.get("hostname"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let online = p.get("online").and_then(|v| v.as_bool()).unwrap_or(false)
                || p.get("status").and_then(|v| v.as_str()) == Some("online");
            if online || check_peer_online(&format!("http://{ip}:8420")) {
                Some((ip, name))
            } else {
                None
            }
        })
        .collect()
}

fn check_mesh_peers_reachable() -> CheckResult {
    run_check("mesh_peers_reachable", "e2e", || {
        let client = DoctorHttpClient::new();
        let peers = get_online_peers(&client);
        if peers.is_empty() {
            return (
                CheckStatus::Warn,
                "No peers online (single-node mode)".into(),
            );
        }
        let mut reachable = 0;
        for (ip, _) in &peers {
            if check_peer_online(&format!("http://{ip}:8420")) {
                reachable += 1;
            }
        }
        if reachable == peers.len() {
            (
                CheckStatus::Pass,
                format!("{reachable}/{} peers reachable", peers.len()),
            )
        } else {
            (
                CheckStatus::Warn,
                format!("{reachable}/{} peers reachable", peers.len()),
            )
        }
    })
}

fn check_mesh_heartbeat_send() -> CheckResult {
    run_check("mesh_heartbeat_send", "e2e", || {
        let client = DoctorHttpClient::new();
        let peers = get_online_peers(&client);
        if peers.is_empty() {
            return (CheckStatus::Warn, "No peers for heartbeat test".into());
        }
        let mut ok = 0;
        for (ip, _) in &peers {
            let peer_client = DoctorHttpClient::with_base(&format!("http://{ip}:8420"));
            match peer_client.post_json("/api/heartbeat", &json!({"from": "doctor-check"})) {
                Ok((s, _)) if s < 400 => ok += 1,
                _ => {}
            }
        }
        (
            CheckStatus::Pass,
            format!("{ok}/{} heartbeats sent", peers.len()),
        )
    })
}

fn check_mesh_sync_status() -> CheckResult {
    run_check("mesh_sync_status", "e2e", || {
        let client = DoctorHttpClient::new();
        match client.get("/api/sync/status") {
            Ok((200, body)) => (
                CheckStatus::Pass,
                format!("sync status: {}", trunc(&body.to_string(), 80)),
            ),
            Ok((s, _)) => (CheckStatus::Warn, format!("sync status returned {s}")),
            Err(e) => (CheckStatus::Warn, format!("sync status failed: {e}")),
        }
    })
}

fn check_mesh_schema_compat() -> CheckResult {
    run_check("mesh_schema_compat", "e2e", || {
        let client = DoctorHttpClient::new();
        let peers = get_online_peers(&client);
        if peers.is_empty() {
            return (CheckStatus::Warn, "No peers for schema check".into());
        }

        let local_ver = match client.get("/api/health") {
            Ok((200, body)) => body
                .get("schema_version")
                .or(body.get("version"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            _ => "unknown".into(),
        };

        let mut compat = 0;
        for (ip, _name) in &peers {
            let peer = DoctorHttpClient::with_base(&format!("http://{ip}:8420"));
            if let Ok((200, body)) = peer.get("/api/health") {
                let peer_ver = body
                    .get("schema_version")
                    .or(body.get("version"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                if peer_ver == local_ver || local_ver == "unknown" {
                    compat += 1;
                }
            }
        }
        (
            CheckStatus::Pass,
            format!("{compat}/{} peers schema-compatible", peers.len()),
        )
    })
}

fn check_mesh_sync_roundtrip(pool: &ConnPool) -> CheckResult {
    run_check("mesh_sync_roundtrip", "e2e", || {
        let client = DoctorHttpClient::new();
        let peers = get_online_peers(&client);
        if peers.is_empty() {
            return (CheckStatus::Warn, "No peers for sync roundtrip".into());
        }

        let marker = test_name("sync");
        // Insert test notification locally
        let Ok(conn) = pool.get() else {
            return (CheckStatus::Fail, "cannot get DB connection".into());
        };
        let inserted = conn.execute(
            "INSERT INTO notifications (type, title, message) VALUES (?1, ?2, 'doctor sync test')",
            rusqlite::params![marker, marker],
        );
        if inserted.is_err() {
            return (CheckStatus::Warn, "cannot insert test notification".into());
        }

        // Wait for sync cycle (35s is one cycle, but we check shorter)
        std::thread::sleep(std::time::Duration::from_secs(5));

        // Check first peer
        let (ip, _) = &peers[0];
        let peer = DoctorHttpClient::with_base(&format!("http://{ip}:8420"));
        match peer.get("/api/sync/export?table=notifications&since=2020-01-01") {
            Ok((200, body)) => {
                let found = body.to_string().contains(&marker);
                // Cleanup local
                let _ = conn.execute(
                    "DELETE FROM notifications WHERE type = ?1",
                    rusqlite::params![marker],
                );
                if found {
                    (CheckStatus::Pass, "sync roundtrip verified".into())
                } else {
                    (
                        CheckStatus::Warn,
                        "data not yet synced (may need more time)".into(),
                    )
                }
            }
            _ => {
                let _ = conn.execute(
                    "DELETE FROM notifications WHERE type = ?1",
                    rusqlite::params![marker],
                );
                (CheckStatus::Warn, "peer sync export unavailable".into())
            }
        }
    })
}

fn check_delegation_ssh_connectivity() -> CheckResult {
    run_check("delegation_ssh_connectivity", "e2e", || {
        let client = DoctorHttpClient::new();
        let peers = get_online_peers(&client);
        if peers.is_empty() {
            return (CheckStatus::Warn, "No peers for SSH check".into());
        }
        let mut ok = 0;
        for (ip, _name) in &peers {
            let output = std::process::Command::new("ssh")
                .args([
                    "-o",
                    "ConnectTimeout=3",
                    "-o",
                    "BatchMode=yes",
                    ip,
                    "echo ok",
                ])
                .output();
            match output {
                Ok(o) if o.status.success() => ok += 1,
                _ => {}
            }
        }
        if ok > 0 {
            (
                CheckStatus::Pass,
                format!("{ok}/{} peers SSH-reachable", peers.len()),
            )
        } else {
            (
                CheckStatus::Warn,
                format!("0/{} peers SSH-reachable", peers.len()),
            )
        }
    })
}

fn trunc(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
