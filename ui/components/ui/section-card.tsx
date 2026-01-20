"use client";

import type { ReactNode } from "react";

export function SectionCard({
  title,
  subtitle,
  right,
  children,
  className,
  headerClassName,
  bodyClassName,
}: {
  title?: ReactNode;
  subtitle?: ReactNode;
  right?: ReactNode;
  children: ReactNode;
  className?: string;
  headerClassName?: string;
  bodyClassName?: string;
}) {
  return (
    <div
      className={`rounded-xl border border-zinc-800/60 bg-zinc-900/40 overflow-hidden ${
        className ?? ""
      }`.trim()}
    >
      {(title || subtitle || right) && (
        <div
          className={`px-5 py-4 border-b border-zinc-800/60 flex items-start justify-between gap-4 ${
            headerClassName ?? ""
          }`.trim()}
        >
          <div className="min-w-0">
            {title ? <div className="text-sm font-semibold text-zinc-100">{title}</div> : null}
            {subtitle ? <div className="mt-1 text-xs text-zinc-500">{subtitle}</div> : null}
          </div>
          {right ? <div className="shrink-0">{right}</div> : null}
        </div>
      )}
      <div className={`p-5 ${bodyClassName ?? ""}`.trim()}>{children}</div>
    </div>
  );
}
