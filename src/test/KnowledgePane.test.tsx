import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { callsOf, mockInvoke } from "./tauri-mock";
import { KnowledgePane } from "../components/KnowledgePane";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

const KNOWLEDGE = [
  "# people.md",
  "",
  "## Dan",
  "Asked for the notch state 09:48-09:59",
  "",
  "# commitments.md",
  "",
  "- [ ] ship the notch state",
  "",
  "# threads.md",
  "",
  "(not ingested)",
  "",
  "# products.md",
  "",
  "Nothing evident.",
  "",
  "# issues.md",
  "",
  "- the eye stutters on wake",
  "",
  "# reading.md",
  "",
  "Nothing evident.",
  "",
].join("\n");

const MANIFEST = [
  "---",
  "date: 2026-08-27",
  "ingest_apps.disposition: accepted",
  "ingest_apps.at: 2026-08-27T06:00:12+10:00",
  "ingest_messages.disposition: accepted",
  "ingest_messages.at: 2026-08-27T05:58:40+10:00",
  "ingest_websites.disposition: rejected",
  "ingest_websites.at: 2026-08-27T06:01:30+10:00",
  "---",
].join("\n");

const props = {
  date: "2026-08-27",
  refreshKey: 0,
  running: false,
  step: null,
  hasAgent: true,
  onGenerate: () => {},
};

function built() {
  mockInvoke((command, args) => {
    if (command !== "read_kb") throw new Error(`unexpected command ${command}`);
    return args?.file === "manifest.md" ? MANIFEST : KNOWLEDGE;
  });
}

function unbuilt() {
  mockInvoke((command) => {
    if (command !== "read_kb") throw new Error(`unexpected command ${command}`);
    return null;
  });
}

describe("KnowledgePane", () => {
  afterEach(cleanup);

  it("renders the chosen section and the line saying when it was built", async () => {
    built();
    render(<KnowledgePane {...props} section="people.md" />);

    expect(await screen.findByText("Built 06:00 from messages and apps")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "People" })).toBeTruthy();
    expect(screen.getByText("Dan")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Issues" })).toBeNull();
    // One call for the six files, one for the manifest, and no per-file calls.
    expect(callsOf("read_kb")).toHaveLength(2);
  });

  it("reads a sentinel and a never-written file the same way", async () => {
    built();
    const { rerender } = render(<KnowledgePane {...props} section="threads.md" />);
    expect(await screen.findByText("Nothing evident")).toBeTruthy();
    rerender(<KnowledgePane {...props} section="products.md" />);
    expect(await screen.findByText("Nothing evident")).toBeTruthy();
  });

  it("offers Generate and says what it builds when nothing has been built yet", async () => {
    const onGenerate = vi.fn();
    unbuilt();
    render(<KnowledgePane {...props} section="people.md" onGenerate={onGenerate} />);

    expect(await screen.findByText("Nothing built for this day yet.")).toBeTruthy();
    expect(screen.getByText(/builds a structured wiki/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    expect(onGenerate).toHaveBeenCalledTimes(1);
  });

  it("names the running step instead of the empty state while it builds", async () => {
    unbuilt();
    render(
      <KnowledgePane {...props} section="people.md" running step="Reading apps (2 of 3)" />,
    );
    expect(await screen.findByText(/Reading apps \(2 of 3\)/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Generate" })).toBeNull();
  });
});
