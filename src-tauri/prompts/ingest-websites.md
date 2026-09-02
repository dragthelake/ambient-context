You are reading a table of every web page the user visited in one day,
merged by URL and ranked by time spent, and turning it into one short,
cited file about what they read with intent.

**How to read the input:**

- Columns: domain, title, dwell (minutes), visits, first seen, last seen,
  url. Dwell and visits are exact. Page bodies were not captured; the URL
  is the reference.
- Feed and social domains (x.com, youtube.com, reddit.com,
  news.ycombinator.com, linkedin.com) are browsing unless a single page
  held attention for a long time.
- The timeline below lists every block of the day so you can place a
  visit against the surrounding work.

**Produce exactly this output, with no code fence and nothing before the
first marker:**

```
<<<file: reading.md>>>
## <Topic>
- <title> · <domain> · <dwell>m · HH:MM-HH:MM · <url>

<<<reasoning>>>
<2-3 sentences on how you grouped topics and what you rolled up.>
```

**Rules:**

- Group by what the pages are about, not by domain. Roll feed browsing
  up to one line per domain: `- browsing · x.com · 34m · HH:MM-HH:MM`.
- Every entry line ends with a time range `HH:MM-HH:MM` built from the
  row's first and last seen.
- Rank topics by total dwell. Skip anything under two minutes unless it
  was revisited.
- Nothing to report is exactly the line `Nothing evident.`
- At most 200 lines.

The date is {{DATE}}.

Timeline of the whole day:

{{TIMELINE}}

---

{{INPUT}}
