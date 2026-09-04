import { describe, expect, it } from "vitest";
import { defaultRule, isConfigured, RULE_ORDER, scopeLabel, summariseRule } from "./rules";
import type { RuleSpec } from "../types";

describe("defaultRule", () => {
  it("produces a usable rule for every kind", () => {
    for (const kind of RULE_ORDER) {
      const r = defaultRule(kind);
      expect(r.kind).toBe(kind);
      expect(r.enabled).toBe(true);
      expect(r.id).toMatch(/[0-9a-f-]{36}/);
      expect(r.scope).toEqual({ stem: true, ext: false });
    }
  });

  it("gives every rule a distinct id", () => {
    const ids = RULE_ORDER.map((k) => defaultRule(k).id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe("summariseRule", () => {
  it("reads like the card titles in the spec", () => {
    const replace = {
      ...defaultRule("replace"),
      find: "^IMG_(\\d+)",
      with: "shot-$1",
      regex: true,
    } as RuleSpec;
    expect(summariseRule(replace)).toBe("regex “^IMG_(\\d+)” → “shot-$1”");
  });

  it("says so plainly when a rule is not configured yet", () => {
    expect(summariseRule(defaultRule("replace"))).toBe("not configured");
    expect(summariseRule(defaultRule("template"))).toBe("not configured");
    expect(summariseRule(defaultRule("csv_map"))).toBe("no file chosen");
  });

  it("describes numbering including padding and per-folder reset", () => {
    const r = { ...defaultRule("number"), pad: 3, reset_per_folder: true } as RuleSpec;
    expect(summariseRule(r)).toBe("from 1, padded to 3, per folder, at the end");
  });

  it("names every case style", () => {
    const styles = ["lower", "upper", "title", "sentence", "camel", "pascal", "snake", "kebab"] as const;
    for (const style of styles) {
      const r = { ...defaultRule("case"), style } as RuleSpec;
      const s = summariseRule(r);
      expect(s.length).toBeGreaterThan(0);
      expect(s).not.toBe(style === "lower" ? "" : "");
    }
    expect(summariseRule({ ...defaultRule("case"), style: "snake" } as RuleSpec)).toBe("snake_case");
  });

  it("truncates long values so a card stays one line", () => {
    const r = { ...defaultRule("template"), template: "x".repeat(80) } as RuleSpec;
    expect(summariseRule(r).length).toBeLessThanOrEqual(30);
    expect(summariseRule(r).endsWith("…")).toBe(true);
  });

  it("returns something for every rule kind", () => {
    for (const kind of RULE_ORDER) {
      expect(summariseRule(defaultRule(kind))).toBeTruthy();
    }
  });

  it("describes insert positions", () => {
    const base = defaultRule("insert");
    expect(summariseRule({ ...base, text: "IMG_", at: "prefix" } as RuleSpec)).toContain("at the start");
    expect(summariseRule({ ...base, text: "x", at: "index", index: 3 } as RuleSpec)).toContain("at position 3");
    expect(summariseRule({ ...base, text: "x", at: "before", marker: "-", all: false } as RuleSpec)).toContain("before");
  });
});

describe("isConfigured", () => {
  it("marks empty text rules as unconfigured", () => {
    expect(isConfigured(defaultRule("replace"))).toBe(false);
    expect(isConfigured(defaultRule("insert"))).toBe(false);
    expect(isConfigured(defaultRule("template"))).toBe(false);
  });

  it("treats rules with no required input as ready", () => {
    expect(isConfigured(defaultRule("case"))).toBe(true);
    expect(isConfigured(defaultRule("trim"))).toBe(true);
    expect(isConfigured(defaultRule("sanitise"))).toBe(true);
    expect(isConfigured(defaultRule("number"))).toBe(true);
    expect(isConfigured(defaultRule("extension"))).toBe(true);
  });

  it("requires an extension when one is being set", () => {
    const r = { ...defaultRule("extension"), mode: "set", ext: "" } as RuleSpec;
    expect(isConfigured(r)).toBe(false);
    expect(isConfigured({ ...r, ext: "jpg" } as RuleSpec)).toBe(true);
  });
});

describe("scopeLabel", () => {
  it("names each combination", () => {
    expect(scopeLabel({ stem: true, ext: false })).toBe("name");
    expect(scopeLabel({ stem: false, ext: true })).toBe("extension");
    expect(scopeLabel({ stem: true, ext: true })).toBe("name + ext");
  });
});
