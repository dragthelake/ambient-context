import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import appIcon from "../assets/app-icon.png";
import { GITHUB_NEW_ISSUE, GITHUB_REPO } from "../lib/github";

function closeWindow() {
  void getCurrentWindow().close();
}

function open(url: string) {
  void invoke("open_link", { url });
}

export function About() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    void invoke<string>("app_version").then(setVersion);
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeWindow();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <main className="window about-window">
      <div className="titlebar" data-tauri-drag-region>
        <span className="titlebar-text" data-tauri-drag-region>
          ABOUT
        </span>
        <div className="titlebar-controls">
          <button type="button" aria-label="Close" onClick={closeWindow} />
        </div>
      </div>

      <div className="window-body about-body">
        {/* The app icon rather than the live eye: this window is a
            colophon, not a status display, and a second moving indicator
            would only compete with the one on the Overview tab. */}
        <img className="about-icon" src={appIcon} alt="" width={128} height={128} />

        <h1 className="about-name">Ambient Context</h1>
        <p className="about-version">
          {version ? `Version ${version}` : " "}
        </p>

        <p className="about-line">
          A macOS menu bar app that keeps a written record of what you work
          on, for your own LLM to read. While the eye in your menu bar is
          open, Ambient Context reads the text of whichever window you have
          focused (via the macOS accessibility tree, every few seconds) and
          appends it to plain markdown files in a folder you choose. Point
          Claude Code or any other agent at that folder and it can answer
          &ldquo;what did I work on Tuesday?&rdquo;, build memory about your
          projects, or write your standup for you.
        </p>

        <section className="about-github" aria-label="Source and feedback">
          <p className="about-line">
            Ambient Context is open source. If it is useful, a star on GitHub
            helps others find it. Bugs, rough edges and ideas are welcome as
            issues.
          </p>
          <div className="about-actions">
            <button type="button" onClick={() => open(GITHUB_REPO)}>
              Star on GitHub
            </button>
            <button type="button" onClick={() => open(GITHUB_NEW_ISSUE)}>
              Report a bug
            </button>
          </div>
        </section>
      </div>

      <footer className="about-credit">
        <span className="about-credit-label">Built by</span>
        <button
          type="button"
          className="about-credit-name"
          onClick={() => open("https://twitter.com/cameronsmith")}
        >
          Cameron Smith
        </button>
      </footer>
    </main>
  );
}
