import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { DayView } from "./DayView";
import { EngineSettings } from "./EngineSettings";
import { McpSettings } from "./McpSettings";
import { Overview } from "./Overview";
import { CLOSE_GLYPH, HELP_GLYPH, PixelGlyph } from "./PixelGlyph";
import { PromptSettings } from "./PromptSettings";
import { RecordingSettings } from "./RecordingSettings";
import { RulesSettings } from "./RulesSettings";
import { SoundSettings } from "./SoundSettings";
import { applySoundSettings, bind, play } from "../lib/sound";
import { useAppStatus } from "../lib/status";
import type { Settings } from "../lib/days";
import "../main-window.css";

const TABS = [
  { id: "overview", label: "Overview" },
  { id: "context", label: "Context" },
  { id: "settings", label: "Settings" },
] as const;

type Tab = (typeof TABS)[number]["id"];

export function Main() {
  const [tab, setTab] = useState<Tab>("overview");
  const status = useAppStatus();

  // Wire up the declarative cues once, then hand the engine the user's
  // saved preferences. Both are read-once: the Settings tab applies its
  // own changes as they are made.
  useEffect(() => {
    bind();
    void invoke<Settings>("get_settings").then((saved) =>
      applySoundSettings(saved.sound_enabled, saved.sound_volume),
    );
  }, []);

  return (
    <div className="window main-window">
      <div className="titlebar" data-tauri-drag-region>
        <span className="titlebar-text" data-tauri-drag-region>
          AMBIENT_CONTEXT
        </span>
        {/* The native traffic lights are hidden on this window, so these
            are the only way out of it. Closing hides rather than quits:
            the app lives in the menu bar and keeps recording. */}
        <div className="titlebar-buttons">
          <button
            type="button"
            className="titlebar-button"
            aria-label="About Ambient Context"
            title="About Ambient Context"
            onClick={() => {
              play("chime");
              void invoke("open_about");
            }}
          >
            <PixelGlyph pattern={HELP_GLYPH} />
          </button>
          <button
            type="button"
            className="titlebar-button"
            aria-label="Close window"
            title="Close window"
            onClick={() => void getCurrentWindow().close()}
          >
            <PixelGlyph pattern={CLOSE_GLYPH} />
          </button>
        </div>
      </div>

      <div className="tabstrip" role="tablist" aria-label="Views">
        {TABS.map(({ id, label }) => (
          <button
            key={id}
            type="button"
            role="tab"
            id={`tab-${id}`}
            aria-controls={`pane-${id}`}
            aria-selected={tab === id}
            className={tab === id ? "tab is-current" : "tab"}
            onClick={() => {
              if (id !== tab) play("tick");
              setTab(id);
            }}
          >
            {label}
          </button>
        ))}
      </div>

      {/* One pane at a time, mounted only while its tab is chosen: the Day
          view and the settings panels each fetch on mount, and there is no
          reason for a hidden tab to be talking to the backend. */}
      <div
        className={`tabpane tabpane-${tab}`}
        role="tabpanel"
        id={`pane-${tab}`}
        aria-labelledby={`tab-${tab}`}
      >
        {/* The frame and the scrolling are separate elements on purpose.
            A scroll container's padding is inside its scrollable area, so
            content travels through it, and an inset box-shadow is painted
            under its descendants: put the bevel on the scroller and the
            content rides over the frame at both ends. The outer div holds
            the bevel and never scrolls; this one scrolls inside it. */}
        <div className="tabpane-scroll">
          {tab === "overview" && <Overview status={status} />}
          {tab === "context" && <DayView />}
          {tab === "settings" && (
            <div className="settings-stack">
              <EngineSettings />
              <RulesSettings />
              <PromptSettings />
              <RecordingSettings />
              <SoundSettings />
              <McpSettings />
            </div>
          )}
        </div>
      </div>

      <div className="statusbar">
        <span className="statusbar-cell">
          {status.capture.running ? "REC: ON" : "REC: OFF"}
        </span>
        <span className="statusbar-cell">
          {status.permission === "granted" ? "AX: GRANTED" : "AX: NOT GRANTED"}
        </span>
        <span className="statusbar-cell statusbar-cell-wide">
          {status.folder ?? "NO FOLDER SET"}
        </span>
      </div>
    </div>
  );
}
