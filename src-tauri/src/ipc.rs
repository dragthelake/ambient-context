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
    StartCapture { client: String },
    StopCapture { client: String },
    SummariseDay { date: String, client: String },
    JobStatus { id: String },
    SetConfig { patch: serde_json::Value, client: String },
    AddRule { rule: crate::rules::Rule, client: String },
    UpdateRule { rule: crate::rules::Rule, client: String },
    RemoveRule { id: String, client: String },
    SetPrompt { text: String, client: String },
    OpenDay { date: String },
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
        assert!(path.as_os_str().len() < 104, "{} bytes", path.as_os_str().len());
    }
}
