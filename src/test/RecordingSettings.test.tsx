import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { callsOf, mockInvoke } from "./tauri-mock";
import { RecordingSettings } from "../components/RecordingSettings";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

function handler(command: string, args?: Record<string, unknown>) {
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
        idle_secs: 120,
        max_block_chars: 4000,
        sound_enabled: true,
        sound_volume: 0.6,
        write_references: true,
        extra_redaction_patterns: [],
      };
    case "set_settings": {
      const next = args?.next as { extra_redaction_patterns: string[] };
      const bad = next.extra_redaction_patterns.findIndex((p) => p === "([bad");
      // Tauri rejects with the command's Err(String), not with an Error.
      if (bad >= 0) {
        return Promise.reject(
          `pattern ${bad + 1} is not a valid regular expression: unclosed group`,
        );
      }
      return null;
    }
    default:
      throw new Error(`unexpected command ${command}`);
  }
}

describe("RecordingSettings", () => {
  afterEach(cleanup);

  it("shows the loaded idle threshold", async () => {
    mockInvoke(handler);
    render(<RecordingSettings />);
    const field = (await screen.findByLabelText(
      "Idle after (seconds)",
    )) as HTMLInputElement;
    expect(field.value).toBe("120");
  });

  it("saves a new idle threshold on blur", async () => {
    mockInvoke(handler);
    render(<RecordingSettings />);
    const field = await screen.findByLabelText("Idle after (seconds)");
    fireEvent.change(field, { target: { value: "300" } });
    fireEvent.blur(field);
    const saves = callsOf("set_settings");
    expect(saves).toHaveLength(1);
    expect(
      (saves[0].args?.next as { idle_secs: number }).idle_secs,
    ).toBe(300);
  });

  it("names the pattern the backend refused and keeps the draft", async () => {
    mockInvoke(handler);
    render(<RecordingSettings />);
    const field = await screen.findByLabelText(
      "Extra patterns, one regular expression per line",
    );
    fireEvent.change(field, { target: { value: "Kestrel\n([bad" } });
    fireEvent.blur(field);
    expect(
      await screen.findByText(/pattern 2 is not a valid regular expression/),
    ).toBeTruthy();
    expect((field as HTMLTextAreaElement).value).toBe("Kestrel\n([bad");
  });
});
