"use client";

import { useMemo } from "react";
import { useCopyToClipboard } from "@/src/lib/useCopyToClipboard";
import { CheckIcon, CopyIcon } from "@/components/icons";

interface CopyBlockProps {
  value: string;
  label?: string;
  language?: "url" | "json" | "bash" | "text";
  compact?: boolean;
}

export function CopyBlock({ value, label, language = "text", compact = false }: CopyBlockProps) {
  const { copied, copy } = useCopyToClipboard(value);

  const syntaxHighlight = useMemo(
    () => (text: string) => {
      if (language === "url") {
        // Highlight protocol and path segments
        return text.replace(
          /(https?:\/\/)?([^/]+)(\/[^\s]*)?/g,
          (_, protocol = "", host, path = "") => {
            return `<span class="text-zinc-500">${protocol}</span><span class="text-violet-400">${host}</span><span class="text-emerald-400">${path}</span>`;
          },
        );
      }
      if (language === "json") {
        return text
          .replace(/"([^"]+)":/g, '<span class="text-violet-400">"$1"</span>:')
          .replace(/: "([^"]+)"/g, ': <span class="text-emerald-400">"$1"</span>');
      }
      if (language === "bash") {
        return text.replace(/^(\$|>)\s*/gm, '<span class="text-zinc-500">$1 </span>');
      }
      return text;
    },
    [language],
  );

  if (compact) {
    return (
      <div className="group flex items-center gap-2">
        <code className="flex-1 min-w-0 truncate text-sm font-mono text-zinc-300">{value}</code>
        <button
          onClick={copy}
          type="button"
          className={`
            shrink-0 p-1.5 rounded-md transition-all duration-150
            ${
              copied
                ? "bg-emerald-500/20 text-emerald-400"
                : "text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800 opacity-0 group-hover:opacity-100"
            }
          `}
          title="Copy to clipboard"
        >
          {copied ? <CheckIcon className="w-4 h-4" /> : <CopyIcon className="w-4 h-4" />}
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-1.5">
      {label && (
        <div className="flex items-center justify-between">
          <span className="text-xs font-medium text-zinc-400 uppercase tracking-wide">{label}</span>
          <button
            onClick={copy}
            type="button"
            className={`
              flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium transition-all duration-150
              ${
                copied
                  ? "bg-emerald-500/20 text-emerald-400"
                  : "text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800"
              }
            `}
          >
            {copied ? (
              <>
                <CheckIcon className="w-3.5 h-3.5" />
                Copied
              </>
            ) : (
              <>
                <CopyIcon className="w-3.5 h-3.5" />
                Copy
              </>
            )}
          </button>
        </div>
      )}
      <div className="relative group">
        <pre className="p-3 rounded-lg bg-zinc-900/80 border border-zinc-800 overflow-x-auto">
          <code
            className="text-sm font-mono text-zinc-300 whitespace-pre-wrap break-all"
            dangerouslySetInnerHTML={{ __html: syntaxHighlight(value) }}
          />
        </pre>
        {!label && (
          <button
            onClick={copy}
            type="button"
            className={`
              absolute top-2 right-2 p-1.5 rounded-md transition-all duration-150
              ${
                copied
                  ? "bg-emerald-500/20 text-emerald-400"
                  : "text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800 opacity-0 group-hover:opacity-100"
              }
            `}
            title="Copy to clipboard"
          >
            {copied ? <CheckIcon className="w-4 h-4" /> : <CopyIcon className="w-4 h-4" />}
          </button>
        )}
      </div>
    </div>
  );
}
