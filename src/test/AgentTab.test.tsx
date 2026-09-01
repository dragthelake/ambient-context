import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { countOf, mockInvoke } from "./tauri-mock";
import { AgentTab } from "../components/AgentTab";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: vi.fn(),
}));

const CLAUDE = { label: "Claude Code", command: "/usr/local/bin/claude", args: ["-p"], timeout_secs: 600 };
const CODEX = { label: "Codex", command: "/usr/local/bin/codex", args: ["exec"], timeout_secs: 600 };

// The settings and prompt every test below needs answered, since AgentTab
// now composes PromptSettings and mounts it alongside the agent picker.
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
        schedule_hhmm: null,
        editor: null,
        launch_at_login: true,
        max_block_chars: 0,
        write_references: true,
        extra_redaction_patterns: [],
      };
    case "agent_detect":
      return [];
    case "get_prompt":
      return { text: "You are a careful summariser.", customised: false, path: "/tmp/prompt.md" };
    default:
      throw new Error(`unexpected command ${command} ${JSON.stringify(args)}`);
  }
}

describe("AgentTab", () => {
  afterEach(cleanup);

  it("probes every detected agent, including the saved one", async () => {
    mockInvoke((command, args) => {
      switch (command) {
        case "get_settings":
          // The saved agent used to be shown as signed in without asking.
          return {
            folder: "/tmp/capture",
            enabled: true,
            interval_secs: 5,
            min_dwell_secs: 10,
            similarity_threshold: 0.5,
            agent: CLAUDE,
            schedule_hhmm: null,
            editor: null,
            launch_at_login: true,
            max_block_chars: 0,
            write_references: true,
            extra_redaction_patterns: [],
          };
        case "agent_detect":
          return [CLAUDE, CODEX];
        case "agent_auth":
          return (args?.agentConfig as { command: string }).command === CLAUDE.command
            ? { state: "not_logged_in", fix: "Run claude login" }
            : { state: "ok" };
        case "get_prompt":
          return { text: "You are a careful summariser.", customised: false, path: "/tmp/prompt.md" };
        default:
          throw new Error(`unexpected command ${command}`);
      }
    });

    render(<AgentTab />);
    await waitFor(() => expect(countOf("agent_auth")).toBe(2));
    expect(await screen.findByText(/Not signed in\. Run claude login/)).toBeTruthy();
  });

  it("carries the schedule and the prompt, so summarising is configured in one place", async () => {
    mockInvoke(handler);
    render(<AgentTab />);
    expect(await screen.findByText("Schedule")).toBeTruthy();
    // PromptSettings' own legend reads "Daily summary prompt", not the bare
    // word, and "prompt" also turns up in its body copy, so this pins the
    // match to the legend rather than doing an exact or a loose text match.
    expect(
      await screen.findByText("Daily summary prompt", { selector: "legend" }),
    ).toBeTruthy();
  });

  it("does not carry launch at login, which is an application preference", async () => {
    mockInvoke(handler);
    render(<AgentTab />);
    await screen.findByText("Schedule");
    expect(screen.queryByText("Launch at login")).toBe(null);
  });
});
