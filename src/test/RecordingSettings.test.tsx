import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { callsOf, mockInvoke } from "./tauri-mock";
import { RecordingSettings } from "../components/RecordingSettings";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

/// The backend's settings.json, as a value the handler reads and writes.
/// `set_settings` has no merge, so a test that shares one object with the
/// component is the only way to see whether a save carried a stale copy.
let stored: Record<string, unknown>;

function reset() {
  stored = {
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
    write_references: true,
    extra_redaction_patterns: [],
  };
}

function handler(command: string, args?: Record<string, unknown>) {
  switch (command) {
    case "get_settings":
      return { ...stored };
    case "set_settings": {
      const next = args?.next as {
        extra_redaction_patterns: string[];
      } & Record<string, unknown>;
      const bad = next.extra_redaction_patterns.findIndex((p) => p === "([bad");
      // Tauri rejects with the command's Err(String), not with an Error.
      if (bad >= 0) {
        return Promise.reject(
          `pattern ${bad + 1} is not a valid regular expression: unclosed group`,
        );
      }
      stored = { ...next };
      return null;
    }
    default:
      throw new Error(`unexpected command ${command}`);
  }
}

describe("RecordingSettings", () => {
  beforeEach(reset);
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
    await waitFor(() => expect(callsOf("set_settings")).toHaveLength(1));
    expect(
      (callsOf("set_settings")[0].args?.next as { idle_secs: number }).idle_secs,
    ).toBe(300);
  });

  it("keeps a folder chosen after this panel loaded", async () => {
    // The Storage panel above writes the folder through choose_folder, so a
    // save built from this panel's mount-time snapshot would put back the
    // folder as it was, which on a first run is null and stops capture
    // writing anything at all.
    mockInvoke(handler);
    render(<RecordingSettings />);
    const field = await screen.findByLabelText("Idle after (seconds)");
    stored.folder = "/tmp/chosen-later";
    fireEvent.change(field, { target: { value: "300" } });
    fireEvent.blur(field);
    await waitFor(() => expect(callsOf("set_settings")).toHaveLength(1));
    const next = callsOf("set_settings")[0].args?.next as {
      folder: string;
      idle_secs: number;
    };
    expect(next.folder).toBe("/tmp/chosen-later");
    expect(next.idle_secs).toBe(300);
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
