"use client";

import { type ReactNode } from "react";

type BadgeVariant = "default" | "success" | "warning" | "error" | "info" | "violet";

interface BadgeProps {
  children: ReactNode;
  variant?: BadgeVariant;
  dot?: boolean;
  className?: string;
}

const variantStyles: Record<BadgeVariant, { bg: string; text: string; dot: string }> = {
  default: {
    bg: "bg-zinc-800/80",
    text: "text-zinc-300",
    dot: "bg-zinc-500",
  },
  success: {
    bg: "bg-emerald-500/10",
    text: "text-emerald-400",
    dot: "bg-emerald-500",
  },
  warning: {
    bg: "bg-amber-500/10",
    text: "text-amber-400",
    dot: "bg-amber-500",
  },
  error: {
    bg: "bg-red-500/10",
    text: "text-red-400",
    dot: "bg-red-500",
  },
  info: {
    bg: "bg-sky-500/10",
    text: "text-sky-400",
    dot: "bg-sky-500",
  },
  violet: {
    bg: "bg-violet-500/10",
    text: "text-violet-400",
    dot: "bg-violet-500",
  },
};

export function Badge({ children, variant = "default", dot = false, className = "" }: BadgeProps) {
  const styles = variantStyles[variant];

  return (
    <span
      className={`
        inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium
        border border-transparent
        ${styles.bg} ${styles.text}
        ${className}
      `}
    >
      {dot && <span className={`w-1.5 h-1.5 rounded-full ${styles.dot} animate-pulse`} />}
      {children}
    </span>
  );
}

// Specialized status badge for enabled/disabled
interface StatusBadgeProps {
  enabled: boolean;
  className?: string;
}

export function StatusBadge({ enabled, className = "" }: StatusBadgeProps) {
  return (
    <Badge variant={enabled ? "success" : "default"} dot className={className}>
      {enabled ? "Enabled" : "Disabled"}
    </Badge>
  );
}

// Auth mode badge
interface AuthModeBadgeProps {
  mode: "disabled" | "apiKeyInitializeOnly" | "apiKeyEveryRequest" | "jwtEveryRequest";
  className?: string;
}

const authModeLabels: Record<AuthModeBadgeProps["mode"], { label: string; variant: BadgeVariant }> =
  {
    disabled: { label: "No Auth", variant: "warning" },
    apiKeyInitializeOnly: { label: "API Key (init)", variant: "info" },
    apiKeyEveryRequest: { label: "API Key (all)", variant: "info" },
    jwtEveryRequest: { label: "JWT", variant: "violet" },
  };

export function AuthModeBadge({ mode, className = "" }: AuthModeBadgeProps) {
  const { label, variant } = authModeLabels[mode];
  return (
    <Badge variant={variant} className={className}>
      {label}
    </Badge>
  );
}
