pub mod macos;
pub mod windows;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    pub app: String,
    pub window_title: Option<String>,
    /// Path of the window's backing file, where the app exposes one.
    pub document: Option<String>,
    /// Page URL of the window's main web area, for browsers.
    pub url: Option<String>,
    pub text: Vec<String>,
    /// Set by redaction when a rule says record the heading and drop the
    /// body. Carried this far because only the writer can act on it.
    pub headings_only: bool,
}

pub trait WindowReader {
    fn snapshot(&self) -> Option<Snapshot>;
}

pub struct PlatformReader;

impl WindowReader for PlatformReader {
    fn snapshot(&self) -> Option<Snapshot> {
        #[cfg(target_os = "macos")]
        {
            macos::snapshot()
        }
        #[cfg(target_os = "windows")]
        {
            windows::snapshot()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    NotGranted,
    Granted,
}

/// Swift returns a bare `Int32` across the C boundary because `swift!`
/// declarations have no Result-typed return. This turns the code into a
/// type. Pure, so it is unit tested directly rather than through Swift.
pub fn permission_from_code(code: i32) -> Permission {
    match code {
        1 => Permission::Granted,
        _ => Permission::NotGranted,
    }
}

#[derive(Deserialize)]
struct RawSnapshot {
    app: String,
    window_title: Option<String>,
    #[serde(default)]
    document: Option<String>,
    #[serde(default)]
    url: Option<String>,
    text: Vec<String>,
}

/// Swift signals failure by returning a string prefixed `"ERROR: "` rather
/// than throwing across the C boundary. Pure, so it is unit tested directly.
pub fn parse_snapshot_json(raw: &str) -> Result<Snapshot, String> {
    if let Some(message) = raw.strip_prefix("ERROR: ") {
        return Err(message.to_string());
    }
    let parsed: RawSnapshot =
        serde_json::from_str(raw).map_err(|e| format!("malformed snapshot: {e}"))?;
    Ok(Snapshot {
        app: parsed.app,
        window_title: parsed.window_title,
        document: parsed.document,
        url: parsed.url,
        text: parsed.text,
        // Snapshots arrive from the accessibility walk with no rule applied.
        headings_only: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_one_to_granted() {
        assert_eq!(permission_from_code(1), Permission::Granted);
    }

    #[test]
    fn maps_zero_to_not_granted() {
        assert_eq!(permission_from_code(0), Permission::NotGranted);
    }

    #[test]
    fn maps_unexpected_codes_to_not_granted() {
        assert_eq!(permission_from_code(99), Permission::NotGranted);
        assert_eq!(permission_from_code(-1), Permission::NotGranted);
    }

    #[test]
    fn parses_a_full_snapshot() {
        let raw = r#"{"app":"Linear","window_title":"YN-102","text":["a","b"]}"#;
        let snap = parse_snapshot_json(raw).unwrap();
        assert_eq!(snap.app, "Linear");
        assert_eq!(snap.window_title.as_deref(), Some("YN-102"));
        assert_eq!(snap.text, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn parses_a_snapshot_with_no_window_title() {
        let raw = r#"{"app":"Finder","window_title":null,"text":[]}"#;
        let snap = parse_snapshot_json(raw).unwrap();
        assert_eq!(snap.window_title, None);
        assert!(snap.text.is_empty());
    }

    #[test]
    fn turns_the_error_prefix_into_err() {
        let err = parse_snapshot_json("ERROR: permission not granted").unwrap_err();
        assert_eq!(err, "permission not granted");
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(parse_snapshot_json("{not json").is_err());
    }
}
