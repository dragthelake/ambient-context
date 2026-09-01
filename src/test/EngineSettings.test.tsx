import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { countOf, mockInvoke } from "./tauri-mock";
import { AgentSettings } from "../components/EngineSettings";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

const CLAUDE = { label: "Claude Code", command: "/usr/local/bin/claude", args: ["-p"], timeout_secs: 600 };
const CODEX = { label: "Codex", command: "/usr/local/bin/codex", args: ["exec"], timeout_secs: 600 };

describe("AgentSettings", () => {
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
        default:
          throw new Error(`unexpected command ${command}`);
      }
    });

    render(<AgentSettings />);
    await waitFor(() => expect(countOf("agent_auth")).toBe(2));
    expect(await screen.findByText(/Not signed in\. Run claude login/)).toBeTruthy();
  });
});
