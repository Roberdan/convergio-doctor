// Copyright (c) 2026 Roberto D'Angelo. All rights reserved.
// Task command handlers — split from cli_task.rs for 250-line limit.

use super::cli_task::TaskCommands;

pub async fn handle(cmd: TaskCommands) -> Result<(), crate::cli_error::CliError> {
    match cmd {
        TaskCommands::Update {
            task_id,
            status,
            agent_id,
            summary,
            human,
            api_url,
        } => {
            let agent_id = agent_id.or_else(|| std::env::var("CVG_AGENT_ID").ok());
            let body = serde_json::json!({
                "task_id": task_id,
                "status": status,
                "agent_id": agent_id,
                "notes": summary,
            });
            crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/task/update"),
                &body,
                human,
            )
            .await?;
        }
        TaskCommands::Validate {
            task_id,
            plan_id,
            human,
            api_url,
        } => {
            let url = format!("{api_url}/api/plan-db/validate-task/{task_id}/{plan_id}");
            let val = crate::cli_http::get_and_return(&url)
                .await
                .map_err(|code| {
                    crate::cli_error::CliError::ApiCallFailed(format!("daemon error (code {code})"))
                })?;
            if human {
                crate::cli_task_format::print_mechanical_human(&val);
            } else {
                println!("{val}");
            }
            let rejected = val
                .get("mechanical")
                .and_then(|m| m.get("status"))
                .and_then(serde_json::Value::as_str)
                == Some("REJECTED");
            if rejected {
                return Err(crate::cli_error::CliError::ValidationRejected(
                    "mechanical gates rejected".into(),
                ));
            }
        }
        TaskCommands::KbSearch {
            query,
            limit,
            human,
            api_url,
        } => {
            crate::cli_http::fetch_and_print(
                &format!("{api_url}/api/plan-db/kb-search?q={query}&limit={limit}"),
                human,
            )
            .await?;
        }
        TaskCommands::Create {
            plan_id,
            wave_db_id,
            task_id,
            title,
            priority,
            task_type,
            model,
            description,
            human,
            api_url,
        } => {
            let body = serde_json::json!({
                "plan_id": plan_id,
                "wave_id": wave_db_id,
                "task_id": task_id,
                "title": title,
                "priority": priority,
                "type": task_type,
                "model": model,
                "description": description,
            });
            crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/task/create"),
                &body,
                human,
            )
            .await?;
        }
        TaskCommands::Approve {
            task_id,
            comment,
            human,
            api_url,
        } => {
            crate::cli_task_approve::handle(task_id, comment, human, &api_url).await?;
        }
        TaskCommands::Delete {
            task_id,
            human,
            api_url,
        } => {
            crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/task/delete/{task_id}"),
                &serde_json::json!({}),
                human,
            )
            .await?;
        }
        TaskCommands::Complete {
            task_id,
            agent_id,
            pr_url,
            test_command,
            test_output,
            test_exit_code,
            notes,
            human,
            api_url,
        } => {
            let body = serde_json::json!({
                "task_db_id": task_id,
                "agent_id": agent_id,
                "pr_url": pr_url,
                "test_command": test_command,
                "test_output": test_output,
                "test_exit_code": test_exit_code,
                "notes": notes,
            });
            crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/task/complete-flow"),
                &body,
                human,
            )
            .await?;
        }
    }
    Ok(())
}
