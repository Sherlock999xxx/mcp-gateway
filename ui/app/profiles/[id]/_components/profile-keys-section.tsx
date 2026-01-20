"use client";

import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, ConfirmModal, Modal, ModalActions } from "@/components/ui";
import { InfoIconAlt } from "@/components/icons";
import * as tenantApi from "@/src/lib/tenantApi";
import { qk } from "@/src/lib/queryKeys";
import { useToastStore } from "@/src/lib/toast-store";
import { CreateApiKeyModal } from "@/components/api-keys/create-api-key-modal";
import { formatUnix, formatUnixRelative } from "@/src/lib/display";

const InfoIcon = InfoIconAlt;

export function ProfileKeysSection({
  profileId,
  mcpUrl,
  profileApiKeys,
  loading,
}: {
  profileId: string;
  mcpUrl: string;
  profileApiKeys: Array<{
    id: string;
    name: string;
    prefix: string;
    createdAtUnix: number;
    lastUsedAtUnix: number | null;
    totalRequestsAttempted: number;
  }>;
  loading: boolean;
}) {
  const queryClient = useQueryClient();
  const pushToast = useToastStore((s) => s.push);

  const [showCreateKeyModal, setShowCreateKeyModal] = useState(false);
  const [showRevokeKeyModal, setShowRevokeKeyModal] = useState<string | null>(null);
  const [showApiKeyHelp, setShowApiKeyHelp] = useState(false);

  const revokeKeyMutation = useMutation({
    mutationFn: (id: string) => tenantApi.revokeApiKey(id),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: qk.apiKeys() });
      pushToast({ variant: "success", message: "API key revoked" });
      setShowRevokeKeyModal(null);
    },
    onError: (e) => {
      pushToast({
        variant: "error",
        message: e instanceof Error ? e.message : "Failed to revoke key",
      });
      setShowRevokeKeyModal(null);
    },
  });

  return (
    <>
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <p className="text-sm text-zinc-400">
            API keys for authenticating requests to this profile&apos;s MCP endpoint.
          </p>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setShowApiKeyHelp(true)}
              className="inline-flex items-center gap-2 rounded-lg px-3 py-2 text-sm text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/60 transition-colors"
              aria-label="API keys help"
            >
              <InfoIcon className="w-4 h-4" />
              Help
            </button>
            <Button type="button" onClick={() => setShowCreateKeyModal(true)}>
              Create API Key
            </Button>
          </div>
        </div>

        <div className="rounded-xl border border-zinc-800/60 bg-zinc-900/40 overflow-hidden">
          <div className="px-5 py-4 border-b border-zinc-800/60">
            <div className="text-sm font-medium text-zinc-200">Profile keys</div>
            <div className="mt-1 text-xs text-zinc-500">
              Only keys scoped to this profile are shown here. Tenant-wide keys are listed on the
              API Keys page.
            </div>
          </div>
          <div className="divide-y divide-zinc-800/40">
            {loading ? (
              <div className="p-5 text-sm text-zinc-400">Loading…</div>
            ) : profileApiKeys.length === 0 ? (
              <div className="p-5 text-sm text-zinc-500">No profile-scoped API keys yet.</div>
            ) : (
              profileApiKeys.map((k) => (
                <div key={k.id} className="p-5 hover:bg-zinc-800/20 transition-colors">
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-3">
                        <h3 className="text-sm font-semibold text-zinc-100">{k.name}</h3>
                        <span className="px-2 py-0.5 rounded-full text-xs font-medium bg-blue-500/10 text-blue-400 border border-blue-500/20">
                          Profile-scoped
                        </span>
                      </div>
                      <div className="mt-2 flex items-center gap-4 text-xs text-zinc-500">
                        <span className="font-mono bg-zinc-800/60 px-2 py-1 rounded">
                          {k.prefix}••••••••
                        </span>
                      </div>
                      <div className="mt-3 flex items-center gap-4 text-xs text-zinc-500">
                        <span>Created {formatUnix(k.createdAtUnix)}</span>
                        <span className="w-1 h-1 rounded-full bg-zinc-700" />
                        <span>Last used {formatUnixRelative(k.lastUsedAtUnix)}</span>
                        <span className="w-1 h-1 rounded-full bg-zinc-700" />
                        <span>{k.totalRequestsAttempted.toLocaleString()} requests</span>
                      </div>
                    </div>
                    <button
                      onClick={() => setShowRevokeKeyModal(k.id)}
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
      </div>

      {showCreateKeyModal && (
        <CreateApiKeyModal
          onClose={() => setShowCreateKeyModal(false)}
          scope="profile"
          profileId={profileId}
        />
      )}

      <Modal
        open={showApiKeyHelp}
        onClose={() => setShowApiKeyHelp(false)}
        title="API keys help"
        description="Tenant-wide vs profile-scoped keys"
        size="lg"
      >
        <div className="space-y-4">
          <div className="text-sm text-zinc-300">
            API keys control access to MCP endpoints. You can create keys in two scopes:
          </div>
          <div className="rounded-xl border border-zinc-800/60 bg-zinc-950/30 p-4 space-y-2">
            <div className="text-sm font-semibold text-zinc-100">Profile-scoped key</div>
            <div className="text-sm text-zinc-300">
              Grants access to <span className="font-semibold">only this profile</span>.
            </div>
            <div className="text-xs text-zinc-500">This exact endpoint:</div>
            <div className="rounded-lg border border-zinc-800/60 bg-zinc-950/60 px-3 py-2 font-mono text-xs text-zinc-200 break-all">
              {mcpUrl}
            </div>
          </div>
          <div className="rounded-xl border border-zinc-800/60 bg-zinc-950/30 p-4 space-y-2">
            <div className="text-sm font-semibold text-zinc-100">Tenant-wide key</div>
            <div className="text-sm text-zinc-300">
              Grants access to <span className="font-semibold">all profiles</span> in this tenant
              (useful for shared clients/automation).
            </div>
            <div className="text-xs text-zinc-500">
              Tenant-wide keys are created on the global{" "}
              <span className="font-semibold">API Keys</span> page.
            </div>
          </div>
          <div className="rounded-xl border border-zinc-800/60 bg-zinc-950/30 p-4 space-y-2">
            <div className="text-sm font-semibold text-zinc-100">Future</div>
            <div className="text-sm text-zinc-300">
              More granular keys (tenant-level keys restricted to a specific set of profiles) are
              planned for a future release.
            </div>
          </div>
        </div>
        <ModalActions>
          <Button type="button" onClick={() => setShowApiKeyHelp(false)}>
            Close
          </Button>
        </ModalActions>
      </Modal>

      <ConfirmModal
        open={!!showRevokeKeyModal}
        onClose={() => setShowRevokeKeyModal(null)}
        onConfirm={() => {
          if (!showRevokeKeyModal) return;
          revokeKeyMutation.mutate(showRevokeKeyModal);
        }}
        title="Revoke API key?"
        description="This will immediately invalidate the key. Any applications using it will lose access. This action cannot be undone."
        confirmLabel="Revoke Key"
        danger
        loading={revokeKeyMutation.isPending}
      />
    </>
  );
}
