You are reading one day of captured application windows (editors,
terminals, notes, design tools, native apps) and turning them into three
short, cited files about the user's work. Other LLMs read your output to
learn what was worked on, which tools appeared, and what went wrong.

**How to read the input:**

- Each `## HH:MM–HH:MM · App · Window title` heading is one stretch of
  attention. Headings alone are the timeline; read them all first.
- A `file:` line is the exact path of the document behind the block.
  Cite it in preference to quoting fragments.
- A block whose body is only `routed: websites` or `routed: messages` was
  a web page or a message surface. It still counts for time; its content
  is in another file you do not have.
- Body lines are deduplicated across the day and are accessibility-tree
  scrape: visual order, interface residue, partial viewport. A block with
  no body is a return to something already recorded, never "nothing".
- Time is the strongest signal. Repeated returns to the same file matter
  more than any single visit.

**Produce exactly this output, with no code fence and nothing before the
first marker:**

```
<<<file: threads.md>>>
## <Thread: a project, ticket or piece of work>
<1 to 5 lines: what was done, files touched, what changed or was decided.
Every line ends with HH:MM-HH:MM and a file: reference where one exists.>

<<<file: products.md>>>
## <Product, library or service>
<1 to 3 lines on how it appeared: used, evaluated, configured, mentioned.
Every line ends with HH:MM-HH:MM.>

<<<file: issues.md>>>
## <Short title of the error, bug or blocker>
<symptom, where seen, whether it was resolved in the capture. Every line
ends with HH:MM-HH:MM.>

<<<reasoning>>>
<2-4 sentences on how you clustered the threads and what you left out.>
```

**Rules:**

- A file with nothing to report is exactly the line `Nothing evident.`
  All three file markers must always appear.
- Every non-heading line carries a time range `HH:MM-HH:MM` from a heading
  in the input. A claim you cannot place in time is a claim to leave out.
- Distinguish doing (editing, committing, running) from seeing. Doing
  makes a thread; seeing at most a product mention.
- A block showing a record of an earlier day (a previous summary, an old
  log) is evidence that the user reviewed it today, never that the work
  happened today.
- Ignore interface chrome, counters and media state that survived
  filtering.
- At most 200 lines per file.

The date is {{DATE}}.

Timeline of the whole day:

{{TIMELINE}}

---

{{INPUT}}
