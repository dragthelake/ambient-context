# Coverage census

Record one row per application. Focus it with real content on screen, then use **Sample focused window** in Setup. Chromium accessibility is enabled automatically on first contact (AXManualAccessibility for Electron, AXEnhancedUserInterface for Chrome; the dev log records which one each app accepted), and the tree takes a few seconds to build, so sample twice for Chromium apps and record the second read.

Mark each **rich**, **partial** or **empty**.

| App | Title | Fragments | Chars | Walk ms | Sample | Verdict | Attribute accepted |
|---|---|---|---|---|---|---|---|
| Safari | | | | | | | |
| Chrome | | | | | | | |
| Slack | | | | | | | |
| Linear | | | | | | | |
| Obsidian | | | | | | | |
| Visual Studio Code | | | | | | | |
| Figma | | | | | | | |
| Mail | | | | | | | |
| Messages | | | | | | | |
| Terminal | | | | | | | |
| iTerm2 | | | | | | | |
| Notes | | | | | | | |
| Preview | | | | | | | |
| Eagle | | | | | | | |

## Chromium cost

Ten minutes per condition, from Activity Monitor.

| App | Condition | CPU % | Resident memory |
|---|---|---|---|
| Chrome | accessibility off, idle | | |
| Chrome | accessibility off, scrolling | | |
| Chrome | accessibility on, idle | | |
| Chrome | accessibility on, scrolling | | |
| Slack | accessibility off, idle | | |
| Slack | accessibility off, scrolling | | |
| Slack | accessibility on, idle | | |
| Slack | accessibility on, scrolling | | |

## Verdict

What proportion of an ordinary working day is legible this way, and at what cost?

_Not yet measured._
