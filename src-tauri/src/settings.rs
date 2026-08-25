use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub folder: Option<PathBuf>,
    pub interval_secs: u64,
    pub min_dwell_secs: i64,
    pub similarity_threshold: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            folder: None,
            interval_secs: 5,
            min_dwell_secs: 30,
            similarity_threshold: 0.5,
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

fn settings_path<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    app.path()
        .app_config_dir()
        .expect("app config dir")
        .join("settings.json")
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
        assert_eq!(settings.min_dwell_secs, 30);
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
        assert_eq!(settings.min_dwell_secs, 30);
    }
}
