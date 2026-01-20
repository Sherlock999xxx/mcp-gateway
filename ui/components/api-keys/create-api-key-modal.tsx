"use client";

import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Input, Modal, ModalActions } from "@/components/ui";
import { CheckCircleIcon, WarningIconAlt } from "@/components/icons";
import { useCopyToClipboard } from "@/src/lib/useCopyToClipboard";
import { qk } from "@/src/lib/queryKeys";
import * as tenantApi from "@/src/lib/tenantApi";
import { useToastStore } from "@/src/lib/toast-store";

export function CreateApiKeyModal({
  onClose,
  scope,
  profileId,
}: {
  onClose: () => void;
  scope: "tenant" | "profile";
  profileId?: string;
}) {
  const queryClient = useQueryClient();
  const pushToast = useToastStore((s) => s.push);
  const [name, setName] = useState("");
  const [secret, setSecret] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const fixedProfileId = scope === "profile" ? (profileId ?? "") : "";

  const createMutation = useMutation({
    mutationFn: async () => {
      if (scope === "profile" && !fixedProfileId) {
        throw new Error("Missing profile id");
      }
      const resp = await tenantApi.createApiKey({
        name: name.trim() ? name.trim() : undefined,
        profileId: scope === "profile" ? fixedProfileId : undefined,
      });
      return resp.secret;
    },
    onSuccess: async (s) => {
      await queryClient.invalidateQueries({ queryKey: qk.apiKeys() });
      setSecret(s);
      setError(null);
      pushToast({ variant: "success", message: "API key created" });
    },
    onError: (e) => {
      setError(e instanceof Error ? e.message : "Failed to create API key");
    },
  });

  const close = () => {
    setName("");
    setSecret(null);
    setError(null);
    onClose();
  };

  return (
    <Modal
      open
      onClose={close}
      title={secret ? "API Key Created" : "Create API Key"}
      description="API key secrets are only displayed at creation time."
      size="lg"
    >
      {secret ? (
        <div>
          <div className="flex items-center gap-2 text-sm font-medium text-emerald-400 mb-4">
            <CheckCircleIcon className="w-5 h-5" />
            Key created successfully
          </div>

          <div className="p-4 rounded-xl bg-zinc-950/80 border border-zinc-800">
            <div className="flex items-center justify-between gap-2 mb-2">
              <span className="text-xs text-zinc-500">API Key Secret</span>
              <CopyButton text={secret} />
            </div>
            <code className="text-sm font-mono text-emerald-400 break-all">{secret}</code>
          </div>

          <div className="mt-4 flex items-start gap-2 text-xs text-amber-400">
            <WarningIcon className="w-4 h-4 shrink-0 mt-0.5" />
            <span>Copy this key now. You won&apos;t be able to see it again after closing.</span>
          </div>

          <div className="mt-6">
            <Button className="w-full" variant="secondary" onClick={close}>
              Done
            </Button>
          </div>
        </div>
      ) : (
        <form
          className="space-y-4"
          onSubmit={(e) => {
            e.preventDefault();
            createMutation.mutate();
          }}
        >
          {error && (
            <div className="rounded-xl border border-red-500/20 bg-red-500/5 p-3 text-sm text-red-200">
              {error}
            </div>
          )}

          <Input
            label="Key Name (optional)"
            placeholder={scope === "tenant" ? "e.g., Tenant-wide key" : "e.g., Profile key"}
            value={name}
            onChange={(e) => setName(e.target.value)}
          />

          <div className="rounded-xl border border-zinc-800/60 bg-zinc-950/30 p-4 text-sm text-zinc-400">
            {scope === "tenant" ? (
              <>
                <div className="text-zinc-200 font-medium">Tenant-wide key</div>
                <div className="mt-1 text-xs text-zinc-500">
                  Can be used to authenticate to any profile in this tenant.
                </div>
              </>
            ) : (
              <>
                <div className="text-zinc-200 font-medium">Profile-scoped key</div>
                <div className="mt-1 text-xs text-zinc-500">
                  Works only for the selected profile.
                </div>
                <div className="mt-3">
                  <Input label="Profile ID" value={fixedProfileId} disabled className="font-mono" />
                </div>
              </>
            )}
          </div>

          <ModalActions>
            <Button
              type="button"
              variant="ghost"
              onClick={close}
              disabled={createMutation.isPending}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              loading={createMutation.isPending}
              disabled={scope === "profile" && !fixedProfileId}
            >
              Create Key
            </Button>
          </ModalActions>
        </form>
      )}
    </Modal>
  );
}

function CopyButton({ text }: { text: string }) {
  const { copied, copy } = useCopyToClipboard(text);

  return (
    <button
      type="button"
      onClick={async () => {
        await copy();
      }}
      className="px-2 py-1 rounded text-xs font-medium text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 transition-colors"
    >
      {copied ? "Copied!" : "Copy"}
    </button>
  );
}

const WarningIcon = WarningIconAlt;
