# Engine spike

Measured on 2026-08-31 on the build machine (macOS, Apple Silicon). This
resolves the assumption-based "engine" section of the V1 spec. Task 4 of the
0.2.0 plan copies the "Decisions for Task 4" section verbatim.

## 1. The environment difference

A Tauri app launched from Finder or the Dock inherits the launchd GUI
domain's environment, not a shell's. On this machine that domain sets
nothing but `SSH_AUTH_SOCK`:

```
$ launchctl print gui/$(id -u) | sed -n '/environment = {/,/}/p'
	environment = {
		SSH_AUTH_SOCK => /var/run/com.apple.launchd.LczxwSmkyF/Listeners
	}
$ launchctl getenv PATH
(empty)
```

With no `PATH` set in the domain, a GUI process gets the launchd default,
`/usr/bin:/bin:/usr/sbin:/sbin`. None of the agent CLIs live there. (Task 10
prints `std::env::var("PATH")` from the first window build as the in-app
confirmation; this session measured the domain rather than a bundle launch.)

The same problem shows up between shells: the shell this spike ran in had no
`~/.cargo/bin` on its `PATH`, so `cargo` was "command not found" until the
path was added by hand. Nothing about `PATH` can be assumed at 6am.

Login-shell capture recovers it. `$SHELL` is `/bin/zsh`.

| Invocation | Variables | PATH entries of interest | Time |
|---|---|---|---|
| `/bin/zsh -lc env` | 41 | `/opt/homebrew/bin`, `~/.cargo/bin`, `~/.volta/bin`, `~/.local/bin`, `~/.bun/bin`, `~/.opencode/bin`, `~/.asdf/shims` | ~0.3 s |
| `/bin/zsh -lic env` | 54 | all of the above plus `~/.dotnet`, `~/.yarn/bin`, miniconda, `/opt/homebrew/opt/ruby/bin` | 0.52 s |

Every CLI found below is on the `-lc` (login, non-interactive) path already.
The interactive variant adds tool managers that some users only initialise
in `.zshrc`, at the cost of running their interactive rc, which can print,
prompt, or hang. Decision: run `-lic` with a 5 second timeout and fall back
to `-lc`; if both fail, fall back to a fixed candidate list (below).

## 2. Where each CLI lives

```
claude       -> /Users/cameronsmith/.local/bin/claude
                (symlink to ~/.local/share/claude/versions/2.1.251)
codex        -> /Users/cameronsmith/.volta/bin/codex
                (volta shim; volta-shim resolves the real binary per call)
opencode     -> /opt/homebrew/bin/opencode
                (symlink into /opt/homebrew/Cellar/opencode/1.18.20/...)
cursor-agent -> /Users/cameronsmith/.local/bin/cursor-agent
                (symlink to ~/.local/share/cursor-agent/versions/2026.01.23-.../cursor-agent)
gh           -> /opt/homebrew/bin/gh          (no copilot extension installed)
goose        -> MISSING
gemini       -> MISSING
amp          -> MISSING
copilot      -> MISSING
```

**Store the path as found on `PATH`, never its realpath.** Three of the
four installed CLIs are symlinks or shims whose target contains a version
number and changes on every update. `~/.local/bin/claude` is stable across
updates; `~/.local/share/claude/versions/2.1.251` is dead after the next one.

## 3. One-shot invocation, verbatim

All runs from `/tmp`, prompt on stdin unless stated, wall time from `time`.
The 40 KB prompt is the first 40,000 bytes of a real day file with a
one-line instruction on top.

### Claude Code 2.1.251

```
$ echo "Reply with exactly the word: ok" | claude -p
ok
exit=0                                   5.3 s
$ claude -p < /tmp/prompt40k.txt
Mail
exit=0                                   5.4 s (40 KB)
```

Unauthenticated (empty `HOME`):

```
$ HOME=/tmp/emptyhome claude -p
Not logged in · Please run /login         (on STDOUT, not stderr)
exit=1
```

Auth check without spending a call:

```
$ claude auth status
{ "loggedIn": true, "authMethod": "claude.ai", ... }
exit=0
```

Notes: `-p` reads stdin when no prompt argument is given. `--output-format
text` is the default and is passed explicitly so a future default change
cannot break parsing. `claude -p` loads `CLAUDE.md` from the working
directory, so the app must spawn it in a directory it owns and keeps empty.
The user's own `~/.claude/CLAUDE.md` still applies; that is their agent
behaving as they configured it.

### Codex CLI 0.147.0

```
$ echo "Reply with exactly the word: ok" | codex exec --skip-git-repo-check -
ok                                        (stdout)
exit=0                                   15.0 s (7.2 s on a second run)
$ codex exec --skip-git-repo-check - < /tmp/prompt40k.txt
Mail
exit=0                                   24.8 s (40 KB, gpt-5.5, 25,608 tokens)
```

stderr carries a banner (`OpenAI Codex v0.147.0`, workdir, model, provider,
approval, sandbox), a `tokens used` line, and on this machine a repeating
`rmcp::transport::worker ... http://127.0.0.1:29979/mcp` error from a
user-configured MCP server that is not running. stdout is the final message
only. `-` makes it read the prompt from stdin; without `--skip-git-repo-check`
it refuses to run outside a git repository. Default sandbox is read-only,
which is what a summariser wants.

Unauthenticated (empty `HOME`, `CODEX_HOME` pointing at an empty dir):

```
ERROR: Reconnecting... 1/5 ... 5/5
ERROR: unexpected status 401 Unauthorized: Missing bearer or basic authentication in header ...
exit=1                                   (slow: five reconnect attempts first)
```

Auth check:

```
$ codex login status
Logged in using ChatGPT
exit=0
$ HOME=/tmp/emptyhome CODEX_HOME=/tmp/emptyhome/.codex codex login status
Not logged in
exit=1
```

`-o, --output-last-message <FILE>` also exists and writes the final message
to a file; stdout is sufficient and is what the engine contract reads.

### opencode 1.18.20

```
$ echo "Reply with exactly the word: ok" | opencode run
ok                                        (stdout)
exit=0                                   8.4 s
$ opencode run < /tmp/prompt40k.txt
Mail
exit=0                                   11.8 s (40 KB, z-ai/glm-5.3-flash via OpenRouter)
```

stderr carries an ANSI-coloured banner (`> build · z-ai/glm-5.3-flash`). The
prompt may also be passed as an argument; stdin is used for parity with the
others. The model is whatever the user configured in opencode; the app has
no say and should display the banner's model name in the ledger.

Unauthenticated (empty `HOME` and XDG dirs): **it does not fail.** It falls
back to a free hosted model:

```
> build · big-pickle
ok
exit=0
```

Auth check:

```
$ opencode auth list
●  OpenRouter api
└  1 credentials
$ (empty home) opencode auth list
└  0 credentials
```

Zero credentials is a warning state, not a failure: "opencode has no
provider configured and will answer with a free model".

### cursor-agent 2026.01.23

```
$ cursor-agent -p --output-format text "Reply with exactly the word: ok"
ok
exit=0                                   13.7 s   (first run)
$ cursor-agent -p --output-format text "Reply with exactly the word: ok"
⚠ Workspace Trust Required
exit=1                                            (identical command, minutes later)
$ echo "..." | cursor-agent -p --output-format text --force
ok
exit=0
```

Headless mode gates on per-directory workspace trust and the gate fired
inconsistently for the same directory. `--force` bypasses it, but `--force`
means "force allow commands unless explicitly denied", which is the wrong
flag to hand a summariser. Unauthenticated: `Error: Authentication
required. Please run 'agent login' first, or set CURSOR_API_KEY environment
variable.` exit 1.

### goose

Not installed on this machine. The `goose run -i -` template from the plan is
unverified.

## Decisions for Task 4

**Environment capture.** Run `$SHELL -lic env` (falling back to `/bin/zsh`
when `SHELL` is unset) with a 5 second timeout; on failure run `$SHELL -lc
env`; on failure use the launchd environment plus this candidate list, in
order: `/opt/homebrew/bin`, `/usr/local/bin`, `~/.local/bin`,
`~/.volta/bin`, `~/.bun/bin`, `~/.opencode/bin`, `~/.cargo/bin`,
`~/.npm-global/bin`, `/usr/bin`, `/bin`. Parse with `engine::parse_env`.
Capture once per app launch and again on demand from the settings page.

**Resolution.** For each preset, walk the captured `PATH` and take the first
entry where `<dir>/<name>` exists and is executable. Store that path. Do not
canonicalise it.

**Presets.** Three, all verified above. Each is spawned with the captured
environment, with the working directory set to `<app_data_dir>/engine-cwd/`
(created empty, kept empty), with the prompt written to stdin and stdin then
closed, with stdout read to end and trimmed as the result, with stderr kept
for the ledger and for failure display after stripping ANSI escapes, and
with a 600 second default timeout.

| Label | Binary | Args | Success | Auth check |
|---|---|---|---|---|
| Claude Code | `claude` | `-p --output-format text` | exit 0, stdout non-empty | `claude auth status` exit 0 and JSON `loggedIn: true` |
| Codex | `codex` | `exec --skip-git-repo-check --color never -` | exit 0, stdout non-empty | `codex login status` exit 0 |
| opencode | `opencode` | `run` | exit 0, stdout non-empty | `opencode auth list` reports at least one credential; zero is a warning, not a failure |

**Not presets.** `cursor-agent` (trust gate, and the only bypass is a flag
that widens permissions), `goose`, `gemini`, `amp`, `copilot` (not installed
here, so unverified). All remain reachable through the manual template
(command, arguments, whether the prompt goes on stdin or as the last
argument). If any is later verified, it becomes a preset by adding a row.

**Unauthenticated states to show in Settings.** Claude Code: "Not logged in.
Run `claude` in a terminal and use `/login`." Codex: "Not logged in. Run
`codex login`." opencode: "No provider configured. opencode will answer with
a free model; run `opencode auth login` to use your own." The detection is
the auth-check column above, run once per settings page open, never on the
6am schedule.

**Timing.** A 40 KB day file took 5 s (Claude Code), 12 s (opencode) and
25 s (Codex). Real day files run to several MB before the summariser's own
trimming; the 600 s default timeout is generous and the runner must never
block capture while it waits.
