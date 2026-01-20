"use client";

import { useToastStore } from "@/src/lib/toast-store";

export function ToastViewport() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  return (
    <div className="fixed bottom-4 right-4 z-[100] w-[360px] max-w-[calc(100vw-2rem)] space-y-2">
      {toasts.map((t) => {
        const styles =
          t.variant === "success"
            ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-100"
            : t.variant === "error"
              ? "border-red-500/30 bg-red-500/10 text-red-100"
              : "border-zinc-700/60 bg-zinc-900/80 text-zinc-100";

        const titleColor =
          t.variant === "success"
            ? "text-emerald-200"
            : t.variant === "error"
              ? "text-red-200"
              : "text-zinc-200";

        return (
          <div
            key={t.id}
            className={`rounded-xl border backdrop-blur-sm shadow-lg shadow-black/30 ${styles}`}
          >
            <div className="p-4 flex items-start gap-3">
              <div className="min-w-0 flex-1">
                {t.title && <div className={`text-sm font-semibold ${titleColor}`}>{t.title}</div>}
                <div className="text-sm text-zinc-200/90 break-words">{t.message}</div>
              </div>
              <button
                onClick={() => dismiss(t.id)}
                className="shrink-0 p-1.5 rounded-lg text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/60 transition-colors"
                aria-label="Dismiss"
              >
                <XIcon className="w-4 h-4" />
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}

function XIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth={2}
    >
      <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
    </svg>
  );
}
