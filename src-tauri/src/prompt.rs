use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptId {
    DayContext,
    IngestMessages,
    IngestApps,
    IngestWebsites,
}

/// The headings summary validation depends on. A prompt that stops asking
/// for one of these produces summaries the validator rejects, so the
/// prompt is refused instead.
pub const REQUIRED_HEADINGS: &[&str] = &[
    "## Sessions",
    "## Work and outcomes",
    "## Reading and research",
    "## Open loops",
    "## Worth remembering",
    "## Key references",
    "## Reasoning",
];

impl PromptId {
    pub fn all() -> [PromptId; 4] {
        [
            PromptId::DayContext,
            PromptId::IngestMessages,
            PromptId::IngestApps,
            PromptId::IngestWebsites,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PromptId::DayContext => "day-context",
            PromptId::IngestMessages => "ingest-messages",
            PromptId::IngestApps => "ingest-apps",
            PromptId::IngestWebsites => "ingest-websites",
        }
    }

    pub fn parse(name: &str) -> Option<PromptId> {
        PromptId::all().into_iter().find(|id| id.as_str() == name)
    }

    pub fn bundled(self) -> &'static str {
        match self {
            PromptId::DayContext => include_str!("../prompts/day-context.md"),
            PromptId::IngestMessages => include_str!("../prompts/ingest-messages.md"),
            PromptId::IngestApps => include_str!("../prompts/ingest-apps.md"),
            PromptId::IngestWebsites => include_str!("../prompts/ingest-websites.md"),
        }
    }

    pub fn placeholders(self) -> &'static [&'static str] {
        match self {
            PromptId::DayContext => &["{{DATE}}", "{{TIMELINE}}", "{{KB}}"],
            _ => &["{{DATE}}", "{{INPUT}}", "{{TIMELINE}}"],
        }
    }

    /// The file markers an ingest prompt must ask for, which are the files
    /// its call writes. Empty for the summary prompt.
    pub fn markers(self) -> &'static [&'static str] {
        match self {
            PromptId::DayContext => &[],
            PromptId::IngestMessages => &["<<<file: people.md>>>", "<<<file: commitments.md>>>"],
            PromptId::IngestApps => &[
                "<<<file: threads.md>>>",
                "<<<file: products.md>>>",
                "<<<file: issues.md>>>",
            ],
            PromptId::IngestWebsites => &["<<<file: reading.md>>>"],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptError {
    Empty,
    MissingHeading(String),
    MissingPlaceholder(String),
    MissingMarker(String),
    Io(String),
}

impl std::fmt::Display for PromptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptError::Empty => write!(f, "the prompt is empty"),
            PromptError::MissingHeading(heading) => write!(
                f,
                "the prompt no longer asks for {heading}, which summary validation requires"
            ),
            PromptError::MissingPlaceholder(placeholder) => write!(
                f,
                "the prompt no longer contains {placeholder}, which the app fills in"
            ),
            PromptError::MissingMarker(marker) => write!(
                f,
                "the prompt no longer asks for {marker}, which its ingest call writes"
            ),
            PromptError::Io(message) => write!(f, "{message}"),
        }
    }
}

pub fn prompt_path(config_dir: &Path, id: PromptId) -> PathBuf {
    config_dir
        .join("prompts")
        .join(format!("{}.md", id.as_str()))
}

pub fn is_customised(config_dir: &Path, id: PromptId) -> bool {
    std::fs::read_to_string(prompt_path(config_dir, id))
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false)
}

pub fn current(config_dir: &Path, id: PromptId) -> String {
    match std::fs::read_to_string(prompt_path(config_dir, id)) {
        Ok(text) if !text.trim().is_empty() => text,
        _ => id.bundled().to_string(),
    }
}

pub fn validate(id: PromptId, text: &str) -> Result<(), PromptError> {
    if text.trim().is_empty() {
        return Err(PromptError::Empty);
    }
    if id == PromptId::DayContext {
        for heading in REQUIRED_HEADINGS {
            if !text.contains(heading) {
                return Err(PromptError::MissingHeading((*heading).to_string()));
            }
        }
    }
    for placeholder in id.placeholders() {
        if !text.contains(placeholder) {
            return Err(PromptError::MissingPlaceholder((*placeholder).to_string()));
        }
    }
    for marker in id.markers() {
        if !text.contains(marker) {
            return Err(PromptError::MissingMarker((*marker).to_string()));
        }
    }
    Ok(())
}

/// Validates, then writes. An invalid prompt never reaches the file, so a
/// bad edit from any surface cannot break tomorrow's scheduled run.
pub fn set(config_dir: &Path, id: PromptId, text: &str) -> Result<(), PromptError> {
    validate(id, text)?;
    let path = prompt_path(config_dir, id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| PromptError::Io(format!("could not create {parent:?}: {e}")))?;
    }
    std::fs::write(&path, text)
        .map_err(|e| PromptError::Io(format!("could not write {path:?}: {e}")))
}

/// Removes the customised copy so the bundled prompt is used again. An
/// absent file is already reset.
pub fn reset(config_dir: &Path, id: PromptId) -> std::io::Result<()> {
    match std::fs::remove_file(prompt_path(config_dir, id)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn every_bundled_prompt_passes_its_own_validation() {
        for id in PromptId::all() {
            validate(id, id.bundled()).unwrap_or_else(|e| panic!("{}: {e}", id.as_str()));
        }
    }

    #[test]
    fn the_summary_prompt_needs_its_placeholders() {
        let text = PromptId::DayContext
            .bundled()
            .replace("{{KB}}", "{{DAY_FILE}}");
        assert_eq!(
            validate(PromptId::DayContext, &text).unwrap_err(),
            PromptError::MissingPlaceholder("{{KB}}".into())
        );
    }

    #[test]
    fn an_ingest_prompt_needs_its_file_markers() {
        let text = PromptId::IngestApps
            .bundled()
            .replace("<<<file: issues.md>>>", "<<<file: problems.md>>>");
        assert_eq!(
            validate(PromptId::IngestApps, &text).unwrap_err(),
            PromptError::MissingMarker("<<<file: issues.md>>>".into())
        );
    }

    #[test]
    fn prompts_are_customised_independently() {
        let dir = tempdir().unwrap();
        let mine = format!("{}\n\nExtra.\n", PromptId::IngestMessages.bundled());
        set(dir.path(), PromptId::IngestMessages, &mine).unwrap();
        assert!(is_customised(dir.path(), PromptId::IngestMessages));
        assert!(!is_customised(dir.path(), PromptId::DayContext));
        assert_eq!(
            prompt_path(dir.path(), PromptId::IngestMessages),
            dir.path().join("prompts").join("ingest-messages.md")
        );
        reset(dir.path(), PromptId::IngestMessages).unwrap();
        assert_eq!(
            current(dir.path(), PromptId::IngestMessages),
            PromptId::IngestMessages.bundled()
        );
    }

    #[test]
    fn ids_round_trip_through_strings() {
        for id in PromptId::all() {
            assert_eq!(PromptId::parse(id.as_str()), Some(id));
        }
        assert_eq!(PromptId::parse("nope"), None);
    }

    #[test]
    fn an_empty_prompt_is_rejected() {
        assert_eq!(
            validate(PromptId::DayContext, "   \n  ").unwrap_err(),
            PromptError::Empty
        );
    }

    #[test]
    fn a_prompt_missing_a_required_heading_names_it() {
        let text = PromptId::DayContext
            .bundled()
            .replace("## Open loops", "## Loose ends");
        assert_eq!(
            validate(PromptId::DayContext, &text).unwrap_err(),
            PromptError::MissingHeading("## Open loops".to_string())
        );
    }

    #[test]
    fn a_prompt_missing_the_reasoning_section_is_rejected() {
        let text = PromptId::DayContext
            .bundled()
            .replace("## Reasoning", "## Notes");
        assert_eq!(
            validate(PromptId::DayContext, &text).unwrap_err(),
            PromptError::MissingHeading("## Reasoning".to_string())
        );
    }

    #[test]
    fn current_is_the_bundled_prompt_until_it_is_customised() {
        let dir = tempdir().unwrap();
        assert!(!is_customised(dir.path(), PromptId::DayContext));
        assert_eq!(
            current(dir.path(), PromptId::DayContext),
            PromptId::DayContext.bundled()
        );
    }

    #[test]
    fn set_writes_the_customised_copy_and_current_returns_it() {
        let dir = tempdir().unwrap();
        let bundled = PromptId::DayContext.bundled();
        let mine = format!("{bundled}\n\nAlways write in Australian English.\n");
        set(dir.path(), PromptId::DayContext, &mine).unwrap();
        assert!(is_customised(dir.path(), PromptId::DayContext));
        assert_eq!(current(dir.path(), PromptId::DayContext), mine);
        assert_eq!(
            prompt_path(dir.path(), PromptId::DayContext),
            dir.path().join("prompts").join("day-context.md")
        );
    }

    #[test]
    fn set_refuses_an_invalid_prompt_and_leaves_the_file_alone() {
        let dir = tempdir().unwrap();
        set(
            dir.path(),
            PromptId::DayContext,
            PromptId::DayContext.bundled(),
        )
        .unwrap();
        assert!(set(dir.path(), PromptId::DayContext, "just some words").is_err());
        assert_eq!(
            current(dir.path(), PromptId::DayContext),
            PromptId::DayContext.bundled()
        );
    }

    #[test]
    fn reset_removes_the_customised_copy_and_is_safe_to_repeat() {
        let dir = tempdir().unwrap();
        set(
            dir.path(),
            PromptId::DayContext,
            &format!("{}\n\nExtra.\n", PromptId::DayContext.bundled()),
        )
        .unwrap();
        reset(dir.path(), PromptId::DayContext).unwrap();
        assert!(!is_customised(dir.path(), PromptId::DayContext));
        assert_eq!(
            current(dir.path(), PromptId::DayContext),
            PromptId::DayContext.bundled()
        );
        reset(dir.path(), PromptId::DayContext).unwrap();
    }

    #[test]
    fn an_unreadable_customised_copy_falls_back_to_the_bundled_prompt() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("prompts")).unwrap();
        std::fs::write(prompt_path(dir.path(), PromptId::DayContext), "").unwrap();
        assert_eq!(
            current(dir.path(), PromptId::DayContext),
            PromptId::DayContext.bundled()
        );
    }
}
