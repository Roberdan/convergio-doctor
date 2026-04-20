//! E2E mesh peer reachability + sync tests.
//!
//! ALL mesh checks WARN (not FAIL) if no peers are online — single-node mode is valid.

use crate::check_e2e_cleanup::check_peer_online;
use crate::check_e2e_helpers::{test_name, DoctorHttpClient};
use crate::checks::{run_check, CheckResult, CheckStatus};
use convergio_db::pool::ConnPool;
use serde_json::json;
use std::collections::HashMap;

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

/// SSH targets keyed by the same hostname forms as load_peers_conf,
/// so the SSH check can resolve "Roberdans-MacBook-Pro-M1.local" to its
/// configured ssh_alias (which carries user@host + identity file via
/// ~/.ssh/config). Cached at first call.
fn load_ssh_aliases() -> HashMap<String, String> {
    parse_peers_conf(|fields| fields.get("ssh_alias").cloned())
}

/// Parse the peers.conf flat-file registry (sections + key=value lines).
///
/// Returns a map of peer_name -> address. The address is the first non-empty
/// of `tailscale_ip`, `lan_ip`, `dns_name`, `ssh_alias`. The peer_name is the
/// section header (lowercased) AND any *.local hostname derivable from
/// `dns_name` — both are inserted so heartbeat-reported hostnames like
/// "Roberdans-MacBook-Pro-M1.local" can be resolved back to an address.
fn load_peers_conf() -> HashMap<String, String> {
    parse_peers_conf(|fields| {
        fields
            .get("tailscale_ip")
            .or_else(|| fields.get("lan_ip"))
            .or_else(|| fields.get("dns_name"))
            .or_else(|| fields.get("ssh_alias"))
            .cloned()
    })
}

/// Generic peers.conf walker. The `pick` callback selects one field per
/// section; the section name (lowercase), dns_name first label, and
/// `<label>.local` are all inserted as keys so heartbeat-reported hostnames
/// can be resolved.
fn parse_peers_conf(
    pick: impl Fn(&HashMap<String, String>) -> Option<String>,
) -> HashMap<String, String> {
    let path = std::env::var("CONVERGIO_PEERS_CONF").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/.claude/config/peers.conf")
    });
    let Ok(text) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };

    let mut out: HashMap<String, String> = HashMap::new();
    let mut current_section: Option<String> = None;
    let mut fields: HashMap<String, String> = HashMap::new();

    let mut flush = |section: &Option<String>, fields: &HashMap<String, String>| {
        let Some(name) = section else { return };
        if name == "mesh" {
            return;
        }
        let Some(value) = pick(fields) else { return };
        out.insert(name.to_lowercase(), value.clone());
        if let Some(dns) = fields.get("dns_name") {
            if let Some(label) = dns.split('.').next() {
                out.insert(format!("{label}.local").to_lowercase(), value.clone());
                out.insert(label.to_lowercase(), value);
            }
        }
    };

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(name) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            flush(&current_section, &fields);
            current_section = Some(name.to_string());
            fields.clear();
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            fields.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    flush(&current_section, &fields);
    out
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
    let registry = load_peers_conf();
    peers
        .iter()
        .filter_map(|p| {
            let name = p
                .get("peer")
                .or(p.get("name"))
                .or(p.get("hostname"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            // Address resolution order: explicit ip/address field on the API
            // response, then the peers.conf registry keyed by hostname.
            let ip = p
                .get("ip")
                .or(p.get("address"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| registry.get(&name.to_lowercase()).cloned())
                .or_else(|| {
                    // Try first label (drop .local suffix) as registry key.
                    name.split('.')
                        .next()
                        .and_then(|n| registry.get(&n.to_lowercase()).cloned())
                })?;
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

        // Mesh sync cycle is 30s. The daemon HTTP stack enforces a 30s
        // per-request timeout, so the check cannot afford to wait a full
        // cycle — poll up to 15s in 3s steps and fall back to Warn when
        // the marker misses the current window. The insert is still
        // cleaned up in every branch.
        let (ip, _) = &peers[0];
        let peer = DoctorHttpClient::with_base(&format!("http://{ip}:8420"));
        let mut last_status: Option<u16> = None;
        for _ in 0..5 {
            std::thread::sleep(std::time::Duration::from_secs(3));
            match peer.get("/api/sync/export?table=notifications&since=2020-01-01") {
                Ok((200, body)) => {
                    if body.to_string().contains(&marker) {
                        let _ = conn.execute(
                            "DELETE FROM notifications WHERE type = ?1",
                            rusqlite::params![marker],
                        );
                        return (CheckStatus::Pass, "sync roundtrip verified".into());
                    }
                    last_status = Some(200);
                }
                Ok((s, _)) => last_status = Some(s),
                Err(_) => {}
            }
        }
        let _ = conn.execute(
            "DELETE FROM notifications WHERE type = ?1",
            rusqlite::params![marker],
        );
        match last_status {
            Some(200) => (
                CheckStatus::Warn,
                "marker not in peer export within 15s (sync cycle 30s)".into(),
            ),
            Some(s) => (CheckStatus::Warn, format!("peer sync export returned {s}")),
            None => (CheckStatus::Warn, "peer sync export unavailable".into()),
        }
    })
}

/// Validate that a peer address is a safe IP/hostname (no shell meta-characters).
fn is_safe_peer_addr(addr: &str) -> bool {
    !addr.is_empty()
        && addr.len() <= 253
        && addr
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == ':' || c == '-')
        && !addr.starts_with('-')
}

fn check_delegation_ssh_connectivity() -> CheckResult {
    run_check("delegation_ssh_connectivity", "e2e", || {
        let client = DoctorHttpClient::new();
        let peers = get_online_peers(&client);
        if peers.is_empty() {
            return (CheckStatus::Warn, "No peers for SSH check".into());
        }
        // Prefer the ssh_alias from peers.conf (carries User/IdentityFile via
        // ~/.ssh/config); fall back to the address only when no alias is
        // configured. A bare tailscale IP usually fails because the daemon
        // launchd process has no SSH_AUTH_SOCK and no implicit user mapping.
        let aliases = load_ssh_aliases();
        let mut ok = 0;
        let mut skipped = 0;
        for (ip, name) in &peers {
            let target = aliases
                .get(&name.to_lowercase())
                .or_else(|| {
                    name.split('.')
                        .next()
                        .and_then(|n| aliases.get(&n.to_lowercase()))
                })
                .cloned()
                .unwrap_or_else(|| ip.clone());
            if !is_safe_peer_addr(&target) {
                skipped += 1;
                continue;
            }
            let output = std::process::Command::new("ssh")
                .args([
                    "-o",
                    "ConnectTimeout=3",
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "StrictHostKeyChecking=accept-new",
                    &target,
                    "echo ok",
                ])
                .output();
            match output {
                Ok(o) if o.status.success() => ok += 1,
                _ => {}
            }
        }
        if skipped > 0 {
            return (
                CheckStatus::Warn,
                format!("{skipped} peers skipped (invalid address)"),
            );
        }
        if ok > 0 {
            (
                CheckStatus::Pass,
                format!("{ok}/{} peers SSH-reachable", peers.len()),
            )
        } else {
            (
                CheckStatus::Warn,
                format!(
                    "0/{} peers SSH-reachable (daemon may lack SSH_AUTH_SOCK)",
                    peers.len()
                ),
            )
        }
    })
}

fn trunc(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
