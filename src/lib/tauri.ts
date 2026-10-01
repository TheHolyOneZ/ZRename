import { invoke } from "@tauri-apps/api/core";
import type {
  ApplyResult, Capabilities, DupeGroup, HistoryEntry, Preset, RegexTest,
  RowPage, RowQuery, RuleSpec, ScanOptions, ScanResult, StartupArgs, Summary,
  UndoResult, ConflictPolicy, MissingToken,
} from "../types";

let queue: Promise<unknown> = Promise.resolve();

function serial<T>(run: () => Promise<T>): Promise<T> {
  const next = queue.then(run, run);
  queue = next.catch(() => {});
  return next;
}

export const api = {
  startupArgs: () => invoke<StartupArgs>("startup_args"),

  capabilities: () => invoke<Capabilities>("capabilities"),

  scanPaths: (paths: string[], options: ScanOptions) =>
    serial(() => invoke<ScanResult>("scan_paths", { paths, options })),

  setRules: (rules: RuleSpec[]) => serial(() => invoke<Summary>("set_rules", { rules })),

  setConflictPolicy: (policy: ConflictPolicy) =>
    serial(() => invoke<Summary>("set_conflict_policy", { policy })),

  setPlaceholder: (placeholder: string) =>
    serial(() => invoke<Summary>("set_placeholder", { placeholder })),

  setScanOptions: (options: ScanOptions) =>
    serial(() => invoke<Summary>("set_scan_options", { options })),

  getRows: (query: RowQuery) => invoke<RowPage>("get_rows", { query }),

  setRowExcluded: (index: number, excluded: boolean) =>
    serial(() => invoke<Summary>("set_row_excluded", { index, excluded })),

  excludeRows: (indices: number[], excluded: boolean) =>
    serial(() => invoke<Summary>("exclude_rows", { indices, excluded })),

  clearExclusions: () => serial(() => invoke<Summary>("clear_exclusions")),

  setLongPaths: (enabled: boolean) => serial(() => invoke<Summary>("set_long_paths", { enabled })),

  setMissingToken: (policy: MissingToken) =>
    serial(() => invoke<Summary>("set_missing_token", { policy })),

  rescan: () => serial(() => invoke<Summary>("rescan")),

  closeFolder: () => serial(() => invoke<Summary>("close_folder")),

  apply: (preset: string | null, paranoid: boolean) =>
    serial(() => invoke<ApplyResult>("apply", { preset, paranoid })),

  listHistory: () => invoke<HistoryEntry[]>("list_history"),

  clearHistory: () => invoke<number>("clear_history"),

  undoBatch: (id: string | null, force: boolean) =>
    serial(() => invoke<UndoResult>("undo_batch", { id, force })),

  listPresets: () => invoke<Preset[]>("list_presets"),
  savePreset: (preset: Preset) => invoke<string>("save_preset", { preset }),
  deletePreset: (name: string) => invoke<void>("delete_preset", { name }),
  importPreset: (path: string) => invoke<Preset>("import_preset", { path }),
  exportPreset: (preset: Preset, path: string) =>
    invoke<void>("export_preset", { preset, path }),

  regexTest: (pattern: string, sample: string, replacement: string, caseSensitive: boolean) =>
    invoke<RegexTest>("regex_test", {
      pattern, sample, replacement, caseSensitive,
    }),

  exportPlan: (format: "csv" | "markdown") =>
    invoke<string>("export_plan", { format }),

  findDupes: () => invoke<DupeGroup[]>("find_dupes"),

  watchStart: (preset: string | null) => invoke<void>("watch_start", { preset }),
  watchStop: () => invoke<void>("watch_stop"),
  watchStatus: () => invoke<boolean>("watch_status"),
};


export function describeError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
