pub mod files;
pub mod tools;

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub const LATEST_PROTOCOL: &str = "2025-11-25";
const SUPPORTED_PROTOCOLS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];

pub struct Server {
    pub config_dir: PathBuf,
    pub app_data_dir: PathBuf,
    /// clientInfo.name from initialize. It names the actor in every ledger
    /// entry this session writes, so it is captured before any tool runs.
    pub client: String,
}

/// The macOS locations, computed without Tauri because this process never
/// builds an App. The environment overrides exist so the integration test can
/// point the subcommand at a tempdir.
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("AMBIENT_CONTEXT_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    home().join("Library/Application Support/com.0x0000007a.ambientcontext")
}

pub fn app_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("AMBIENT_CONTEXT_DATA_DIR") {
        return PathBuf::from(dir);
    }
    home().join("Library/Application Support/com.0x0000007a.ambientcontext")
}

fn home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"))
}

/// Reads newline-delimited JSON-RPC from stdin and writes it to stdout until
/// stdin closes, then exits 0. Everything else goes to stderr: one stray line
/// on stdout breaks every client at once.
pub fn run_stdio(config_dir: PathBuf, app_data_dir: PathBuf) -> ! {
    let mut server = Server {
        config_dir,
        app_data_dir,
        client: "unknown MCP client".to_string(),
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let message: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                let answer = serde_json::json!({
                    "jsonrpc": "2.0", "id": serde_json::Value::Null,
                    "error": { "code": -32700, "message": format!("Parse error: {error}") }
                });
                write_line(&mut stdout, &answer);
                continue;
            }
        };
        if let Some(answer) = dispatch(&mut server, &message) {
            write_line(&mut stdout, &answer);
        }
    }
    std::process::exit(0)
}

fn write_line(stdout: &mut std::io::Stdout, value: &serde_json::Value) {
    // to_string never emits a raw newline: control characters inside strings
    // are escaped, which is what keeps the framing intact.
    let _ = writeln!(stdout, "{value}");
    let _ = stdout.flush();
}

/// One message in, at most one message out. `None` means the message was a
/// notification, which must never be answered.
pub fn dispatch(server: &mut Server, message: &serde_json::Value) -> Option<serde_json::Value> {
    let method = message.get("method")?.as_str()?;
    let id = message.get("id").cloned();
    if id.is_none() {
        return None;
    }
    let id = id.unwrap_or(serde_json::Value::Null);
    let params = message.get("params").cloned().unwrap_or(serde_json::json!({}));

    match method {
        "initialize" => {
            if let Some(name) = params["clientInfo"]["name"].as_str() {
                server.client = name.to_string();
            }
            let requested = params["protocolVersion"].as_str().unwrap_or(LATEST_PROTOCOL);
            let version = if SUPPORTED_PROTOCOLS.contains(&requested) {
                requested
            } else {
                LATEST_PROTOCOL
            };
            Some(result(
                id,
                serde_json::json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": {
                        "name": "ambient-context",
                        "title": "Ambient Context",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "instructions": "Ambient Context keeps a plain-text record of what the user \
                        worked on, one markdown file per day, with generated summaries beside it. \
                        Read tools work whether or not the app is running. Tools that change \
                        capture, settings, rules or the prompt need the app open, and every change \
                        they make is written to the day's ledger with this client named.",
                }),
            ))
        }
        "ping" => Some(result(id, serde_json::json!({}))),
        "tools/list" => Some(result(
            id,
            serde_json::json!({ "tools": crate::mcp::tools::list() }),
        )),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or_default().to_string();
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            if !crate::mcp::tools::exists(&name) {
                return Some(error(id, -32602, format!("Unknown tool: {name}")));
            }
            Some(result(id, crate::mcp::tools::call(server, &name, &arguments)))
        }
        other => Some(error(id, -32601, format!("Method not found: {other}"))),
    }
}

fn result(id: serde_json::Value, value: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": value })
}

fn error(id: serde_json::Value, code: i64, message: String) -> serde_json::Value {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> Server {
        Server {
            config_dir: std::path::PathBuf::from("/nonexistent/config"),
            app_data_dir: std::path::PathBuf::from("/nonexistent/data"),
            client: "unknown MCP client".to_string(),
        }
    }

    #[test]
    fn initialize_answers_with_the_tools_capability_and_the_server_name() {
        let mut server = server();
        let answer = dispatch(
            &mut server,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": { "name": "Claude Code", "version": "2.0.0" }
                }
            }),
        )
        .unwrap();
        assert_eq!(answer["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(answer["result"]["serverInfo"]["name"], "ambient-context");
        assert!(answer["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn initialize_records_the_client_name_for_the_ledger() {
        let mut server = server();
        dispatch(
            &mut server,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "clientInfo": { "name": "Zed" } }
            }),
        );
        assert_eq!(server.client, "Zed");
    }

    #[test]
    fn an_unsupported_protocol_version_gets_ours_rather_than_a_failure() {
        let mut server = server();
        let answer = dispatch(
            &mut server,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "1999-01-01" }
            }),
        )
        .unwrap();
        assert_eq!(answer["result"]["protocolVersion"], LATEST_PROTOCOL);
    }

    #[test]
    fn a_supported_older_version_is_echoed_back() {
        let mut server = server();
        let answer = dispatch(
            &mut server,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18" }
            }),
        )
        .unwrap();
        assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");
    }

    #[test]
    fn a_notification_gets_no_answer_at_all() {
        let mut server = server();
        assert!(dispatch(
            &mut server,
            &serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
        )
        .is_none());
    }

    #[test]
    fn ping_answers_with_an_empty_result() {
        let mut server = server();
        let answer = dispatch(
            &mut server,
            &serde_json::json!({ "jsonrpc": "2.0", "id": 7, "method": "ping" }),
        )
        .unwrap();
        assert_eq!(answer["id"], 7);
        assert_eq!(answer["result"], serde_json::json!({}));
    }

    #[test]
    fn an_unknown_method_is_a_protocol_error() {
        let mut server = server();
        let answer = dispatch(
            &mut server,
            &serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" }),
        )
        .unwrap();
        assert_eq!(answer["error"]["code"], -32601);
    }

    // unignored in Task 9, when tools::call is real
    #[test]
    #[ignore]
    fn an_unknown_tool_name_is_a_protocol_error_not_a_tool_error() {
        let mut server = server();
        let answer = dispatch(
            &mut server,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "rm_rf", "arguments": {} }
            }),
        )
        .unwrap();
        assert_eq!(answer["error"]["code"], -32602);
        assert!(answer["error"]["message"].as_str().unwrap().contains("rm_rf"));
    }

    // unignored in Task 9, when tools::call is real
    #[test]
    #[ignore]
    fn a_tool_failure_is_a_result_with_is_error_so_the_model_can_read_it() {
        let mut server = server();
        // No app, no config: a control tool must fail as a tool error.
        let answer = dispatch(
            &mut server,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": { "name": "start_capture", "arguments": {} }
            }),
        )
        .unwrap();
        assert_eq!(answer["result"]["isError"], true);
        let text = answer["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("not running"), "{text}");
    }

    #[test]
    fn no_answer_ever_contains_an_embedded_newline() {
        let mut server = server();
        let answer = dispatch(
            &mut server,
            &serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
        )
        .unwrap();
        let line = serde_json::to_string(&answer).unwrap();
        assert!(!line.contains('\n'), "a raw newline would break the stdio framing");
    }
}