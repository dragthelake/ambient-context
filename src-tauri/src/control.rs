use crate::ipc::{Request, Response};
use crate::{capture, jobs, ledger, settings, tray};
use tauri::{AppHandle, Manager};

/// Turns one control request into one response. Every arm runs on a socket
/// thread, never on the capture thread, and never holds a lock across a
/// file write.
pub fn handle(app: &AppHandle, request: Request) -> Response {
    match request {
        Request::CaptureStatus => capture_status(app),
        Request::StartCapture { client } => start_capture(app, &client),
        Request::StopCapture { client } => stop_capture(app, &client),
        Request::SummariseDay { date, client } => summarise_day(app, &date, &client),
        Request::JobStatus { id } => job_status(app, &id),
        Request::OpenDay { date } => open_day(app, &date),
        Request::SetConfig { patch, client } => writes::set_config(app, patch, &client),
        Request::AddRule { rule, client } => writes::add_rule(app, rule, &client),
        Request::UpdateRule { rule, client } => writes::update_rule(app, rule, &client),
        Request::RemoveRule { id, client } => writes::remove_rule(app, &id, &client),
        Request::SetPrompt { text, client } => writes::set_prompt(app, text, &client),
    }
}

fn capture_status(app: &AppHandle) -> Response {
    let state = app.state::<capture::CaptureState>();
    let queue = app.state::<jobs::JobQueue>();
    Response::Ok(serde_json::json!({
        "running": state.is_running(),
        "blocks_today": state.blocks_today(),
        "focused_app": crate::reader::macos::snapshot().map(|snap| snap.app),
        "jobs": queue.recent(),
    }))
}

fn start_capture(app: &AppHandle, client: &str) -> Response {
    let state = app.state::<capture::CaptureState>().inner().clone();
    let mut config = settings::load(app);
    if config.folder.is_none() {
        return Response::err(
            "invalid",
            "No capture folder is set. Open Ambient Context and choose one first.",
        );
    }
    if !state.is_running() {
        config.enabled = true;
        if let Err(error) = settings::save(app, &config) {
            return Response::err("io", error.to_string());
        }
        capture::start(app.clone(), &state, config);
        tray::refresh(app, true);
    }
    ledger_config_write(app, client, "start_capture", "enabled = true");
    capture_status(app)
}

fn stop_capture(app: &AppHandle, client: &str) -> Response {
    let state = app.state::<capture::CaptureState>().inner().clone();
    let mut config = settings::load(app);
    if state.is_running() {
        config.enabled = false;
        if let Err(error) = settings::save(app, &config) {
            return Response::err("io", error.to_string());
        }
        capture::stop(&state);
        tray::refresh(app, false);
    }
    ledger_config_write(app, client, "stop_capture", "enabled = false");
    capture_status(app)
}

fn summarise_day(app: &AppHandle, date: &str, client: &str) -> Response {
    let Ok(date) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") else {
        return Response::err(
            "invalid",
            format!("{date} is not a date in YYYY-MM-DD form."),
        );
    };
    let config = settings::load(app);
    if config.engine.is_none() {
        return Response::err(
            "no_engine",
            "No engine is connected. Connect one in Settings, then try again.",
        );
    }
    let Some(folder) = config.folder.clone() else {
        return Response::err("invalid", "No capture folder is set.");
    };
    if crate::days::read_day(&folder, date).is_none() {
        return Response::err("not_found", format!("There is no capture for {date}."));
    }
    let queue = app.state::<jobs::JobQueue>();
    let id = queue.enqueue_summarise_with(
        date,
        ledger::Trigger::Mcp {
            client: client.to_string(),
        },
    );
    Response::Ok(serde_json::json!({
        "job_id": id.to_string(),
        "status": "queued",
        "note": "Poll capture_status and look for this job id under jobs.",
    }))
}

fn job_status(app: &AppHandle, id: &str) -> Response {
    let queue = app.state::<jobs::JobQueue>();
    match queue.find(id) {
        Some(job) => Response::Ok(serde_json::to_value(job).unwrap_or(serde_json::Value::Null)),
        None => Response::err("not_found", format!("No job with id {id}.")),
    }
}

fn open_day(app: &AppHandle, date: &str) -> Response {
    let Ok(parsed) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") else {
        return Response::err(
            "invalid",
            format!("{date} is not a date in YYYY-MM-DD form."),
        );
    };
    crate::open_main_window_on(app, parsed);
    Response::Ok(serde_json::json!({ "opened": date }))
}

fn ledger_config_write(app: &AppHandle, client: &str, action: &str, detail: &str) {
    writes::ledger_write(
        app,
        client,
        action,
        &settings::config_dir(app).join("settings.json"),
        detail.to_string(),
        ledger::Disposition::Applied,
    );
}

/// Every write in the product funnels through here. The validation is the
/// same code the Settings UI calls, which is the whole reason writes do not
/// happen in the `mcp` process.
pub mod writes {
    use super::*;
    use crate::{prompt, rules};

    /// (code, message), matching the ipc error vocabulary.
    pub type Refusal = (&'static str, String);

    /// Exactly the keys the Settings page exposes. There is deliberately no
    /// retention key: nothing in the product deletes captured content.
    pub(crate) const SETTABLE_KEYS: &[&str] = &[
        "folder",
        "interval_secs",
        "min_dwell_secs",
        "similarity_threshold",
        "engine",
        "schedule_hhmm",
        "launch_at_login",
        "editor",
        "max_block_chars",
        "write_references",
        "extra_redaction_patterns",
    ];

    pub fn apply_patch(
        current: &settings::Settings,
        patch: serde_json::Value,
    ) -> Result<settings::Settings, Refusal> {
        let serde_json::Value::Object(patch) = patch else {
            return Err((
                "invalid",
                "The patch must be a JSON object of setting keys.".into(),
            ));
        };
        let mut merged = match serde_json::to_value(current) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => return Err(("io", "The current settings could not be read.".into())),
        };
        for (key, value) in patch {
            if key == "enabled" {
                return Err((
                    "unknown_key",
                    "Capture is turned on and off with start_capture and stop_capture, \
                     not through set_config."
                        .into(),
                ));
            }
            if key.contains("retain") || key.contains("retention") {
                return Err((
                    "unknown_key",
                    format!(
                        "There is no {key} setting. Ambient Context has no retention sweep: \
                         nothing deletes captured content, and deletion is done in Finder."
                    ),
                ));
            }
            if !SETTABLE_KEYS.contains(&key.as_str()) {
                return Err((
                    "unknown_key",
                    format!(
                        "{key} is not a setting. The settable keys are: {}.",
                        SETTABLE_KEYS.join(", ")
                    ),
                ));
            }
            merged.insert(key, value);
        }
        serde_json::from_value::<settings::Settings>(serde_json::Value::Object(merged))
            .map_err(|error| ("invalid", error.to_string()))
    }

    /// Records one config-file write. The file is hashed before it changes,
    /// because that is what makes the entry reproducible: the ledger names the
    /// input the actor saw, not the output it left behind.
    pub fn ledger_write(
        app: &AppHandle,
        client: &str,
        action: &str,
        target: &std::path::Path,
        output: String,
        disposition: ledger::Disposition,
    ) {
        let Some(folder) = settings::load(app).folder else {
            return;
        };
        let inputs = ledger::hash_file(target)
            .map(|input| vec![input])
            .unwrap_or_default();
        let entry = ledger::Entry {
            at: chrono::Local::now(),
            trigger: ledger::Trigger::Mcp {
                client: client.to_string(),
            },
            action: action.to_string(),
            prompt_id: None,
            prompt_sha256: None,
            engine: None,
            inputs,
            output: Some(output),
            reasoning: None,
            disposition,
        };
        if let Err(error) = ledger::append(&folder, &entry) {
            eprintln!("ledger write failed for {action}: {error}");
        }
        // Settings shows the last MCP write without parsing markdown back out
        // of the ledger, so the same fact is recorded once more, structured.
        if let Ok(data_dir) = app.path().app_data_dir() {
            let note = serde_json::json!({
                "at": chrono::Local::now().to_rfc3339(),
                "action": action,
                "client": client,
            });
            let _ = std::fs::write(
                data_dir.join("last-mcp-write.json"),
                serde_json::to_string_pretty(&note).unwrap_or_default(),
            );
        }
    }

    fn rule_refusal(error: rules::RuleError) -> Response {
        match error {
            rules::RuleError::Duplicate(id) => {
                Response::err("duplicate", format!("A rule with id {id} already exists."))
            }
            rules::RuleError::NotFound(id) => {
                Response::err("not_found", format!("There is no rule with id {id}."))
            }
            rules::RuleError::Locked(id) => Response::err(
                "locked",
                format!(
                    "{id} is a built-in protection. Built-in protections are shown so you can see \
                     what is never recorded, and cannot be changed from any surface."
                ),
            ),
            // Invalid already carries a sentence worth showing; Display is the
            // same string the Settings UI puts in front of a person.
            other => Response::err("invalid", other.to_string()),
        }
    }

    pub fn set_config(app: &AppHandle, patch: serde_json::Value, client: &str) -> Response {
        let dir = settings::config_dir(app);
        let current = settings::load(app);
        let patched = match apply_patch(&current, patch.clone()) {
            Ok(patched) => patched,
            Err((code, message)) => {
                ledger_write(
                    app,
                    client,
                    "set_config",
                    &dir.join("settings.json"),
                    patch.to_string(),
                    ledger::Disposition::Rejected {
                        reason: message.clone(),
                    },
                );
                return Response::err(code, message);
            }
        };
        ledger_write(
            app,
            client,
            "set_config",
            &dir.join("settings.json"),
            patch.to_string(),
            ledger::Disposition::Applied,
        );
        if let Err(error) = settings::save(app, &patched) {
            return Response::err("io", error.to_string());
        }
        crate::apply_settings_change(app, &current, &patched);
        Response::Ok(serde_json::to_value(&patched).unwrap_or(serde_json::Value::Null))
    }

    pub fn add_rule(app: &AppHandle, mut rule: rules::Rule, client: &str) -> Response {
        // An agent naming its own ids will eventually collide with one the UI
        // generated. An empty id means "give me one", the same as the UI's
        // add button does.
        if rule.id.trim().is_empty() {
            rule.id = rules::new_id(&rules::load(&settings::config_dir(app)));
        }
        write_rules(app, client, "add_rule", |set| set.add(rule.clone()))
    }

    pub fn update_rule(app: &AppHandle, rule: rules::Rule, client: &str) -> Response {
        write_rules(app, client, "update_rule", |set| set.update(rule.clone()))
    }

    pub fn remove_rule(app: &AppHandle, id: &str, client: &str) -> Response {
        write_rules(app, client, "remove_rule", |set| set.remove(id))
    }

    fn write_rules<F>(app: &AppHandle, client: &str, action: &str, mutate: F) -> Response
    where
        F: Fn(&mut rules::Rules) -> Result<(), rules::RuleError>,
    {
        let dir = settings::config_dir(app);
        let target = dir.join("rules.json");
        let mut set = rules::load(&dir);
        if let Err(error) = mutate(&mut set) {
            let response = rule_refusal(error);
            let reason = match &response {
                Response::Err { message, .. } => message.clone(),
                Response::Ok(_) => String::new(),
            };
            ledger_write(
                app,
                client,
                action,
                &target,
                String::new(),
                ledger::Disposition::Rejected { reason },
            );
            return response;
        }
        if let Err(error) = rules::validate(&set) {
            let response = rule_refusal(error);
            let reason = match &response {
                Response::Err { message, .. } => message.clone(),
                Response::Ok(_) => String::new(),
            };
            ledger_write(
                app,
                client,
                action,
                &target,
                String::new(),
                ledger::Disposition::Rejected { reason },
            );
            return response;
        }
        let output = serde_json::to_string(&set).unwrap_or_default();
        ledger_write(
            app,
            client,
            action,
            &target,
            output,
            ledger::Disposition::Applied,
        );
        if let Err(error) = rules::save(&dir, &set) {
            return Response::err("io", error.to_string());
        }
        Response::Ok(serde_json::json!({
            "rules": set.rules,
            "built_ins": rules::built_ins(),
        }))
    }

    pub fn set_prompt(app: &AppHandle, text: String, client: &str) -> Response {
        let dir = settings::config_dir(app);
        let target = dir.join("prompts").join("day-context.md");
        match prompt::set(&dir, &text) {
            Ok(()) => {
                ledger_write(
                    app,
                    client,
                    "set_prompt",
                    &target,
                    text.clone(),
                    ledger::Disposition::Applied,
                );
                Response::Ok(
                    serde_json::json!({ "customised": true, "chars": text.chars().count() }),
                )
            }
            Err(prompt::PromptError::Empty) => {
                let reason = "The prompt cannot be empty.".to_string();
                ledger_write(
                    app,
                    client,
                    "set_prompt",
                    &target,
                    text,
                    ledger::Disposition::Rejected {
                        reason: reason.clone(),
                    },
                );
                Response::err("invalid", reason)
            }
            Err(prompt::PromptError::MissingHeading(heading)) => {
                let reason = format!(
                    "The prompt must still ask for the {heading} heading, or summary validation \
                     will reject every summary it produces."
                );
                ledger_write(
                    app,
                    client,
                    "set_prompt",
                    &target,
                    text,
                    ledger::Disposition::Rejected {
                        reason: reason.clone(),
                    },
                );
                Response::err("invalid", reason)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::writes::apply_patch;
    use crate::settings::Settings;

    #[test]
    fn a_known_key_is_applied_and_the_rest_is_left_alone() {
        let settings = Settings {
            interval_secs: 5,
            ..Settings::default()
        };
        let patched = apply_patch(&settings, serde_json::json!({ "interval_secs": 12 })).unwrap();
        assert_eq!(patched.interval_secs, 12);
        assert_eq!(patched.min_dwell_secs, settings.min_dwell_secs);
    }

    #[test]
    fn an_unknown_key_is_refused_by_name() {
        let error = apply_patch(
            &Settings::default(),
            serde_json::json!({ "colour": "blue" }),
        )
        .unwrap_err();
        assert_eq!(error.0, "unknown_key");
        assert!(error.1.contains("colour"), "{}", error.1);
    }

    #[test]
    fn retention_is_refused_with_an_explanation_rather_than_as_a_typo() {
        let error = apply_patch(
            &Settings::default(),
            serde_json::json!({ "retention_days": 30 }),
        )
        .unwrap_err();
        assert_eq!(error.0, "unknown_key");
        assert!(
            error.1.contains("nothing deletes captured content"),
            "{}",
            error.1
        );
    }

    #[test]
    fn enabled_is_refused_and_names_the_capture_tools() {
        let error = apply_patch(
            &Settings::default(),
            serde_json::json!({ "enabled": false }),
        )
        .unwrap_err();
        assert!(error.1.contains("stop_capture"), "{}", error.1);
    }

    #[test]
    fn the_schedule_is_patched_by_its_real_key_name() {
        let patched = apply_patch(
            &Settings::default(),
            serde_json::json!({ "schedule_hhmm": "07:30" }),
        )
        .unwrap();
        assert_eq!(patched.schedule_hhmm.as_deref(), Some("07:30"));
    }

    #[test]
    fn the_schedule_can_be_cleared_back_to_manual_only() {
        let settings = Settings {
            schedule_hhmm: Some("06:00".to_string()),
            ..Settings::default()
        };
        let patched = apply_patch(&settings, serde_json::json!({ "schedule_hhmm": null })).unwrap();
        assert_eq!(patched.schedule_hhmm, None);
    }

    #[test]
    fn a_value_of_the_wrong_type_is_refused_as_invalid() {
        let error = apply_patch(
            &Settings::default(),
            serde_json::json!({ "interval_secs": "fast" }),
        )
        .unwrap_err();
        assert_eq!(error.0, "invalid");
    }

    #[test]
    fn a_patch_that_is_not_an_object_is_refused() {
        let error = apply_patch(&Settings::default(), serde_json::json!([1, 2])).unwrap_err();
        assert_eq!(error.0, "invalid");
    }
}
