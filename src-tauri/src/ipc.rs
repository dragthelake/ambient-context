use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One request from the `mcp` subcommand to the running app. Internally
/// tagged on `op`, so the wire form of a request with no fields is a single
/// key and stays readable in a log.
///
/// `client` travels on every write because the ledger entry names the actor,
/// and the app process is the only place that writes ledger entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    CaptureStatus,
    StartCapture {
        client: String,
    },
    StopCapture {
        client: String,
    },
    SummariseDay {
        date: String,
        client: String,
    },
    JobStatus {
        id: String,
    },
    SetConfig {
        patch: serde_json::Value,
        client: String,
    },
    AddRule {
        rule: crate::rules::Rule,
        client: String,
    },
    UpdateRule {
        rule: crate::rules::Rule,
        client: String,
    },
    RemoveRule {
        id: String,
        client: String,
    },
    SetPrompt {
        text: String,
        client: String,
    },
    OpenDay {
        date: String,
    },
}

/// Error codes, exhaustive:
/// - `not_running`   the socket is absent; produced by the client, never the app
/// - `bad_request`   the line did not deserialise into a Request
/// - `unknown_key`   a set_config patch named a key the settings page does not expose
/// - `invalid`       a value failed validation (rules::RuleError::Invalid, prompt::PromptError)
/// - `duplicate`     rules::RuleError::Duplicate
/// - `not_found`     rules::RuleError::NotFound, or a date with no capture
/// - `locked`        rules::RuleError::Locked, a built-in protection
/// - `no_engine`     summarise_day with no engine connected
/// - `io`            a filesystem failure, with the OS message
///
/// One response. Adjacently tagged rather than internally tagged: an `Ok`
/// body may be an array or a string, which an internally tagged newtype
/// variant cannot serialise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "body", rename_all = "snake_case")]
pub enum Response {
    Ok(serde_json::Value),
    Err { code: String, message: String },
}

impl Response {
    pub fn err(code: &str, message: impl Into<String>) -> Response {
        Response::Err {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

pub fn socket_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("control.sock")
}

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;

/// Binds the control socket, replacing a socket file a crashed process left
/// behind. The permissions are set twice on purpose: the directory to 0700
/// first, so that the window between `bind` and the socket's own `chmod` is
/// not reachable by another user in the first place.
pub fn bind(socket: &Path) -> std::io::Result<UnixListener> {
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    if socket.exists() {
        // A socket file survives a crash, and bind on it fails with AddrInUse
        // whether or not anything is listening. Connecting is the only way to
        // tell a live server from a corpse.
        if UnixStream::connect(socket).is_ok() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "another Ambient Context is already listening on the control socket",
            ));
        }
        std::fs::remove_file(socket)?;
    }
    let listener = UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Accepts connections forever. Blocking, so call it on its own thread.
/// One thread per connection: a client holds a connection open for the length
/// of a session, and there is never more than a handful of them.
pub fn serve<H>(listener: UnixListener, handler: H)
where
    H: Fn(Request) -> Response + Send + Sync + 'static,
{
    let handler = Arc::new(handler);
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let handler = Arc::clone(&handler);
        std::thread::spawn(move || serve_connection(stream, handler.as_ref()));
    }
}

/// A client that opens a connection and then says nothing holds a thread
/// forever. Ten seconds is longer than any request takes to arrive and
/// short enough that a wedged client releases the thread; a client makes
/// one connection per call, so this never cuts a live session short.
pub const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// The longest request line the server will read. A megabyte is far more
/// than any tool call needs and far less than a client can use to exhaust
/// the app's memory.
pub const MAX_LINE_BYTES: u64 = 1024 * 1024;

fn serve_connection<H: Fn(Request) -> Response>(stream: UnixStream, handler: &H) {
    let Ok(clone) = stream.try_clone() else {
        return;
    };
    let _ = clone.set_read_timeout(Some(READ_TIMEOUT));
    let mut reader = BufReader::new(clone);
    let mut writer = stream;
    loop {
        let mut line = String::new();
        let read = (&mut reader).take(MAX_LINE_BYTES + 1).read_line(&mut line);
        let Ok(count) = read else { return };
        if count == 0 {
            return;
        }
        if count as u64 > MAX_LINE_BYTES {
            // The rest of the oversized line is still in the stream and
            // cannot be told apart from the next request, so answer and
            // close rather than guess.
            let _ = write_response(
                &mut writer,
                &Response::err(
                    "bad_request",
                    format!("A request may be at most {MAX_LINE_BYTES} bytes."),
                ),
            );
            return;
        }
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => handler(request),
            Err(error) => Response::err("bad_request", error.to_string()),
        };
        if write_response(&mut writer, &response).is_err() {
            return;
        }
    }
}

fn write_response(writer: &mut UnixStream, response: &Response) -> std::io::Result<()> {
    let mut body = serde_json::to_string(response).unwrap_or_else(|_| {
        r#"{"status":"err","body":{"code":"io","message":"the response could not be serialised"}}"#
            .to_string()
    });
    body.push('\n');
    writer.write_all(body.as_bytes())?;
    writer.flush()
}

#[derive(Debug)]
pub enum ClientError {
    /// No socket, or nothing listening on it.
    NotRunning,
    /// The app answered with an error response.
    Refused { code: String, message: String },
    /// The connection broke, or the answer was not a Response.
    Transport(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::NotRunning => write!(
                f,
                "Ambient Context is not running. Open the app and try again; \
                 reading days, summaries, rules and settings works either way."
            ),
            ClientError::Refused { code, message } => write!(f, "{message} ({code})"),
            ClientError::Transport(detail) => write!(f, "The control socket failed: {detail}"),
        }
    }
}

/// One request, one response, one connection. Opening a connection per call
/// costs microseconds on a local socket and removes every question about a
/// stale handle after the app restarts.
pub fn request(socket: &Path, request: &Request) -> Result<serde_json::Value, ClientError> {
    let stream = UnixStream::connect(socket).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
            ClientError::NotRunning
        }
        _ => ClientError::Transport(error.to_string()),
    })?;
    let clone = stream
        .try_clone()
        .map_err(|e| ClientError::Transport(e.to_string()))?;
    let mut writer = stream;
    let mut line =
        serde_json::to_string(request).map_err(|e| ClientError::Transport(e.to_string()))?;
    line.push('\n');
    writer
        .write_all(line.as_bytes())
        .map_err(|e| ClientError::Transport(e.to_string()))?;
    writer
        .flush()
        .map_err(|e| ClientError::Transport(e.to_string()))?;

    let mut answer = String::new();
    BufReader::new(clone)
        .read_line(&mut answer)
        .map_err(|e| ClientError::Transport(e.to_string()))?;
    if answer.trim().is_empty() {
        return Err(ClientError::Transport(
            "the app closed the connection".into(),
        ));
    }
    match serde_json::from_str::<Response>(&answer) {
        Ok(Response::Ok(value)) => Ok(value),
        Ok(Response::Err { code, message }) => Err(ClientError::Refused { code, message }),
        Err(error) => Err(ClientError::Transport(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn a_unit_request_is_a_bare_op() {
        let json = serde_json::to_string(&Request::CaptureStatus).unwrap();
        assert_eq!(json, r#"{"op":"capture_status"}"#);
    }

    #[test]
    fn a_struct_request_carries_its_fields_beside_the_op() {
        let json = serde_json::to_string(&Request::SummariseDay {
            date: "2026-08-30".to_string(),
            client: "Claude Code".to_string(),
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"op":"summarise_day","date":"2026-08-30","client":"Claude Code"}"#
        );
    }

    #[test]
    fn requests_round_trip() {
        let request = Request::SetPrompt {
            text: "# Day context\n".to_string(),
            client: "Claude Code".to_string(),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(serde_json::from_str::<Request>(&json).unwrap(), request);
    }

    #[test]
    fn an_ok_response_carries_its_value_under_body() {
        let response = Response::Ok(serde_json::json!({ "running": true }));
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(json, r#"{"status":"ok","body":{"running":true}}"#);
    }

    #[test]
    fn an_ok_response_may_carry_a_bare_array() {
        // Adjacent tagging is chosen precisely so a non-object body works.
        let response = Response::Ok(serde_json::json!(["a", "b"]));
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(json, r#"{"status":"ok","body":["a","b"]}"#);
    }

    #[test]
    fn an_error_response_carries_a_code_and_a_message() {
        let response = Response::Err {
            code: "locked".to_string(),
            message: "Built-in protections cannot be changed.".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(
            json,
            r#"{"status":"err","body":{"code":"locked","message":"Built-in protections cannot be changed."}}"#
        );
    }

    #[test]
    fn an_unknown_op_fails_to_deserialise_rather_than_defaulting() {
        assert!(serde_json::from_str::<Request>(r#"{"op":"delete_everything"}"#).is_err());
    }

    #[test]
    fn the_socket_sits_in_the_app_data_dir() {
        let path = socket_path(Path::new(
            "/Users/x/Library/Application Support/com.0x0000007a.ambientcontext",
        ));
        assert!(path.ends_with("control.sock"));
    }

    #[test]
    fn the_socket_path_fits_in_sun_path() {
        // macOS sockaddr_un.sun_path is 104 bytes including the terminator.
        // A path over that fails at bind with a message that explains nothing,
        // so assert the real path is comfortably inside it.
        let path = socket_path(Path::new(
            "/Users/averylongusername/Library/Application Support/com.0x0000007a.ambientcontext",
        ));
        assert!(
            path.as_os_str().len() < 104,
            "{} bytes",
            path.as_os_str().len()
        );
    }

    use std::os::unix::fs::PermissionsExt;

    fn echo_server(dir: &Path) -> std::path::PathBuf {
        let socket = socket_path(dir);
        let listener = bind(&socket).unwrap();
        std::thread::spawn(move || {
            serve(listener, |request| match request {
                Request::CaptureStatus => Response::Ok(serde_json::json!({ "running": true })),
                Request::RemoveRule { id, .. } => {
                    Response::err("locked", format!("{id} is a built-in protection."))
                }
                other => Response::Ok(serde_json::json!({ "echoed": format!("{other:?}") })),
            });
        });
        socket
    }

    #[test]
    fn the_socket_is_created_with_mode_0600() {
        let dir = tempfile::tempdir().unwrap();
        let socket = echo_server(dir.path());
        let mode = std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "socket mode was {mode:o}");
    }

    #[test]
    fn a_request_gets_its_response_back() {
        let dir = tempfile::tempdir().unwrap();
        let socket = echo_server(dir.path());
        let value = request(&socket, &Request::CaptureStatus).unwrap();
        assert_eq!(value["running"], serde_json::json!(true));
    }

    #[test]
    fn an_error_response_becomes_a_refused_client_error() {
        let dir = tempfile::tempdir().unwrap();
        let socket = echo_server(dir.path());
        let error = request(
            &socket,
            &Request::RemoveRule {
                id: "password-managers".into(),
                client: "test".into(),
            },
        )
        .unwrap_err();
        match error {
            ClientError::Refused { code, message } => {
                assert_eq!(code, "locked");
                assert!(message.contains("built-in"));
            }
            other => panic!("expected Refused, got {other:?}"),
        }
    }

    #[test]
    fn an_absent_socket_is_not_running_rather_than_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let error = request(&socket_path(dir.path()), &Request::CaptureStatus).unwrap_err();
        assert!(matches!(error, ClientError::NotRunning));
    }

    #[test]
    fn a_stale_socket_file_from_a_crash_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let socket = socket_path(dir.path());
        // What a crashed process leaves behind: the file exists, nothing listens.
        std::fs::write(&socket, b"").unwrap();
        let listener = bind(&socket).unwrap();
        drop(listener);
        let socket = echo_server(dir.path());
        assert_eq!(
            request(&socket, &Request::CaptureStatus).unwrap()["running"],
            serde_json::json!(true)
        );
    }

    #[test]
    fn a_live_socket_is_not_stolen_from_the_process_holding_it() {
        let dir = tempfile::tempdir().unwrap();
        let _socket = echo_server(dir.path());
        let error = bind(&socket_path(dir.path())).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
    }

    #[test]
    fn a_request_over_the_line_cap_is_refused_rather_than_read_whole() {
        use std::io::{BufRead, BufReader, Write};
        let dir = tempfile::tempdir().unwrap();
        let socket = echo_server(dir.path());
        let stream = std::os::unix::net::UnixStream::connect(&socket).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = stream;
        let huge = vec![b'x'; (MAX_LINE_BYTES + 16) as usize];
        // A client that never sends a newline must not be able to grow the
        // server's memory a byte at a time.
        let _ = writer.write_all(&huge);
        let mut answer = String::new();
        reader.read_line(&mut answer).unwrap();
        assert!(answer.contains("bad_request"), "{answer}");
    }

    #[test]
    fn a_malformed_line_is_answered_rather_than_killing_the_connection() {
        use std::io::{BufRead, BufReader, Write};
        let dir = tempfile::tempdir().unwrap();
        let socket = echo_server(dir.path());
        let stream = std::os::unix::net::UnixStream::connect(&socket).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = stream;
        writer.write_all(b"{ not json\n").unwrap();
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        assert!(first.contains("bad_request"), "{first}");
        // The same connection still works afterwards.
        writer.write_all(b"{\"op\":\"capture_status\"}\n").unwrap();
        let mut second = String::new();
        reader.read_line(&mut second).unwrap();
        assert!(second.contains("\"running\":true"), "{second}");
    }
}
