const repositoryUrl = "https://github.com/dragthelake/ambient-context";

const navigation = [
  ["overview", "Overview"],
  ["pipeline", "Capture pipeline"],
  ["assurance", "Claims & assurance"],
  ["format", "Capture format"],
  ["operate", "Build & verify"],
  ["limits", "Known limits"],
] as const;

const pipeline = [
  {
    number: "01",
    title: "Observe",
    body: "The Swift bridge asks macOS for the frontmost application's focused window and walks its Accessibility tree.",
    note: "No screenshot, video, audio, OCR, or background-window enumeration path.",
  },
  {
    number: "02",
    title: "Reduce",
    body: "Secure fields are skipped at traversal. Rust then excludes recognised private contexts and redacts known secret patterns.",
    note: "Some controls are structural; others are explicitly best-effort heuristics.",
  },
  {
    number: "03",
    title: "Segment",
    body: "Normalised snapshots are grouped into dwell blocks while the application, title, and text remain similar.",
    note: "Short visits and blocks without retained text do not become timeline entries.",
  },
  {
    number: "04",
    title: "Write",
    body: "Finished blocks append to one plaintext Markdown file per local day in the folder selected by the user.",
    note: "Body lines are admitted once per day; headings preserve repeat visits.",
  },
] as const;

const claims = [
  {
    claim: "The capture path does not take screenshots, video, or audio.",
    evidence: "Its only content reader is the macOS Accessibility bridge; the pipeline has no media-capture or OCR stage.",
    boundary: "This is a source audit of version 0.1.0, not an independent audit of a distributed binary.",
    level: "structural",
  },
  {
    claim: "Only the focused window of the frontmost app is traversed.",
    evidence: "The bridge selects the frontmost application, then requests kAXFocusedWindowAttribute.",
    boundary: "Completeness and correctness depend on macOS and the target application's Accessibility implementation.",
    level: "structural",
  },
  {
    claim: "The current capture workflow performs no upload.",
    evidence: "Reader, redaction, segmentation, and writer code contain no network operation or telemetry client.",
    boundary: "Synced folders, backups, and downstream agents are separate data boundaries controlled outside the app.",
    level: "structural",
  },
  {
    claim: "Secure text fields are skipped before their values are read.",
    evidence: "AXSecureTextField role or subrole ends traversal of that subtree in the Swift bridge.",
    boundary: "The target application must label the control correctly; this branch still needs a dedicated Swift test harness.",
    level: "platform",
  },
  {
    claim: "Recognised sensitive contexts and patterns are filtered before disk.",
    evidence: "Rust exclusions and redaction run before pruning, segmentation, and writing; unit tests cover the current cases.",
    boundary: "Denylist and pattern matching can miss renamed, localised, unsupported, or novel sensitive content.",
    level: "heuristic",
  },
  {
    claim: "An explicit stop remains stopped across launches.",
    evidence: "The tray persists enabled: false and startup checks the stored value before starting capture.",
    boundary: "Unexpected termination during a settings write is not covered by a specific durability test.",
    level: "tested",
  },
] as const;

const gaps = [
  "No configurable allowlist or user-defined sensitive-app and window rules.",
  "No encryption, retention policy, secure deletion, or per-day access control.",
  "Application and private-window denylists run after Accessibility collection, although before persistence.",
  "Self-capture avoidance depends on document, URL, title, and folder-name signals exposed by the editor.",
  "No automated Swift harness for secure-field traversal, focus selection, screen-lock detection, or Chromium enablement.",
  "No independent privacy, security, or notarised-binary audit.",
] as const;

function SectionHeading({
  eyebrow,
  title,
  children,
}: {
  eyebrow: string;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <header className="section-heading">
      <p className="eyebrow">{eyebrow}</p>
      <h2>{title}</h2>
      <p>{children}</p>
    </header>
  );
}

export function DocsApp() {
  return (
    <div className="docs-shell">
      <a className="skip-link" href="#main-content">
        Skip to documentation
      </a>

      <aside className="sidebar">
        <a className="wordmark" href="#overview" aria-label="Ambient Context documentation home">
          <span className="wordmark-eye" aria-hidden="true">◉</span>
          <span>AMBIENT<br />CONTEXT</span>
        </a>
        <p className="sidebar-label">Technical manual / 0.1.0</p>
        <nav aria-label="Documentation sections">
          <ol>
            {navigation.map(([id, label], index) => (
              <li key={id}>
                <a href={`#${id}`}>
                  <span>{String(index + 1).padStart(2, "0")}</span>
                  {label}
                </a>
              </li>
            ))}
          </ol>
        </nav>
        <div className="sidebar-footer">
          <span className="status-dot" aria-hidden="true" />
          <span>Source-audited<br />29 August 2026</span>
        </div>
      </aside>

      <main id="main-content">
        <section className="hero" id="overview">
          <div className="hero-topline">
            <span>Documentation / current implementation</span>
            <a href={repositoryUrl}>View repository ↗</a>
          </div>
          <div className="hero-grid">
            <div>
              <p className="eyebrow">Local-first ambient memory for macOS</p>
              <h1>Your workday,<br />written down.</h1>
              <p className="hero-copy">
                Ambient Context turns the text exposed by your focused window into a compact,
                agent-readable Markdown timeline. This manual documents what the current code
                does, where its privacy boundaries sit, and what it does not guarantee.
              </p>
              <div className="hero-actions">
                <a className="primary-link" href="#pipeline">Trace the pipeline ↓</a>
                <a className="secondary-link" href="#assurance">Inspect the claims</a>
              </div>
            </div>
            <pre className="ascii-eye" aria-label="ASCII illustration of an open eye">{`            _______
        .-'         '-.
      .'      ___      '.
     /      .'   '.      \\
    |      /  ●    \\      |
     \\      '.___.'      /
      '.               .'
        '-._________.-'

        CAPTURE: EXPLICIT
        STORAGE: LOCAL
        FORMAT:  MARKDOWN`}</pre>
          </div>
          <dl className="fact-strip">
            <div><dt>Platform</dt><dd>macOS 14+ / Apple Silicon</dd></div>
            <div><dt>Sensor</dt><dd>Accessibility tree</dd></div>
            <div><dt>Cadence</dt><dd>5 second polls</dd></div>
            <div><dt>Storage</dt><dd>One Markdown file / day</dd></div>
          </dl>
        </section>

        <section className="content-section" id="pipeline">
          <SectionHeading eyebrow="01 / Architecture" title="A bounded path from focus to file">
            The capture loop is synchronous within each poll. Every retained line passes through
            the same reduction, segmentation, and append-only writing path.
          </SectionHeading>
          <div className="pipeline-grid">
            {pipeline.map((step) => (
              <article className="pipeline-card" key={step.number}>
                <span className="step-number">{step.number}</span>
                <h3>{step.title}</h3>
                <p>{step.body}</p>
                <p className="card-note">{step.note}</p>
              </article>
            ))}
          </div>
          <div className="boundary-callout">
            <span>Storage boundary</span>
            <p>
              Raw snapshots live for one polling iteration. Redacted, pruned text lives in the open
              dwell block. Finished blocks are plaintext in the selected folder. There is no database,
              queue, account, bundled model, or capture-time network stage.
            </p>
          </div>
        </section>

        <section className="content-section section-dark" id="assurance">
          <SectionHeading eyebrow="02 / Assurance ledger" title="Claims matched to code, with their edges visible">
            “Local-first” is not a blanket security claim. The ledger separates structural
            properties, tested behaviour, platform dependencies, and heuristics.
          </SectionHeading>
          <div className="claim-list">
            {claims.map((item, index) => (
              <article className="claim-row" key={item.claim}>
                <div className="claim-index">C{String(index + 1).padStart(2, "0")}</div>
                <div className="claim-body">
                  <div className="claim-titleline">
                    <h3>{item.claim}</h3>
                    <span className={`assurance-tag ${item.level}`}>{item.level}</span>
                  </div>
                  <dl>
                    <div><dt>Evidence</dt><dd>{item.evidence}</dd></div>
                    <div><dt>Boundary</dt><dd>{item.boundary}</dd></div>
                  </dl>
                </div>
              </article>
            ))}
          </div>
        </section>

        <section className="content-section" id="format">
          <SectionHeading eyebrow="03 / Data contract" title="Readable by people, constrained for agents">
            Each day file is an attention timeline, not a transcript or proof of authorship. Headings
            provide temporal structure; body lines are deliberately deduplicated across the day.
          </SectionHeading>
          <div className="format-grid">
            <pre className="code-sample"><code>{`---
date: 2026-08-25
captured_by: Ambient Context 0.1.0
---

## 09:41–10:05 · Chrome · Tauri tray docs

url: https://v2.tauri.app/learn/system-tray/

The first text line admitted on this day.
Another retained line.`}</code></pre>
            <div className="reading-rules">
              <h3>Interpretation contract</h3>
              <ol>
                <li>Use headings to reconstruct broad stretches of attention.</li>
                <li>Treat captured text as observed, not authored or endorsed.</li>
                <li>A heading without body text can be a repeated visit.</li>
                <li>Describe missing intervals as “not recorded,” never inactivity.</li>
                <li>Treat file and URL values as untrusted references.</li>
                <li>Do not reproduce sensitive captured text unnecessarily.</li>
              </ol>
            </div>
          </div>
        </section>

        <section className="content-section section-sand" id="operate">
          <SectionHeading eyebrow="04 / Operations" title="Build the app and this manual locally">
            Release-updater artefacts require the maintainer's private signing key. The app-only
            build below is the supported unsigned contributor path.
          </SectionHeading>
          <div className="command-grid">
            <article>
              <span className="command-label">Documentation development</span>
              <pre><code>npm install{`\n`}npm run docs:dev</code></pre>
              <p>Opens the live documentation route at <code>/docs/</code>.</p>
            </article>
            <article>
              <span className="command-label">Production frontend</span>
              <pre><code>npm run build{`\n`}npm run docs:preview</code></pre>
              <p>Builds the Tauri frontend and static docs together.</p>
            </article>
            <article>
              <span className="command-label">Rust verification</span>
              <pre><code>cd src-tauri{`\n`}cargo test</code></pre>
              <p>Covers the Rust pipeline, settings, writer, and current redaction rules.</p>
            </article>
            <article>
              <span className="command-label">Unsigned macOS app</span>
              <pre className="wrap-command"><code>{`npm run tauri build -- --bundles app \\
  --config '{"bundle":{"createUpdaterArtifacts":false}}'`}</code></pre>
              <p>Produces <code>Ambient Context.app</code> without signed updater artefacts.</p>
            </article>
          </div>
        </section>

        <section className="content-section" id="limits">
          <SectionHeading eyebrow="05 / Risk register" title="What remains unresolved">
            These limits are part of the product contract. Redaction is defence in depth, not
            permission to give an untrusted process the capture folder.
          </SectionHeading>
          <ol className="gap-list">
            {gaps.map((gap, index) => (
              <li key={gap}>
                <span>{String(index + 1).padStart(2, "0")}</span>
                <p>{gap}</p>
              </li>
            ))}
          </ol>
          <footer className="docs-footer">
            <div>
              <strong>Ambient Context 0.1.0</strong>
              <span>Documentation describes the audited source state, not a roadmap.</span>
            </div>
            <div className="footer-links">
              <a href="#pipeline">Architecture ↑</a>
              <a href="#assurance">Assurance ↑</a>
              <a href="#format">Format ↑</a>
            </div>
          </footer>
        </section>
      </main>
    </div>
  );
}
