import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Read from disk rather than imported. Vitest stubs CSS imports, so both
// `import "./x.css"` and `./x.css?raw` hand back an empty string here, and
// a test built on those passes because it is checking nothing at all.

// setup.css and main-window.css share one namespace, and App.tsx imports
// Main (and so main-window.css) before setup.css, which means setup.css is
// injected second and wins every tie on source order. Four bugs came from
// that during the v1 UI pass: the window padding, the document background,
// the tab press style and a stray declaration left at the end of a rule.
// Each one looked like a rule that simply did not work.
//
// These tests are a tripwire, not a full cascade model. They catch the two
// shapes that are always wrong rather than trying to decide which of two
// different selectors matches the same element.

type Rule = { selector: string; decls: Map<string, string> };

function parse(source: string): Rule[] {
  const src = source
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/@(import|charset)[^;]*;/g, "");
  const rules: Rule[] = [];
  for (const match of src.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const head = match[1].trim();
    // at-rule preludes and keyframe stops are not selectors
    if (!head || head.startsWith("@") || /^\d+%$/.test(head)) continue;
    const decls = new Map<string, string>();
    for (const decl of match[2].split(";")) {
      const at = decl.indexOf(":");
      if (at === -1) continue;
      decls.set(decl.slice(0, at).trim(), decl.slice(at + 1).trim());
    }
    for (const selector of head.split(",").map((one: string) => one.trim())) {
      if (selector) rules.push({ selector, decls });
    }
  }
  return rules;
}

const setup = parse(readFileSync("src/setup.css", "utf8"));
const main = parse(readFileSync("src/main-window.css", "utf8"));

describe("the two stylesheets", () => {
  it("never set the same property on the same selector in both files", () => {
    const bySelector = new Map<string, Rule[]>();
    for (const rule of setup) {
      const list = bySelector.get(rule.selector) ?? [];
      list.push(rule);
      bySelector.set(rule.selector, list);
    }

    const dead: string[] = [];
    for (const rule of main) {
      for (const other of bySelector.get(rule.selector) ?? []) {
        for (const [property, value] of rule.decls) {
          // !important is a deliberate override and wins regardless
          if (value.includes("!important")) continue;
          if (other.decls.has(property)) {
            dead.push(
              `${rule.selector} { ${property} } is dead: main-window.css sets ` +
                `${value}, setup.css sets ${other.decls.get(property)} and loads second`,
            );
          }
        }
      }
    }
    expect(dead).toEqual([]);
  });

  // The press style swaps the bevel and shifts the label with text-shadow.
  // It must not repad: padding shifts resize the box and move neighbours.
  it("does not repad buttons on press", () => {
    const pressed = setup.filter(
      (rule) => rule.selector.startsWith("button:active") && rule.decls.has("padding"),
    );
    expect(pressed).toHaveLength(0);
  });

  it("excludes chrome that owns its own press treatment from the global press style", () => {
    const pressed = setup.filter(
      (rule) => rule.selector.startsWith("button:active") && rule.decls.has("box-shadow"),
    );
    expect(pressed).toHaveLength(1);
    for (const excluded of [".tab", ".defrag-cell", ".link-button"]) {
      expect(pressed[0].selector).toContain(`:not(${excluded})`);
    }
  });

  it("gives the defrag cell its own pressed and focused treatment", () => {
    const selectors = main.map((rule) => rule.selector);
    expect(selectors).toContain(".defrag-cell:active:not(:disabled)");
    expect(selectors).toContain(".defrag-cell:focus-visible");
  });

  it("keeps defrag cells at map size rather than the Win98 button floor", () => {
    const cell = main.find((rule) => rule.selector === ".defrag-cell");
    expect(cell?.decls.get("height")).toBe("11px");
    expect(cell?.decls.get("min-height")).toBe("0");
  });

  it.each([
    ["setup.css", setup],
    ["main-window.css", main],
  ])("declare each property once per selector in %s", (_name, rules) => {
    const seen = new Map<string, Map<string, string>>();
    const shadowed: string[] = [];
    for (const rule of rules) {
      const already = seen.get(rule.selector) ?? new Map<string, string>();
      for (const [property, value] of rule.decls) {
        if (already.has(property)) {
          shadowed.push(
            `${rule.selector} { ${property} } is set twice: ` +
              `${already.get(property)} then ${value}`,
          );
        }
        already.set(property, value);
      }
      seen.set(rule.selector, already);
    }
    expect(shadowed).toEqual([]);
  });
});
