import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { callsOf, mockInvoke } from "./tauri-mock";
import { KbPane } from "../components/KbPane";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

describe("KbPane", () => {
  afterEach(cleanup);

  it("shows each file on its own tab and the empty states", async () => {
    mockInvoke((command, args) => {
      if (command !== "read_kb") throw new Error(`unexpected command ${command}`);
      if (args?.file === "people.md")
        return "---\ndate: 2026-08-27\n---\n\n## Dan\nAsked for the notch state 09:48-09:59\n";
      if (args?.file === "reading.md") return "---\n---\n\nNothing evident.\n";
      return null;
    });
    render(<KbPane date="2026-08-27" refreshKey={0} />);
    expect(await screen.findByText("Dan")).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "Reading" }));
    expect(await screen.findByText("Nothing evident.")).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "Threads" }));
    expect(await screen.findByText("Not ingested yet.")).toBeTruthy();
    expect(callsOf("read_kb").some((c) => c.args?.file === "threads.md")).toBe(true);
  });
});
