import { useState } from "react";
import { DayView } from "./DayView";
import { EngineSettings } from "./EngineSettings";
import "../main-window.css";

type View = "day" | "settings";

export function Main() {
  const [view, setView] = useState<View>("day");

  return (
    <div className="main-window">
      <nav className="rail">
        <button
          className={view === "day" ? "rail-item is-current" : "rail-item"}
          onClick={() => setView("day")}
        >
          Day
        </button>
        <button
          className={view === "settings" ? "rail-item is-current" : "rail-item"}
          onClick={() => setView("settings")}
        >
          Settings
        </button>
      </nav>
      <main className="pane">
        {view === "day" && <DayView />}
        {view === "settings" && <EngineSettings />}
      </main>
    </div>
  );
}
