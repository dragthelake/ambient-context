# Reporting a bug

Open an issue at <https://github.com/dragthelake/ambient-context/issues>.

## What to include

- **Version.** Open the tray menu; the first, greyed line is the version.
- **What you did and what you expected.** One or two sentences each.
- **The ledger entry**, if the problem is a summary, an ingest, a rule change or a setting change. Open the capture folder, then `Ledger/YYYY-MM-DD.md` for the day it happened, and paste the entry (each starts with `## HH:MM:SS · action`). It names the prompt, the agent, the inputs by hash, and how the run ended.
- **The rejected output**, if the ledger says `rejected`. It is kept at `~/Library/Application Support/com.0x0000007a.ambientcontext/rejected/`, named by date (and by call for ingest). Read it before attaching it; it is the model's answer about your day.
- **Console lines**, if the app misbehaved rather than a run. `Console.app`, filter on `ambient-context`. Lines are prefixed `[capture]`, `[ax]`, `[writer]`, `[redact]`, `[jobs]`.

## What not to include

- Anything from `Days/`, `KB/` or `Summaries/`. Those are your record. If a bug needs a sample of one, make a short one on purpose: start capture, do the thing in a window with nothing private in it, stop capture, and attach that block only.
- `settings.json` in full. Name the setting that matters.

## What happens next

Issues are read by one person. A reproducible capture or ledger entry gets fixed first; a description without either gets a question back.
