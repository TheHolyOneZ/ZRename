import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { MissingToken, ScanOptions, ThemeName } from "../types";

interface SettingsState {
  theme: ThemeName;
  comfortable: boolean;
  paranoid: boolean;
  placeholder: string;
  missingToken: MissingToken;
  hideUnchanged: boolean;
  collisionsFirst: boolean;

  columnSplit: number;
  dismissedFsNotice: boolean;
  longPaths: boolean;
  recentFolders: string[];

  lastRoots: string[];
  lastScanOptions: ScanOptions | null;
  lastPreset: string | null;
  setTheme: (t: ThemeName) => void;
  set: <K extends keyof SettingsState>(key: K, value: SettingsState[K]) => void;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      theme: "bench",
      comfortable: false,
      paranoid: false,
      placeholder: "_",
      missingToken: "placeholder",
      hideUnchanged: false,
      collisionsFirst: true,
      columnSplit: 0.5,
      dismissedFsNotice: false,
      longPaths: true,
      recentFolders: [],
      lastRoots: [],
      lastScanOptions: null,
      lastPreset: null,
      setTheme: (theme) => {
        document.documentElement.dataset.theme = theme;
        set({ theme });
      },
      set: (key, value) => set({ [key]: value } as never),
    }),
    { name: "zrename.settings" },
  ),
);


export function initTheme() {
  const theme = useSettingsStore.getState().theme;
  document.documentElement.dataset.theme = theme;
}


export function rowHeight(comfortable: boolean): number {
  return comfortable ? 34 : 26;
}
