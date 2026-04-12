//! Chaos testing — network fault simulation.
//!
//! Tests: bogus peer, slow response, invalid JSON, concurrent requests.

use crate::checks::{run_check, CheckResult, CheckStatus};
use convergio_types::dev_auth_header;
use std::io::Write;

pub fn run_chaos_net_checks() -> Vec<CheckResult> {
    vec![
        check_chaos_net_bogus_peer(),
        check_chaos_net_slow_response(),
        check_chaos_net_invalid_json(),
        check_chaos_net_concurrent_requests(),
    ]
}

fn check_chaos_net_bogus_peer() -> CheckResult {
    run_check("chaos_net_bogus_peer", "chaos", || {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(5))
            .build();
        let Ok(c) = client else {
            return (CheckStatus::Fail, "cannot build client".into());
        };

        match c.get("http://127.0.0.1:19999/api/health").send() {
            Ok(r) => (
                CheckStatus::Warn,
                format!("unexpected response: {}", r.status()),
            ),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Connection refused")
                    || msg.contains("timed out")
                    || msg.contains("connect")
                    || msg.contains("timeout")
                {
                    (
                        CheckStatus::Pass,
                        "bogus peer: clean error (no panic)".into(),
                    )
                } else {
                    (
                        CheckStatus::Pass,
                        format!("bogus peer: error handled: {}", trunc(&msg, 60)),
                    )
                }
            }
        }
    })
}

fn check_chaos_net_slow_response() -> CheckResult {
    run_check("chaos_net_slow_response", "chaos", || {
        // Bind listener on random port, accept but never respond
        let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(e) => return (CheckStatus::Fail, format!("cannot bind: {e}")),
        };
        let port = listener.local_addr().unwrap().port();

        // Accept in background thread (but never write)
        let handle = std::thread::spawn(move || {
            let _ = listener.accept(); // accept, hold open
            std::thread::sleep(std::time::Duration::from_secs(10));
        });

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build();
        let Ok(c) = client else {
            return (CheckStatus::Fail, "cannot build client".into());
        };

        let start = std::time::Instant::now();
        let result = c.get(format!("http://127.0.0.1:{port}/api/health")).send();
        let elapsed = start.elapsed();

        // Don't wait for the accept thread — it'll clean up when dropped
        drop(handle);

        match result {
            Err(_e) if elapsed.as_secs() <= 5 => (
                CheckStatus::Pass,
                format!("timeout after {:.1}s (clean)", elapsed.as_secs_f64()),
            ),
            Err(e) => (
                CheckStatus::Pass,
                format!(
                    "error after {:.1}s: {}",
                    elapsed.as_secs_f64(),
                    trunc(&e.to_string(), 40)
                ),
            ),
            Ok(_) => (
                CheckStatus::Warn,
                "unexpected response from silent server".into(),
            ),
        }
    })
}

fn check_chaos_net_invalid_json() -> CheckResult {
    run_check("chaos_net_invalid_json", "chaos", || {
        let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(e) => return (CheckStatus::Fail, format!("cannot bind: {e}")),
        };
        let port = listener.local_addr().unwrap().port();

        // Respond with invalid JSON
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 8\r\n\r\nnot-json";
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build();
        let Ok(c) = client else {
            return (CheckStatus::Fail, "cannot build client".into());
        };

        let result = c
            .get(format!("http://127.0.0.1:{port}/test"))
            .send()
            .and_then(|r| r.json::<serde_json::Value>());

        let _ = handle.join();

        match result {
            Err(_) => (
                CheckStatus::Pass,
                "invalid JSON handled cleanly (no panic)".into(),
            ),
            Ok(v) => (CheckStatus::Warn, format!("parsed as JSON: {v}")),
        }
    })
}

fn check_chaos_net_concurrent_requests() -> CheckResult {
    run_check("chaos_net_concurrent_requests", "chaos", || {
        let threads = 10;
        let mut handles = Vec::new();

        for _ in 0..threads {
            handles.push(std::thread::spawn(|| {
                let c = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .build()
                    .ok()?;
                let resp = c
                    .get("http://localhost:8420/api/health")
                    .header("Authorization", dev_auth_header())
                    .send()
                    .ok()?;
                Some(resp.status().as_u16())
            }));
        }

        let mut ok = 0;
        let mut failed = 0;
        for h in handles {
            match h.join() {
                Ok(Some(200)) => ok += 1,
                _ => failed += 1,
            }
        }

        if ok == threads {
            (
                CheckStatus::Pass,
                format!("{threads} concurrent requests, all 200"),
            )
        } else {
            (
                CheckStatus::Warn,
                format!("{ok}/{threads} OK, {failed} failed"),
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
