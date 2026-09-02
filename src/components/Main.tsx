import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AgentTab } from "./AgentTab";
import { AppSettings } from "./AppSettings";
import { DayView } from "./DayView";
import { McpSettings } from "./McpSettings";
import { Overview } from "./Overview";
import { RecordingSettings } from "./RecordingSettings";
import { RulesSettings } from "./RulesSettings";
import { StorageSettings } from "./StorageSettings";
import { useAppStatus } from "../lib/status";
import "../main-window.css";

const TABS = [
  { id: "overview", label: "Overview" },
  { id: "context", label: "Context" },
  { id: "agent", label: "Agent" },
  { id: "settings", label: "Settings" },
] as const;

type Tab = (typeof TABS)[number]["id"];

function isTab(value: string): value is Tab {
  return TABS.some(({ id }) => id === value);
}

function tabFromLocation(): Tab {
  const tab = new URLSearchParams(window.location.search).get("tab");
  return tab && isTab(tab) ? tab : "overview";
}

export function Main() {
  const [tab, setTab] = useState<Tab>(tabFromLocation);
  const [contextDate, setContextDate] = useState<string | null>(null);
  const status = useAppStatus();

  const openDay = (date: string) => {
    setContextDate(date);
    setTab("context");
  };

  // Tray Settings on an already-open window. A cold open carries ?tab= in
  // the URL instead, which survives React Strict Mode's remount.
  useEffect(() => {
    const unlisten = listen<string>("open-tab", (event) => {
      if (event.payload && isTab(event.payload)) setTab(event.payload);
    });
    return () => {
      void unlisten.then((off) => off()).catch(() => undefined);
    };
  }, []);

  return (
    <div className="window main-window">
      <div className="titlebar" data-tauri-drag-region>
        <span className="titlebar-text" data-tauri-drag-region>
          AMBIENT_CONTEXT
        </span>
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
              // A tab pressed directly is not a request for a particular day, so
              // drop any day the map asked for. Without this the Context tab would
              // keep reopening the last cell clicked, for the rest of the session.
              setContextDate(null);
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
          {tab === "overview" && (
            <Overview status={status} onOpenDay={openDay} onOpenAgent={() => setTab("agent")} />
          )}
          {tab === "context" && <DayView date={contextDate ?? undefined} />}
          {tab === "agent" && <AgentTab />}
          {tab === "settings" && (
            <div className="settings-stack">
              <StorageSettings />
              <RecordingSettings />
              <RulesSettings />
              <AppSettings />
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
