//! Beta checks for MCP release sync and documentation coverage.

use crate::checks::{run_check, CheckResult, CheckStatus};

const RELEASE_WORKFLOW: &str = include_str!("../../../.github/workflows/release.yml");
const GETTING_STARTED: &str = include_str!("../../../docs/guides/getting-started.md");
const ADR_036: &str = include_str!("../../../docs/adr/ADR-036-mcp-release-sync.md");
const CLI_PLAN_HANDLERS: &str = include_str!("../../convergio-cli/src/cli_plan_handlers.rs");
const CLI_TASK_HANDLERS: &str = include_str!("../../convergio-cli/src/cli_task_handlers.rs");
const API_PLAN_VALIDATE: &str = include_str!("../../convergio-orchestrator/src/plan_validate.rs");
const API_PLAN_EXT: &str = include_str!("../../convergio-orchestrator/src/plan_routes_ext.rs");
const API_TASK_ROUTES: &str = include_str!("../../convergio-orchestrator/src/task_routes.rs");

const REQUIRED_TOOLS: &[&str] = &[
    "cvg_get_execution_tree",
    "cvg_update_task",
    "cvg_complete_task",
    "cvg_validate_plan",
    "cvg_list_night_agents",
    "cvg_trigger_night_agent",
];

pub fn check_mcp_release_sync() -> CheckResult {
    run_check("mcp_release_sync", "beta", || {
        // Use the static fallback defs (always available without daemon)
        let all_defs = convergio_mcp::registry_defs::all_defs();
        let tool_names: Vec<&str> = all_defs.iter().map(|d| d.name.as_str()).collect();
        let missing_tools: Vec<&str> = REQUIRED_TOOLS
            .iter()
            .copied()
            .filter(|name| !tool_names.contains(name))
            .collect();

        let mut issues = Vec::new();
        for (label, needle) in [
            (
                "release gate for convergio-mcp tests",
                "cargo test -p convergio-mcp --tests",
            ),
            (
                "release gate for convergio-doctor tests",
                "cargo test -p convergio-doctor --lib --tests",
            ),
            ("MCP release artifact", "convergio-mcp-server"),
        ] {
            if !RELEASE_WORKFLOW.contains(needle) {
                issues.push(label.to_string());
            }
        }

        for (label, text, needle) in [
            ("ADR-036", ADR_036, "doctor"),
            ("ADR-036", ADR_036, "convergio-mcp-server"),
            ("ADR-036", ADR_036, "API, CLI, and MCP"),
            (
                "getting-started guide",
                GETTING_STARTED,
                "convergio-mcp-server",
            ),
        ] {
            if !text.contains(needle) {
                issues.push(format!("{label} missing '{needle}'"));
            }
        }

        for (feature, api_text, api_needle, cli_text, cli_needle, mcp_tool) in [
            (
                "execution tree",
                API_PLAN_EXT,
                "/api/plan-db/execution-tree/:plan_id",
                CLI_PLAN_HANDLERS,
                "/api/plan-db/execution-tree/{plan_id}",
                "cvg_get_execution_tree",
            ),
            (
                "plan validation",
                API_PLAN_VALIDATE,
                "/api/plan-db/validate",
                CLI_PLAN_HANDLERS,
                "/api/plan-db/validate",
                "cvg_validate_plan",
            ),
            (
                "task completion flow",
                API_TASK_ROUTES,
                "/api/plan-db/task/complete-flow",
                CLI_TASK_HANDLERS,
                "/api/plan-db/task/complete-flow",
                "cvg_complete_task",
            ),
            (
                "task status update",
                API_TASK_ROUTES,
                "/api/plan-db/task/update",
                CLI_TASK_HANDLERS,
                "\"agent_id\": agent_id",
                "cvg_update_task",
            ),
        ] {
            if !api_text.contains(api_needle) {
                issues.push(format!("{feature} missing API exposure"));
            }
            if !cli_text.contains(cli_needle) {
                issues.push(format!("{feature} missing CLI exposure"));
            }
            if !tool_names.contains(&mcp_tool) {
                issues.push(format!("{feature} missing MCP exposure"));
            }
        }

        if missing_tools.is_empty() && issues.is_empty() {
            (
                CheckStatus::Pass,
                format!(
                    "{} MCP tools exported; release workflow, ADR, and docs are in sync",
                    tool_names.len()
                ),
            )
        } else {
            let missing = if missing_tools.is_empty() {
                "none".to_string()
            } else {
                missing_tools.join(", ")
            };
            (
                CheckStatus::Fail,
                format!(
                    "MCP release sync drift — missing tools: {missing}; issues: {}",
                    issues.join("; ")
                ),
            )
        }
    })
}
