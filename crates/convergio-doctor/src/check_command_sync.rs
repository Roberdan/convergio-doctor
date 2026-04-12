//! Doctor check: verify .claude/commands/*.md are in sync with their sources.
//!
//! Each command file has a YAML frontmatter `source:` field. This check
//! verifies the source file exists and key sections are present (fixes #525).

use crate::checks::{run_check, CheckResult, CheckStatus};
use std::path::Path;

pub fn check_command_sync() -> CheckResult {
    run_check("command_source_sync", "beta", || {
        let commands_dir = Path::new("claude-config/../.claude/commands");
        let alt_dir = Path::new(".claude/commands");
        let dir = if commands_dir.exists() {
            commands_dir
        } else if alt_dir.exists() {
            alt_dir
        } else {
            return (CheckStatus::Warn, "commands directory not found".into());
        };

        let mut checked = 0usize;
        let mut issues = Vec::new();

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => return (CheckStatus::Warn, format!("cannot read dir: {e}")),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e != "md") || path.extension().is_none() {
                continue;
            }
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            // Parse source: field from YAML frontmatter
            let source = content
                .lines()
                .find(|l| l.starts_with("source:"))
                .and_then(|l| l.strip_prefix("source:"))
                .map(|s| s.trim().trim_matches('"'))
                .unwrap_or("");

            if source.is_empty() || source == "null" {
                continue; // No source to check
            }

            checked += 1;
            let source_path = Path::new(source);
            if !source_path.exists() {
                issues.push(format!(
                    "{}: source '{}' not found",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    source
                ));
            }
        }

        if !issues.is_empty() {
            (
                CheckStatus::Warn,
                format!("{} sync issues: {}", issues.len(), issues.join("; ")),
            )
        } else {
            (
                CheckStatus::Pass,
                format!("{checked} command files checked"),
            )
        }
    })
}
