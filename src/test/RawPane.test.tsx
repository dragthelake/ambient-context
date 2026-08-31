import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { mockInvoke } from "./tauri-mock";
import { RawPane } from "../components/RawPane";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

const BLOCK = {
  start: "09:00",
  end: "09:30",
  app: "Finder",
  title: "Documents",
  file: null,
  url: null,
  lines: ["a line"],
};

describe("RawPane", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("enables Redact text like this once there is a selection", async () => {
    mockInvoke((command) => {
      switch (command) {
        case "read_day_blocks":
          return [BLOCK];
        case "get_rules":
          return { rules: [], built_ins: [], next_id: "r1", error: null };
        case "get_settings":
          return { engine: null };
        default:
          throw new Error(`unexpected command ${command}`);
      }
    });

    render(<RawPane date="2026-07-04" mode="raw" />);
    const button = await screen.findByRole("button", { name: "Redact text like this" });
    expect((button as HTMLButtonElement).disabled).toBe(true);

    // rangeCount 0 keeps the highlight pill out of this: it only asks the
    // selection for its text.
    vi.spyOn(window, "getSelection").mockReturnValue({
      toString: () => "a line",
      rangeCount: 0,
    } as unknown as Selection);
    await act(async () => {
      document.dispatchEvent(new Event("selectionchange"));
    });

    await waitFor(() => expect((button as HTMLButtonElement).disabled).toBe(false));
  });
});
