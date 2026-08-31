use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};

/// A connected LLM, as a command the app can run. Never a key, never a
/// hosted runtime: the user's own agent CLI, invoked one shot per job.
/// `command` is an absolute path resolved at detection time, because the
/// PATH an app inherits from the Dock is not the PATH the user has in a
/// terminal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Engine {
    pub label: String,
    pub command: String,
    pub args: Vec<String>,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub folder: Option<PathBuf>,
    /// Whether capture starts by itself once the app is set up. Turned off
    /// only by the user explicitly stopping, and it stays off across
    /// launches until they start again.
    pub enabled: bool,
    pub interval_secs: u64,
    pub min_dwell_secs: i64,
    pub similarity_threshold: f64,
    /// None means no engine connected, which is the shipped state and
    /// leaves the app a pure recorder.
    pub engine: Option<Engine>,
    /// Local time of day as "HH:MM". None means manual runs only.
    pub schedule_hhmm: Option<String>,
    /// None uses the prompt bundled with this version of the app.
    pub day_prompt: Option<PathBuf>,
    /// Absolute path to an application to open markdown with. None uses
    /// the system handler.
    pub editor: Option<String>,
    /// Whether macOS starts the app at login. On by default: a record with
    /// a hole in it where a reboot was is worth less than no record.
    pub launch_at_login: bool,
    /// The longest a single block's body can be, in characters. 0 is
    /// unlimited, which is the shipped behaviour.
    pub max_block_chars: usize,
    /// Whether `file:` and `url:` reference lines are written.
    pub write_references: bool,
    /// User redaction patterns, appended to the built-ins.
    pub extra_redaction_patterns: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            folder: None,
            enabled: true,
            interval_secs: 5,
            // Under ten seconds in a window is transit; over it is usually a
            // real interaction. Day-level dedup keeps the cost of borderline
            // blocks to a heading.
            min_dwell_secs: 10,
            similarity_threshold: 0.5,
            engine: None,
            schedule_hhmm: None,
            day_prompt: None,
            editor: None,
            launch_at_login: true,
            max_block_chars: 0,
            write_references: true,
            extra_redaction_patterns: Vec::new(),
        }
    }
}

pub fn read_from(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn write_to(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, json)
}

pub fn config_dir<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    app.path().app_config_dir().expect("app config dir")
}

fn settings_path<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    config_dir(app).join("settings.json")
}

pub fn load<R: Runtime>(app: &AppHandle<R>) -> Settings {
    read_from(&settings_path(app))
}

pub fn save<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> std::io::Result<()> {
    write_to(&settings_path(app), settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_are_five_seconds_and_no_folder() {
        let settings = Settings::default();
        assert_eq!(settings.interval_secs, 5);
        assert_eq!(settings.min_dwell_secs, 10);
        assert_eq!(settings.folder, None);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempdir().unwrap();
        let settings = read_from(&dir.path().join("nope.json"));
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn corrupt_file_yields_defaults_rather_than_panicking() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(read_from(&path), Settings::default());
    }

    #[test]
    fn round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let mut settings = Settings::default();
        settings.folder = Some(PathBuf::from("/tmp/ambient"));
        settings.interval_secs = 10;

        write_to(&path, &settings).unwrap();
        assert_eq!(read_from(&path), settings);
    }

    #[test]
    fn partial_json_fills_the_rest_from_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"interval_secs": 20}"#).unwrap();
        let settings = read_from(&path);
        assert_eq!(settings.interval_secs, 20);
        assert_eq!(settings.min_dwell_secs, 10);
    }

    #[test]
    fn engine_and_schedule_default_to_off_and_login_launch_defaults_to_on() {
        let settings = Settings::default();
        assert_eq!(settings.engine, None);
        assert_eq!(settings.schedule_hhmm, None);
        assert_eq!(settings.day_prompt, None);
        assert_eq!(settings.editor, None);
        // An app whose value is a complete record cannot depend on being
        // opened by hand after a reboot.
        assert!(settings.launch_at_login);
    }

    #[test]
    fn a_zero_one_settings_file_still_loads_and_keeps_its_values() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"folder":"/tmp/ambient","enabled":true,"interval_secs":5,"min_dwell_secs":10,"similarity_threshold":0.5}"#,
        )
        .unwrap();
        let settings = read_from(&path);
        assert_eq!(settings.folder, Some(PathBuf::from("/tmp/ambient")));
        assert_eq!(settings.interval_secs, 5);
        assert_eq!(settings.engine, None);
        assert_eq!(settings.schedule_hhmm, None);
        assert!(settings.launch_at_login);
    }

    #[test]
    fn engine_and_launch_at_login_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut settings = Settings::default();
        settings.engine = Some(Engine {
            label: "Claude Code".to_string(),
            command: "/opt/homebrew/bin/claude".to_string(),
            args: vec!["-p".to_string()],
            timeout_secs: 600,
        });
        settings.schedule_hhmm = Some("06:00".to_string());
        settings.launch_at_login = false;
        write_to(&path, &settings).unwrap();
        assert_eq!(read_from(&path), settings);
    }
}
