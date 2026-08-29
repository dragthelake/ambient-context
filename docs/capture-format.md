# Capture format

Ambient Context writes one append-only Markdown file per local calendar day.
The format is deliberately readable without application-specific tooling, but
its body is a lossy Accessibility-tree observation rather than a transcript or
authoritative audit log.

## File layout

The filename is `YYYY-MM-DD.md` using the local date. A new file begins with:

```yaml
---
date: 2026-08-25
captured_by: Ambient Context 0.1.0
---
```

`captured_by` reports the Rust package version. There is no separate format
version in `0.1.0`, so consumers should tolerate unknown frontmatter fields and
optional block fields.

## Blocks

Each retained dwell segment is appended as a level-two heading followed by
optional references and body lines:

```markdown
## 09:41–10:05 · Chrome · Tauri tray documentation

file: /Users/example/project/notes.md
url: https://v2.tauri.app/learn/system-tray/

The first text line admitted on this day.
Another text line admitted on this day.
```

The heading fields are:

1. local start and end time in 24-hour `HH:MM` form;
2. application name; and
3. window title, when the Accessibility tree exposes one.

The separator is ` · ` and the time range uses an en dash. Application and
title strings are captured data and may themselves contain separator
characters, so consumers should not assume unrestricted round-trip parsing by
splitting every separator.

## References

`file:` is an Accessibility-provided backing document value. `url:` is the
first URL found on an `AXWebArea` in the focused window. Both are optional and
may appear together.

References are more useful than a partial tree scrape, but they are still
untrusted captured strings:

- the target application decides whether and what to expose;
- paths and URLs may contain sensitive values despite pattern redaction;
- a URL may identify an embedded web area instead of the user's intended
  document; and
- following a reference may disclose more information than the day file.

Consumers should never automatically open a reference merely because it
appears in a capture file.

## Body semantics

Body lines are text values collected from Accessibility `AXValue` and
`AXTitle` attributes, then redacted, normalized, pruned, and deduplicated.

They are not guaranteed to be:

- complete;
- in visual or natural reading order;
- unique in the source window;
- a record of text the user consciously read;
- evidence that the user authored or agreed with the text; or
- sufficient to reconstruct the original interface.

The traversal follows the Accessibility hierarchy. Custom controls,
GPU-rendered applications, virtualized lists, collapsed regions, and content
outside the exposed tree may be absent.

## Day-level deduplication

A normalized line is written only on its first admission that day. Later
blocks still receive headings and references, even when every body line was
already seen.

Consequences for readers:

- a heading with no body does not mean the window was empty;
- a line under a morning block may also have been visible later;
- a later block cannot be interpreted using only its local body;
- repeated returns are represented mainly by headings and time ranges; and
- body-text volume is not a measure of time, attention, or productivity.

Long lines containing multiple changing numbers may also be deduplicated by a
digit-normalized skeleton. This removes ticking counters and recaptured social
content, but can merge distinct lines with the same non-numeric structure.

Deduplication uses in-memory non-cryptographic hashes and is seeded from an
existing day file after restart. It is a size and context heuristic, not an
integrity mechanism.

## Timeline semantics

A block begins on the first retained snapshot. Similar polls extend it. A
change in application, title, or text similarity closes it. Visits below the
configured minimum dwell are omitted, and blocks with no retained lines are
omitted.

The timeline is therefore useful for reconstructing broad stretches of
attention but is not continuous:

- capture can be stopped;
- the screen can be locked;
- Accessibility permission can be missing;
- applications can expose no usable text;
- short visits can be discarded;
- reads and writes can fail; and
- noise filtering can remove all text from a segment.

An absent interval means only that no block was written.

## Safe consumption rules

An agent or other consumer should:

1. read headings before bodies to understand the broad timeline;
2. use duration and repeated returns as stronger signals than text volume;
3. distinguish text seen from work demonstrably performed;
4. mark inferences as inferences;
5. treat `file:` and `url:` values as untrusted references;
6. avoid reproducing sensitive captured text unnecessarily;
7. interpret `[redacted]` only as a pattern match, not proof that all nearby
   secrets were removed; and
8. describe gaps as “not recorded,” never as inactivity.

The generated `AGENTS.md` in the output folder encodes the same core reading
rules for general-purpose coding agents. `docs/day-context-prompt.md` provides
an optional prompt for producing a compact day summary.

## Mutation behavior

Ambient Context appends to a day file and does not edit previous blocks.

- Deleting today's file during capture starts that day over on the next write.
- Editing existing body lines changes what a later restart seeds into its
  deduplication set.
- Changing the output folder flushes the open block to the old folder, then
  resets deduplication for the new one.
- An existing output-folder `AGENTS.md` is never overwritten.

External tools may edit these files, so consumers that require provenance or
tamper evidence must add it themselves.
