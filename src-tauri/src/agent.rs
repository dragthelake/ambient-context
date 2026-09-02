//! The connected agent: the user's own CLI (Claude Code, Codex, opencode),
//! run as a subprocess.
//!
//! Apps launched from Finder or the Dock inherit only the launchd
//! environment, which on macOS carries no `PATH` beyond the system
//! defaults, so brew, mise, volta and `~/.local/bin` installs are invisible.
//! The app therefore captures the user's login-shell environment once and
//! resolves absolute paths from it. `parse_env` is the pure half of that.

use std::collections::HashMap;
use std::sync::Mutex;

/// The connected agent lives in settings.rs because it is persisted as
/// part of settings.json; re-exported here so callers can talk about it as
/// `agent::Agent`.
pub use crate::settings::Agent;

/// The outcome of one agent invocation, in the shape the propose pipeline
/// reads: a failed run is data, not an error type.
#[derive(Debug, Clone)]
pub struct RunOutput {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
    pub timed_out: bool,
}

/// Runs the agent with a freshly captured login-shell environment and
/// reports the outcome as data. A missing binary is a failure with status
/// 127 and its explanation on stderr, the same shape a failing CLI
/// produces.
pub fn run(agent: &Agent, stdin: &str) -> RunOutput {
    let env = login_shell_env();
    match run_with_env(agent, stdin, &env) {
        Ok(stdout) => RunOutput {
            stdout,
            stderr: String::new(),
            status: 0,
            timed_out: false,
        },
        Err(AgentError::Timeout) => RunOutput {
            stdout: String::new(),
            stderr: format!("timed out after {}s", agent.timeout_secs),
            status: 124,
            timed_out: true,
        },
        Err(AgentError::NotFound) => RunOutput {
            stdout: String::new(),
            stderr: "the agent command could not be found".to_string(),
            status: 127,
            timed_out: false,
        },
        Err(AgentError::Failed { code, stderr }) => RunOutput {
            stdout: String::new(),
            stderr,
            status: code.unwrap_or(1),
            timed_out: false,
        },
        Err(AgentError::Io(message)) => RunOutput {
            stdout: String::new(),
            stderr: message,
            status: 1,
            timed_out: false,
        },
    }
}

/// Parses the output of `env`: KEY=VALUE per line. Values may contain '='
/// and may be empty; keys may not. Lines without '=' are continuations of a
/// previous multi-line value and are ignored rather than guessed at.
// Consumed by login_shell_env once the agent subprocess lands (0.2.0 plan,
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

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// The part of a failed run a person can act on. Many CLIs print errors on
/// stdout, stderr, or both; an empty stderr with a nonempty stdout is common.
pub fn failure_detail(stdout: &str, stderr: &str) -> String {
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        return stderr.to_string();
    }
    stdout.trim().to_string()
}

#[derive(Debug)]
pub enum AgentError {
    /// The command does not exist at that path. Almost always a stale
    /// absolute path after the user moved or reinstalled the CLI.
    NotFound,
    Timeout,
    Failed {
        code: Option<i32>,
        stderr: String,
    },
    Io(String),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::NotFound => write!(f, "the agent command could not be found"),
            AgentError::Timeout => write!(f, "the agent took too long and was stopped"),
            AgentError::Failed { code, stderr } => {
                let code = code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "signal".into());
                let detail = stderr.trim();
                if detail.is_empty() {
                    write!(f, "the agent exited with {code}")
                } else {
                    write!(f, "the agent exited with {code}: {detail}")
                }
            }
            AgentError::Io(message) => write!(f, "{message}"),
        }
    }
}

/// Runs the agent once: prompt in on stdin, response out on stdout.
///
/// The child is moved into a waiter thread so the caller can give up on it.
/// std has no wait-with-timeout, and rather than take a dependency for one
/// call site we keep the pid and send it a signal through /bin/kill, which
/// is always present on macOS.
/// The app never runs two agent invocations at once, from any path: the
/// scheduler, the on-demand queue, the test button and highlight-to-instruct
/// all pass through here. The queue keeps the scheduled and on-demand runs
/// serial by design; this lock is the floor under everything else.
static AGENT_LOCK: Mutex<()> = Mutex::new(());

/// True while an agent invocation is in progress. Interactive callers ask
/// this first so they can say "a summary is running" instead of parking a
/// button on a lock for up to ten minutes.
pub fn is_busy() -> bool {
    AGENT_LOCK.try_lock().is_err()
}

pub const BUSY_MESSAGE: &str =
    "An agent run is already in progress. Wait for it to finish and try again.";

pub fn run_with_env(
    agent: &crate::settings::Agent,
    prompt: &str,
    env: &HashMap<String, String>,
) -> Result<String, AgentError> {
    let _one_at_a_time = AGENT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut command = Command::new(&agent.command);
    command
        .args(&agent.args)
        .current_dir(agent_cwd())
        .env_clear()
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(AgentError::NotFound)
        }
        Err(error) => return Err(AgentError::Io(error.to_string())),
    };

    let pid = child.id();

    // Write on a thread: a prompt larger than the pipe buffer deadlocks if
    // the parent writes while the child is also blocked writing output.
    let mut stdin = child
        .stdin
        .take()
        .ok_or(AgentError::Io("no stdin".into()))?;
    let owned_prompt = prompt.to_string();
    std::thread::spawn(move || {
        let _ = stdin.write_all(owned_prompt.as_bytes());
        // Dropping stdin closes it, which is what tells the child the
        // prompt is complete. Without this, cat waits forever.
    });

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    match rx.recv_timeout(Duration::from_secs(agent.timeout_secs)) {
        Ok(Ok(output)) => {
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(AgentError::Failed {
                    code: output.status.code(),
                    stderr: failure_detail(&stdout, &stderr),
                })
            }
        }
        Ok(Err(error)) => Err(AgentError::Io(error.to_string())),
        Err(_) => {
            let _ = Command::new("/bin/kill")
                .args(["-9", &pid.to_string()])
                .status();
            Err(AgentError::Timeout)
        }
    }
}

/// Where every agent runs. Claude Code reads CLAUDE.md and Codex reads
/// AGENTS.md from the working directory, so the child must start somewhere
/// the app owns and keeps empty, never in whatever directory the app
/// happened to inherit. Created on first use.
pub fn agent_cwd() -> std::path::PathBuf {
    let dir = std::env::temp_dir()
        .join("com.0x0000007a.ambientcontext")
        .join("agent-cwd");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Runs a shell with the given arguments and returns its stdout, giving up
/// after `timeout`. Same waiter-thread shape as `run`, because std has no
/// wait-with-timeout and an interactive rc file can print, prompt or hang.
fn shell_output(shell: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let child = Command::new(shell)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) if output.status.success() => {
            Some(String::from_utf8_lossy(&output.stdout).to_string())
        }
        Ok(_) => None,
        Err(_) => {
            let _ = Command::new("/bin/kill")
                .args(["-9", &pid.to_string()])
                .status();
            None
        }
    }
}

/// Directories an agent CLI is likely to live in, for when no shell can be
/// run at all. Measured on the build machine: claude and cursor-agent in
/// ~/.local/bin, codex behind a volta shim, opencode in /opt/homebrew/bin.
const CANDIDATE_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "~/.local/bin",
    "~/.volta/bin",
    "~/.bun/bin",
    "~/.opencode/bin",
    "~/.cargo/bin",
    "~/.npm-global/bin",
    "/usr/bin",
    "/bin",
];

/// The environment a terminal would have. Apps launched from the Dock get
/// only the launchd environment (measured: it sets nothing but
/// SSH_AUTH_SOCK, so PATH is the launchd default /usr/bin:/bin:/usr/sbin:/sbin),
/// so brew, volta and ~/.local/bin are missing and every agent CLI is
/// invisible. Capture this once at startup and pass it to every child.
///
/// Order: the interactive login shell first, because some users only set
/// PATH in .zshrc (0.5 s on the build machine, 54 variables), then the
/// non-interactive login shell (41 variables, every preset still found),
/// then the launchd environment with a fixed candidate PATH prepended.
pub fn login_shell_env() -> HashMap<String, String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let timeout = Duration::from_secs(5);
    if let Some(raw) = shell_output(&shell, &["-l", "-i", "-c", "env"], timeout) {
        let env = parse_env(&raw);
        if env.contains_key("PATH") {
            return env;
        }
    }
    if let Some(raw) = shell_output(&shell, &["-l", "-c", "env"], timeout) {
        let env = parse_env(&raw);
        if env.contains_key("PATH") {
            return env;
        }
    }
    let mut env: HashMap<String, String> = std::env::vars().collect();
    let home = env.get("HOME").cloned().unwrap_or_default();
    let candidates: Vec<String> = CANDIDATE_DIRS
        .iter()
        .map(|d| d.replacen("~", &home, 1))
        .collect();
    let inherited = env.get("PATH").cloned().unwrap_or_default();
    env.insert(
        "PATH".to_string(),
        format!("{}:{}", candidates.join(":"), inherited),
    );
    env
}

use std::path::Path;

pub const DEFAULT_TIMEOUT_SECS: u64 = 600;

/// argv for one-shot Claude Code runs. Kept in one place so detection,
/// saved settings and the auth probe all stay aligned.
pub const CLAUDE_CODE_ARGS: &[&str] = &[
    "-p",
    "--output-format",
    "text",
    "--model",
    "claude-opus-5",
    "--effort",
    "medium",
];

pub const CLAUDE_DEFAULT_MODEL: &str = "claude-opus-5";

/// The models the Agent tab offers for Claude Code. AgentTab.tsx carries
/// the same list with display names; the two move together.
pub const CLAUDE_MODELS: &[&str] = &[
    "claude-fable-5-1",
    "claude-opus-5",
    "claude-sonnet-5",
    "claude-haiku-4-5",
];

/// argv for one Claude Code model. Haiku 4.5 takes no effort levels, so
/// the flag only goes on the models that accept it.
pub fn claude_code_args_for(model: &str) -> Vec<String> {
    let mut args: Vec<String> = ["-p", "--output-format", "text", "--model", model]
        .iter()
        .map(|arg| arg.to_string())
        .collect();
    if model != "claude-haiku-4-5" {
        args.push("--effort".to_string());
        args.push("medium".to_string());
    }
    args
}

#[allow(dead_code)] // normalised onto connected agents; kept for preset migration tests
pub fn claude_code_args() -> Vec<String> {
    claude_code_args_for(CLAUDE_DEFAULT_MODEL)
}

/// The value following "--model" in an argv, if any.
pub fn claude_model_of(args: &[String]) -> Option<&str> {
    let at = args.iter().position(|arg| arg == "--model")?;
    args.get(at + 1).map(String::as_str)
}

/// Refreshes Claude Code argv when the preset gains new flags, so an older
/// settings.json does not keep running without them. The chosen model
/// survives the refresh; anything not on the offered list falls back to
/// the default rather than being passed through to the CLI unchecked.
pub fn normalize_claude_agent(agent: &mut Agent) {
    if agent.label == "Claude Code" {
        let model = claude_model_of(&agent.args)
            .filter(|m| CLAUDE_MODELS.contains(m))
            .unwrap_or(CLAUDE_DEFAULT_MODEL)
            .to_string();
        agent.args = claude_code_args_for(&model);
    }
}

/// Label, binary name, and the argv that makes it read one prompt from
/// stdin and print one answer to stdout. Taken verbatim from
/// docs/engine-spike.md; do not invent entries for CLIs nobody has run.
const PRESETS: &[(&str, &str, &[&str])] = &[
    ("Claude Code", "claude", CLAUDE_CODE_ARGS),
    (
        "Codex",
        "codex",
        &["exec", "--skip-git-repo-check", "--color", "never", "-"],
    ),
    ("opencode", "opencode", &["run"]),
];
// Not presets, per the spike: cursor-agent (its headless mode gates on
// per-directory workspace trust and the only bypass also force-allows
// commands), and goose, gemini, amp and copilot (not installed on the
// build machine, so unverified). All are reachable through the manual
// template in Settings.

/// Resolves a binary name against a PATH, the way a shell would. Used
/// instead of shelling out to `which` so detection cannot itself depend on
/// a shell being available.
pub fn which(name: &str, env: &HashMap<String, String>) -> Option<String> {
    let path = env.get("PATH")?;
    for dir in path.split(':').filter(|d| !d.is_empty()) {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

pub fn detect(env: &HashMap<String, String>) -> Vec<crate::settings::Agent> {
    detect_from(PRESETS, env)
}

pub fn detect_from(
    presets: &[(&str, &str, &[&str])],
    env: &HashMap<String, String>,
) -> Vec<crate::settings::Agent> {
    presets
        .iter()
        .filter_map(|(label, binary, args)| {
            which(binary, env).map(|command| crate::settings::Agent {
                label: label.to_string(),
                command,
                args: args.iter().map(|a| a.to_string()).collect(),
                timeout_secs: DEFAULT_TIMEOUT_SECS,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AuthState {
    /// The CLI reports it is signed in.
    Ok,
    /// The CLI reports it is not, and this is what the user should run.
    NotLoggedIn { fix: String },
    /// The CLI works without credentials but will use a free model.
    NoProvider { fix: String },
    /// A manual template, or a preset whose check could not run.
    Unknown,
}

/// The auth probe for a preset, by label. Manual agents return None.
fn auth_probe(label: &str) -> Option<(&'static [&'static str], &'static str)> {
    match label {
        "Claude Code" => Some((
            &["auth", "status"],
            "Run `claude` in a terminal and use /login.",
        )),
        "Codex" => Some((&["login", "status"], "Run `codex login`.")),
        "opencode" => Some((
            &["auth", "list"],
            "Run `opencode auth login` to use your own provider.",
        )),
        _ => None,
    }
}

/// Asks the agent whether it is signed in. Ten second cap; a probe that
/// hangs is Unknown, not a failure.
pub fn auth_state(agent: &crate::settings::Agent, env: &HashMap<String, String>) -> AuthState {
    let Some((args, fix)) = auth_probe(&agent.label) else {
        return AuthState::Unknown;
    };
    let child = match Command::new(&agent.command)
        .args(args)
        .current_dir(agent_cwd())
        .env_clear()
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return AuthState::Unknown,
    };
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let output = match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(output)) => output,
        _ => {
            let _ = Command::new("/bin/kill")
                .args(["-9", &pid.to_string()])
                .status();
            return AuthState::Unknown;
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    match agent.label.as_str() {
        "Claude Code" => {
            if output.status.success() && stdout.contains("\"loggedIn\": true") {
                AuthState::Ok
            } else {
                AuthState::NotLoggedIn {
                    fix: fix.to_string(),
                }
            }
        }
        "Codex" => {
            if output.status.success() {
                AuthState::Ok
            } else {
                AuthState::NotLoggedIn {
                    fix: fix.to_string(),
                }
            }
        }
        "opencode" => {
            if stdout.contains("0 credentials") {
                AuthState::NoProvider {
                    fix: fix.to_string(),
                }
            } else {
                AuthState::Ok
            }
        }
        _ => AuthState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Agent;

    fn agent_for(command: &str, args: &[&str], timeout_secs: u64) -> Agent {
        Agent {
            label: "test".to_string(),
            command: command.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            timeout_secs,
        }
    }

    #[test]
    fn feeds_the_prompt_on_stdin_and_returns_stdout() {
        // /bin/cat copies stdin to stdout, which is exactly the contract.
        let agent = agent_for("/bin/cat", &[], 10);
        let out = run_with_env(&agent, "hello prompt", &HashMap::new()).unwrap();
        assert_eq!(out, "hello prompt");
    }

    #[test]
    fn a_missing_binary_is_not_found_rather_than_a_generic_io_error() {
        let agent = agent_for("/nope/not/here", &[], 10);
        assert!(matches!(
            run_with_env(&agent, "x", &HashMap::new()),
            Err(AgentError::NotFound)
        ));
    }

    #[test]
    fn a_nonzero_exit_carries_its_stderr() {
        let agent = agent_for("/bin/sh", &["-c", "echo boom >&2; exit 3"], 10);
        let out = run(&agent, "x");
        assert_eq!(out.status, 3);
        assert!(out.stderr.contains("boom"), "stderr was {:?}", out.stderr);
    }

    #[test]
    fn a_nonzero_exit_falls_back_to_stdout_when_stderr_is_empty() {
        let agent = agent_for("/bin/sh", &["-c", "echo out of tokens; exit 1"], 10);
        let out = run(&agent, "x");
        assert_eq!(out.status, 1);
        assert_eq!(out.stderr, "out of tokens");
    }

    #[test]
    fn failure_detail_prefers_stderr() {
        assert_eq!(failure_detail("stdout only", "stderr wins"), "stderr wins");
        assert_eq!(failure_detail("stdout only", ""), "stdout only");
        assert_eq!(failure_detail("", ""), "");
    }

    #[test]
    fn a_slow_agent_times_out_instead_of_hanging_forever() {
        let agent = agent_for("/bin/sh", &["-c", "sleep 30"], 1);
        let started = std::time::Instant::now();
        assert!(matches!(
            run_with_env(&agent, "x", &HashMap::new()),
            Err(AgentError::Timeout)
        ));
        assert!(
            started.elapsed().as_secs() < 10,
            "run did not return promptly on timeout"
        );
    }

    #[test]
    fn the_supplied_environment_reaches_the_child() {
        let agent = agent_for("/bin/sh", &["-c", "printf %s \"$AC_TEST_VAR\""], 10);
        let mut env = HashMap::new();
        env.insert("AC_TEST_VAR".to_string(), "present".to_string());
        assert_eq!(run_with_env(&agent, "", &env).unwrap(), "present");
    }

    #[test]
    fn which_finds_a_binary_on_the_supplied_path() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        assert_eq!(which("cat", &env), Some("/bin/cat".to_string()));
    }

    #[test]
    fn which_returns_none_when_the_binary_is_absent() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        assert_eq!(which("definitely-not-a-real-binary", &env), None);
    }

    #[test]
    fn which_ignores_directories_that_do_not_exist() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/nope/nope:/bin".to_string());
        assert_eq!(which("cat", &env), Some("/bin/cat".to_string()));
    }

    #[test]
    fn detect_returns_nothing_when_no_cli_is_installed() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/nope/nope".to_string());
        assert!(detect(&env).is_empty());
    }

    #[test]
    fn a_detected_agent_carries_an_absolute_path_and_a_timeout() {
        // Stand in for a real CLI: /bin/cat exists everywhere.
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/bin".to_string());
        let found = detect_from(&[("Cat", "cat", &[])], &env);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].command, "/bin/cat");
        assert_eq!(found[0].label, "Cat");
        assert_eq!(found[0].timeout_secs, DEFAULT_TIMEOUT_SECS);
    }

    /// A stand-in CLI: a script that prints `stdout` and exits with `code`,
    /// whatever arguments it is given.
    fn fake_cli(
        dir: &std::path::Path,
        label: &str,
        stdout: &str,
        code: i32,
    ) -> crate::settings::Agent {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(format!("fake-{}", label.to_lowercase().replace(' ', "-")));
        std::fs::write(
            &path,
            format!("#!/bin/sh\nprintf '%s\\n' '{stdout}'\nexit {code}\n"),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        crate::settings::Agent {
            label: label.to_string(),
            command: path.to_string_lossy().to_string(),
            args: vec![],
            timeout_secs: 5,
        }
    }

    #[test]
    fn claude_code_args_pin_opus_with_medium_effort() {
        assert_eq!(
            claude_code_args(),
            vec![
                "-p",
                "--output-format",
                "text",
                "--model",
                "claude-opus-5",
                "--effort",
                "medium",
            ]
        );
    }

    #[test]
    fn normalize_claude_agent_refreshes_stale_argv() {
        let mut agent = Agent {
            label: "Claude Code".to_string(),
            command: "/usr/local/bin/claude".to_string(),
            args: vec!["-p".to_string()],
            timeout_secs: 600,
        };
        normalize_claude_agent(&mut agent);
        assert_eq!(agent.args, claude_code_args());
    }

    #[test]
    fn haiku_takes_no_effort_flag() {
        let args = claude_code_args_for("claude-haiku-4-5");
        assert!(!args.contains(&"--effort".to_string()));
        assert_eq!(claude_model_of(&args), Some("claude-haiku-4-5"));
    }

    #[test]
    fn normalize_claude_agent_keeps_the_chosen_model() {
        let mut agent = Agent {
            label: "Claude Code".to_string(),
            command: "/usr/local/bin/claude".to_string(),
            // Stale argv from an older preset, but with a model the user chose.
            args: vec![
                "-p".to_string(),
                "--model".to_string(),
                "claude-sonnet-5".to_string(),
            ],
            timeout_secs: 600,
        };
        normalize_claude_agent(&mut agent);
        assert_eq!(claude_model_of(&agent.args), Some("claude-sonnet-5"));
        assert!(agent.args.contains(&"--effort".to_string()));
    }

    #[test]
    fn normalize_claude_agent_rejects_unknown_models() {
        let mut agent = Agent {
            label: "Claude Code".to_string(),
            command: "/usr/local/bin/claude".to_string(),
            args: vec!["--model".to_string(), "not-a-model".to_string()],
            timeout_secs: 600,
        };
        normalize_claude_agent(&mut agent);
        assert_eq!(claude_model_of(&agent.args), Some(CLAUDE_DEFAULT_MODEL));
    }

    #[test]
    fn a_manual_agent_has_unknown_auth() {
        let dir = tempfile::tempdir().unwrap();
        let env = HashMap::new();
        let agent = fake_cli(dir.path(), "Something else", "", 0);
        assert_eq!(auth_state(&agent, &env), AuthState::Unknown);
    }

    #[test]
    fn claude_logged_in_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let env = HashMap::new();
        let agent = fake_cli(dir.path(), "Claude Code", "{ \"loggedIn\": true }", 0);
        assert_eq!(auth_state(&agent, &env), AuthState::Ok);
    }

    #[test]
    fn codex_logged_out_names_the_fix() {
        let dir = tempfile::tempdir().unwrap();
        let env = HashMap::new();
        let agent = fake_cli(dir.path(), "Codex", "Not logged in", 1);
        assert_eq!(
            auth_state(&agent, &env),
            AuthState::NotLoggedIn {
                fix: "Run `codex login`.".to_string()
            }
        );
    }

    #[test]
    fn opencode_with_no_credentials_is_a_warning_not_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let env = HashMap::new();
        let agent = fake_cli(dir.path(), "opencode", "└  0 credentials", 0);
        assert!(matches!(
            auth_state(&agent, &env),
            AuthState::NoProvider { .. }
        ));
    }

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
