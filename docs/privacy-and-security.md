# Privacy and security

Ambient Context processes an unusually sensitive data source: text visible in
the window a user is actively viewing. Its privacy posture is local-first and
data-minimizing, but it is not a data-loss-prevention system and its redaction
rules are not a guarantee that all sensitive text will be removed.

This document separates structural properties from tested controls,
best-effort heuristics, and user-controlled operational choices. It describes
the `0.1.0` source at the current `main` branch.

## Trust boundary

Ambient Context trusts:

- the local macOS account and filesystem permissions;
- macOS Accessibility and frontmost-window reporting;
- applications to expose an accurate Accessibility tree and secure-field role;
- the user to select an appropriate output folder; and
- any downstream agent the user grants access to that folder.

It does not protect day files from another process or person that can read the
user's files. It does not encrypt output, manage retention, control backups, or
constrain what a separate LLM client does with the files.

## Data inventory

| Data | Lifetime | Location | Protection |
| --- | --- | --- | --- |
| Raw focused-window snapshot | One polling iteration | Process memory | Bounded traversal; secure subtrees skipped at source |
| Open dwell block | Until window/content transition, stop, or failure flush | Process memory | Redacted and pruned text only |
| Day-level dedup hashes | Current capture run, reset by date or folder | Process memory | Non-cryptographic hashes of admitted lines and skeletons |
| Day timeline and text | Until user deletes or moves it | Selected output folder | Plaintext Markdown and ordinary filesystem permissions |
| Capture settings | Until changed or app data is removed | Tauri application config directory | Plaintext JSON and ordinary filesystem permissions |

The application stores no screenshots, video, audio, embeddings, model
outputs, account identifiers, or telemetry events.

## Control layers

### Before text is read

The Swift Accessibility walker checks each element's role and subrole. An
`AXSecureTextField` subtree is not read or traversed. This is the strongest
redaction boundary in the current implementation, but it depends on the target
application marking sensitive controls correctly.

The walker also stops when traversal bounds are reached and returns no snapshot
while macOS reports the session as locked.

### After text is read, before disk

The Accessibility bridge first constructs a raw in-memory snapshot. The Rust
redaction layer then discards the entire snapshot when the application name
contains one of these case-insensitive markers:

`1Password`, `Bitwarden`, `Dashlane`, `Enpass`, `KeePassXC`, `Keychain Access`,
`LastPass`, `NordPass`, `Proton Pass`, or `Strongbox`.

It also discards a snapshot when its window title contains `Private Browsing`,
`Incognito`, or `InPrivate`, case-insensitively.

These are denylists, not general password-manager or private-session
detection. Renamed, localized, unsupported, or newly introduced applications
and browser modes may not match. Because exclusion happens after the bridge
returns, the discarded text may have existed briefly in process memory even
though it is never passed to segmentation or written.

### Pattern replacement before disk

For retained snapshots, the Rust layer replaces matches for:

- AWS access key IDs beginning with `AKIA`;
- `sk-`, `sk_`, `pk-`, `pk_`, `rk-`, and `rk_`-style values of sufficient
  length;
- long bearer tokens;
- values following labels such as `api_key`, `secret`, `token`, `password`,
  or `passwd`; and
- card-shaped runs of 13–19 digits with optional spaces or hyphens.

The replacement is applied to window titles, document paths, URLs, and body
text before writing.

The patterns intentionally do not claim to detect every secret or piece of
personal information. Examples that may remain include names, messages,
health and financial content, email addresses, phone numbers, cookies,
unlabelled high-entropy values, non-AWS credentials, private file paths, and
secrets split across Accessibility elements.

### Self-capture avoidance

Before segmentation, the capture loop tries to recognize its own output using
an exposed document path or URL, today's filename in the window title, or a
combination of the date and output-folder name.

This prevents the common recursive-capture case. It is best effort: an editor
that exposes neither a useful document path nor a recognizable title can evade
the check.

## Claims matrix

| Claim | Current evidence | Qualification |
| --- | --- | --- |
| No screenshots, video, or audio are captured | The only reader is the Accessibility bridge; no screen or media-capture API appears in the capture path | Static source audit, not a notarized-binary audit |
| Only the focused window is traversed | The bridge selects the frontmost application and requests `kAXFocusedWindowAttribute` | Depends on macOS and the target app exposing correct Accessibility state |
| Capture stops while the screen is locked | The bridge checks `CGSSessionScreenIsLocked`; failed reads eventually flush the block | Depends on that session property being present and accurate |
| Secure password fields are skipped before reading | `AXSecureTextField` role/subrole terminates traversal; the Rust suite covers downstream redaction, not this Swift branch | Depends on correct target-app semantics; requires manual AX testing |
| Recognized password-manager and private-window snapshots are not written | Rust exclusions run before pruning, segmentation, and writing; unit tests cover named cases | Denylist/title heuristic; raw snapshot already existed in memory |
| Recognized secret patterns are redacted before writing | All snapshot fields pass through `redact_snapshot`; unit tests cover each current pattern | Pattern matching has false negatives and possible false positives |
| Nothing is uploaded by the capture pipeline | No network operation occurs in reader, redaction, segmentation, or writer code | User-selected synced folders, backups, and downstream agents are outside this guarantee |
| The current app makes no automatic update request | Updater support is configured, but `check_for_updates` has no call site | A future signed release is intended to add an update check |
| Output remains stopped after an explicit stop | The tray writes `enabled: false`; startup checks it; unit tests cover state helpers and settings | Unexpected termination during a settings write is not specifically tested |
| Output avoids recording itself | Document, URL, and title heuristics are covered by unit tests | Best effort when the editor does not expose those attributes |
| Each normalized body line is written once per day | Day-level hashes are seeded from an existing file; tests cover restart, deletion, and folder changes | Deduplication deliberately removes later context and uses non-cryptographic hashes |

## Network behavior

The current capture workflow is offline. The source includes Tauri's updater
plugin, a GitHub release endpoint, an updater implementation, and permission
for the setup window to use the plugin. Nothing calls that implementation in
the current application, so it does not perform an automatic update check.

The settings page has a user-initiated author link. Clicking it launches the
default browser for an HTTPS URL; the application does not fetch the page
itself.

Build tools and package managers use the network to obtain dependencies. That
is development-time behavior, not capture-time behavior.

## Output-folder considerations

The default `~/Ambient Context` folder is outside `~/Documents` to avoid the
common macOS Desktop and Documents iCloud configuration. The UI warns for
several recognizable iCloud Drive path forms when the user selects another
folder.

This is not general sync detection. Dropbox, Google Drive, network homes,
custom iCloud paths, backup software, Spotlight indexing, and other local
services may copy or index plaintext files. Users who require stronger
protection should choose a suitable local location and manage encryption,
backup, indexing, and retention at the operating-system level.

## Downstream LLM boundary

Ambient Context does not send day files to a model. A user or agent must be
given access separately. At that point the privacy behavior belongs to the
chosen client, model provider, permissions, and workflow.

Before granting access, consider whether that tool:

- runs locally or uploads context;
- reads only selected files or the whole folder;
- retains prompts or uses them for training;
- can follow `file:` and `url:` references into more sensitive sources; and
- can write summaries into a synced or shared location.

Redaction should be treated as defense in depth, not permission to give an
untrusted tool the capture folder.

## Known gaps

- Application and private-window exclusions occur after Accessibility-tree
  collection instead of before it.
- The exclusion lists are embedded in source and are not user-configurable.
- There is no allowlist mode for applications, sites, folders, or window
  titles.
- There is no pause-on-sensitive-app rule that users can extend themselves.
- There is no encryption, retention policy, secure deletion, or per-day
  access control.
- There is no automated Swift test harness for secure-field traversal,
  frontmost-window selection, lock detection, or Chromium enablement.
- There is no independent privacy or security audit.

## Reporting a problem

The project is early and currently directs reports to GitHub issues. Do not
include captured secrets or private day-file contents in a public report.
Describe the affected application, macOS version, reproduction shape, and the
kind of data exposed using synthetic examples.
