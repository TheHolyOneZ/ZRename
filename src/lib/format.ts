import type { DiffSpan } from "../types";

export interface Part { text: string; ws: boolean }


export function edgeBounds(name: string): { leadEnd: number; trailStart: number } {
  let leadEnd = 0;
  while (leadEnd < name.length && isSpace(name[leadEnd])) leadEnd++;

  let trailStart = name.length;
  while (trailStart > leadEnd && isSpace(name[trailStart - 1])) trailStart--;

  return { leadEnd, trailStart };
}

function isSpace(c: string): boolean {
  return c === " " || c === "\t" || c === " ";
}


export function splitEdges(
  text: string,
  absStart: number,
  bounds: { leadEnd: number; trailStart: number },
): Part[] {
  const parts: Part[] = [];
  let run = "";
  let runWs: boolean | null = null;

  for (let i = 0; i < text.length; i++) {
    const abs = absStart + i;
    const ws = isSpace(text[i]) && (abs < bounds.leadEnd || abs >= bounds.trailStart);
    if (runWs === null || ws === runWs) {
      run += text[i];
      runWs = ws;
    } else {
      parts.push({ text: run, ws: runWs });
      run = text[i];
      runWs = ws;
    }
  }
  if (run.length > 0) parts.push({ text: run, ws: runWs ?? false });
  return parts;
}


export function sideSpans(spans: DiffSpan[], side: "old" | "new"): DiffSpan[] {
  const drop = side === "old" ? "insert" : "delete";
  return spans.filter((s) => s.op !== drop);
}


export function sideText(spans: DiffSpan[], side: "old" | "new"): string {
  return sideSpans(spans, side)
    .map((s) => s.text)
    .join("");
}

export function formatCount(n: number): string {
  return n.toLocaleString("en-US");
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}


export function formatBatchTime(id: string): string {
  const m = id.match(/^(\d{4})-(\d{2})-(\d{2})T(\d{2})-(\d{2})-(\d{2})/);
  if (!m) return id;
  const [, y, mo, d, h, min] = m;
  const today = new Date();
  const sameDay =
    today.getFullYear() === Number(y) &&
    today.getMonth() + 1 === Number(mo) &&
    today.getDate() === Number(d);
  return sameDay ? `${h}:${min}` : `${d}/${mo} ${h}:${min}`;
}


export function middleEllipsis(s: string, max: number): string {
  if (s.length <= max) return s;
  const keep = Math.max(1, Math.floor((max - 1) / 2));
  return `${s.slice(0, keep)}…${s.slice(s.length - (max - 1 - keep))}`;
}


export function parseSize(input: string): number | null | undefined {
  const text = input.trim();
  if (text === "") return null;

  const m = /^(\d+(?:[.,]\d+)?)\s*([a-z]*)$/i.exec(text);
  if (!m) return undefined;

  const value = Number(m[1].replace(",", "."));
  if (!Number.isFinite(value) || value < 0) return undefined;

  const units: Record<string, number> = {
    "": 1, b: 1,
    k: 1024, kb: 1024, kib: 1024,
    m: 1024 ** 2, mb: 1024 ** 2, mib: 1024 ** 2,
    g: 1024 ** 3, gb: 1024 ** 3, gib: 1024 ** 3,
    t: 1024 ** 4, tb: 1024 ** 4, tib: 1024 ** 4,
  };
  const mult = units[m[2].toLowerCase()];
  if (mult === undefined) return undefined;

  return Math.round(value * mult);
}


export function parseDate(input: string, endOfDay = false): number | null | undefined {
  const text = input.trim();
  if (text === "") return null;
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text);
  if (!m) return undefined;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  if (Number.isNaN(d.getTime())) return undefined;
  if (endOfDay) d.setHours(23, 59, 59, 999);
  return Math.floor(d.getTime() / 1000);
}
