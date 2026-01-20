"use client";

import { create } from "zustand";

export type ToastVariant = "success" | "error" | "info";

export type Toast = {
  id: string;
  title?: string;
  message: string;
  variant: ToastVariant;
  createdAtUnixMs: number;
};

type ToastInput = {
  title?: string;
  message: string;
  variant?: ToastVariant;
  timeoutMs?: number;
};

type ToastState = {
  toasts: Toast[];
  push: (toast: ToastInput) => string;
  dismiss: (id: string) => void;
  clear: () => void;
};

function newId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return Math.random().toString(16).slice(2) + Date.now().toString(16);
}

export const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],
  push: (toast) => {
    const id = newId();
    const variant: ToastVariant = toast.variant ?? "info";
    const createdAtUnixMs = Date.now();
    const entry: Toast = {
      id,
      title: toast.title,
      message: toast.message,
      variant,
      createdAtUnixMs,
    };
    set((s) => ({ toasts: [...s.toasts, entry] }));

    const timeoutMs = toast.timeoutMs ?? (variant === "error" ? 7000 : 4000);
    if (timeoutMs > 0) {
      setTimeout(() => {
        get().dismiss(id);
      }, timeoutMs);
    }

    return id;
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  clear: () => set({ toasts: [] }),
}));
