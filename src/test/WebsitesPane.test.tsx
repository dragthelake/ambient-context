import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { mockInvoke } from "./tauri-mock";
import { WebsitesPane } from "../components/WebsitesPane";

vi.mock("@tauri-apps/api/core", async () => {
  const mock = await import("./tauri-mock");
  return { invoke: mock.invoke };
});

describe("WebsitesPane", () => {
  afterEach(cleanup);

  it("renders totals ranked by dwell with minutes and visits", async () => {
    mockInvoke((command) => {
      if (command === "website_totals") {
        return [
          {
            url: "https://v2.tauri.app/",
            domain: "v2.tauri.app",
            title: "Tauri",
            dwell_secs: 960,
            visits: 2,
            first: "09:30",
            last: "10:05",
          },
        ];
      }
      throw new Error(`unexpected command ${command}`);
    });
    render(<WebsitesPane date="2026-08-27" />);
    expect(await screen.findByText("v2.tauri.app")).toBeTruthy();
    expect(screen.getByText("16m")).toBeTruthy();
    expect(screen.getByText("2")).toBeTruthy();
    expect(screen.getByTitle("https://v2.tauri.app/")).toBeTruthy();
  });

  it("says so when nothing was visited", async () => {
    mockInvoke((command) => {
      if (command === "website_totals") return [];
      throw new Error(`unexpected command ${command}`);
    });
    render(<WebsitesPane date="2026-08-27" />);
    expect(await screen.findByText("No websites recorded.")).toBeTruthy();
  });
});
