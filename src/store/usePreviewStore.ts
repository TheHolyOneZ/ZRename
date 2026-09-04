import { create } from "zustand";
import { api, describeError } from "../lib/tauri";
import { toast } from "./useToastStore";
import type { Row } from "../types";


const OVERSCAN = 60;

interface PreviewState {
  start: number;
  rows: Row[];
  total: number;
  loading: boolean;
  search: string;
  onlyProblems: boolean;

  selected: number | null;
  selectedRow: Row | null;

  ensure: (first: number, last: number, filters: ViewFilters) => void;
  invalidate: (filters: ViewFilters) => void;
  setSearch: (search: string, filters: ViewFilters) => void;
  setOnlyProblems: (v: boolean, filters: ViewFilters) => void;
  select: (row: Row | null) => void;
  reset: () => void;
}

export interface ViewFilters {
  hideUnchanged: boolean;
  collisionsFirst: boolean;
}

let seq = 0;

export const usePreviewStore = create<PreviewState>((set, get) => ({
  start: 0,
  rows: [],
  total: 0,
  loading: false,
  search: "",
  onlyProblems: false,
  selected: null,
  selectedRow: null,

  ensure: (first, last, filters) => {
    const { start, rows, loading } = get();
    const covered = first >= start && last < start + rows.length;
    if (covered || loading) return;

    const offset = Math.max(0, first - OVERSCAN);
    const limit = last - first + 1 + OVERSCAN * 2;
    const mine = ++seq;
    set({ loading: true });

    api
      .getRows({
        offset,
        limit,
        hideUnchanged: filters.hideUnchanged,
        onlyProblems: get().onlyProblems,
        search: get().search,
        collisionsFirst: filters.collisionsFirst,
      })
      .then((page) => {
        if (mine !== seq) return;
        set({ start: offset, rows: page.rows, total: page.total, loading: false });
      })
      .catch((e) => {
        if (mine !== seq) return;
        set({ loading: false });
        toast.error("Could not read the preview", describeError(e));
      });
  },


  invalidate: (filters) => {
    seq++;
    set({ start: 0, rows: [], loading: false });


    get().ensure(0, 80, filters);
  },

  setSearch: (search, filters) => {
    set({ search });
    get().invalidate(filters);
  },

  setOnlyProblems: (onlyProblems, filters) => {
    set({ onlyProblems });
    get().invalidate(filters);
  },

  select: (row) => set({ selected: row?.index ?? null, selectedRow: row }),


  reset: () => set({ start: 0, rows: [], selected: null, selectedRow: null, loading: false }),
}));


export function rowAt(state: PreviewState, viewIndex: number): Row | undefined {
  return state.rows[viewIndex - state.start];
}
