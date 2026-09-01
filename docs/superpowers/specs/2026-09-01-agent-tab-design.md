# The Agent tab, and renaming "engine": design

Renames the summarising program from "engine" to "agent" everywhere except
one place, gives it its own tab in the main window, and prompts from the
Overview when none is configured.

## Why

"Engine" describes what the thing is to this codebase, not what it is to the
person using it. What actually runs is Claude Code, Codex or opencode: an
agent, on the user's own subscription. Calling it an engine hides that.

The options for it are also buried. They sit inside a "Daily summary"
fieldset on the Settings tab, below five other groups, which is a long way
from the Summarise button on the Overview that does nothing without them.

## The name

`agent` in code, "Agent" in copy. Not "AI agent": the extra word dates the
product and buys nothing at the point of use.

The word is already overloaded here, because MCP clients are agents too.
Everything on that side is called a **client**, consistently, in code and in
docs. Where both appear in one sentence, the sentence says which.

## The rename

537 occurrences across 39 files. Mechanical, but with three points that are
not.

| From | To |
| --- | --- |
| `Engine`, `EngineError`, `EngineEnv` | `Agent`, `AgentError`, `AgentEnv` |
| `engine` field on `Settings` | `agent` |
| `engine_detect`, `engine_test`, `engine_auth`, `refresh_engine_env` | `agent_detect`, `agent_test`, `agent_auth`, `refresh_agent_env` |
| `src-tauri/src/engine.rs` | `src-tauri/src/agent.rs` |
| `src/components/EngineSettings.tsx` | `src/components/AgentTab.tsx` |
| `no_engine` MCP error code | `no_agent` |
| "Connect an engine in Settings to use this." | "Connect an agent on the Agent tab to use this." |
| `.engine-list`, `.engine-row`, `.engine-label`, `.engine-path` | `.agent-list`, `.agent-row`, `.agent-label`, `.agent-path` |

### Settings, which needs a migration

`Settings` carries `#[serde(default)]`, so unknown keys are ignored and
missing ones take their default. A bare rename would therefore be silent and
destructive: an existing `settings.json` holds `engine`, the new struct looks
for `agent`, finds nothing, and defaults to `None`. The user's configured
agent disappears with no error.

One line prevents it:

```rust
#[serde(alias = "engine")]
pub agent: Option<Agent>,
```

Deserialisation accepts either key, serialisation writes `agent`, and the
file normalises on the next save. No migration code, no version stamp.

A test must cover it: a settings JSON written with `engine` must load with
`agent` populated.

### The ledger keeps `engine`

`ledger.rs` writes an `engine` field into the audit entries stored in the
user's capture folder. Those files already exist on disk and the app's whole
posture is that it writes them once and leaves them alone.

That field is not renamed. It is a machine-readable record, not copy anyone
reads in the app, and changing it would either strand old entries or mean
rewriting files the user is entitled to consider settled. A comment on the
field states this, so the inconsistency reads as a decision rather than a
miss.

### MCP is a breaking change, taken deliberately

`no_engine` is a documented error code in `docs/mcp.md` that clients match
on. Renaming it to `no_agent` breaks any client that does.

The version is `0.1.0` and the repository has no tags, so nothing has been
released and no client can yet depend on it. This is the cheapest this
change will ever be. `docs/mcp.md` is updated in the same commit, including
the error table and the prose that names the engine.

## The Agent tab

A fourth tab, ordered **Overview, Context, Agent, Settings**. Agent sits
before Settings because it is a thing you configure once and then care
about, where Settings is a drawer of preferences.

It holds what governs summarising, moved out of Settings:

- **Agent**: detection, the radio list, the manual command, Test and
  Connect. Today's `EngineSettings` minus the parts below.
- **Schedule**: the daily time and its explanation.
- **Prompt**: the existing `PromptSettings` component, moved whole.

`EngineSettings.tsx` currently mixes four things under one "Daily summary"
fieldset. Two do not belong on this tab:

- **Launch at login** is an application preference, not an agent one. It
  moves to Settings, into a new `AppSettings` component with an
  "Application" legend. Not into `RecordingSettings`, despite that being the
  nearest existing home: its legend is "Recording" and it opens by saying
  "These change what is recorded from now on", which launch at login does
  not. One toggle in its own group is honest, and it is where a second
  application preference would go.
- **The Prompt pointer**, a paragraph saying the prompt lives further down
  the page, is deleted. The prompt is now on the same tab, so the pointer
  has nothing to point at.

The file is 329 lines doing four jobs. Splitting it is part of this work:
`AgentTab.tsx` for the agent and schedule, with `PromptSettings` composed
beside it, and the launch-at-login block relocated.

## The prompt when no agent is configured

A line in the Record panel on the Overview, above the controls, shown only
when `settings.agent` is null:

> No agent connected. Summarising needs one.  **[Set up an agent]**

The button switches to the Agent tab. It sits where the disabled Summarise
button is, so it appears exactly when the user would have reached for it.

`Overview` already reads `get_settings` to decide `hasEngine` for
`DefragControls`. The same value drives this, renamed to `hasAgent`. No new
IPC.

The disabled tooltips on `DefragControls` and `HighlightPill` are reworded
to name the tab rather than Settings.

## Tab strip

Four tabs where there were three. The strip is a flex row with a 2px gap and
no width constraint, and the window is 1000px wide, so a fourth label adds
roughly 70px against hundreds spare. No layout change is needed, but the tab
strip is one of the two places the window's look was tuned by eye, so it gets
looked at rather than assumed.

## Files

| File | Change |
| --- | --- |
| `src-tauri/src/engine.rs` | Renamed to `agent.rs`, types and functions renamed. |
| `src-tauri/src/settings.rs` | `agent` field with `#[serde(alias = "engine")]`. |
| `src-tauri/src/lib.rs` | Commands renamed and re-registered. |
| `src-tauri/src/control.rs` | `no_engine` becomes `no_agent`, message reworded. |
| `src-tauri/src/{jobs,propose}.rs` | Call sites. |
| `src-tauri/src/ledger.rs` | Unchanged, plus a comment on why. |
| `docs/mcp.md` | Error code, tool descriptions, prose. |
| `src/components/AgentTab.tsx` | New, from `EngineSettings.tsx`. |
| `src/components/AppSettings.tsx` | New. Launch at login, under an "Application" legend. |
| `src/components/Main.tsx` | Fourth tab, routing. |
| `src/components/Overview.tsx` | The no-agent prompt. |
| `src/components/{DefragControls,HighlightPill}.tsx` | Tooltip copy. |
| `src/lib/days.ts` | `Agent` type, `agent` field. |
| `src/{setup,main-window}.css` | Class renames. |

## Testing

- **Settings compatibility, Rust.** A JSON payload with the old `engine` key
  deserialises with `agent` populated. This is the one that stops the rename
  eating people's configuration.
- **Serialisation, Rust.** Saving writes `agent`, not `engine`.
- **The error code, Rust.** Requesting a summary with no agent returns
  `no_agent`.
- **The prompt, component.** Overview shows the prompt and the button when
  settings report no agent, and neither when one is set.
- **Navigation, component.** Pressing the button selects the Agent tab.
- **The tab, component.** The Agent tab renders the agent list, the schedule
  and the prompt editor, and Settings no longer renders them.
- **Launch at login, component.** It renders under Settings in its own
  Application group, and not on the Agent tab.

## Out of scope

- The ledger's `engine` field, as above.
- Any change to how agents are detected, tested or run. This is a rename and
  a relocation, not a rework.
- `settings.rs` currently swallows a parse failure and returns defaults for
  every setting, not just the unreadable one. Pre-existing, worth fixing,
  not here.
