use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};

const EXPECTED: [&str; 18] = [
    "add_rule", "capture_status", "get_config", "get_prompt", "list_days", "list_rules",
    "open_day", "read_day", "read_ledger", "read_summary", "remove_rule", "search_record",
    "set_config", "set_prompt", "start_capture", "stop_capture", "summarise_day", "update_rule",
];

struct Session {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Session {
    /// Launches the subcommand the way a client does, pointed at directories
    /// that do not exist: an MCP client may start this on a machine where the
    /// app has never been opened, and it must still speak the protocol.
    fn start() -> Session {
        let absent = std::env::temp_dir().join("ambient-context-absent-config");
        let mut child = Command::new(env!("CARGO_BIN_EXE_ambient-context"))
            .arg("mcp")
            .env_remove("DISPLAY")
            .env("AMBIENT_CONTEXT_CONFIG_DIR", &absent)
            .env("AMBIENT_CONTEXT_DATA_DIR", &absent)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the mcp subcommand should start");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Session {
            child,
            stdin,
            stdout,
        }
    }

    fn call(&mut self, message: serde_json::Value) -> serde_json::Value {
        writeln!(self.stdin, "{message}").expect("write");
        self.stdin.flush().expect("flush");
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read");
        assert!(!line.trim().is_empty(), "the server answered with nothing");
        serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("not JSON: {error}: {line}"))
    }

    fn notify(&mut self, message: serde_json::Value) {
        writeln!(self.stdin, "{message}").expect("write");
        self.stdin.flush().expect("flush");
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn the_subcommand_initialises_and_lists_the_eighteen_tools_with_no_app_and_no_config() {
    let mut session = Session::start();

    let initialised = session.call(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "integration test", "version": "0.0.0" }
        }
    }));
    assert_eq!(initialised["result"]["serverInfo"]["name"], "ambient-context");
    assert_eq!(initialised["result"]["protocolVersion"], "2025-11-25");
    assert!(initialised["result"]["capabilities"]["tools"].is_object());

    session.notify(serde_json::json!({
        "jsonrpc": "2.0", "method": "notifications/initialized"
    }));

    let listed = session.call(serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/list"
    }));
    let tools = listed["result"]["tools"].as_array().expect("tools array");
    let mut names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(names, EXPECTED);

    for tool in tools {
        assert!(
            tool["description"].as_str().unwrap().len() > 30,
            "{}",
            tool["name"]
        );
        assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
        assert!(
            tool["annotations"]["readOnlyHint"].is_boolean(),
            "{}",
            tool["name"]
        );
        assert!(
            tool["annotations"]["destructiveHint"].is_boolean(),
            "{}",
            tool["name"]
        );
    }
}

#[test]
fn a_write_tool_with_the_app_closed_fails_as_a_tool_error_rather_than_a_crash() {
    let mut session = Session::start();
    session.call(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "clientInfo": { "name": "integration test" } }
    }));
    let out = session.call(serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "stop_capture", "arguments": {} }
    }));
    assert_eq!(out["result"]["isError"], true);
    assert!(out["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Ambient Context is not running"));
}

#[test]
fn the_process_exits_cleanly_when_stdin_closes() {
    let mut session = Session::start();
    session.call(serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }));
    // Moving the real handle out of the struct and dropping it closes the
    // pipe, which is what ends the read loop.
    let stdin = std::mem::replace(
        &mut session.stdin,
        {
            let mut throwaway = Command::new("/bin/cat")
                .stdin(Stdio::piped())
                .spawn()
                .unwrap();
            throwaway.stdin.take().unwrap()
        },
    );
    drop(stdin);
    let status = session.child.wait().expect("wait");
    assert!(status.success(), "exited with {status}");
}
