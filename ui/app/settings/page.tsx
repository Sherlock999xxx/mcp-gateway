"use client";

import { useState } from "react";
import { AppShell, PageContent, PageHeader } from "@/components/layout";
import { useQuery } from "@tanstack/react-query";
import { ConfirmModal, CopyButton } from "@/components/ui";
import { qk } from "@/src/lib/queryKeys";
import { clearTenantSessionCookies, getTenantExpFromCookies } from "@/src/lib/tenant-session";
import { LockIcon, ServerIconDevice, UserIcon } from "@/components/icons";
import { GATEWAY_DATA_BASE, UI_VERSION } from "@/src/lib/env";

export const dynamic = "force-dynamic";

type GatewayStatusResponse =
  | {
      ok: true;
      status: {
        version?: string;
        license?: string;
        uptimeSecs?: number;
        configLoaded?: boolean;
        profileCount?: number;
        oidcConfigured?: boolean;
        oidcIssuer?: string;
      };
    }
  | { ok: false; error?: string; status?: number };

export default function SettingsPage() {
  const [showConfirmLock, setShowConfirmLock] = useState(false);
  const dataBase = GATEWAY_DATA_BASE;
  const uiVersion = UI_VERSION;
  const exp = getTenantExpFromCookies();
  const expHuman = exp ? new Date(exp * 1000).toLocaleString() : "unknown";
  const gatewayStatusQuery = useQuery({
    queryKey: qk.gatewayStatus(),
    queryFn: async () => {
      const res = await fetch("/api/gateway/status", { cache: "no-store" });
      return (await res.json()) as GatewayStatusResponse;
    },
  });
  const gatewayStatus = gatewayStatusQuery.data ?? null;
  const gatewayVersionLabel = gatewayStatusQuery.isPending
    ? "loading…"
    : gatewayStatus?.ok
      ? gatewayStatus.status.version
        ? `v${gatewayStatus.status.version}`
        : "unknown"
      : "unavailable";
  const gatewayLicenseLabel = gatewayStatusQuery.isPending
    ? "loading…"
    : gatewayStatus?.ok
      ? (gatewayStatus.status.license ?? "unknown")
      : "unavailable";

  return (
    <AppShell>
      <PageHeader title="Settings" description="Gateway configuration and tenant settings" />

      <PageContent className="space-y-6">
        {/* Environment Info */}
        <section className="rounded-xl border border-zinc-800/60 bg-zinc-900/40 overflow-hidden">
          <div className="px-5 py-4 border-b border-zinc-800/60">
            <h2 className="text-sm font-semibold text-zinc-100 flex items-center gap-2">
              <ServerIconDevice className="w-5 h-5 text-violet-400" />
              Gateway Environment
            </h2>
          </div>
          <div className="p-5 space-y-4">
            <ConfigRow
              label="Data Plane URL"
              value={dataBase}
              description="Public URL for MCP client connections"
              copyable
            />
            <ConfigRow
              label="Gateway Status"
              value={
                gatewayStatusQuery.isPending ? "loading" : gatewayStatus?.ok ? "online" : "error"
              }
              description="Derived from the Gateway control plane /status"
            />
            <ConfigRow
              label="Operating Mode"
              value="Mode 3 (Postgres)"
              description="Multi-tenant mode with Postgres backend"
            />
            <ConfigRow
              label="Session Storage"
              value="Browser cookie"
              description="UI stores the tenant session in browser-set cookies (not httpOnly yet)"
            />
          </div>
        </section>

        {/* Current Tenant */}
        <section className="rounded-xl border border-zinc-800/60 bg-zinc-900/40 overflow-hidden">
          <div className="px-5 py-4 border-b border-zinc-800/60">
            <h2 className="text-sm font-semibold text-zinc-100 flex items-center gap-2">
              <UserIcon className="w-5 h-5 text-emerald-400" />
              Current Tenant
            </h2>
          </div>
          <div className="p-5 space-y-4">
            <ConfigRow
              label="Session Expires"
              value={expHuman}
              description="When the current unlock token expires"
            />
          </div>
          <div className="px-5 py-4 border-t border-zinc-800/60 bg-zinc-950/40">
            <button
              onClick={() => setShowConfirmLock(true)}
              className="inline-flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium text-amber-400 hover:text-amber-300 hover:bg-amber-500/10 transition-colors"
            >
              <LockIcon className="w-4 h-4" />
              Lock Tenant Session
            </button>
          </div>
        </section>

        {/* About */}
        <section className="rounded-xl border border-zinc-800/60 bg-zinc-900/40 p-5">
          <div className="flex items-center gap-4">
            <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-violet-500 to-violet-600 flex items-center justify-center shadow-lg shadow-violet-500/20">
              <span className="text-white font-black text-xl leading-none tracking-tight">U</span>
            </div>
            <div>
              <h3 className="text-sm font-semibold text-zinc-100">MCP Gateway</h3>
              <div className="text-xs text-zinc-500 mt-0.5">
                by{" "}
                <a
                  href="https://unrelated.ai"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-violet-400 hover:text-violet-300 transition-colors"
                >
                  unrelated.ai
                </a>
              </div>
            </div>
          </div>
          <div className="mt-4 pt-4 border-t border-zinc-800/60 grid grid-cols-3 gap-4 text-center">
            <div>
              <div className="text-sm font-semibold text-zinc-200">{gatewayVersionLabel}</div>
              <div className="text-xs text-zinc-500">Version</div>
            </div>
            <div>
              <div className="text-sm font-semibold text-zinc-200">{gatewayLicenseLabel}</div>
              <div className="text-xs text-zinc-500">License</div>
            </div>
            <div>
              <a
                href="https://github.com/unrelated-ai/mcp-gateway"
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm font-semibold text-violet-400 hover:text-violet-300 transition-colors"
              >
                GitHub
              </a>
              <div className="text-xs text-zinc-500">Repository</div>
            </div>
          </div>
        </section>

        {/* Web UI */}
        <section className="rounded-xl border border-zinc-800/60 bg-zinc-900/40 p-5">
          <div className="flex items-center gap-4">
            <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-emerald-500 to-emerald-600 flex items-center justify-center shadow-lg shadow-emerald-500/20">
              <span className="text-white font-black text-base leading-none tracking-tight">U</span>
            </div>
            <div>
              <h3 className="text-sm font-semibold text-zinc-100">Web UI</h3>
              <div className="text-xs text-zinc-500 mt-0.5">
                Dashboard for managing profiles and sources.
              </div>
            </div>
          </div>
          <div className="mt-4 pt-4 border-t border-zinc-800/60 grid grid-cols-3 gap-4 text-center">
            <div>
              <div className="text-sm font-semibold text-zinc-200">{uiVersion}</div>
              <div className="text-xs text-zinc-500">Version</div>
            </div>
            <div>
              <div className="text-sm font-semibold text-zinc-200">MIT</div>
              <div className="text-xs text-zinc-500">License</div>
            </div>
            <div>
              <a
                href="https://github.com/unrelated-ai/mcp-gateway"
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm font-semibold text-emerald-400 hover:text-emerald-300 transition-colors"
              >
                GitHub
              </a>
              <div className="text-xs text-zinc-500">Repository</div>
            </div>
          </div>
        </section>
      </PageContent>

      {/* Lock Confirmation Modal */}
      <ConfirmModal
        open={showConfirmLock}
        onClose={() => setShowConfirmLock(false)}
        onConfirm={() => {
          clearTenantSessionCookies();
          window.location.href = "/unlock";
        }}
        title="Lock session?"
        description="This will clear your session and return you to the unlock screen. You'll need your tenant token to access the dashboard again."
        confirmLabel="Lock Session"
      />
    </AppShell>
  );
}

function ConfigRow({
  label,
  value,
  description,
  copyable = false,
}: {
  label: string;
  value: string;
  description?: string;
  copyable?: boolean;
}) {
  return (
    <div className="flex items-start justify-between gap-4 py-2 border-b border-zinc-800/40 last:border-0 last:pb-0 first:pt-0">
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium text-zinc-200">{label}</div>
        {description && <div className="text-xs text-zinc-500 mt-0.5">{description}</div>}
      </div>
      <div className="flex items-center gap-2">
        <code
          title={value}
          className="text-sm font-mono text-zinc-400 bg-zinc-800/60 px-2 py-1 rounded truncate max-w-[260px] sm:max-w-[360px] md:max-w-[520px] lg:max-w-[680px]"
        >
          {value}
        </code>
        {copyable && (
          <CopyButton
            text={value}
            variant="icon"
            className="text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800 transition-colors"
          />
        )}
      </div>
    </div>
  );
}
