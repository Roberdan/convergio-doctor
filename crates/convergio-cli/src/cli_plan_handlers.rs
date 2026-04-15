use crate::cli_error::CliError;
use crate::cli_plan::PlanCommands;

pub async fn dispatch(cmd: PlanCommands) -> Result<(), CliError> {
    match cmd {
        // --- GET-based subcommands ---
        PlanCommands::List {
            status,
            limit,
            human,
            api_url,
        } => {
            let mut params = Vec::new();
            // Send "all" when no status filter — server defaults to active-only otherwise (#684)
            let effective_status = status.as_deref().unwrap_or("all");
            params.push(format!("status={effective_status}"));
            if let Some(l) = limit {
                params.push(format!("limit={l}"));
            }
            let qs = if params.is_empty() {
                String::new()
            } else {
                format!("?{}", params.join("&"))
            };
            let url = format!("{api_url}/api/plan-db/list{qs}");
            if let Err(e) = crate::cli_http::fetch_and_print(&url, human).await {
                eprintln!("error: {e}");
            }
        }
        PlanCommands::Tree {
            plan_id,
            human,
            api_url,
        } => {
            if human {
                let url = format!("{api_url}/api/plan-db/execution-tree/{plan_id}");
                if let Ok(val) = crate::cli_http::get_and_return(&url).await {
                    crate::cli_plan_tree_fmt::print_execution_tree(&val);
                }
            } else {
                if let Err(e) = crate::cli_http::fetch_and_print(
                    &format!("{api_url}/api/plan-db/execution-tree/{plan_id}"),
                    false,
                )
                .await
                {
                    eprintln!("error: {e}");
                }
            }
        }
        PlanCommands::Show {
            plan_id,
            human,
            api_url,
        } => {
            if human {
                let url = format!("{api_url}/api/plan-db/json/{plan_id}");
                if let Ok(val) = crate::cli_http::get_and_return(&url).await {
                    crate::cli_plan_show::print_plan_human(&val);
                }
            } else if let Err(e) = crate::cli_http::fetch_and_print(
                &format!("{api_url}/api/plan-db/json/{plan_id}"),
                false,
            )
            .await
            {
                eprintln!("error: {e}");
            }
        }
        PlanCommands::Drift {
            plan_id: _,
            human: _,
            api_url: _,
        } => {
            eprintln!("not implemented — tracked in plan 1");
        }
        PlanCommands::Validate {
            plan_id,
            human,
            api_url,
        } => {
            let body = serde_json::json!({"plan_id": plan_id});
            if let Err(e) = crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/validate"),
                &body,
                human,
            )
            .await
            {
                eprintln!("error: {e}");
            }
        }
        // --- POST-based subcommands ---
        PlanCommands::Create {
            project_id,
            name,
            objective,
            motivation,
            requester,
            source_file,
            parent,
            execution_mode,
            human,
            api_url,
        } => {
            let body = serde_json::json!({
                "project_id": project_id,
                "name": name,
                "objective": objective.unwrap_or_else(|| name.clone()),
                "motivation": motivation.unwrap_or_default(),
                "requester": requester,
                "source_file": source_file,
                "parent_plan_id": parent,
                "execution_mode": execution_mode,
            });
            if let Err(e) = crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/create"),
                &body,
                human,
            )
            .await
            {
                eprintln!("error: {e}");
            }
        }
        PlanCommands::Import {
            plan_id,
            spec_file,
            mode,
            human,
            api_url,
        } => {
            let content = std::fs::read_to_string(&spec_file).map_err(|e| {
                let hint = if e.kind() == std::io::ErrorKind::NotFound {
                    format!(". Does '{spec_file}' exist? Use absolute path or check cwd.")
                } else {
                    String::new()
                };
                CliError::InvalidInput(format!("cannot read spec file '{spec_file}': {e}{hint}"))
            })?;
            let body = serde_json::json!({
                "plan_id": plan_id,
                "spec": content,
                "import_mode": mode,
            });
            if let Err(e) = crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/import"),
                &body,
                human,
            )
            .await
            {
                eprintln!("error: {e}");
            }
        }
        PlanCommands::Start {
            plan_id,
            human,
            api_url,
        } => {
            let body = serde_json::json!({});
            if let Err(e) = crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/start/{plan_id}"),
                &body,
                human,
            )
            .await
            {
                eprintln!("error: {e}");
            }
        }
        PlanCommands::Complete {
            plan_id,
            human,
            api_url,
        } => {
            let body = serde_json::json!({});
            if let Err(e) = crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/complete/{plan_id}"),
                &body,
                human,
            )
            .await
            {
                eprintln!("error: {e}");
            }
        }
        PlanCommands::Cancel {
            plan_id,
            reason,
            human,
            api_url,
        } => {
            let body = serde_json::json!({ "reason": reason });
            if let Err(e) = crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/cancel/{plan_id}"),
                &body,
                human,
            )
            .await
            {
                eprintln!("error: {e}");
            }
        }
        PlanCommands::Approve {
            plan_id,
            human,
            api_url,
        } => {
            let body = serde_json::json!({});
            if let Err(e) = crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/approve/{plan_id}"),
                &body,
                human,
            )
            .await
            {
                eprintln!("error: {e}");
            }
        }
        PlanCommands::Readiness {
            plan_id,
            human,
            api_url,
        } => {
            if let Err(e) = crate::cli_http::fetch_and_print(
                &format!("{api_url}/api/plan-db/readiness/{plan_id}"),
                human,
            )
            .await
            {
                eprintln!("error: {e}");
            }
        }
        PlanCommands::Template => {
            crate::cli_plan_template::print_template();
        }
        PlanCommands::TaskEdit {
            task_id,
            title,
            description,
            model,
            effort,
            executor,
            human,
            api_url,
        } => {
            crate::cli_task_edit::handle_task_edit(crate::cli_task_edit::TaskEditArgs {
                task_id,
                title,
                description,
                model,
                effort,
                executor,
                human,
                api_url,
            })
            .await;
        }
        PlanCommands::Purge {
            status,
            name_prefix,
            human,
            api_url,
        } => {
            let mut body = serde_json::Map::new();
            if let Some(s) = status {
                body.insert("status".into(), serde_json::json!(s));
            }
            if let Some(p) = name_prefix {
                body.insert("name_prefix".into(), serde_json::json!(p));
            }
            let _ = crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/purge"),
                &serde_json::Value::Object(body),
                human,
            )
            .await
            .map_err(|e| eprintln!("error: {e}"));
        }
        PlanCommands::Close {
            plan_id,
            human,
            api_url,
        } => {
            let _ = crate::cli_http::post_and_print(
                &format!("{api_url}/api/plan-db/complete/{plan_id}"),
                &serde_json::json!({}),
                human,
            )
            .await
            .map_err(|e| eprintln!("error: {e}"));
        }
    }
    Ok(())
}
