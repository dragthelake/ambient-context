use serde_json::json;

pub struct Def {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub input_schema: serde_json::Value,
    pub read_only: bool,
    pub destructive: bool,
    pub idempotent: bool,
}

fn none() -> serde_json::Value {
    json!({ "type": "object", "additionalProperties": false })
}

fn args(properties: serde_json::Value, required: &[&str]) -> serde_json::Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn date_property() -> serde_json::Value {
    json!({ "type": "string", "description": "A date in YYYY-MM-DD form, for example 2026-08-30." })
}

fn rule_property() -> serde_json::Value {
    json!({
        "type": "object",
        "description": "A capture rule. Built-in protections are not rules and cannot be written.",
        "properties": {
            "id": { "type": "string", "description": "Unique id for the rule. Leave it out of add_rule and one is generated; update_rule and remove_rule need the id list_rules reported." },
            "target": {
                "type": "object",
                "description": "Exactly one of app, website or title.",
                "properties": {
                    "app": { "type": "string", "description": "Matched against the application name." },
                    "website": { "type": "string", "description": "Matched against the host of the captured url: reference, by exact host or dotted suffix, falling back to a title substring where the browser exposed no URL." },
                    "title": { "type": "string", "description": "A case-insensitive substring of the window title." }
                },
                "additionalProperties": false
            },
            "action": {
                "type": "string",
                "enum": ["exclude", "headings_only", "full"],
                "description": "exclude drops the window entirely, headings_only keeps the heading and drops the body, full is the default and carves an exception out of a broader rule."
            },
            "note": { "type": "string", "description": "Optional note explaining why the rule exists." }
        },
        "required": ["target", "action"],
        "additionalProperties": false
    })
}

pub fn defs() -> Vec<Def> {
    vec![
        Def {
            name: "capture_status",
            title: "Capture status",
            description: "Reports whether capture is running, how many blocks were recorded today, which app is focused, and the eight most recent summary jobs with their status. Poll this after summarise_day to see whether a job finished.",
            input_schema: none(),
            read_only: true, destructive: false, idempotent: true,
        },
        Def {
            name: "start_capture",
            title: "Start capture",
            description: "Turns capture on, the same as clicking the menu bar icon. Writes the change to settings and records it in the day's ledger. Needs Ambient Context to be running.",
            input_schema: none(),
            read_only: false, destructive: false, idempotent: true,
        },
        Def {
            name: "stop_capture",
            title: "Stop capture",
            description: "Turns capture off and leaves it off across restarts, the same as clicking the menu bar icon. Writes the change to settings and records it in the day's ledger. Nothing already recorded is removed.",
            input_schema: none(),
            read_only: false, destructive: false, idempotent: true,
        },
        Def {
            name: "list_days",
            title: "List recorded days",
            description: "Lists every day in the capture folder with its date, whether a summary exists, the size of the day file in bytes, and the summary's title where there is one.",
            input_schema: none(),
            read_only: true, destructive: false, idempotent: true,
        },
        Def {
            name: "read_day",
            title: "Read a day's record",
            description: "Returns the raw record for one day exactly as it is on disk: time-stamped blocks with the application, window title and file or url reference. Supply from and to to keep only the blocks that start inside that time range.",
            input_schema: args(
                json!({
                    "date": date_property(),
                    "from": { "type": "string", "description": "Optional start time, 24-hour HH:MM. Blocks starting before it are dropped." },
                    "to": { "type": "string", "description": "Optional end time, 24-hour HH:MM, exclusive. Blocks starting at or after it are dropped." }
                }),
                &["date"],
            ),
            read_only: true, destructive: false, idempotent: true,
        },
        Def {
            name: "read_summary",
            title: "Read a day's summary",
            description: "Returns the generated summary for one day. Summaries interpret and cite the raw record; read_day is the evidence behind them.",
            input_schema: args(json!({ "date": date_property() }), &["date"]),
            read_only: true, destructive: false, idempotent: true,
        },
        Def {
            name: "search_record",
            title: "Search the record",
            description: "Case-insensitive substring search across every day file and every summary. Returns the date, the layer, the line number, the matching line and two lines of context either side.",
            input_schema: args(
                json!({
                    "query": { "type": "string", "description": "The text to look for. Matching is plain substring, not regular expressions." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200, "description": "Maximum number of hits. Defaults to 50." }
                }),
                &["query"],
            ),
            read_only: true, destructive: false, idempotent: true,
        },
        Def {
            name: "read_ledger",
            title: "Read a day's ledger",
            description: "Returns the ledger for one day: every model action and every configuration change, with what triggered it, the inputs and their hashes, the output, the stated reasoning and the outcome.",
            input_schema: args(json!({ "date": date_property() }), &["date"]),
            read_only: true, destructive: false, idempotent: true,
        },
        Def {
            name: "summarise_day",
            title: "Summarise a day",
            description: "Queues a summary for one day using the engine the user connected, and replaces the existing summary if there is one. Returns a job id immediately because a run can take minutes; poll capture_status for the outcome. Needs Ambient Context to be running with an engine connected.",
            input_schema: args(json!({ "date": date_property() }), &["date"]),
            read_only: false, destructive: true, idempotent: false,
        },
        Def {
            name: "list_rules",
            title: "List capture rules",
            description: "Lists the user's capture rules and the locked built-in protections. Built-ins are shown so you can see what is never recorded; they cannot be changed.",
            input_schema: none(),
            read_only: true, destructive: false, idempotent: true,
        },
        Def {
            name: "add_rule",
            title: "Add a capture rule",
            description: "Adds one capture rule and writes it to the rules file, which changes what is recorded from the next snapshot onwards. Records the change in the day's ledger. Needs Ambient Context to be running.",
            input_schema: args(json!({ "rule": rule_property() }), &["rule"]),
            read_only: false, destructive: false, idempotent: false,
        },
        Def {
            name: "update_rule",
            title: "Update a capture rule",
            description: "Replaces an existing capture rule with the same id, which changes what is recorded from the next snapshot onwards. Records the change in the day's ledger. Needs Ambient Context to be running.",
            input_schema: args(json!({ "rule": rule_property() }), &["rule"]),
            read_only: false, destructive: true, idempotent: true,
        },
        Def {
            name: "remove_rule",
            title: "Remove a capture rule",
            description: "Removes one capture rule by id, which changes what is recorded from the next snapshot onwards. Built-in protections are refused. Records the change in the day's ledger. Needs Ambient Context to be running.",
            input_schema: args(
                json!({ "id": { "type": "string", "description": "The id of the rule to remove." } }),
                &["id"],
            ),
            read_only: false, destructive: true, idempotent: true,
        },
        Def {
            name: "get_prompt",
            title: "Read the summary prompt",
            description: "Returns the prompt used to generate day summaries, and whether it is the bundled default or a customised copy.",
            input_schema: none(),
            read_only: true, destructive: false, idempotent: true,
        },
        Def {
            name: "set_prompt",
            title: "Replace the summary prompt",
            description: "Replaces the summary prompt in full, which changes the shape of every summary generated afterwards. The text is rejected if it drops a heading that summary validation requires. Records the change in the day's ledger. Needs Ambient Context to be running.",
            input_schema: args(
                json!({ "text": { "type": "string", "description": "The complete replacement prompt, in markdown." } }),
                &["text"],
            ),
            read_only: false, destructive: true, idempotent: true,
        },
        Def {
            name: "get_config",
            title: "Read the settings",
            description: "Returns every setting the Settings page exposes: capture folder, poll interval, minimum dwell, similarity threshold, engine, the daily summary time as schedule_hhmm, launch at login, editor, block size cap, whether references are written, and extra redaction patterns, plus the app version and the list of keys set_config accepts.",
            input_schema: none(),
            read_only: true, destructive: false, idempotent: true,
        },
        Def {
            name: "set_config",
            title: "Change the settings",
            description: "Changes one or more settings, writes them to the settings file and applies them immediately. Only keys the Settings page exposes are accepted; there is no retention setting, and capture is turned on and off with start_capture and stop_capture. Records the change in the day's ledger. Needs Ambient Context to be running.",
            input_schema: args(
                json!({
                    "patch": {
                        "type": "object",
                        "description": "The keys to change, using the same names get_config returns. Keys not present are left alone. The daily summary time is schedule_hhmm, a 24-hour HH:MM string, or null for manual only."
                    }
                }),
                &["patch"],
            ),
            read_only: false, destructive: true, idempotent: true,
        },
        Def {
            name: "open_day",
            title: "Open a day in the app",
            description: "Opens the Ambient Context window on a given day and brings it to the front, for handoffs that end with a person checking it looks right. Changes no files. Needs Ambient Context to be running.",
            input_schema: args(json!({ "date": date_property() }), &["date"]),
            read_only: false, destructive: false, idempotent: true,
        },
    ]
}

pub fn list() -> Vec<serde_json::Value> {
    defs()
        .into_iter()
        .map(|def| {
            json!({
                "name": def.name,
                "title": def.title,
                "description": def.description,
                "inputSchema": def.input_schema,
                "annotations": {
                    "title": def.title,
                    "readOnlyHint": def.read_only,
                    "destructiveHint": def.destructive,
                    "idempotentHint": def.idempotent,
                    // Nothing here touches the network or anything outside the
                    // user's own capture folder and config directory.
                    "openWorldHint": false
                }
            })
        })
        .collect()
}

pub fn exists(name: &str) -> bool {
    defs().iter().any(|def| def.name == name)
}

// Replaced by the real dispatcher in Task 8.
pub fn call(
    _server: &mut crate::mcp::Server,
    _name: &str,
    _arguments: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": "not yet implemented" }],
        "isError": true
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED: [&str; 18] = [
        "add_rule", "capture_status", "get_config", "get_prompt", "list_days", "list_rules",
        "open_day", "read_day", "read_ledger", "read_summary", "remove_rule", "search_record",
        "set_config", "set_prompt", "start_capture", "stop_capture", "summarise_day",
        "update_rule",
    ];

    #[test]
    fn there_are_exactly_eighteen_tools_and_they_are_the_spec_table() {
        let mut names: Vec<&str> = defs().iter().map(|def| def.name).collect();
        names.sort_unstable();
        assert_eq!(names, EXPECTED);
    }

    #[test]
    fn every_tool_has_a_description_that_says_what_it_does() {
        for def in defs() {
            assert!(def.description.len() > 30, "{} has a stub description", def.name);
        }
    }

    #[test]
    fn every_write_tool_says_so_in_its_description() {
        for def in defs().iter().filter(|def| !def.read_only) {
            let text = def.description.to_lowercase();
            let states_effect = ["writes", "changes", "turns", "removes", "adds", "replaces", "opens", "queues"]
                .iter()
                .any(|verb| text.contains(verb));
            assert!(states_effect, "{} does not state its effect", def.name);
        }
    }

    #[test]
    fn the_eight_readers_are_annotated_read_only_and_never_destructive() {
        let readers = [
            "capture_status", "list_days", "read_day", "read_summary", "search_record",
            "read_ledger", "list_rules", "get_prompt", "get_config",
        ];
        for name in readers {
            let def = defs().into_iter().find(|def| def.name == name).unwrap();
            assert!(def.read_only, "{name} should be readOnlyHint true");
            assert!(!def.destructive, "{name} should be destructiveHint false");
        }
    }

    #[test]
    fn the_tools_that_overwrite_something_are_annotated_destructive() {
        for name in ["set_config", "set_prompt", "update_rule", "remove_rule", "summarise_day"] {
            let def = defs().into_iter().find(|def| def.name == name).unwrap();
            assert!(def.destructive, "{name} overwrites and should say so");
        }
    }

    #[test]
    fn add_rule_is_a_write_but_not_destructive() {
        let def = defs().into_iter().find(|def| def.name == "add_rule").unwrap();
        assert!(!def.read_only);
        assert!(!def.destructive);
    }

    #[test]
    fn every_input_schema_is_an_object_schema() {
        for def in defs() {
            assert_eq!(def.input_schema["type"], "object", "{}", def.name);
        }
    }

    #[test]
    fn a_tool_with_no_arguments_accepts_only_an_empty_object() {
        let def = defs().into_iter().find(|def| def.name == "capture_status").unwrap();
        assert_eq!(def.input_schema["additionalProperties"], false);
        assert!(def.input_schema.get("properties").is_none());
    }

    #[test]
    fn the_json_form_carries_annotations_the_client_can_read() {
        let listed = list();
        let read_day = listed.iter().find(|tool| tool["name"] == "read_day").unwrap();
        assert_eq!(read_day["annotations"]["readOnlyHint"], true);
        assert_eq!(read_day["annotations"]["openWorldHint"], false);
        assert!(read_day["inputSchema"]["properties"]["date"].is_object());
    }

    #[test]
    fn exists_answers_for_a_real_name_and_a_made_up_one() {
        assert!(exists("read_summary"));
        assert!(!exists("delete_day"));
    }
}
