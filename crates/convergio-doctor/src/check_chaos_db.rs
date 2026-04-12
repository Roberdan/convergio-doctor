//! Chaos testing — SQLite pool stress + write conflicts.
//!
//! Targets the local rusqlite/SQLite pool. Tests concurrent access,
//! write conflicts, large payloads, and WAL checkpoint.

use crate::check_e2e_helpers::TEST_PREFIX;
use crate::checks::{run_check, CheckResult, CheckStatus};
use convergio_db::pool::ConnPool;
use std::sync::Arc;

pub fn run_chaos_db_checks(pool: &ConnPool) -> Vec<CheckResult> {
    vec![
        check_chaos_db_pool_stress(pool),
        check_chaos_db_write_conflict(pool),
        check_chaos_db_large_payload(pool),
        check_chaos_db_wal_checkpoint(pool),
    ]
}

fn check_chaos_db_pool_stress(pool: &ConnPool) -> CheckResult {
    run_check("chaos_db_pool_stress", "chaos", || {
        let pool = Arc::new(pool.clone());
        let workers = 8;
        let queries_per_worker = 100;
        let mut handles = Vec::new();

        for _ in 0..workers {
            let p = Arc::clone(&pool);
            handles.push(std::thread::spawn(move || {
                let mut ok = 0u32;
                let mut fail = 0u32;
                for _ in 0..queries_per_worker {
                    match p.get() {
                        Ok(conn) => {
                            let r: Result<i64, _> = conn.query_row("SELECT 1", [], |r| r.get(0));
                            if r.is_ok() {
                                ok += 1;
                            } else {
                                fail += 1;
                            }
                        }
                        Err(_) => fail += 1,
                    }
                }
                (ok, fail)
            }));
        }

        let mut total_ok = 0u32;
        let mut total_fail = 0u32;
        for h in handles {
            match h.join() {
                Ok((ok, fail)) => {
                    total_ok += ok;
                    total_fail += fail;
                }
                Err(_) => total_fail += queries_per_worker as u32,
            }
        }

        if total_fail == 0 {
            (
                CheckStatus::Pass,
                format!("{workers} workers, {total_ok} queries, 0 failures"),
            )
        } else {
            (
                CheckStatus::Warn,
                format!("{workers} workers, {total_ok} ok, {total_fail} failed"),
            )
        }
    })
}

fn check_chaos_db_write_conflict(pool: &ConnPool) -> CheckResult {
    run_check("chaos_db_write_conflict", "chaos", || {
        let pool = Arc::new(pool.clone());
        let key = format!("{TEST_PREFIX}conflict");

        // Create test row — notifications schema: type, title, message
        if let Ok(conn) = pool.get() {
            let _ = conn.execute(
                "INSERT INTO notifications (type, title, message) VALUES (?1, ?2, 'init')",
                rusqlite::params![key, key],
            );
        }

        let writers = 4;
        let mut handles = Vec::new();
        for i in 0..writers {
            let p = Arc::clone(&pool);
            let k = key.clone();
            handles.push(std::thread::spawn(move || {
                let mut ok = 0u32;
                for j in 0..50 {
                    if let Ok(conn) = p.get() {
                        let r = conn.execute(
                            "INSERT INTO notifications (type, title, message) \
                             VALUES (?1, ?2, ?3)",
                            rusqlite::params![k, k, format!("writer-{i}-iter-{j}")],
                        );
                        if r.is_ok() {
                            ok += 1;
                        }
                    }
                }
                ok
            }));
        }

        let total: u32 = handles.into_iter().filter_map(|h| h.join().ok()).sum();

        // Cleanup
        if let Ok(conn) = pool.get() {
            let _ = conn.execute(
                "DELETE FROM notifications WHERE type = ?1",
                rusqlite::params![key],
            );
        }

        if total > 0 {
            (
                CheckStatus::Pass,
                format!("{writers} writers, {total} successful writes, no panics"),
            )
        } else {
            (CheckStatus::Fail, "all writes failed".into())
        }
    })
}

fn check_chaos_db_large_payload(pool: &ConnPool) -> CheckResult {
    run_check("chaos_db_large_payload", "chaos", || {
        let key = format!("{TEST_PREFIX}large");
        let payload = "X".repeat(100_000); // 100KB

        let Ok(conn) = pool.get() else {
            return (CheckStatus::Fail, "cannot get connection".into());
        };

        // Insert large payload — notifications schema: type, title, message
        let insert = conn.execute(
            "INSERT INTO notifications (type, title, message) VALUES (?1, ?2, ?3)",
            rusqlite::params![key, key, payload],
        );
        if insert.is_err() {
            return (CheckStatus::Fail, "failed to insert 100KB payload".into());
        }

        // Read back and verify
        let readback: Result<String, _> = conn.query_row(
            "SELECT message FROM notifications WHERE type = ?1 AND title = ?2",
            rusqlite::params![key, key],
            |r| r.get(0),
        );
        let cleanup = || {
            let _ = conn.execute(
                "DELETE FROM notifications WHERE type = ?1",
                rusqlite::params![key],
            );
        };
        match readback {
            Ok(msg) if msg.len() == 100_000 => {
                cleanup();
                (
                    CheckStatus::Pass,
                    "100KB payload: insert → read → verified OK".into(),
                )
            }
            Ok(msg) => {
                cleanup();
                (
                    CheckStatus::Fail,
                    format!("readback size mismatch: {} vs 100000", msg.len()),
                )
            }
            Err(e) => {
                cleanup();
                (CheckStatus::Fail, format!("readback failed: {e}"))
            }
        }
    })
}

fn check_chaos_db_wal_checkpoint(pool: &ConnPool) -> CheckResult {
    run_check("chaos_db_wal_checkpoint", "chaos", || {
        let Ok(conn) = pool.get() else {
            return (CheckStatus::Fail, "cannot get connection".into());
        };

        // WAL checkpoint
        let ckpt = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");
        if let Err(e) = ckpt {
            return (CheckStatus::Warn, format!("WAL checkpoint failed: {e}"));
        }

        // Verify DB still works
        let test: Result<i64, _> = conn.query_row("SELECT 1", [], |r| r.get(0));
        match test {
            Ok(1) => (CheckStatus::Pass, "WAL checkpoint + post-check OK".into()),
            Ok(v) => (CheckStatus::Warn, format!("SELECT 1 returned {v}")),
            Err(e) => (
                CheckStatus::Fail,
                format!("post-checkpoint query failed: {e}"),
            ),
        }
    })
}
