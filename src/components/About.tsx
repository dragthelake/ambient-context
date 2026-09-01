import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CLOSE_GLYPH, PixelGlyph } from "./PixelGlyph";
import appIcon from "../assets/app-icon.png";

function closeWindow() {
  void getCurrentWindow().close();
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
        <button
          type="button"
          className="titlebar-button"
          aria-label="Close about"
          onClick={closeWindow}
        >
          <PixelGlyph pattern={CLOSE_GLYPH} />
        </button>
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
          A written record of what you worked on. One markdown file per day,
          on your computer.
        </p>
        <p className="about-line">
          Nothing is sent anywhere. There is no account and no server.
        </p>

        <p className="credit">
          Built by{" "}
          <button
            type="button"
            className="credit-link"
            onClick={() =>
              void invoke("open_link", {
                url: "https://twitter.com/cameronsmith",
              })
            }
          >
            Cameron Smith
          </button>
        </p>
      </div>
    </main>
  );
}
