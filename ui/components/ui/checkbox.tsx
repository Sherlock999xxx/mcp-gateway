"use client";

import { type ReactNode } from "react";

type CheckboxSize = "sm" | "md";

export function Checkbox({
  checked,
  onChange,
  label,
  description,
  disabled = false,
  size = "md",
  className = "",
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: ReactNode;
  description?: ReactNode;
  disabled?: boolean;
  size?: CheckboxSize;
  className?: string;
}) {
  const boxSize = size === "sm" ? "h-4 w-4" : "h-5 w-5";
  const iconSize = size === "sm" ? "h-3 w-3" : "h-3.5 w-3.5";

  return (
    <label
      className={`flex items-start gap-3 ${disabled ? "opacity-50 cursor-not-allowed" : "cursor-pointer"} ${className}`}
    >
      <input
        type="checkbox"
        className="sr-only peer"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span
        className={`
          ${boxSize} shrink-0 rounded-md border flex items-center justify-center
          transition-colors
          border-zinc-700/80 bg-zinc-900/60
          peer-checked:border-violet-500/60 peer-checked:bg-violet-500/15
          peer-focus-visible:outline-none peer-focus-visible:ring-2 peer-focus-visible:ring-violet-500/50 peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-zinc-900
        `}
      >
        <CheckIcon
          className={`${iconSize} ${checked ? "opacity-100" : "opacity-0"} text-violet-300 transition-opacity`}
        />
      </span>

      {(label || description) && (
        <div className="flex flex-col min-w-0">
          {label ? <span className="text-sm font-medium text-zinc-200">{label}</span> : null}
          {description ? <span className="text-xs text-zinc-500">{description}</span> : null}
        </div>
      )}
    </label>
  );
}

function CheckIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
      <path
        fillRule="evenodd"
        d="M16.704 5.29a1 1 0 010 1.42l-7.25 7.25a1 1 0 01-1.42 0l-3.25-3.25a1 1 0 011.42-1.42l2.54 2.54 6.54-6.54a1 1 0 011.42 0z"
        clipRule="evenodd"
      />
    </svg>
  );
}
