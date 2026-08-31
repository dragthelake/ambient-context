import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

type Registration = {
  binary: string;
  quoted_binary: string;
  running: boolean;
  last_write: { at: string; action: string; client: string } | null;
};

const CLIENTS = ["Claude Code", "Claude Desktop", "Cursor", "Zed", "Generic"] as const;
type Client = (typeof CLIENTS)[number];

function registrationBlock(client: Client, binary: string, quoted: string): string {
  switch (client) {
    case "Claude Code":
      return `claude mcp add --scope user --transport stdio ambient-context -- ${quoted} mcp`;
    case "Claude Desktop":
      return JSON.stringify(
        { mcpServers: { "ambient-context": { command: binary, args: ["mcp"] } } },
        null,
        2,
      );
    case "Cursor":
      return JSON.stringify(
        { mcpServers: { "ambient-context": { command: binary, args: ["mcp"] } } },
        null,
        2,
      );
    case "Zed":
      return JSON.stringify(
        {
          context_servers: {
            "ambient-context": { command: binary, args: ["mcp"], env: {} },
          },
        },
        null,
        2,
      );
    case "Generic":
      return JSON.stringify(
        { name: "ambient-context", command: binary, args: ["mcp"] },
        null,
        2,
      );
  }
}

const FILE_HINTS: Record<Client, string> = {
  "Claude Code": "Run this in a terminal.",
  "Claude Desktop":
    "Into ~/Library/Application Support/Claude/claude_desktop_config.json",
  Cursor: "Into ~/.cursor/mcp.json for every project, or .cursor/mcp.json for one.",
  Zed: "Into settings.json",
  Generic: "For anything else that speaks stdio MCP.",
};

export function McpSettings() {
  const [registration, setRegistration] = useState<Registration | null>(null);
  const [client, setClient] = useState<Client>("Claude Code");
  const [copied, setCopied] = useState(false);

  const read = useCallback(async () => {
    const next = await invoke<Registration>("mcp_registration");
    setRegistration(next);
  }, []);

  useEffect(() => {
    void read();
  }, [read]);

  if (!registration) return null;

  const block = registrationBlock(
    client,
    registration.binary,
    registration.quoted_binary,
  );

  const copy = async () => {
    await writeText(block);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <fieldset>
      <legend>MCP server</legend>
      <p>
        Ambient Context is also an MCP server: the same program with an{" "}
        <code>mcp</code> subcommand. It exposes eighteen tools covering
        everything in this app. Reading days, summaries and settings works
        whether or not Ambient Context is open; changing anything needs it
        open, and every change is written to the day's ledger with your
        client's name on it.
      </p>

      <div className="button-row mcp-picker">
        {CLIENTS.map((name) => (
          <button
            key={name}
            type="button"
            className={client === name ? "mcp-client is-current" : "mcp-client"}
            onClick={() => setClient(name)}
          >
            {name}
          </button>
        ))}
      </div>

      <p className="settings-note">{FILE_HINTS[client]}</p>
      <pre className="mcp-block">{block}</pre>
      <button type="button" onClick={() => void copy()}>
        {copied ? "Copied" : "Copy"}
      </button>

      <p className="settings-note mcp-status">
        {!registration.running
          ? "Not connected. Reading days and summaries works anyway; changing settings needs Ambient Context open."
          : registration.last_write
            ? `Connected. Last change from ${registration.last_write.client} at ${registration.last_write.at.slice(11, 16)} today: ${registration.last_write.action}.`
            : "Connected. No agent has changed anything yet."}
      </p>
    </fieldset>
  );
}
