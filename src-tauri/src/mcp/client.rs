use crate::ipc::{self, ClientError, Request};
use crate::mcp::tools::{ok_json, tool_error};
use crate::mcp::Server;

/// Everything that needs the running app. The mcp process never writes to the
/// capture folder or the config directory itself: validation and ledgering
/// live in one place, and that place is the app.
pub fn call(server: &mut Server, name: &str, arguments: &serde_json::Value) -> serde_json::Value {
    let request = match build(server, name, arguments) {
        Ok(request) => request,
        Err(error) => return error,
    };
    match ipc::request(&ipc::socket_path(&server.app_data_dir), &request) {
        Ok(value) => ok_json(value),
        Err(ClientError::NotRunning) => tool_error(ClientError::NotRunning.to_string()),
        Err(error) => tool_error(error.to_string()),
    }
}

fn build(
    server: &Server,
    name: &str,
    arguments: &serde_json::Value,
) -> Result<Request, serde_json::Value> {
    let client = server.client.clone();
    let date = || -> Result<String, serde_json::Value> {
        let raw = arguments["date"]
            .as_str()
            .ok_or_else(|| tool_error("The date argument is required and must be a string."))?;
        chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .map_err(|_| tool_error(format!("{raw} is not a date. Use YYYY-MM-DD.")))?;
        Ok(raw.to_string())
    };
    let rule = || -> Result<crate::rules::Rule, serde_json::Value> {
        // add_rule may arrive without an id; the app generates one with
        // rules::new_id. Deserialisation must not require the field, so an
        // absent id is filled with an empty string here and replaced there.
        let mut value = arguments["rule"].clone();
        if let serde_json::Value::Object(map) = &mut value {
            map.entry("id").or_insert(serde_json::json!(""));
        }
        serde_json::from_value::<crate::rules::Rule>(value).map_err(|error| {
            tool_error(format!(
                "The rule argument is not a valid rule: {error}. A rule needs a target with \
                 exactly one of app, website or title, and an action of exclude, headings_only \
                 or full. The id is optional when adding and required when updating or removing."
            ))
        })
    };

    Ok(match name {
        "capture_status" => Request::CaptureStatus,
        "start_capture" => Request::StartCapture { client },
        "stop_capture" => Request::StopCapture { client },
        "summarise_day" => Request::SummariseDay {
            date: date()?,
            client,
        },
        "open_day" => Request::OpenDay { date: date()? },
        "add_rule" => Request::AddRule {
            rule: rule()?,
            client,
        },
        "update_rule" => {
            let rule = rule()?;
            if rule.id.trim().is_empty() {
                return Err(tool_error(
                    "update_rule needs the rule's id. Call list_rules to find it.",
                ));
            }
            Request::UpdateRule { rule, client }
        }
        "remove_rule" => Request::RemoveRule {
            id: arguments["id"]
                .as_str()
                .ok_or_else(|| tool_error("The id argument is required and must be a string."))?
                .to_string(),
            client,
        },
        "set_prompt" => Request::SetPrompt {
            text: arguments["text"]
                .as_str()
                .ok_or_else(|| tool_error("The text argument is required and must be a string."))?
                .to_string(),
            client,
        },
        "set_config" => {
            let patch = arguments.get("patch").cloned().unwrap_or(serde_json::Value::Null);
            if !patch.is_object() {
                return Err(tool_error(
                    "The patch argument is required and must be an object of setting keys. \
                     Call get_config to see the keys that can be set.",
                ));
            }
            Request::SetConfig { patch, client }
        }
        other => return Err(tool_error(format!("Unknown tool: {other}"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{self, Request, Response};

    fn server_with_app(data_dir: &std::path::Path) -> crate::mcp::Server {
        let socket = ipc::socket_path(data_dir);
        let listener = ipc::bind(&socket).unwrap();
        std::thread::spawn(move || {
            ipc::serve(listener, |request| match request {
                Request::CaptureStatus => Response::Ok(serde_json::json!({
                    "running": true, "blocks_today": 12, "focused_app": "Xcode", "jobs": []
                })),
                Request::SummariseDay { date, client } => Response::Ok(serde_json::json!({
                    "job_id": format!("job-{date}-{client}"), "status": "queued"
                })),
                Request::RemoveRule { id, .. } => {
                    Response::err("locked", format!("{id} is a built-in protection."))
                }
                Request::SetConfig { patch, .. } => Response::Ok(patch),
                other => Response::Ok(serde_json::json!({ "ok": format!("{other:?}") })),
            });
        });
        crate::mcp::Server {
            config_dir: data_dir.to_path_buf(),
            app_data_dir: data_dir.to_path_buf(),
            client: "Claude Code".to_string(),
        }
    }

    #[test]
    fn capture_status_comes_back_as_structured_content() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = server_with_app(dir.path());
        let out = call(&mut server, "capture_status", &serde_json::json!({}));
        assert_eq!(out["isError"], false);
        assert_eq!(out["structuredContent"]["blocks_today"], 12);
    }

    #[test]
    fn the_client_name_from_initialize_travels_with_every_write() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = server_with_app(dir.path());
        let out = call(
            &mut server,
            "summarise_day",
            &serde_json::json!({ "date": "2026-08-30" }),
        );
        assert_eq!(
            out["structuredContent"]["job_id"],
            "job-2026-08-30-Claude Code"
        );
    }

    #[test]
    fn a_refusal_from_the_app_becomes_a_tool_error_with_the_apps_message() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = server_with_app(dir.path());
        let out = call(
            &mut server,
            "remove_rule",
            &serde_json::json!({ "id": "password-managers" }),
        );
        assert_eq!(out["isError"], true);
        assert!(out["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("built-in protection"));
    }

    #[test]
    fn a_closed_app_is_a_tool_error_that_says_reads_still_work() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = crate::mcp::Server {
            config_dir: dir.path().to_path_buf(),
            app_data_dir: dir.path().to_path_buf(),
            client: "test".to_string(),
        };
        let out = call(&mut server, "start_capture", &serde_json::json!({}));
        assert_eq!(out["isError"], true);
        let text = out["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Ambient Context is not running"), "{text}");
        assert!(text.contains("reading days"), "{text}");
    }

    #[test]
    fn set_config_passes_the_patch_through_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = server_with_app(dir.path());
        let out = call(
            &mut server,
            "set_config",
            &serde_json::json!({ "patch": { "interval_secs": 9 } }),
        );
        assert_eq!(out["structuredContent"]["interval_secs"], 9);
    }

    #[test]
    fn a_rule_argument_that_is_not_a_rule_fails_before_the_socket() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = server_with_app(dir.path());
        let out = call(
            &mut server,
            "add_rule",
            &serde_json::json!({ "rule": "never record Slack" }),
        );
        assert_eq!(out["isError"], true);
        assert!(out["content"][0]["text"].as_str().unwrap().contains("rule"));
    }
}
