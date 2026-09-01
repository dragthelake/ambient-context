import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { mockInvoke } from "./tauri-mock";
import { Main } from "../components/Main";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});
vi.mock("@tauri-apps/api/event", async () => {
  const mock = await import("./tauri-mock");
  return { listen: mock.listen };
});
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ close: () => undefined }),
}));

// Only the Overview tab's commands: the other panes are not mounted until
// their tab is chosen, which is the behaviour the second test pins down.
function handler(command: string) {
  switch (command) {
    case "capture_status":
      return { running: false, blocks_today: 0 };
    case "permission_status":
      return "granted";
    case "current_folder":
      return "/Users/someone/Ambient Context";
    case "get_settings":
      // Only the keys the sound engine is handed on mount.
      return { sound_enabled: true, sound_volume: 0.6 };
    default:
      throw new Error(`unexpected command ${command}`);
  }
}

afterEach(cleanup);

describe("the main window's tab strip", () => {
  it("opens on Overview", () => {
    mockInvoke(handler);
    render(<Main />);

    const overview = screen.getByRole("tab", { name: "Overview" });
    expect(overview.getAttribute("aria-selected")).toBe("true");
    for (const name of ["Context", "Settings"]) {
      expect(
        screen.getByRole("tab", { name }).getAttribute("aria-selected"),
      ).toBe("false");
    }
  });

  it("shows only the chosen tab's pane", () => {
    mockInvoke(handler);
    render(<Main />);

    // The record toggle belongs to Overview and nothing else, so its
    // presence is what "the Overview pane is the one showing" means.
    expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
    expect(screen.getByRole("button", { name: /recording/i })).toBeTruthy();
  });
});
