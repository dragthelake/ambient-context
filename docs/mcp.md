# Ambient Context MCP server

Ambient Context ships the same binary you launch from the menu bar with an
`mcp` subcommand. Any MCP client that can start a stdio server can drive the
app through it. The read tools open the capture folder and the config
directory directly, so they work whether or not the app is running. The tools
that change anything connect to the running app over a local Unix socket, so
validation and the ledger happen in exactly one place: every write lands in
the day's ledger with your client's name on it, including writes that were
refused.

There is no authentication token on the socket. It is a Unix domain socket at
mode 0600 inside your own app data directory; anything that could reach it
could also read your capture folder directly, and the app never holds
credentials of any kind.

## Registration

The path below is the release bundle path. If you registered a debug build
during development, remove it first: `claude mcp remove ambient-context`.

### Claude Code

Run this in a terminal:

```
claude mcp add --scope user --transport stdio ambient-context -- "/Applications/Ambient Context.app/Contents/MacOS/ambient-context" mcp
```

### Claude Desktop

Into `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "ambient-context": {
      "command": "/Applications/Ambient Context.app/Contents/MacOS/ambient-context",
      "args": ["mcp"]
    }
  }
}
```

### Cursor

Into `~/.cursor/mcp.json` for every project, or `.cursor/mcp.json` inside one
project:

```json
{
  "mcpServers": {
    "ambient-context": {
      "command": "/Applications/Ambient Context.app/Contents/MacOS/ambient-context",
      "args": ["mcp"]
    }
  }
}
```

### Zed

Into `settings.json`:

```json
{
  "context_servers": {
    "ambient-context": {
      "command": "/Applications/Ambient Context.app/Contents/MacOS/ambient-context",
      "args": ["mcp"],
      "env": {}
    }
  }
}
```

### Generic

For anything else that speaks stdio MCP:

```json
{
  "name": "ambient-context",
  "command": "/Applications/Ambient Context.app/Contents/MacOS/ambient-context",
  "args": ["mcp"]
}
```

## The tools

Eighteen tools, in the order of the spec's table. Every date is `YYYY-MM-DD`.
Every write names the calling client in the ledger.

### `capture_status`

Reports whether capture is running, how many blocks were recorded today,
which app is focused, and the eight most recent summary jobs. Needs the app
running. Input:

```json
{ "type": "object", "additionalProperties": false }
```

Result:

```json
{ "running": true, "blocks_today": 12, "focused_app": "Xcode", "jobs": [] }
```

Errors: `not_running` when the app is closed.

### `start_capture`

Turns capture on, the same as clicking the menu bar icon. Writes the change
to settings and records it in the ledger. Needs the app running. Input: the
empty object above. Result: the capture status object. Errors:
`not_running`, `invalid` (no capture folder set), `io`.

### `stop_capture`

Turns capture off and leaves it off across restarts. Nothing already
recorded is removed. Needs the app running. Input and result as for
`start_capture`. Errors: `not_running`, `io`.

### `list_days`

Lists every day in the capture folder with date, whether capture and summary
exist, whether a KB folder exists, byte size (the sum of the three day files)
and summary title. Works with the app closed. Input: the empty object.
Result:

```json
{
  "folder": "/Users/x/Ambient Context",
  "days": [
    { "date": "2026-08-30", "has_capture": true, "has_summary": false,
      "has_kb": false, "bytes": 48211, "title": null }
  ]
}
```

Errors: tool error "No capture folder is set" when settings has no folder.

### `read_day`

Returns one of the day's raw files exactly as it is on disk. Optional `file`
is `apps` (default), `websites` or `messages`. Optional `from` and `to`
(24-hour `HH:MM`, `to` exclusive) keep only the blocks that start in the
range on `apps` and `messages`; `websites` is returned whole. Works with the
app closed. Input:

```json
{
  "type": "object",
  "properties": {
    "date": { "type": "string", "description": "A date in YYYY-MM-DD form, for example 2026-08-30." },
    "file": { "type": "string", "enum": ["apps", "websites", "messages"] },
    "from": { "type": "string" },
    "to": { "type": "string" }
  },
  "required": ["date"],
  "additionalProperties": false
}
```

Result: the requested day file text as one content block. Errors: `NoCapture`
("There is no capture for 2026-08-29."), `BadTime` for a malformed time,
tool error when `file` is not one of apps, websites or messages.

### `read_summary`

Returns one day's generated summary. Works with the app closed. Input:
`date` only. Result: the summary markdown as one content block. Errors:
"There is no summary for {date} yet. Call summarise_day to generate one."

### `search_record`

Case-insensitive substring search across every day file and every summary.
Works with the app closed. Input:

```json
{
  "type": "object",
  "properties": {
    "query": { "type": "string" },
    "limit": { "type": "integer", "minimum": 1, "maximum": 200 }
  },
  "required": ["query"],
  "additionalProperties": false
}
```

Result: `{ "query": "...", "hits": [ { "date", "layer", "line", "text",
"context" } ], "truncated": false }`. `layer` is `"apps"`, `"websites"`,
`"messages"` or `"summary"`.

### `read_ledger`

Returns one day's ledger: every model action and configuration change with
trigger, inputs and hashes, output, reasoning and outcome. Works with the app
closed. Input: `date` only. Result: the ledger file as one content block.
Errors: "There are no ledger entries for {date}."

### `summarise_day`

Queues a summary for one day using the connected agent, replacing any
existing summary. Returns a job id immediately; poll `capture_status` until
the job reports done or failed. Needs the app running with an agent. Input:
`date` only. Result:

```json
{ "job_id": "job-7", "status": "queued",
  "note": "Poll capture_status and look for this job id under jobs." }
```

Errors: `no_agent`, `not_found` (no capture for that date), `invalid`
(bad date or no folder), `not_running`.

### `list_rules`

Lists the user's rules and the locked built-in protections. Works with the
app closed. Input: the empty object. Result:

```json
{ "rules": [ { "id": "r1", "target": { "app": "Slack" }, "action": "exclude" } ],
  "built_ins": [ { "id": "builtin:password-managers", "description": "..." } ],
  "note": "Built-in protections are shown so you can see what is never recorded." }
```

### `add_rule`

Adds one rule and writes the rules file, which changes capture from the next
snapshot. Ledgered with your client named. Needs the app running. Input:

```json
{
  "type": "object",
  "properties": {
    "rule": {
      "type": "object",
      "properties": {
        "id": { "type": "string" },
        "target": {
          "type": "object",
          "properties": {
            "app": { "type": "string" },
            "website": { "type": "string" },
            "title": { "type": "string" }
          },
          "additionalProperties": false
        },
        "action": { "type": "string", "enum": ["exclude", "headings_only", "full", "route_messages"] },
        "note": { "type": "string" }
      },
      "required": ["target", "action"],
      "additionalProperties": false
    }
  },
  "required": ["rule"],
  "additionalProperties": false
}
```

Leave `id` out of `add_rule` and one is generated. Result: the full rules
payload. Errors: `locked` (a built-in protection, or a rule that would weaken
one), `duplicate`, `invalid`, `not_running`, `io`.

### `update_rule`

Replaces an existing rule with the same id. Ledgered. Needs the app running.
Input as for `add_rule`, with `id` required. Errors as for `add_rule` plus
`not_found`.

### `remove_rule`

Removes one rule by id. Built-in protections are refused. Ledgered. Needs
the app running. Input: `id` only. Result: the full rules payload. Errors:
`locked`, `not_found`, `not_running`.

### `get_prompt`

Returns the summary prompt and whether it is the bundled default or a
customised copy. Works with the app closed. Input: the empty object. Result:

```json
{ "text": "You are turning one day of ambient screen capture...", "customised": false }
```

### `set_prompt`

Replaces the summary prompt in full. Rejected if it drops a heading summary
validation requires. Ledgered. Needs the app running. Input: `text` (the
complete markdown prompt). Result: `{ "customised": true, "chars": 2841 }`.
Errors: `invalid` (empty, or a missing required heading, named exactly),
`not_running`.

### `get_config`

Returns every setting the Settings page exposes, the app version, the list
of keys `set_config` accepts, and whether the prompt is customised. Works
with the app closed. Input: the empty object. Result: the settings JSON plus
`version`, `settable_keys` and `prompt_customised`.

### `set_config`

Changes one or more settings and applies them immediately. Keys not in the
patch are left alone. There is no retention setting, and capture is turned on
and off with `start_capture` and `stop_capture`, not through this tool.
Ledgered. Needs the app running. Input:

```json
{
  "type": "object",
  "properties": { "patch": { "type": "object" } },
  "required": ["patch"],
  "additionalProperties": false
}
```

Example: `{ "patch": { "schedule_hhmm": "07:30" } }` or
`{ "patch": { "ingest_max_chars": 250000 } }`. The ingest agent is chosen in
the app, not over MCP (like `agent`). Result: the full patched settings. Errors: `unknown_key` (names the key; explains that nothing
deletes captured content when the key looks like a retention setting),
`invalid`, `not_running`, `io`.

### `open_day`

Opens the Ambient Context window on a given day and brings it to the front.
Changes no files. Needs the app running. Input: `date` only. Result:
`{ "opened": "2026-08-30" }`. Errors: `invalid`, `not_running`.

## Errors

Two mechanisms. An unknown tool name is a JSON-RPC protocol error, code
`-32602`, named in the message. Everything else is a normal result with
`isError: true` and one sentence in the content that says what to do next:
a missing day, a refused rule, an unauthenticated agent, an app that is not
running.

The refusal codes the app can return over the control socket: `not_running`,
`bad_request`, `unknown_key`, `invalid`, `duplicate`, `not_found`, `locked`,
`no_agent` and `io`.

`no_agent` was called `no_engine` before 0.2.0. The app was unreleased at
the time, so no client should be matching the old code.

## What there is no tool for

- **Writing or deleting captured content.** The record is evidence; nothing
  in the app or over MCP edits it. Deletion is Finder, if you must.
- **Editing the built-in protections.** They are listed so you can see what
  is never recorded, and refused on every surface for the same reason.
- **Setting a retention period.** There is no retention sweep in the product;
  a patch that asks for one is refused by name.
