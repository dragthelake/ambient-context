# V1 build handover

Built by an unattended agent run on 2026-08-31 from the three written plans
(0.2.0, 0.3.0, 0.4.0), on stacked branches `build/0.2.0` -> `build/0.3.0` ->
`build/0.4.0`. Every task was committed after its gate (`cargo test`, plus
`npm run build` for tasks touching `src/`). Prove-it tasks (Plan A Task 14,
Plan B Task 13, Plan C Task 13) were not attempted and are listed under
"Needs human verification". Visual tasks were built and compiled but not
judged on screen; they are listed under "Needs visual pass".

## Status
| Plan | Branch | Tasks done | Tasks blocked | Last commit |
|---|---|---|---|---|
| 0.2.0 (A) | build/0.2.0 | 2-13 (11 of 13; 14 is prove-it) | none | ea0f47d |
| 0.3.0 (B) | build/0.3.0 | | | |
| 0.4.0 (C) | build/0.4.0 | | | |

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
- Plan A Task 8: `summarise_day` takes the template as a parameter and
  `run_one` reads `settings.day_prompt` falling back to `BUNDLED_PROMPT`,
  exactly as written. Plan B Task 4 Step 7 redirects this through
  `prompt::current`; that change happens on the 0.3.0 branch.

## Deviations from the plan
Anything you did differently from the written step, with the reason. Empty is the goal.

- (0.2.0) Plan A Task 11 Steps 1-2 and 7-8 were not executed: reference
  screenshots, design-skill invocation and screenshot judgement are visual
  work the run cannot see. All components, states and empty states were built,
  compile, and render. Logged under "Needs visual pass".
- (0.2.0) Plan A Task 13 Steps 8-11 (dev-run verification against a real CLI,
  System Settings login items, end-to-end summary) were not executed; the
  mechanical parts are done. Logged under "Needs human verification".

## Needs visual pass
| Component | Path | States implemented |
|---|---|---|
| Calendar rail | src/components/CalendarRail.tsx | month grid, capture dot, summary ring, today vs selected, month arrows, empty legend |
| Day header | src/components/DayHeader.tsx | date + arrows + Today, stats, summary state (none/generated/failed with stderr disclosure), Raw (disabled, "Coming in 0.3") / Summary segmented control, Summarise/Regenerate, Open in editor, Reveal in Finder, action error line |
| Summary pane | src/components/SummaryPane.tsx | summary rendered (hand-rolled renderer: hidden frontmatter, h1/h2, paragraphs, lists), four empty states: no capture, no engine, engine + no summary, running |
| Day view shell | src/components/DayView.tsx | owns selected date, today default, arrow-key navigation, 5s live refresh for today |
| Engine settings | src/components/EngineSettings.tsx | explanation, engine picker with auth states, manual template, test button (testing/ok/failed), schedule with backfill note, launch at login, prompt display with revert |

## Needs human verification
| Plan | Task | What it proves |
|---|---|---|
| 0.2.0 | Task 10 Step 13 | Browse Days opens a focusable resizable window in Cmd-Tab, `list_days` invoke returns an array, closing the window leaves the app running with no Dock icon |
| 0.2.0 | Task 13 Steps 8-11 | engine test failure and success paths in the real window, launch at login appearing in System Settings Login Items, a real summary end to end with tray status update |
| 0.2.0 | Task 14 (all) | prove-it: a week unattended, sleep/reboot catch-up, capture unaffected, summaries judged against memory, ledger hashes checked |

## Test evidence
(pasted at the end of each plan)
