import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { callsOf, countOf, mockInvoke } from "./tauri-mock";
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
const OPENCODE = { label: "opencode", command: "/usr/local/bin/opencode", args: [], timeout_secs: 600 };

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
        ingest_agent: null,
        ingest_max_chars: 400_000,
        schedule_hhmm: null,
        editor: null,
        launch_at_login: true,
        max_block_chars: 0,
        write_references: true,
        extra_redaction_patterns: [],
      };
    case "agent_detect":
      return [CLAUDE, OPENCODE];
    case "agent_auth":
      return { state: "ok" };
    case "get_prompt": {
      const id = (args?.id as string) ?? "day-context";
      return {
        id,
        text: `prompt body for ${id}`,
        customised: false,
        path: "/p",
      };
    }
    case "set_settings":
      return null;
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
            ingest_agent: null,
            ingest_max_chars: 400_000,
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
          return {
            id: "day-context",
            text: "You are a careful summariser.",
            customised: false,
            path: "/tmp/prompt.md",
          };
        default:
          throw new Error(`unexpected command ${command}`);
      }
    });

    render(<AgentTab />);
    await waitFor(() => expect(countOf("agent_auth")).toBe(2));
    expect(await screen.findByText(/Not signed in\. Run claude login/)).toBeTruthy();
  });

  it("connects Claude Code with the chosen model, and Haiku without an effort flag", async () => {
    let savedAgent: { args: string[] } | null = null;
    mockInvoke((command, args) => {
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
            ingest_agent: null,
            ingest_max_chars: 400_000,
            max_block_chars: 0,
            write_references: true,
            extra_redaction_patterns: [],
          };
        case "set_settings":
          savedAgent = (args?.next as { agent: { args: string[] } }).agent;
          return null;
        case "agent_detect":
          return [CLAUDE];
        case "agent_auth":
          return { state: "ok" };
        case "get_prompt":
          return {
            id: "day-context",
            text: "You are a careful summariser.",
            customised: false,
            path: "/tmp/prompt.md",
          };
        default:
          throw new Error(`unexpected command ${command} ${JSON.stringify(args)}`);
      }
    });

    render(<AgentTab />);
    fireEvent.click(await screen.findByRole("radio", { name: "Claude Code" }));
    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "claude-haiku-4-5" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await screen.findByText("Connected");
    expect(savedAgent!.args).toContain("claude-haiku-4-5");
    // Haiku 4.5 takes no effort levels; passing the flag would fail the run.
    expect(savedAgent!.args).not.toContain("--effort");
  });

  it("shows a failed test, and a result from an abandoned run never lands", async () => {
    let failNext = true;
    let release: (() => void) | null = null;
    mockInvoke((command, args) => {
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
            ingest_agent: null,
            ingest_max_chars: 400_000,
            max_block_chars: 0,
            write_references: true,
            extra_redaction_patterns: [],
          };
        case "agent_detect":
          return [CLAUDE, CODEX];
        case "agent_auth":
          return { state: "ok" };
        case "agent_test":
          if (failNext) return Promise.reject("exited with status 1");
          // Held open until the test releases it, after switching provider.
          return new Promise((resolve) => {
            release = () => resolve("late reply");
          });
        case "get_prompt":
          return {
            id: "day-context",
            text: "You are a careful summariser.",
            customised: false,
            path: "/tmp/prompt.md",
          };
        default:
          throw new Error(`unexpected command ${command} ${JSON.stringify(args)}`);
      }
    });

    render(<AgentTab />);
    fireEvent.click(await screen.findByRole("radio", { name: "Claude Code" }));
    fireEvent.click(screen.getByRole("button", { name: "Test" }));
    expect(await screen.findByText(/Test failed: exited with status 1/)).toBeTruthy();

    // A slow run abandoned by switching provider must not write its result
    // into the newly selected provider's view.
    failNext = false;
    fireEvent.click(screen.getByRole("button", { name: "Test" }));
    await screen.findByText(/Waiting for the agent to reply/);
    fireEvent.click(screen.getByRole("radio", { name: "Codex" }));
    release!();
    await waitFor(() => expect(screen.queryByText(/Test passed/)).toBe(null));
  });

  it("keeps a passing test result across Connect", async () => {
    mockInvoke((command, args) => {
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
            ingest_agent: null,
            ingest_max_chars: 400_000,
            max_block_chars: 0,
            write_references: true,
            extra_redaction_patterns: [],
          };
        case "set_settings":
          return null;
        case "agent_detect":
          return [CLAUDE];
        case "agent_auth":
          return { state: "ok" };
        case "agent_test":
          return "Claude replied.";
        case "get_prompt":
          return {
            id: "day-context",
            text: "You are a careful summariser.",
            customised: false,
            path: "/tmp/prompt.md",
          };
        default:
          throw new Error(`unexpected command ${command} ${JSON.stringify(args)}`);
      }
    });

    render(<AgentTab />);
    fireEvent.click(await screen.findByRole("radio", { name: "Claude Code" }));
    fireEvent.click(screen.getByRole("button", { name: "Test" }));
    await screen.findByText(/Test passed/);
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    // Connect applies the provider that was just tested, so the pass holds:
    // it must not be replaced by "has not passed a test yet".
    await screen.findByText("Connected");
    expect(screen.getByText(/Test passed/)).toBeTruthy();
    expect(screen.queryByText(/has not passed a test yet/)).toBe(null);
  });

  it("locks the other providers while one is connected", async () => {
    mockInvoke((command, args) => {
      switch (command) {
        case "get_settings":
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
            ingest_agent: null,
            ingest_max_chars: 400_000,
            max_block_chars: 0,
            write_references: true,
            extra_redaction_patterns: [],
          };
        case "agent_detect":
          return [CLAUDE, CODEX];
        case "agent_auth":
          return { state: "ok" };
        case "get_prompt":
          return {
            id: "day-context",
            text: "You are a careful summariser.",
            customised: false,
            path: "/tmp/prompt.md",
          };
        default:
          throw new Error(`unexpected command ${command} ${JSON.stringify(args)}`);
      }
    });

    render(<AgentTab />);
    // The connected provider says so, offers Disconnect, and holds the
    // schedule; the other row cannot be chosen until it is let go.
    expect(await screen.findByText("Connected")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Disconnect" })).toBeTruthy();
    expect(screen.getByText("Schedule")).toBeTruthy();
    const codex = screen.getByRole("radio", { name: "Codex" }) as HTMLInputElement;
    expect(codex.disabled).toBe(true);
    expect(screen.queryByRole("button", { name: "Connect" })).toBe(null);
  });

  it("carries the schedule and the prompt, so summarising is configured in one place", async () => {
    mockInvoke(handler);
    render(<AgentTab />);
    // The schedule lives inside the chosen provider's section, so nothing
    // shows until a provider is picked.
    expect(screen.queryByText("Schedule")).toBe(null);
    fireEvent.click(await screen.findByRole("radio", { name: "Something else" }));
    expect(await screen.findByText("Schedule")).toBeTruthy();
    // PromptSettings' own legend reads "Daily summary prompt", not the bare
    // word, and "prompt" also turns up in its body copy, so this pins the
    // match to the legend rather than doing an exact or a loose text match.
    expect(await screen.findByText("Prompts", { selector: "legend" })).toBeTruthy();
  });

  it("saves a separate ingest agent and the input cap", async () => {
    mockInvoke((command, args) => {
      switch (command) {
        case "get_settings":
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
            ingest_agent: null,
            ingest_max_chars: 400_000,
            max_block_chars: 0,
            write_references: true,
            extra_redaction_patterns: [],
          };
        case "agent_detect":
          return [CLAUDE, OPENCODE];
        case "agent_auth":
          return { state: "ok" };
        case "get_prompt": {
          const id = (args?.id as string) ?? "day-context";
          return {
            id,
            text: `prompt body for ${id}`,
            customised: false,
            path: "/p",
          };
        }
        case "set_settings":
          return null;
        default:
          throw new Error(`unexpected command ${command} ${JSON.stringify(args)}`);
      }
    });
    render(<AgentTab />);
    await screen.findByText("Connected");
    const picker = (await screen.findByLabelText("Ingest agent")) as HTMLSelectElement;
    fireEvent.change(picker, { target: { value: "/usr/local/bin/opencode" } });
    await waitFor(() => expect(callsOf("set_settings").length).toBe(1));
    const next = callsOf("set_settings")[0].args?.next as {
      ingest_agent: { command: string } | null;
    };
    expect(next.ingest_agent?.command).toBe("/usr/local/bin/opencode");
    const cap = screen.getByLabelText("Longest ingest input (characters)") as HTMLInputElement;
    fireEvent.change(cap, { target: { value: "250000" } });
    fireEvent.blur(cap);
    await waitFor(() => expect(callsOf("set_settings").length).toBe(2));
    expect(
      (callsOf("set_settings")[1].args?.next as { ingest_max_chars: number }).ingest_max_chars,
    ).toBe(250000);
  });

  it("switches the prompt editor between the four prompts", async () => {
    mockInvoke(handler);
    render(<AgentTab />);
    const select = (await screen.findByLabelText("Prompt")) as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "ingest-apps" } });
    await waitFor(() =>
      expect(callsOf("get_prompt").some((c) => c.args?.id === "ingest-apps")).toBe(true),
    );
  });

  it("does not carry launch at login, which is an application preference", async () => {
    mockInvoke(handler);
    render(<AgentTab />);
    await screen.findByText("Provider");
    expect(screen.queryByText("Launch at login")).toBe(null);
  });
});
