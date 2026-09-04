import { describe, expect, it } from "vitest";
import {
  edgeBounds, formatBatchTime, formatBytes, formatCount,
  middleEllipsis, parseDate, parseSize, sideText, splitEdges,
} from "./format";
import type { DiffSpan } from "../types";

describe("edgeBounds", () => {
  it("finds leading and trailing whitespace", () => {
    expect(edgeBounds("  a  ")).toEqual({ leadEnd: 2, trailStart: 3 });
    expect(edgeBounds("clean")).toEqual({ leadEnd: 0, trailStart: 5 });
    expect(edgeBounds("trailing ")).toEqual({ leadEnd: 0, trailStart: 8 });
    expect(edgeBounds(" leading")).toEqual({ leadEnd: 1, trailStart: 8 });
  });

  it("handles a name that is entirely whitespace", () => {
    expect(edgeBounds("   ")).toEqual({ leadEnd: 3, trailStart: 3 });
    expect(edgeBounds("")).toEqual({ leadEnd: 0, trailStart: 0 });
  });

  it("leaves interior spaces alone", () => {
    const b = edgeBounds("scan  03.pdf");
    expect(b.leadEnd).toBe(0);
    expect(b.trailStart).toBe("scan  03.pdf".length);
  });
});

describe("splitEdges", () => {
  it("marks only whitespace at the ends", () => {
    const name = " scan 03 ";
    const parts = splitEdges(name, 0, edgeBounds(name));
    expect(parts).toEqual([
      { text: " ", ws: true },
      { text: "scan 03", ws: false },
      { text: " ", ws: true },
    ]);
  });

  it("works on a span that starts partway through the name", () => {
    const name = "report  ";
    const bounds = edgeBounds(name);
    expect(splitEdges("  ", 6, bounds)).toEqual([{ text: "  ", ws: true }]);
    expect(splitEdges("report", 0, bounds)).toEqual([{ text: "report", ws: false }]);
  });

  it("returns nothing for empty text", () => {
    expect(splitEdges("", 0, { leadEnd: 0, trailStart: 0 })).toEqual([]);
  });

  it("keeps the text intact when reassembled", () => {
    const name = "  a b  ";
    const parts = splitEdges(name, 0, edgeBounds(name));
    expect(parts.map((p) => p.text).join("")).toBe(name);
  });
});

describe("sideText", () => {
  const spans: DiffSpan[] = [
    { op: "equal", text: "photo." },
    { op: "delete", text: "JPG" },
    { op: "insert", text: "jpg" },
  ];

  it("rebuilds both sides of the diff", () => {
    expect(sideText(spans, "old")).toBe("photo.JPG");
    expect(sideText(spans, "new")).toBe("photo.jpg");
  });

  it("handles an empty diff", () => {
    expect(sideText([], "old")).toBe("");
  });
});

describe("formatting helpers", () => {
  it("groups thousands the way the commit bar shows them", () => {
    expect(formatCount(1281)).toBe("1,281");
    expect(formatCount(0)).toBe("0");
    expect(formatCount(1000000)).toBe("1,000,000");
  });

  it("scales byte counts", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
    expect(formatBytes(20 * 1024 * 1024)).toBe("20 MB");
  });

  it("reads a batch id as a clock time", () => {
    expect(formatBatchTime("2026-09-03T14-02-11.123")).toMatch(/14:02/);
    expect(formatBatchTime("not-an-id")).toBe("not-an-id");
  });

  it("truncates paths in the middle so the filename survives", () => {
    const p = "/home/z/Pictures/import/holiday/IMG_4821.JPG";
    const out = middleEllipsis(p, 20);
    expect(out.length).toBeLessThanOrEqual(20);
    expect(out).toContain("…");
    expect(out.endsWith("JPG")).toBe(true);
    expect(middleEllipsis("short", 20)).toBe("short");
  });
});

describe("parseSize", () => {
  it("reads the units people actually type", () => {
    expect(parseSize("512")).toBe(512);
    expect(parseSize("512B")).toBe(512);
    expect(parseSize("10k")).toBe(10240);
    expect(parseSize("10KB")).toBe(10240);
    expect(parseSize("10 kib")).toBe(10240);
    expect(parseSize("10MB")).toBe(10 * 1024 ** 2);
    expect(parseSize("1.5GB")).toBe(Math.round(1.5 * 1024 ** 3));
    expect(parseSize("1,5 GB")).toBe(Math.round(1.5 * 1024 ** 3));
  });

  it("treats blank as no bound, and nonsense as an error", () => {
    expect(parseSize("")).toBeNull();
    expect(parseSize("   ")).toBeNull();
    expect(parseSize("big")).toBeUndefined();
    expect(parseSize("10 furlongs")).toBeUndefined();
    expect(parseSize("-5MB")).toBeUndefined();
  });
});

describe("parseDate", () => {
  it("reads a date field and can bound the whole day", () => {
    const start = parseDate("2026-08-14")!;
    const end = parseDate("2026-08-14", true)!;
    expect(end - start).toBe(86399);
    expect(new Date(start * 1000).getFullYear()).toBe(2026);
  });

  it("treats blank as no bound, and nonsense as an error", () => {
    expect(parseDate("")).toBeNull();
    expect(parseDate("14/08/2026")).toBeUndefined();
  });
});
