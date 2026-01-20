"use client";

import { useMemo, useState } from "react";
import { AppShell, PageContent, PageHeader } from "@/components/layout";
import type { ApiKeyMetadata } from "@/src/lib/types";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ConfirmModal } from "@/components/ui";
import { qk } from "@/src/lib/queryKeys";
import * as tenantApi from "@/src/lib/tenantApi";
import { useToastStore } from "@/src/lib/toast-store";
import { CreateApiKeyModal } from "@/components/api-keys/create-api-key-modal";
import {
  ChartIcon,
  FolderIcon,
  GlobeIcon,
  InfoIconAlt,
  KeyIcon,
  PlusIcon,
} from "@/components/icons";
import { useDisclosure } from "@/src/lib/useDisclosure";
import { formatUnix, formatUnixRelative } from "@/src/lib/display";

const EMPTY_API_KEYS: ApiKeyMetadata[] = [];

export default function ApiKeysPage() {
  const [showRevokeModal, setShowRevokeModal] = useState<string | null>(null);
  const createModal = useDisclosure(false);
  const queryClient = useQueryClient();
  const pushToast = useToastStore((s) => s.push);

  const apiKeysQuery = useQuery({
    queryKey: qk.apiKeys(),
    queryFn: tenantApi.listApiKeys,
  });
  const apiKeys: ApiKeyMetadata[] = apiKeysQuery.data ?? EMPTY_API_KEYS;

  const tenantWideCount = useMemo(() => apiKeys.filter((k) => !k.profileId).length, [apiKeys]);
  const totalRequests = useMemo(
    () => apiKeys.reduce((acc, k) => acc + (k.totalRequestsAttempted ?? 0), 0),
    [apiKeys],
  );
  const revokeMutation = useMutation({
    mutationFn: (id: string) => tenantApi.revokeApiKey(id),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: qk.apiKeys() });
      pushToast({ variant: "success", message: "API key revoked" });
      setShowRevokeModal(null);
    },
    onError: (e) => {
      pushToast({
        variant: "error",
        message: e instanceof Error ? e.message : "Failed to revoke key",
      });
      setShowRevokeModal(null);
    },
  });

  return (
    <AppShell>
      <PageHeader
        title="API Keys"
        description="Manage authentication keys for MCP endpoints"
        actions={
          <button
            onClick={createModal.onOpen}
            className="inline-flex items-center gap-2 px-4 py-2.5 rounded-xl bg-gradient-to-b from-violet-500 to-violet-600 text-white font-medium text-sm shadow-lg shadow-violet-500/25 hover:from-violet-400 hover:to-violet-500 transition-all duration-150"
          >
            <PlusIcon className="w-4 h-4" />
            Create API Key
          </button>
        }
      />

      <PageContent>
        {apiKeysQuery.error && (
          <div className="mb-6 rounded-xl border border-red-500/20 bg-red-500/5 p-4 text-sm text-red-200">
            {apiKeysQuery.error instanceof Error
              ? apiKeysQuery.error.message
              : "Failed to load API keys"}
          </div>
        )}

        {/* Stats */}
        <div className="grid grid-cols-3 gap-4 mb-6">
          <div className="rounded-xl border border-zinc-800/60 bg-zinc-900/40 p-4">
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-lg bg-violet-500/10 text-violet-400">
                <KeyIcon className="w-5 h-5" />
              </div>
              <div>
                <div className="text-2xl font-bold text-white">{apiKeys.length}</div>
                <div className="text-xs text-zinc-500">Total Keys</div>
              </div>
            </div>
          </div>
          <div className="rounded-xl border border-zinc-800/60 bg-zinc-900/40 p-4">
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-lg bg-emerald-500/10 text-emerald-400">
                <GlobeIcon className="w-5 h-5" />
              </div>
              <div>
                <div className="text-2xl font-bold text-white">{tenantWideCount}</div>
                <div className="text-xs text-zinc-500">Tenant-Wide</div>
              </div>
            </div>
          </div>
          <div className="rounded-xl border border-zinc-800/60 bg-zinc-900/40 p-4">
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-lg bg-blue-500/10 text-blue-400">
                <ChartIcon className="w-5 h-5" />
              </div>
              <div>
                <div className="text-2xl font-bold text-white">
                  {totalRequests.toLocaleString()}
                </div>
                <div className="text-xs text-zinc-500">Total Requests</div>
              </div>
            </div>
          </div>
        </div>

        {/* Info banner */}
        <div className="mb-6 flex items-start gap-3 p-4 rounded-xl bg-amber-500/5 border border-amber-500/20">
          <InfoIconAlt className="w-5 h-5 text-amber-500 shrink-0 mt-0.5" />
          <div>
            <p className="text-sm font-medium text-amber-400">Secret shown once</p>
            <p className="mt-1 text-xs text-zinc-400">
              API key secrets are only displayed at creation time. Make sure to copy and store them
              securely. You cannot retrieve the full secret later.
            </p>
          </div>
        </div>

        {/* Keys list */}
        <div className="rounded-xl border border-zinc-800/60 bg-zinc-900/40 overflow-hidden">
          <div className="divide-y divide-zinc-800/40">
            {apiKeysQuery.isPending ? (
              <div className="p-5 text-sm text-zinc-400">Loading…</div>
            ) : apiKeys.length === 0 ? (
              <div className="p-5 text-sm text-zinc-500">No API keys yet.</div>
            ) : (
              apiKeys.map((key) => (
                <div key={key.id} className="p-5 hover:bg-zinc-800/20 transition-colors">
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-3">
                        <h3 className="text-sm font-semibold text-zinc-100">{key.name}</h3>
                        {key.profileId ? (
                          <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-blue-500/10 text-blue-400 border border-blue-500/20">
                            Profile-scoped
                          </span>
                        ) : (
                          <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                            Tenant-wide
                          </span>
                        )}
                      </div>

                      <div className="mt-2 flex items-center gap-4 text-xs text-zinc-500">
                        <span className="font-mono bg-zinc-800/60 px-2 py-1 rounded">
                          {key.prefix}••••••••
                        </span>
                        {key.profileId && (
                          <span className="flex items-center gap-1 font-mono">
                            <FolderIcon className="w-3.5 h-3.5" />
                            {key.profileId}
                          </span>
                        )}
                      </div>

                      <div className="mt-3 flex items-center gap-4 text-xs text-zinc-500">
                        <span>Created {formatUnix(key.createdAtUnix)}</span>
                        <span className="w-1 h-1 rounded-full bg-zinc-700" />
                        <span>Last used {formatUnixRelative(key.lastUsedAtUnix)}</span>
                        <span className="w-1 h-1 rounded-full bg-zinc-700" />
                        <span>{key.totalRequestsAttempted.toLocaleString()} requests</span>
                      </div>
                    </div>

                    <button
                      onClick={() => setShowRevokeModal(key.id)}
                      className="px-3 py-1.5 rounded-lg text-xs font-medium text-red-400 hover:text-red-300 hover:bg-red-500/10 transition-colors"
                    >
                      Revoke
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      </PageContent>

      {/* Create Key Modal */}
      {createModal.open && <CreateApiKeyModal onClose={createModal.onClose} scope="tenant" />}

      {/* Revoke Modal */}
      <ConfirmModal
        open={!!showRevokeModal}
        onClose={() => setShowRevokeModal(null)}
        onConfirm={() => {
          if (!showRevokeModal) return;
          revokeMutation.mutate(showRevokeModal);
        }}
        title="Revoke API key?"
        description="This will immediately invalidate the key. Any applications using it will lose access. This action cannot be undone."
        confirmLabel="Revoke Key"
        danger
        loading={revokeMutation.isPending}
      />
    </AppShell>
  );
}
