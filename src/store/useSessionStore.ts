import { create } from "zustand";
import { api, describeError } from "../lib/tauri";
import { toast } from "./useToastStore";
import { useSettingsStore } from "./useSettingsStore";
import { useRuleStore } from "./useRuleStore";
import { usePreviewStore } from "./usePreviewStore";
import {
  defaultScanOptions, emptySummary,
  type ApplyResult, type Capabilities, type ConflictPolicy,
  type HistoryEntry, type RuleSpec, type ScanOptions, type ScanResult, type Summary,
} from "../types";

interface SessionState {
  caps: Capabilities | null;
  scan: ScanResult | null;
  summary: Summary;
  scanOptions: ScanOptions;
  conflict: ConflictPolicy;
  presetName: string | null;
  history: HistoryEntry[];
  busy: boolean;
  planning: boolean;

  lastApply: ApplyResult | null;

  init: () => Promise<void>;
  load: (paths: string[], remember?: boolean) => Promise<void>;
  replan: (rules: RuleSpec[]) => void;
  setScanOptions: (patch: Partial<ScanOptions>) => Promise<void>;
  setConflict: (policy: ConflictPolicy) => Promise<void>;
  rescan: (announce?: boolean) => Promise<void>;
  apply: () => Promise<void>;
  undo: (id: string | null, force?: boolean) => Promise<void>;
  refreshHistory: () => Promise<void>;
  setRowExcluded: (index: number, excluded: boolean) => Promise<void>;
  excludeRows: (indices: number[], excluded: boolean) => Promise<void>;
  clearExclusions: () => Promise<void>;
  setLongPaths: (enabled: boolean) => Promise<void>;
  setMissingToken: (policy: import("../types").MissingToken) => Promise<void>;
  setPresetName: (name: string | null) => void;
  dismissConfirmation: () => void;
}


let planTimer: ReturnType<typeof setTimeout> | null = null;
let planSeq = 0;
let confirmTimer: ReturnType<typeof setTimeout> | null = null;

export const useSessionStore = create<SessionState>((set, get) => ({
  caps: null,
  scan: null,
  summary: emptySummary(),
  scanOptions: defaultScanOptions(),
  conflict: "stop",
  presetName: null,
  history: [],
  busy: false,
  planning: false,
  lastApply: null,

  init: async () => {
    try {
      set({ caps: await api.capabilities() });
      await get().refreshHistory();
      const settings = useSettingsStore.getState();
      const args = await api.startupArgs();
      if (args.preset) {
        const match = (await api.listPresets()).find(
          (p) => p.name.toLowerCase() === args.preset!.toLowerCase(),
        );
        if (match) {
          useRuleStore.getState().setRules(
            match.rules.map((r) => ({ ...r, id: r.id || crypto.randomUUID() })),
          );
          set({ presetName: match.name });
          if (match.scan) set({ scanOptions: match.scan });
        } else {
          toast.warn(`No preset called \u201c${args.preset}\u201d`);
        }
      }
      if (args.paths.length > 0) {
        await get().load(args.paths);
        return;
      }


      if (settings.lastScanOptions) set({ scanOptions: settings.lastScanOptions });
      if (settings.lastPreset && !get().presetName) set({ presetName: settings.lastPreset });
      if (settings.lastRoots.length > 0) await get().load(settings.lastRoots, false);
    } catch (e) {
      toast.error("Could not start up", describeError(e));
    }
  },

  load: async (paths, remember = true) => {
    if (paths.length === 0) return;
    set({ busy: true });
    try {
      const scan = await api.scanPaths(paths, get().scanOptions);
      usePreviewStore.getState().reset();
      set({ scan });

      if (remember) {
        const s = useSettingsStore.getState();
        s.set("lastRoots", paths);
        s.set(
          "recentFolders",
          [...paths, ...s.recentFolders.filter((p) => !paths.includes(p))].slice(0, 8),
        );
      }
      if (scan.total === 0) {
        toast.warn("Nothing matched", "Check the filters, or turn on subfolders.");
      }
      get().replan(useRuleStore.getState().rules);
    } catch (e) {
      toast.error("Could not read that folder", describeError(e));
    } finally {
      set({ busy: false });
    }
  },

  replan: (rules) => {
    if (planTimer) clearTimeout(planTimer);
    set({ planning: true });
    const seq = ++planSeq;

    planTimer = setTimeout(async () => {
      try {
        const summary = await api.setRules(rules);
        if (seq !== planSeq) return;
        set({ summary, planning: false });
      } catch (e) {
        if (seq !== planSeq) return;
        set({ planning: false });
        toast.error("That rule could not run", describeError(e));
      }
    }, 110);
  },

  setScanOptions: async (patch) => {
    const scanOptions = { ...get().scanOptions, ...patch };
    set({ scanOptions });
    useSettingsStore.getState().set("lastScanOptions", scanOptions);
    if (!get().scan) return;
    try {
      const summary = await api.setScanOptions(scanOptions);
      set({ summary });
    } catch (e) {
      toast.error("Filter is not valid", describeError(e));
    }
  },

  setConflict: async (conflict) => {
    set({ conflict });
    if (!get().scan) return;
    try {
      set({ summary: await api.setConflictPolicy(conflict) });
    } catch (e) {
      toast.error("Could not change the conflict policy", describeError(e));
    }
  },

  rescan: async (announce = true) => {
    if (!get().scan) return;
    set({ busy: true });
    try {
      const summary = await api.rescan();
      usePreviewStore.getState().reset();
      set({ summary });
      if (announce) toast.info("Re-read from disk");
    } catch (e) {
      toast.error("Could not re-read the folder", describeError(e));
    } finally {
      set({ busy: false });
    }
  },

  apply: async () => {
    const { summary, presetName } = get();
    if (!summary.canApply) return;
    set({ busy: true });
    try {
      const result = await api.apply(presetName, useSettingsStore.getState().paranoid);
      set({ lastApply: result });

      if (result.clean) {
        toast.success(`Renamed ${result.renamed} file${result.renamed === 1 ? "" : "s"}`);
      } else {
        toast.warn(
          `Renamed ${result.renamed}, with problems`,
          [
            result.failed.length ? `${result.failed.length} failed` : "",
            result.stranded.length ? `${result.stranded.length} left at a temporary name` : "",
          ]
            .filter(Boolean)
            .join(" · "),
        );
      }

      if (confirmTimer) clearTimeout(confirmTimer);
      confirmTimer = setTimeout(() => set({ lastApply: null }), 30_000);

      await get().rescan(false);
      await get().refreshHistory();
    } catch (e) {
      toast.error("Nothing was renamed", describeError(e));
    } finally {
      set({ busy: false });
    }
  },

  undo: async (id, force = false) => {
    set({ busy: true });
    try {
      const r = await api.undoBatch(id, force);
      if (r.clean) {
        toast.success(`Put ${r.reverted} file${r.reverted === 1 ? "" : "s"} back`);
      } else {
        toast.warn(
          `Reverted ${r.reverted} of ${r.total}`,
          r.skipped.length
            ? `${r.skipped.length} changed since the rename and were left alone.`
            : undefined,
        );
      }
      set({ lastApply: null });
      await get().rescan(false);
      await get().refreshHistory();
    } catch (e) {
      toast.error("Could not undo", describeError(e));
    } finally {
      set({ busy: false });
    }
  },

  refreshHistory: async () => {
    try {
      set({ history: await api.listHistory() });
    } catch {

    }
  },

  setRowExcluded: async (index, excluded) => {
    try {
      set({ summary: await api.setRowExcluded(index, excluded) });
    } catch (e) {
      toast.error("Could not change that row", describeError(e));
    }
  },

  excludeRows: async (indices, excluded) => {
    try {
      set({ summary: await api.excludeRows(indices, excluded) });
    } catch (e) {
      toast.error("Could not change those rows", describeError(e));
    }
  },

  clearExclusions: async () => {
    try {
      set({ summary: await api.clearExclusions() });
    } catch (e) {
      toast.error("Could not restore the rows", describeError(e));
    }
  },

  setLongPaths: async (enabled) => {
    try {
      set({ summary: await api.setLongPaths(enabled) });
    } catch (e) {
      toast.error("Could not change the path limit", describeError(e));
    }
  },

  setMissingToken: async (policy) => {
    try {
      set({ summary: await api.setMissingToken(policy) });
    } catch (e) {
      toast.error("Could not change that", describeError(e));
    }
  },

  setPresetName: (presetName) => {
    useSettingsStore.getState().set("lastPreset", presetName);
    set({ presetName });
  },

  dismissConfirmation: () => {
    if (confirmTimer) clearTimeout(confirmTimer);
    set({ lastApply: null });
  },
}));
