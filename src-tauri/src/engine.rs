//! The connected engine: the user's own agent CLI, run as a subprocess.
//!
//! Apps launched from Finder or the Dock inherit only the launchd
//! environment, which on macOS carries no `PATH` beyond the system
//! defaults, so brew, mise, volta and `~/.local/bin` installs are invisible.
//! The app therefore captures the user's login-shell environment once and
//! resolves absolute paths from it. `parse_env` is the pure half of that.

use std::collections::HashMap;

/// Parses the output of `env`: KEY=VALUE per line. Values may contain '='
/// and may be empty; keys may not. Lines without '=' are continuations of a
/// previous multi-line value and are ignored rather than guessed at.
// Consumed by login_shell_env once the engine subprocess lands (0.2.0 plan,
// Task 3); until then the spike's tests are its only caller.
#[allow(dead_code)]
pub fn parse_env(raw: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in raw.lines() {
        if let Some((key, value)) = line.split_once('=') {
            if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                out.insert(key.to_string(), value.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_key_value_lines() {
        let env = parse_env("PATH=/usr/bin:/bin\nHOME=/Users/x\n");
        assert_eq!(env.get("PATH").unwrap(), "/usr/bin:/bin");
        assert_eq!(env.get("HOME").unwrap(), "/Users/x");
    }

    #[test]
    fn keeps_equals_signs_inside_values() {
        let env = parse_env("OPTS=a=b=c\n");
        assert_eq!(env.get("OPTS").unwrap(), "a=b=c");
    }

    #[test]
    fn keeps_empty_values() {
        let env = parse_env("EMPTY=\n");
        assert_eq!(env.get("EMPTY").unwrap(), "");
    }

    #[test]
    fn ignores_continuation_lines_without_a_key() {
        let env = parse_env("FUNC=() {\n  echo hi\n}\nPATH=/bin\n");
        assert_eq!(env.get("PATH").unwrap(), "/bin");
        assert!(!env.contains_key("  echo hi"));
    }
}
