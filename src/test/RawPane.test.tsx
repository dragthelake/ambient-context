import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { mockInvoke, callsOf } from "./tauri-mock";
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
  routed: null,
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
          return { agent: null };
        default:
          throw new Error(`unexpected command ${command}`);
      }
    });

    render(<RawPane date="2026-07-04" mode="raw" file="apps" />);
    expect(callsOf("read_day_blocks")[0]?.args?.file).toBe("apps");
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

  // Two blocks in the same minute from the same app are ordinary: a poll
  // every few seconds can close one block and open another inside one
  // minute. Keyed on time and app alone they collide, React warns about
  // duplicate keys, and the confirmation lands on every twin at once.
  it("confirms a rule on only the block it was added from", async () => {
    const twin = { ...BLOCK, title: "Downloads" };
    mockInvoke((command) => {
      switch (command) {
        case "read_day_blocks":
          return [BLOCK, twin];
        case "get_rules":
          return { rules: [], built_ins: [], next_id: "r1", error: null };
        case "get_settings":
          return { agent: null };
        case "add_rule":
          // add_rule answers with the whole rules payload, not the rule.
          return {
            rules: [
              {
                id: "r1",
                target: { app: "Finder" },
                action: "exclude",
                note: null,
              },
            ],
            built_ins: [],
            next_id: "r2",
            error: null,
          };
        default:
          throw new Error(`unexpected command ${command}`);
      }
    });

    render(<RawPane date="2026-07-04" mode="raw" file="apps" />);
    const buttons = await screen.findAllByRole("button", {
      name: "Never record this app",
    });
    expect(buttons).toHaveLength(2);

    await act(async () => {
      fireEvent.click(buttons[0]);
    });

    await waitFor(() =>
      expect(screen.getAllByRole("button", { name: "Rule added" })).toHaveLength(1),
    );
  });
});
