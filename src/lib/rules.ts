import type { RuleKind, RuleName, RuleSpec, Scope } from "../types";

export const RULE_LABELS: Record<RuleName, string> = {
  replace: "Find & Replace",
  case: "Case",
  insert: "Insert",
  remove: "Remove",
  trim: "Trim",
  number: "Numbering",
  extension: "Extension",
  sanitise: "Sanitise",
  template: "Template",
  move_into: "Move into folders",
  csv_map: "CSV mapping",
};


export const RULE_ORDER: RuleName[] = [
  "replace", "case", "insert", "remove", "trim",
  "number", "extension", "sanitise", "template", "move_into", "csv_map",
];

export const RULE_HINTS: Record<RuleName, string> = {
  replace: "Swap text, plainly or by regex with $1 capture groups",
  case: "lower, UPPER, Title, camel, snake, kebab",
  insert: "Add text at a position or around a marker",
  remove: "Delete a range, a set of characters, or every digit",
  trim: "Strip surrounding whitespace and collapse runs of spaces",
  number: "Sequential numbering with padding and per-folder reset",
  extension: "Change, lowercase or drop the extension",
  sanitise: "Remove illegal characters and transliterate accents",
  template: "Rebuild the name from tokens such as %exif:DateTimeOriginal%",
  move_into: "File results into subfolders derived from tokens",
  csv_map: "Rename from an external old,new list",
};

export const defaultScope = (): Scope => ({ stem: true, ext: false });

export function newId(): string {
  return crypto.randomUUID();
}


export function defaultRule(kind: RuleName): RuleSpec {
  const base = { id: newId(), enabled: true, scope: defaultScope() };
  const body = ((): RuleKind => {
    switch (kind) {
      case "replace":
        return { kind, find: "", with: "", regex: false, case_sensitive: false, all: true };
      case "case":
        return { kind, style: "lower" };
      case "insert":
        return { kind, text: "", at: "prefix" };
      case "remove":
        return { kind, what: "chars", chars: "" };
      case "trim":
        return { kind, whitespace: true, chars: "", collapse_spaces: true };
      case "number":
        return {
          kind, start: 1, step: 1, pad: 2, reset_per_folder: false,
          sort: "natural", descending: false, at: "suffix",
        };
      case "extension":
        return { kind, mode: "lower" };
      case "sanitise":
        return {
          kind, illegal: true, collapse_spaces: true, transliterate: false,
          replacement: "_", trim_dots_spaces: true,
        };
      case "template":
        return { kind, template: "" };
      case "move_into":
        return { kind, template: "" };
      case "csv_map":
        return { kind, path: "", match_full_name: false };
    }
  })();
  return { ...base, ...body } as RuleSpec;
}

function ellipsis(s: string, max = 22): string {
  if (s.length <= max) return s;
  return s.slice(0, max - 1) + "…";
}

function positionLabel(r: { at: string; index?: number; marker?: string }): string {
  switch (r.at) {
    case "prefix": return "at the start";
    case "suffix": return "at the end";
    case "index": return `at position ${r.index ?? 0}`;
    case "before": return `before “${ellipsis(r.marker ?? "", 10)}”`;
    case "after": return `after “${ellipsis(r.marker ?? "", 10)}”`;
    default: return "";
  }
}


export function summariseRule(rule: RuleSpec): string {
  switch (rule.kind) {
    case "replace": {
      if (!rule.find) return "not configured";
      const mode = rule.regex ? "regex" : "text";
      const to = rule.with ? ellipsis(rule.with) : "nothing";
      return `${mode} “${ellipsis(rule.find)}” → “${to}”`;
    }
    case "case": {
      const names: Record<string, string> = {
        lower: "lowercase", upper: "UPPERCASE", title: "Title Case",
        sentence: "Sentence case", camel: "camelCase", pascal: "PascalCase",
        snake: "snake_case", kebab: "kebab-case",
      };
      return names[rule.style] ?? rule.style;
    }
    case "insert":
      return rule.text
        ? `“${ellipsis(rule.text)}” ${positionLabel(rule)}`
        : "not configured";
    case "remove":
      switch (rule.what) {
        case "range": return `characters ${rule.from} to ${rule.to}`;
        case "chars": return rule.chars ? `characters “${ellipsis(rule.chars)}”` : "not configured";
        case "word": return rule.word ? `“${ellipsis(rule.word)}”${rule.all ? " everywhere" : " once"}` : "not configured";
        case "digits": return "every digit";
        case "duplicates": return rule.text ? `repeats of “${ellipsis(rule.text)}”` : "not configured";
      }
      return "";
    case "trim": {
      const bits: string[] = [];
      if (rule.whitespace) bits.push("whitespace");
      if (rule.chars) bits.push(`“${ellipsis(rule.chars, 8)}”`);
      if (rule.collapse_spaces) bits.push("collapse spaces");
      return bits.length ? bits.join(", ") : "nothing selected";
    }
    case "number": {
      const pad = rule.pad > 1 ? `, padded to ${rule.pad}` : "";
      const reset = rule.reset_per_folder ? ", per folder" : "";
      const step = rule.step !== 1 ? ` step ${rule.step}` : "";
      return `from ${rule.start}${step}${pad}${reset}, ${positionLabel(rule)}`;
    }
    case "extension":
      switch (rule.mode) {
        case "set": return rule.ext ? `set to .${rule.ext}` : "not configured";
        case "fill": return rule.ext ? `.${rule.ext} when missing` : "not configured";
        case "lower": return "lowercase";
        case "upper": return "UPPERCASE";
        case "remove": return "removed";
      }
      return "";
    case "sanitise": {
      const bits: string[] = [];
      if (rule.illegal) bits.push("illegal characters");
      if (rule.transliterate) bits.push("transliterate");
      if (rule.collapse_spaces) bits.push("collapse spaces");
      if (rule.trim_dots_spaces) bits.push("trim dots");
      return bits.length ? bits.join(", ") : "nothing selected";
    }
    case "template":
      return rule.template ? ellipsis(rule.template, 30) : "not configured";
    case "move_into":
      return rule.template ? `→ ${ellipsis(rule.template, 26)}` : "not configured";
    case "csv_map":
      return rule.path ? ellipsis(rule.path.split(/[\\/]/).pop() ?? rule.path, 26) : "no file chosen";
  }
}


export function isConfigured(rule: RuleSpec): boolean {
  switch (rule.kind) {
    case "replace": return rule.find.length > 0;
    case "insert": return rule.text.length > 0;
    case "template":
    case "move_into": return rule.template.length > 0;
    case "csv_map": return rule.path.length > 0;
    case "remove":
      if (rule.what === "chars") return rule.chars.length > 0;
      if (rule.what === "word") return rule.word.length > 0;
      if (rule.what === "duplicates") return rule.text.length > 0;
      return true;
    case "extension":
      if (rule.mode === "set" || rule.mode === "fill") return rule.ext.length > 0;
      return true;
    default:
      return true;
  }
}

export function scopeLabel(scope: Scope): string {
  if (scope.stem && scope.ext) return "name + ext";
  if (scope.ext) return "extension";
  return "name";
}
