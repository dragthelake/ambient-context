import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { mockInvoke } from "./tauri-mock";
import { AppSettings } from "../components/AppSettings";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

function handler(command: string) {
  switch (command) {
    case "get_settings":
      return {
        folder: "/tmp/capture",
        enabled: true,
        interval_secs: 5,
        min_dwell_secs: 10,
        similarity_threshold: 0.5,
        agent: null,
        ingest_agent: null,
        ingest_max_chars: 400_000,
        schedule_hhmm: null,
        editor: null,
        launch_at_login: true,
        max_block_chars: 0,
        write_references: true,
        extra_redaction_patterns: [],
      };
    default:
      throw new Error(`unexpected command ${command}`);
  }
}

describe("AppSettings", () => {
  afterEach(cleanup);

  it("holds launch at login", async () => {
    mockInvoke(handler);
    render(<AppSettings />);
    expect(await screen.findByText("Application")).toBeTruthy();
    expect(
      screen.getByLabelText("Ambient Context opens when you log in"),
    ).toBeTruthy();
  });
});
