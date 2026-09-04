

export type Scope = { stem: boolean; ext: boolean };

export type CaseStyle =
  | "lower" | "upper" | "title" | "sentence"
  | "camel" | "pascal" | "snake" | "kebab";

export type SortKey = "name" | "natural" | "size" | "modified" | "created" | "scan";

export type ConflictPolicy = "stop" | "skip" | "suffix" | "overwrite";

export type MissingToken = "placeholder" | "skip";

export type InsertAt =
  | { at: "index"; index: number }
  | { at: "before"; marker: string; all: boolean }
  | { at: "after"; marker: string; all: boolean }
  | { at: "prefix" }
  | { at: "suffix" };

export type RemoveWhat =
  | { what: "range"; from: number; to: number }
  | { what: "chars"; chars: string }
  | { what: "word"; word: string; all: boolean }
  | { what: "digits" }
  | { what: "duplicates"; text: string };

export type ExtMode =
  | { mode: "set"; ext: string }
  | { mode: "lower" }
  | { mode: "upper" }
  | { mode: "remove" }
  | { mode: "fill"; ext: string };

export type RuleKind =
  | { kind: "replace"; find: string; with: string; regex: boolean; case_sensitive: boolean; all: boolean }
  | { kind: "case"; style: CaseStyle }
  | ({ kind: "insert"; text: string } & InsertAt)
  | ({ kind: "remove" } & RemoveWhat)
  | { kind: "trim"; whitespace: boolean; chars: string; collapse_spaces: boolean }
  | ({
      kind: "number";
      start: number;
      step: number;
      pad: number;
      reset_per_folder: boolean;
      sort: SortKey;
      descending: boolean;
    } & InsertAt)
  | ({ kind: "extension" } & ExtMode)
  | {
      kind: "sanitise";
      illegal: boolean;
      collapse_spaces: boolean;
      transliterate: boolean;
      replacement: string;
      trim_dots_spaces: boolean;
    }
  | { kind: "template"; template: string }
  | { kind: "move_into"; template: string }
  | { kind: "csv_map"; path: string; match_full_name: boolean };

export type RuleName = RuleKind["kind"];

export type RuleSpec = { id: string; enabled: boolean; scope: Scope } & RuleKind;

export interface ScanOptions {
  recursive: boolean;
  max_depth: number | null;
  include_files: boolean;
  include_dirs: boolean;
  include_hidden: boolean;
  follow_symlinks: boolean;
  include_globs: string[];
  exclude_globs: string[];
  extensions: string[];
  name_regex: string | null;
  min_size: number | null;
  max_size: number | null;
  modified_after: number | null;
  modified_before: number | null;
}

export const defaultScanOptions = (): ScanOptions => ({
  recursive: false,
  max_depth: null,
  include_files: true,
  include_dirs: false,
  include_hidden: false,
  follow_symlinks: false,
  include_globs: [],
  exclude_globs: [],
  extensions: [],
  name_regex: null,
  min_size: null,
  max_size: null,
  modified_after: null,
  modified_before: null,
});

export interface ScanResult {
  total: number;
  files: number;
  folders: number;
  roots: string[];
  fsName: string;
  caseInsensitive: boolean;
  maxPath: number | null;
  needsSanitising: boolean;
}

export interface Summary {
  total: number;
  changed: number;
  unchanged: number;
  collisions: number;
  invalid: number;
  skipped: number;
  tooLong: number;
  reserved: number;
  blocking: number;
  canApply: boolean;
  summaryLine: string;
  applyLabel: string;
  fsName: string;
}

export const emptySummary = (): Summary => ({
  total: 0, changed: 0, unchanged: 0, collisions: 0, invalid: 0,
  skipped: 0, tooLong: 0, reserved: 0, blocking: 0, canApply: false,
  summaryLine: "Nothing loaded", applyLabel: "Apply", fsName: "",
});

export type RowStatusKey =
  | "ok" | "unchanged" | "collision" | "invalid"
  | "too_long" | "reserved_name" | "skipped";

export type DiffOp = "equal" | "insert" | "delete";
export interface DiffSpan { op: DiffOp; text: string }

export interface Row {
  index: number;
  fromName: string;
  toName: string;
  fromPath: string;
  toPath: string;
  status: RowStatusKey;
  statusLabel: string;
  blocking: boolean;
  actionable: boolean;
  isDir: boolean;
  isSymlink: boolean;
  caseOnly: boolean;
  moved: boolean;
  excluded: boolean;
  diff: DiffSpan[];
}

export interface RowQuery {
  offset: number;
  limit: number;
  hideUnchanged: boolean;
  onlyProblems: boolean;
  search: string;
  collisionsFirst: boolean;
}

export interface RowPage { rows: Row[]; total: number }

export interface RegexTest {
  valid: boolean;
  error: string | null;
  matched: boolean;
  groups: string[];
  preview: string | null;
}

export interface ApplyResult {
  renamed: number;
  twoPhase: number;
  skipped: [string, string][];
  failed: [string, string][];
  stranded: string[];
  journalId: string | null;
  clean: boolean;
}

export interface UndoSkip { name: string; kind: string; detail: string }

export interface UndoResult {
  reverted: number;
  total: number;
  skipped: UndoSkip[];
  failed: [string, string][];
  clean: boolean;
}

export interface HistoryEntry {
  id: string;
  created: string;
  count: number;
  preset: string | null;
  roots: string[];
}

export interface Preset {
  name: string;
  description?: string | null;
  conflict?: ConflictPolicy | null;
  scan?: ScanOptions | null;
  rules: RuleSpec[];
}

export interface DupeGroup {
  hash: string;
  size: number;
  names: string[];
  paths: string[];
  indices: number[];
}

export interface Capabilities {
  ffprobe: boolean;
  presetDir: string;
  journalDir: string;
  version: string;
}

export type ThemeName = "bench" | "bench-light" | "nord" | "contrast";

export const THEMES: { id: ThemeName; label: string }[] = [
  { id: "bench", label: "Bench" },
  { id: "bench-light", label: "Bench Light" },
  { id: "nord", label: "Nord" },
  { id: "contrast", label: "Contrast" },
];

export interface StartupArgs {
  paths: string[];
  preset: string | null;
}
