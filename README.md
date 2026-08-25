# Ambient Context

A menu bar app that keeps a written record of what you worked on. While it is switched on, it reads the text of whichever window you have focused and appends it to a markdown file in a folder you chose. It takes no screenshots, sends nothing anywhere, and talks to no model.

macOS 14+, Apple Silicon.

## Run

```bash
npm install
npm run tauri dev
```

Left-click the menu bar icon to start and stop capturing. Right-click it for today's file, the folder, setup, and updates.

Capture is off on every launch. Nothing is written until you switch it on and choose a folder. Use a scratch folder while developing, not a real vault.

## Tests

```bash
cd src-tauri && cargo test
```

## Not yet done

These need you, not more code:

- Apple Developer enrolment and a Developer ID certificate (signing + notarisation)
- The coverage census in `docs/census.md`
- A first real-day reading of the output

The updater private key lives at `~/.tauri/ambient-context.key`. Back it up. Losing it orphans every installed copy.
