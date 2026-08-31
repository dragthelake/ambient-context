import { useState } from "react";
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
        {view === "day" && <p>The Day view arrives in Task 11.</p>}
        {view === "settings" && <p>Settings arrive in Task 13.</p>}
      </main>
    </div>
  );
}
