# V1 build handover

Built by an unattended agent run on 2026-08-31 from the three written plans
(0.2.0, 0.3.0, 0.4.0), on stacked branches `build/0.2.0` -> `build/0.3.0` ->
`build/0.4.0`. Every task was committed after its gate (`cargo test`, plus
`npm run build` for tasks touching `src/`). Prove-it tasks (Plan A Task 14,
Plan B Task 13, Plan C Task 13) were not attempted and are listed under
"Needs human verification". Visual tasks were built and compiled but not
judged on screen; they are listed under "Needs visual pass".

Versions remain at 0.1.0 in `src-tauri/Cargo.toml` and `tauri.conf.json`: the
only version-bump step in any plan is Task 13 Step 8 of Plan C (renumber to
1.0.0), which is a prove-it task that was not done. The bundle identifier
`com.0x0000007a.ambientcontext` was never touched, and neither was anything
under signing or the updater pubkey.

## Status
| Plan | Branch | Tasks done | Tasks blocked | Last commit |
|---|---|---|---|---|
| 0.2.0 (A) | build/0.2.0 | 2-13 (11 of 13; 14 is prove-it) | none | ea0f47d |
| 0.3.0 (B) | build/0.3.0 | 1-12 (12 of 13; 13 is prove-it) | none | 4ea15b6 |
| 0.4.0 (C) | build/0.4.0 | 1-12 (12 of 13; 13 is prove-it) | none | e31c68e |

## Blockers
(empty)

## Decisions
One bullet per choice the plans did not settle, with the task reference and why.

- Plan A Task 12: the `onOpenInEditor`/`onReveal` props specified in Task 11's
  `DayHeaderProps` were removed in Task 12 rather than kept unused. Task 12
  Step 5 makes the two handlers call `invoke` directly inside `DayHeader`, so
  the props had no reader and `noUnusedLocals` would flag them. The behaviour
  is exactly as the plan specifies; only the vestigial props are gone.
- Plan A Task 13: the TypeScript `Settings` and `Engine` types live in
  `src/lib/days.ts`, because Plan B Task 9 imports the settings type from
  `src/lib/days.ts` and no such file existed in 0.2.0.
- Plan B Task 7 / Plan C contracts: `Engine` is re-exported from `engine.rs`
  (`pub use crate::settings::Engine`) and `engine::run(engine, stdin) ->
  RunOutput` was implemented as a wrapper over the 0.2.0 three-argument
  `run_with_env` (which the engine test command and the scheduler still use
  for their explicit environment capture). Plan B's preamble declares that
  two-argument signature as its contract while Plan A built `Engine` inside
  settings.rs; the wrapper reconciles them without renaming Plan A's API.
- Plan C Task 3: the 0.2.0 plan never built the `JobQueue` that Plan C's
  contract assumes (`enqueue_summarise(date) -> JobId`, `status(JobId)`,
  history). A `JobQueue` with `push`, `drain` and `record` was added to
  `jobs.rs` and drained by the existing tick so queued runs stay serial and
  off the capture thread; `JobState` still carries the tray's last-outcome
  line.
- Plan C Task 3: `open_main_window_on(app, date)` did not exist (0.3.0 built
  only `open_main_window`). It was added as the existing opener plus an
  `open-day` event to the main window; `DayView` listens and selects the
  date.
- Plan C Task 3: `apply_settings_change(app, previous, next)` did not exist
  despite Plan C calling it "the 0.3.0 function the Settings UI already
  calls". It was written into `lib.rs`: capture restarts with the new
  recording knobs when it is running and a recording-relevant field changed.
- Plan C Task 4: `rules::new_id` takes the rule set as an argument (Plan C's
  contract lists a no-argument `new_id()`); the socket write handler loads
  the current set and passes it, matching the 0.3.0 signature.

## Deviations from the plan
Anything you did differently from the written step, with the reason. Empty is the goal.

- (0.2.0) Plan A Task 11 Steps 1-2 and 7-8 were not executed: reference
  screenshots, design-skill invocation and screenshot judgement are visual
  work the run cannot see. All components, states and empty states were built,
  compile, and render. Logged under "Needs visual pass".
- (0.2.0) Plan A Task 13 Steps 8-11 (dev-run verification against a real CLI,
  System Settings login items, end-to-end summary) were not executed; the
  mechanical parts are done. Logged under "Needs human verification".
- (0.3.0) Plan B Task 7: the plan's tests assert the ledger renders dispositions
  capitalised ("Accepted", "Applied", "Discarded", "Failed", "Rejected"), but
  0.2.0's committed renderer and tests establish lowercase ("accepted",
  "rejected:", "failed:"). The assertions were matched to the committed
  lowercase render; the assertion still verifies the disposition is recorded.
- (0.3.0) Plan B Tasks 9 and 10 Steps calling for reference screenshots, design
  skills and screenshot judgement were not executed (visual work); the
  components, states and wiring are all built and compiling.
- (0.3.0) Plan B Task 10 Step 7 (ten measured proposals against a real engine)
  was not executed; logged under "Needs human verification".
- (0.4.0) Plan C Tasks 3 and 4 share one commit (77febad). Task 3's
  `control::handle` match arms call `control::writes::set_config` and friends,
  which are Task 4's deliverable, so the write handlers, `SETTABLE_KEYS` and
  `apply_patch` had to exist for Task 3 to compile. Task 4's eight patch-merge
  tests are in the same commit and pass; the plan's "watch the tests fail"
  step for Task 4 could not be run because the implementation preceded the
  tests by necessity.
- (0.4.0) Plan C Task 3: the plan's `Request::SummariseDay` test serialises
  only `date`, but the declared variant carries `client` as well, so the
  plan's test as written does not compile against the plan's own type. The
  test was fixed to carry both fields.
- (0.4.0) Plan C Task 5: the plan's search fixture contains
  `url: https://www.postgresql.org/docs`, whose host matches the "POSTGRES"
  query, giving three hits against the test's expected two. The fixture URL
  was changed to `example.org` so the intended one-day-hit, one-summary-hit
  shape holds.
- (0.4.0) Plan C Task 10: the plan's third integration test calls
  `ChildStdin::try_clone`, which does not exist; the test was restructured to
  move the real stdin handle out of the session and drop it, which closes the
  pipe and ends the read loop. The assertion is unchanged.
- (0.4.0) Plan C Task 11 Steps 7-8 (visual pass on the registration section,
  pasting the five blocks into real clients) were not executed; logged under
  the respective sections.

## Needs visual pass
| Component | Path | States implemented |
|---|---|---|
| Calendar rail | src/components/CalendarRail.tsx | month grid, capture dot, summary ring, today vs selected, month arrows, empty legend |
| Day header | src/components/DayHeader.tsx | date + arrows + Today, stats, summary state (none/generated/failed with stderr disclosure), Raw / Summary segmented control, Summarise/Regenerate, Open in editor, Reveal in Finder, action error line |
| Summary pane | src/components/SummaryPane.tsx | summary rendered (hand-rolled renderer: hidden frontmatter, h1/h2, paragraphs, lists), four empty states: no capture, no engine, engine + no summary, running |
| Day view shell | src/components/DayView.tsx | owns selected date, today default, arrow-key navigation, 5s live refresh for today, Raw default for today / Summary for a summarised past day |
| Engine settings | src/components/EngineSettings.tsx | explanation, engine picker with auth states, manual template, test button (testing/ok/failed), schedule with backfill note, launch at login, prompt display with revert |
| Raw pane | src/components/RawPane.tsx | block timeline (time, app, title, file/url references), collapsed bodies with a count, live 5-second refresh for today with held scroll position, quiet blocks explained, the three rule actions with in-place confirmation, duplicate-rule errors shown verbatim |
| Rules settings | src/components/RulesSettings.tsx | user rules (add, edit action in place, remove), built-ins rendered locked with the "cannot be changed" line, validation errors shown verbatim |
| Recording settings | src/components/RecordingSettings.tsx | six controls with current values, "changes what is recorded from now on" note, no-restart note |
| Highlight pill | src/components/HighlightPill.tsx | selection pill with three verbs, disabled-with-reason when no engine, copied confirmation, Escape and outside-click dismissal |
| Propose popover | src/components/ProposePopover.tsx | quoted selection, instruction field, engine name, running state that stays open, failure with engine output behind a disclosure |
| Diff view | src/components/DiffView.tsx | reasoning above, whole-file diff with prefix-plus-colour marks, Discard and Apply below with the "nothing written yet" line |
| MCP registration | src/components/McpSettings.tsx | five client blocks behind a picker (Claude Code, Claude Desktop, Cursor, Zed, generic JSON), copy button with confirmation, status line with last MCP write |

## Needs human verification
| Plan | Task | What it proves |
|---|---|---|
| 0.2.0 | Task 10 Step 13 | Browse Days opens a focusable resizable window in Cmd-Tab, `list_days` invoke returns an array, closing the window leaves the app running with no Dock icon |
| 0.2.0 | Task 13 Steps 8-11 | engine test failure and success paths in the real window, launch at login appearing in System Settings Login Items, a real summary end to end with tray status update |
| 0.2.0 | Task 14 (all) | prove-it: a week unattended, sleep/reboot catch-up, capture unaffected, summaries judged against memory, ledger hashes checked |
| 0.3.0 | Task 10 Step 7 | highlight-to-instruct reliability: ten real proposals against the real engine, counting first-try, retry and double-failure |
| 0.3.0 | Task 13 (all) | prove-it: exclusion and headings-only suppression observed in real day files, rule and prompt round-trips through highlight-to-instruct, every action in the ledger |
| 0.4.0 | Task 10 Step 5 | registration with a real client (`claude mcp add`) and `list_days` called with the app closed |
| 0.4.0 | Task 11 Steps 7-8 | the registration section reads well beside Engine and Rules in light and dark; the five blocks register successfully in real clients |
| 0.4.0 | Task 13 (all) | prove-it: the parity audit walk, a full Claude Code session over the socket, ledger entries naming the client, the eight reads with the app quit, and the renumber to 1.0.0 |

## Test evidence
(pasted at the end of each plan)

### Plan A (0.2.0), run at ea0f47d
```
test result: ok. 150 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
✓ built in 386ms
```

### Plan B (0.3.0), run at 4ea15b6
```
test result: ok. 226 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.42s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 218 filtered out; finished in 0.00s   (settings backward-compat re-run)
✓ built in 413ms
```

### Plan C (0.4.0), final pass on build/0.4.0 at 4d6def0
```
cd src-tauri && cargo test:
test result: ok. 298 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.26s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s
npm run build:
✓ built in 381ms
```
303 tests total across the lib and two integration suites; zero failures, zero
ignored (the two `tools/call` dispatch tests deferred in Task 6 were unignored
in Task 9 as planned).
