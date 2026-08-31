use std::path::{Path, PathBuf};

/// The prompt shipped in the binary. Updates replace this and never touch
/// a customised copy.
pub const BUNDLED: &str = include_str!("../prompts/day-context.md");

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptError {
    Empty,
    MissingHeading(String),
}

impl std::fmt::Display for PromptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptError::Empty => write!(f, "the prompt is empty"),
            PromptError::MissingHeading(heading) => write!(
                f,
                "the prompt no longer asks for {heading}, which summary validation requires"
            ),
        }
    }
}

pub fn prompt_path(config_dir: &Path) -> PathBuf {
    config_dir.join("prompts").join("day-context.md")
}

pub fn is_customised(config_dir: &Path) -> bool {
    std::fs::read_to_string(prompt_path(config_dir))
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false)
}

pub fn current(config_dir: &Path) -> String {
    match std::fs::read_to_string(prompt_path(config_dir)) {
        Ok(text) if !text.trim().is_empty() => text,
        _ => BUNDLED.to_string(),
    }
}

pub fn validate(text: &str) -> Result<(), PromptError> {
    if text.trim().is_empty() {
        return Err(PromptError::Empty);
    }
    for heading in REQUIRED_HEADINGS {
        if !text.contains(heading) {
            return Err(PromptError::MissingHeading((*heading).to_string()));
        }
    }
    Ok(())
}

/// Validates, then writes. An invalid prompt never reaches the file, so a
/// bad edit from any surface cannot break tomorrow's scheduled run.
pub fn set(config_dir: &Path, text: &str) -> Result<(), PromptError> {
    validate(text)?;
    let path = prompt_path(config_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            PromptError::MissingHeading(format!("could not create {parent:?}: {e}"))
        })?;
    }
    std::fs::write(&path, text)
        .map_err(|e| PromptError::MissingHeading(format!("could not write {path:?}: {e}")))
}

/// Removes the customised copy so the bundled prompt is used again. An
/// absent file is already reset.
pub fn reset(config_dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(prompt_path(config_dir)) {
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
    fn the_bundled_prompt_passes_its_own_validation() {
        validate(BUNDLED).unwrap();
    }

    #[test]
    fn an_empty_prompt_is_rejected() {
        assert_eq!(validate("   \n  ").unwrap_err(), PromptError::Empty);
    }

    #[test]
    fn a_prompt_missing_a_required_heading_names_it() {
        let text = BUNDLED.replace("## Open loops", "## Loose ends");
        assert_eq!(
            validate(&text).unwrap_err(),
            PromptError::MissingHeading("## Open loops".to_string())
        );
    }

    #[test]
    fn a_prompt_missing_the_reasoning_section_is_rejected() {
        let text = BUNDLED.replace("## Reasoning", "## Notes");
        assert_eq!(
            validate(&text).unwrap_err(),
            PromptError::MissingHeading("## Reasoning".to_string())
        );
    }

    #[test]
    fn current_is_the_bundled_prompt_until_it_is_customised() {
        let dir = tempdir().unwrap();
        assert!(!is_customised(dir.path()));
        assert_eq!(current(dir.path()), BUNDLED);
    }

    #[test]
    fn set_writes_the_customised_copy_and_current_returns_it() {
        let dir = tempdir().unwrap();
        let mine = format!("{BUNDLED}\n\nAlways write in Australian English.\n");
        set(dir.path(), &mine).unwrap();
        assert!(is_customised(dir.path()));
        assert_eq!(current(dir.path()), mine);
        assert_eq!(
            prompt_path(dir.path()),
            dir.path().join("prompts").join("day-context.md")
        );
    }

    #[test]
    fn set_refuses_an_invalid_prompt_and_leaves_the_file_alone() {
        let dir = tempdir().unwrap();
        set(dir.path(), BUNDLED).unwrap();
        assert!(set(dir.path(), "just some words").is_err());
        assert_eq!(current(dir.path()), BUNDLED);
    }

    #[test]
    fn reset_removes_the_customised_copy_and_is_safe_to_repeat() {
        let dir = tempdir().unwrap();
        set(dir.path(), &format!("{BUNDLED}\n\nExtra.\n")).unwrap();
        reset(dir.path()).unwrap();
        assert!(!is_customised(dir.path()));
        assert_eq!(current(dir.path()), BUNDLED);
        reset(dir.path()).unwrap();
    }

    #[test]
    fn an_unreadable_customised_copy_falls_back_to_the_bundled_prompt() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("prompts")).unwrap();
        std::fs::write(prompt_path(dir.path()), "").unwrap();
        assert_eq!(current(dir.path()), BUNDLED);
    }
}
