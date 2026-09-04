import { create } from "zustand";

export type ToastKind = "info" | "success" | "warn" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
  detail?: string;
}

interface ToastState {
  toasts: Toast[];
  push: (kind: ToastKind, message: string, detail?: string) => number;
  dismiss: (id: number) => void;
}

let nextId = 1;

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],
  push: (kind, message, detail) => {
    const id = nextId++;
    set((s) => ({ toasts: [...s.toasts, { id, kind, message, detail }] }));

    if (kind !== "error") {
      setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), 4200);
    }
    return id;
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

export const toast = {
  info: (m: string, d?: string) => useToastStore.getState().push("info", m, d),
  success: (m: string, d?: string) => useToastStore.getState().push("success", m, d),
  warn: (m: string, d?: string) => useToastStore.getState().push("warn", m, d),
  error: (m: string, d?: string) => useToastStore.getState().push("error", m, d),
};
