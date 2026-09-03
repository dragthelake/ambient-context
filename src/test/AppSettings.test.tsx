import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { callsOf, mockInvoke } from "./tauri-mock";
import { AppSettings } from "../components/AppSettings";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

/// The backend's settings.json. `set_settings` has no merge, so a panel
/// that saved its mount-time snapshot would put back every field another
/// surface had changed since; sharing one object here is what shows that.
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
    editor: "/Applications/iA Writer.app",
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
    case "autostart_error":
      return null;
    case "set_settings":
      stored = { ...(args?.next as Record<string, unknown>) };
      return null;
    case "choose_editor":
      return "/Applications/Obsidian.app";
    default:
      throw new Error(`unexpected command ${command}`);
  }
}

describe("AppSettings", () => {
  beforeEach(reset);
  afterEach(cleanup);

  it("holds launch at login", async () => {
    mockInvoke(handler);
    render(<AppSettings />);
    expect(await screen.findByText("Application")).toBeTruthy();
    expect(
      screen.getByLabelText("Ambient Context opens when you log in"),
    ).toBeTruthy();
  });

  it("shows the editor the settings name", async () => {
    mockInvoke(handler);
    render(<AppSettings />);
    const field = (await screen.findByLabelText(
      "Open files with",
    )) as HTMLInputElement;
    expect(field.value).toBe("/Applications/iA Writer.app");
  });

  it("saves a typed editor path on blur", async () => {
    mockInvoke(handler);
    render(<AppSettings />);
    const field = await screen.findByLabelText("Open files with");
    fireEvent.change(field, { target: { value: "/Applications/Zed.app" } });
    fireEvent.blur(field);
    await waitFor(() => expect(callsOf("set_settings")).toHaveLength(1));
    expect(
      (callsOf("set_settings")[0].args?.next as { editor: string }).editor,
    ).toBe("/Applications/Zed.app");
  });

  it("keeps a recording change made after this panel loaded", async () => {
    // The Recording panel sits on the same page and writes the same object.
    mockInvoke(handler);
    render(<AppSettings />);
    const field = await screen.findByLabelText("Open files with");
    stored.idle_secs = 300;
    fireEvent.change(field, { target: { value: "/Applications/Zed.app" } });
    fireEvent.blur(field);
    await waitFor(() => expect(callsOf("set_settings")).toHaveLength(1));
    const next = callsOf("set_settings")[0].args?.next as {
      editor: string;
      idle_secs: number;
    };
    expect(next.idle_secs).toBe(300);
    expect(next.editor).toBe("/Applications/Zed.app");
  });

  it("saves the application the picker returns", async () => {
    mockInvoke(handler);
    render(<AppSettings />);
    fireEvent.click(await screen.findByText("Choose…"));
    await screen.findByDisplayValue("/Applications/Obsidian.app");
    await waitFor(() => expect(callsOf("set_settings")).toHaveLength(1));
    expect(
      (callsOf("set_settings")[0].args?.next as { editor: string }).editor,
    ).toBe("/Applications/Obsidian.app");
  });

  it("reports a login item the system refused", async () => {
    mockInvoke((command) =>
      command === "autostart_error"
        ? "operation not permitted"
        : handler(command),
    );
    render(<AppSettings />);
    expect(
      await screen.findByText(
        "Login item could not be updated: operation not permitted",
      ),
    ).toBeTruthy();
  });

  it("shows a refusal the toggle itself provokes rather than swallowing it", async () => {
    // The setting is saved before the OS answers, so the box would
    // otherwise sit ticked beside a login item that was never registered.
    let refused: string | null = null;
    mockInvoke((command, args) => {
      if (command === "set_launch_at_login") {
        refused = "operation not permitted";
        return Promise.reject(refused);
      }
      if (command === "autostart_error") return refused;
      return handler(command, args);
    });
    render(<AppSettings />);
    fireEvent.click(await screen.findByLabelText("Ambient Context opens when you log in"));
    expect(
      await screen.findByText(
        "Login item could not be updated: operation not permitted",
      ),
    ).toBeTruthy();
    await waitFor(() => expect(callsOf("get_settings").length).toBeGreaterThan(1));
  });
});
