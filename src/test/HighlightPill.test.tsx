import { describe, expect, it } from "vitest";
import { placePill } from "../components/HighlightPill";

const pill = { width: 300, height: 27 };
const bounds = { left: 40, top: 100, width: 800, height: 600 };

describe("placePill", () => {
  it("centres the pill over the selection and sits it above", () => {
    const at = placePill({ left: 400, top: 300, width: 100, height: 20 }, pill, bounds);
    expect(at).toEqual({ left: 300, top: 300 - 27 - 8 });
  });

  it("keeps the whole pill inside the left edge, not just its centre", () => {
    const at = placePill({ left: 44, top: 300, width: 10, height: 20 }, pill, bounds);
    expect(at?.left).toBe(48);
  });

  it("keeps the whole pill inside the right edge", () => {
    const at = placePill({ left: 830, top: 300, width: 10, height: 20 }, pill, bounds);
    expect(at?.left).toBe(40 + 800 - 300 - 8);
  });

  it("drops below the selection when there is no room above", () => {
    const at = placePill({ left: 400, top: 104, width: 100, height: 20 }, pill, bounds);
    expect(at?.top).toBe(104 + 20 + 8);
  });

  it("goes away when the selection has scrolled out of the pane", () => {
    expect(placePill({ left: 400, top: 20, width: 100, height: 20 }, pill, bounds)).toBeNull();
    expect(placePill({ left: 400, top: 800, width: 100, height: 20 }, pill, bounds)).toBeNull();
  });
});
