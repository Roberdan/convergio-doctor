//! Test data cleanup — runs LAST after all E2E checks.
//!
//! Calls cleanup_test_data to sweep all _doctor_test_ rows from all tables,
//! then verifies zero residual.

use crate::check_e2e_cleanup::{cleanup_test_data, purge_all_doctor_data};
use crate::check_e2e_helpers::{DoctorHttpClient, TEST_PREFIX, TEST_SLUG_PREFIX};
use crate::checks::{run_check, CheckResult, CheckStatus};
use convergio_db::pool::ConnPool;

pub fn run_cleanup_checks(pool: &ConnPool) -> Vec<CheckResult> {
    vec![check_test_data_cleanup(pool)]
}

fn check_test_data_cleanup(pool: &ConnPool) -> CheckResult {
    run_check("test_data_cleanup", "cleanup", || {
        let client = DoctorHttpClient::new();
        let (api_deleted, mut tables) = purge_all_doctor_data(&client);
        let (deleted, db_tables) = cleanup_test_data(pool);
        for table in db_tables {
            if !tables.contains(&table) {
                tables.push(table);
            }
        }
        let total_deleted = api_deleted + deleted;

        // Second pass: count remaining
        let residual = count_residual(pool);

        if residual == 0 {
            if total_deleted > 0 {
                let tbl_list = tables.join(", ");
                (
                    CheckStatus::Pass,
                    format!("cleaned {total_deleted} rows from {tbl_list}"),
                )
            } else {
                (CheckStatus::Pass, "0 test rows found (clean)".into())
            }
        } else {
            // Try one more cleanup pass
            let (deleted2, _) = cleanup_test_data(pool);
            let residual2 = count_residual(pool);
            if residual2 == 0 {
                (
                    CheckStatus::Pass,
                    format!("cleaned {} rows (2 passes)", total_deleted + deleted2),
                )
            } else {
                (
                    CheckStatus::Fail,
                    format!("{residual2} residual rows after cleanup"),
                )
            }
        }
    })
}

fn count_residual(pool: &ConnPool) -> usize {
    let Ok(conn) = pool.get() else { return 0 };
    let Ok(mut stmt) = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
    else {
        return 0;
    };
    let table_names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    let mut total = 0usize;
    for table in &table_names {
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
            let sql = format!(
                "SELECT COUNT(*) FROM \"{table}\" WHERE \"{col}\" LIKE '%{TEST_PREFIX}%' \
                 OR \"{col}\" LIKE '%{TEST_SLUG_PREFIX}%'"
            );
            if let Ok(count) = conn.query_row(&sql, [], |r| r.get::<_, i64>(0)) {
                total += count as usize;
            }
        }
    }
    total
}
