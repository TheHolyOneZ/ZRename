import { create } from "zustand";
import { persist } from "zustand/middleware";
import { defaultRule, newId } from "../lib/rules";
import type { RuleName, RuleSpec } from "../types";

interface RuleState {
  rules: RuleSpec[];
  selectedId: string | null;
  setRules: (rules: RuleSpec[]) => void;
  add: (kind: RuleName) => void;
  update: (id: string, patch: Partial<RuleSpec>) => void;
  remove: (id: string) => void;
  duplicate: (id: string) => void;
  toggle: (id: string) => void;
  move: (from: number, to: number) => void;
  nudge: (id: string, delta: number) => void;
  select: (id: string | null) => void;
  clear: () => void;
}

export const useRuleStore = create<RuleState>()(
    persist(
    (set, get) => ({
    rules: [],
    selectedId: null,

    setRules: (rules) =>
      set({ rules, selectedId: rules.length > 0 ? rules[0].id : null }),

    add: (kind) => {
      const rule = defaultRule(kind);
      set((s) => ({ rules: [...s.rules, rule], selectedId: rule.id }));
    },

    update: (id, patch) =>
      set((s) => ({
        rules: s.rules.map((r) => (r.id === id ? ({ ...r, ...patch } as RuleSpec) : r)),
      })),

    remove: (id) =>
      set((s) => {
        const at = s.rules.findIndex((r) => r.id === id);
        const rules = s.rules.filter((r) => r.id !== id);

        const next = rules[Math.min(at, rules.length - 1)];
        return { rules, selectedId: s.selectedId === id ? (next?.id ?? null) : s.selectedId };
      }),

    duplicate: (id) =>
      set((s) => {
        const at = s.rules.findIndex((r) => r.id === id);
        if (at < 0) return s;
        const copy = { ...s.rules[at], id: newId() } as RuleSpec;
        const rules = [...s.rules];
        rules.splice(at + 1, 0, copy);
        return { rules, selectedId: copy.id };
      }),

    toggle: (id) =>
      set((s) => ({
        rules: s.rules.map((r) => (r.id === id ? { ...r, enabled: !r.enabled } : r)),
      })),

    move: (from, to) =>
      set((s) => {
        if (from === to || from < 0 || to < 0 || from >= s.rules.length || to >= s.rules.length) {
          return s;
        }
        const rules = [...s.rules];
        const [moved] = rules.splice(from, 1);
        rules.splice(to, 0, moved);
        return { rules };
      }),

    nudge: (id, delta) => {
      const { rules, move } = get();
      const at = rules.findIndex((r) => r.id === id);
      if (at < 0) return;
      move(at, Math.max(0, Math.min(rules.length - 1, at + delta)));
    },

    select: (selectedId) => set({ selectedId }),

      clear: () => set({ rules: [], selectedId: null }),
    }),
    {
      name: "zrename.rules",

      partialize: (s) => ({ rules: s.rules }) as RuleState,
    },
  ),
);

export function selectedRule(): RuleSpec | null {
  const { rules, selectedId } = useRuleStore.getState();
  return rules.find((r) => r.id === selectedId) ?? null;
}
