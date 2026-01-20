"use client";

import { useCopyToClipboard } from "@/src/lib/useCopyToClipboard";
import { CheckIcon, CopyIcon } from "@/components/icons";

export function CopyButton({
  text,
  className = "",
  copiedLabel = "Copied",
  label = "Copy",
  size = "sm",
  variant = "button",
}: {
  text: string;
  className?: string;
  label?: string;
  copiedLabel?: string;
  size?: "sm" | "md";
  variant?: "button" | "icon";
}) {
  const { copied, copy } = useCopyToClipboard(text);

  const base =
    variant === "icon"
      ? "p-1.5 rounded transition-colors"
      : "rounded-lg bg-zinc-800 text-zinc-300 text-xs font-medium hover:bg-zinc-700 hover:text-white transition-colors";
  const padding = variant === "icon" ? "" : size === "md" ? "px-3 py-2.5" : "px-3 py-2";

  return (
    <button
      onClick={async () => {
        await copy();
      }}
      className={`${base} ${padding} ${className}`.trim()}
      type="button"
    >
      {variant === "icon" ? (
        copied ? (
          <CheckIcon className="w-4 h-4 text-emerald-400" />
        ) : (
          <CopyIcon className="w-4 h-4" />
        )
      ) : copied ? (
        <span className="flex items-center gap-1.5">
          <CheckIcon className="w-3.5 h-3.5 text-emerald-400" />
          {copiedLabel}
        </span>
      ) : (
        <span className="flex items-center gap-1.5">
          <CopyIcon className="w-3.5 h-3.5" />
          {label}
        </span>
      )}
    </button>
  );
}
