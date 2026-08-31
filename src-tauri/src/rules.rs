use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What a rule points at. The pattern in every variant is matched
/// case-insensitively as a substring, except `Website`, which is matched
/// against the host of the block's `url:` reference by exact host or by
/// dotted suffix, so `example.com` covers `app.example.com` and not
/// `notexample.com`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    App(String),
    Website(String),
    Title(String),
}

impl Target {
    pub fn pattern(&self) -> &str {
        match self {
            Target::App(p) | Target::Website(p) | Target::Title(p) => p,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Exclude,
    HeadingsOnly,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub target: Target,
    pub action: Action,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A protection the app enforces regardless of the rules file. Rendered in
/// the rules list so a user can see what is already refused, never stored
/// in `rules.json`, and refused by every write path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuiltIn {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    Duplicate(String),
    NotFound(String),
    Locked(String),
    Invalid(String),
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleError::Duplicate(id) => {
                write!(f, "a rule with that target or id already exists: {id}")
            }
            RuleError::NotFound(id) => write!(f, "no rule with id {id}"),
            RuleError::Locked(id) => {
                write!(f, "{id} is a built-in protection and cannot be changed")
            }
            RuleError::Invalid(why) => write!(f, "{why}"),
        }
    }
}

pub fn rules_path(config_dir: &Path) -> PathBuf {
    config_dir.join("rules.json")
}

/// A missing or unreadable file is an empty rule set. Rules only ever
/// narrow what is captured, so failing open here loses a protection the
/// user asked for; that is why `save` writes atomically and why the
/// settings page shows the parse failure rather than hiding it.
pub fn load(config_dir: &Path) -> Rules {
    std::fs::read_to_string(rules_path(config_dir))
        .ok()
        .and_then(|raw| parse(&raw).ok())
        .unwrap_or_default()
}

pub fn parse(json: &str) -> Result<Rules, RuleError> {
    let parsed: Rules = serde_json::from_str(json)
        .map_err(|e| RuleError::Invalid(format!("not a rules file: {e}")))?;
    validate(&parsed)?;
    Ok(parsed)
}

pub fn save(config_dir: &Path, rules: &Rules) -> std::io::Result<()> {
    std::fs::create_dir_all(config_dir)?;
    let json = serde_json::to_string_pretty(rules)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let temp = rules_path(config_dir).with_extension("json.tmp");
    std::fs::write(&temp, json)?;
    std::fs::rename(temp, rules_path(config_dir))
}

pub fn built_ins() -> Vec<BuiltIn> {
    vec![
        BuiltIn {
            id: "builtin:password-managers".to_string(),
            description: format!(
                "Nothing is read from password managers: {}.",
                crate::redact::EXCLUDED_APPS.join(", ")
            ),
        },
        BuiltIn {
            id: "builtin:private-windows".to_string(),
            description: format!(
                "A private browsing window is dropped whole, recognised by the title markers: {}.",
                crate::redact::PRIVATE_WINDOW_MARKERS.join(", ")
            ),
        },
        BuiltIn {
            id: "builtin:secure-fields".to_string(),
            description:
                "Secure text fields are never read, refused in the accessibility walk itself."
                    .to_string(),
        },
        BuiltIn {
            id: "builtin:secret-patterns".to_string(),
            description:
                "Keys, tokens, bearer headers, labelled secrets and card-shaped numbers are replaced with [redacted]."
                    .to_string(),
        },
    ]
}

fn is_built_in_id(id: &str) -> bool {
    id.starts_with("builtin:")
}

/// The lowest `rN` not already taken, so ids stay short and readable in the
/// file a user may open in an editor.
pub fn new_id(rules: &Rules) -> String {
    for n in 1.. {
        let candidate = format!("r{n}");
        if !rules.rules.iter().any(|r| r.id == candidate) {
            return candidate;
        }
    }
    unreachable!()
}

fn check_rule(rule: &Rule) -> Result<(), RuleError> {
    if rule.id.trim().is_empty() {
        return Err(RuleError::Invalid("a rule needs an id".to_string()));
    }
    if is_built_in_id(&rule.id) {
        return Err(RuleError::Locked(rule.id.clone()));
    }
    if !rule
        .id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(RuleError::Invalid(format!(
            "rule id {} may only contain letters, digits, hyphen and underscore",
            rule.id
        )));
    }
    let pattern = rule.target.pattern().trim();
    if pattern.is_empty() {
        return Err(RuleError::Invalid(
            "a rule needs something to match".to_string(),
        ));
    }
    if let Target::Website(domain) = &rule.target {
        if domain.contains("://") || domain.contains('/') || domain.contains(' ') {
            return Err(RuleError::Invalid(format!(
                "{domain} is not a bare domain: write example.com, not a full address"
            )));
        }
    }
    // A rule may only make an already-protected application stricter.
    if let Target::App(name) = &rule.target {
        if crate::redact::is_excluded_app(name) && rule.action != Action::Exclude {
            return Err(RuleError::Locked(format!(
                "{name} is a built-in protection and is always excluded"
            )));
        }
    }
    Ok(())
}

pub fn validate(rules: &Rules) -> Result<(), RuleError> {
    for (index, rule) in rules.rules.iter().enumerate() {
        check_rule(rule)?;
        for earlier in &rules.rules[..index] {
            if earlier.id == rule.id {
                return Err(RuleError::Duplicate(rule.id.clone()));
            }
            if same_target(&earlier.target, &rule.target) {
                return Err(RuleError::Duplicate(earlier.id.clone()));
            }
        }
    }
    Ok(())
}

fn same_target(a: &Target, b: &Target) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
        && a.pattern().trim().eq_ignore_ascii_case(b.pattern().trim())
}

impl Rules {
    pub fn add(&mut self, rule: Rule) -> Result<(), RuleError> {
        check_rule(&rule)?;
        for existing in &self.rules {
            if existing.id == rule.id {
                return Err(RuleError::Duplicate(rule.id.clone()));
            }
            if same_target(&existing.target, &rule.target) {
                return Err(RuleError::Duplicate(existing.id.clone()));
            }
        }
        self.rules.push(rule);
        Ok(())
    }

    pub fn update(&mut self, rule: Rule) -> Result<(), RuleError> {
        check_rule(&rule)?;
        let index = self
            .rules
            .iter()
            .position(|r| r.id == rule.id)
            .ok_or_else(|| RuleError::NotFound(rule.id.clone()))?;
        for (other, existing) in self.rules.iter().enumerate() {
            if other != index && same_target(&existing.target, &rule.target) {
                return Err(RuleError::Duplicate(existing.id.clone()));
            }
        }
        self.rules[index] = rule;
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Result<(), RuleError> {
        if is_built_in_id(id) {
            return Err(RuleError::Locked(id.to_string()));
        }
        let index = self
            .rules
            .iter()
            .position(|r| r.id == id)
            .ok_or_else(|| RuleError::NotFound(id.to_string()))?;
        self.rules.remove(index);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn rule(id: &str, target: Target, action: Action) -> Rule {
        Rule {
            id: id.to_string(),
            target,
            action,
            note: None,
        }
    }

    #[test]
    fn parses_the_documented_file_shape() {
        let json = r#"{
          "rules": [
            { "id": "r1", "target": { "app": "Slack" }, "action": "exclude" },
            { "id": "r2", "target": { "website": "news.ycombinator.com" }, "action": "headings_only", "note": "too noisy" },
            { "id": "r3", "target": { "title": "Payroll" }, "action": "full" }
          ]
        }"#;
        let parsed = parse(json).unwrap();
        assert_eq!(parsed.rules.len(), 3);
        assert_eq!(parsed.rules[0].target, Target::App("Slack".to_string()));
        assert_eq!(parsed.rules[1].action, Action::HeadingsOnly);
        assert_eq!(parsed.rules[2].target, Target::Title("Payroll".to_string()));
        assert_eq!(parsed.rules[1].note.as_deref(), Some("too noisy"));
    }

    #[test]
    fn round_trips_through_the_file() {
        let dir = tempdir().unwrap();
        let mut set = Rules::default();
        set.add(rule("r1", Target::App("Slack".to_string()), Action::Exclude))
            .unwrap();
        save(dir.path(), &set).unwrap();
        assert_eq!(load(dir.path()), set);
    }

    #[test]
    fn a_missing_file_is_an_empty_rule_set() {
        let dir = tempdir().unwrap();
        assert_eq!(load(dir.path()), Rules::default());
    }

    #[test]
    fn a_corrupt_file_is_an_empty_rule_set_rather_than_a_panic() {
        let dir = tempdir().unwrap();
        std::fs::write(rules_path(dir.path()), "{ not json").unwrap();
        assert_eq!(load(dir.path()), Rules::default());
    }

    #[test]
    fn rejects_a_duplicate_id() {
        let mut set = Rules::default();
        set.add(rule("r1", Target::App("Slack".to_string()), Action::Exclude))
            .unwrap();
        let err = set
            .add(rule("r1", Target::App("Linear".to_string()), Action::Exclude))
            .unwrap_err();
        assert_eq!(err, RuleError::Duplicate("r1".to_string()));
    }

    #[test]
    fn rejects_a_second_rule_for_the_same_target() {
        let mut set = Rules::default();
        set.add(rule("r1", Target::App("Slack".to_string()), Action::Exclude))
            .unwrap();
        let err = set
            .add(rule("r2", Target::App("slack".to_string()), Action::Full))
            .unwrap_err();
        assert!(matches!(err, RuleError::Duplicate(_)));
    }

    #[test]
    fn rejects_an_empty_pattern() {
        let mut set = Rules::default();
        let err = set
            .add(rule("r1", Target::App("   ".to_string()), Action::Exclude))
            .unwrap_err();
        assert!(matches!(err, RuleError::Invalid(_)));
    }

    #[test]
    fn rejects_a_website_pattern_that_is_a_url_rather_than_a_domain() {
        let mut set = Rules::default();
        let err = set
            .add(rule(
                "r1",
                Target::Website("https://example.com/x".to_string()),
                Action::Exclude,
            ))
            .unwrap_err();
        assert!(matches!(err, RuleError::Invalid(_)));
    }

    #[test]
    fn refuses_to_weaken_a_locked_application_protection() {
        let mut set = Rules::default();
        let err = set
            .add(rule("r1", Target::App("1Password".to_string()), Action::Full))
            .unwrap_err();
        assert!(matches!(err, RuleError::Locked(_)));
    }

    #[test]
    fn allows_a_stricter_rule_on_an_already_locked_application() {
        let mut set = Rules::default();
        set.add(rule("r1", Target::App("1Password".to_string()), Action::Exclude))
            .unwrap();
    }

    #[test]
    fn update_replaces_in_place_and_remove_takes_it_out() {
        let mut set = Rules::default();
        set.add(rule("r1", Target::App("Slack".to_string()), Action::Exclude))
            .unwrap();
        set.update(rule(
            "r1",
            Target::App("Slack".to_string()),
            Action::HeadingsOnly,
        ))
        .unwrap();
        assert_eq!(set.rules[0].action, Action::HeadingsOnly);
        set.remove("r1").unwrap();
        assert!(set.rules.is_empty());
        assert_eq!(
            set.remove("r1").unwrap_err(),
            RuleError::NotFound("r1".to_string())
        );
    }

    #[test]
    fn a_built_in_id_can_never_be_added_updated_or_removed() {
        let locked = built_ins()[0].id.clone();
        let mut set = Rules::default();
        assert!(matches!(
            set.add(rule(
                &locked,
                Target::App("Slack".to_string()),
                Action::Full
            )),
            Err(RuleError::Locked(_))
        ));
        assert!(matches!(set.remove(&locked), Err(RuleError::Locked(_))));
    }

    #[test]
    fn the_built_ins_name_all_four_protections() {
        let ids: Vec<String> = built_ins().into_iter().map(|b| b.id).collect();
        assert!(ids.contains(&"builtin:password-managers".to_string()));
        assert!(ids.contains(&"builtin:private-windows".to_string()));
        assert!(ids.contains(&"builtin:secure-fields".to_string()));
        assert!(ids.contains(&"builtin:secret-patterns".to_string()));
    }

    #[test]
    fn new_id_takes_the_lowest_unused_slot() {
        let mut set = Rules::default();
        assert_eq!(new_id(&set), "r1");
        set.add(rule("r1", Target::App("Slack".to_string()), Action::Exclude))
            .unwrap();
        assert_eq!(new_id(&set), "r2");
    }
}
