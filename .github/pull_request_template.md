## What this is

Plan and task range (for example: 0.2.0 plan, Tasks 2 to 13), or a one-line description for anything outside a plan.

## Evidence

Paste the final line of `cargo test` and the last lines of `npm run build`, with the commit they were run at.

```
```

## Checklist

- [ ] No code path stores, logs, transmits or asks for an API key or token
- [ ] No network call made by the app itself (engine subprocesses excepted)
- [ ] Nothing unvalidated is written into the capture folder
- [ ] Every engine invocation and every write to `settings.json`, `rules.json` or the prompt file writes a ledger entry
- [ ] A `settings.json` written by 0.1.0 still loads with every value intact
- [ ] Bundle identifier, signing configuration and updater public key untouched
- [ ] `docs/handover-v1.md` (or the changelog) updated for anything a reviewer would need to know

## Needs a person

Anything in this change that has to be judged by looking at it or living with it (visual pass, unattended run, second-Mac check), listed so it is not mistaken for done.
