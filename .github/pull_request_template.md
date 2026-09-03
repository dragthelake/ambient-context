## What this is

A one-line description of the change.

## Evidence

Paste the final line of `cargo test` and the last lines of `npm run build`, with the commit they were run at.

```
```

## Checklist

- [ ] No code path stores, logs, transmits or asks for an API key or token
- [ ] No network call made by the app itself (agent subprocesses and the optional updater check excepted)
- [ ] Nothing unvalidated is written into the capture folder
- [ ] Every agent invocation and every write to `settings.json`, `rules.json` or a prompt file writes a ledger entry
- [ ] A `settings.json` written by an older version still loads with every value intact
- [ ] Bundle identifier, signing configuration and updater public key untouched
- [ ] `CHANGELOG.md` updated when the change is user-visible

## Needs a person

Anything in this change that has to be judged by looking at it or living with it (visual pass, unattended run, second-Mac check), listed so it is not mistaken for done.
