"use client";

import { useMemo, useState } from "react";
import { AppShell, PageContent, PageHeader } from "@/components/layout";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import {
  Button,
  ConfirmModal,
  Input,
  Modal,
  ModalActions,
  SectionCard,
  Textarea,
} from "@/components/ui";
import { qk } from "@/src/lib/queryKeys";
import * as tenantApi from "@/src/lib/tenantApi";
import { useToastStore } from "@/src/lib/toast-store";
import { CheckCircleIcon, KeyIcon, PlusIcon, ShieldIcon } from "@/components/icons";
import { useDisclosure } from "@/src/lib/useDisclosure";

type SecretMeta = { name: string };

const EMPTY_SECRETS: SecretMeta[] = [];

export default function SecretsPage() {
  const createModal = useDisclosure(false);
  const [showUpdateModal, setShowUpdateModal] = useState<string | null>(null);
  const [showDeleteModal, setShowDeleteModal] = useState<string | null>(null);
  const queryClient = useQueryClient();
  const pushToast = useToastStore((s) => s.push);

  const secretsQuery = useQuery({
    queryKey: qk.secrets(),
    queryFn: tenantApi.listSecrets,
  });
  const secrets: SecretMeta[] = (secretsQuery.data?.secrets ?? EMPTY_SECRETS) as SecretMeta[];

  const sortedSecrets = useMemo(
    () => [...secrets].sort((a, b) => a.name.localeCompare(b.name)),
    [secrets],
  );
  const deleteMutation = useMutation({
    mutationFn: (name: string) => tenantApi.deleteSecret(name),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: qk.secrets() });
      pushToast({ variant: "success", message: "Secret deleted" });
      setShowDeleteModal(null);
    },
    onError: (e) => {
      pushToast({
        variant: "error",
        message: e instanceof Error ? e.message : "Failed to delete secret",
      });
      setShowDeleteModal(null);
    },
  });

  return (
    <AppShell>
      <PageHeader
        title="Secrets"
        description="Securely store sensitive values like API keys and tokens"
        actions={
          <button
            onClick={createModal.onOpen}
            className="inline-flex items-center gap-2 px-4 py-2.5 rounded-xl bg-gradient-to-b from-violet-500 to-violet-600 text-white font-medium text-sm shadow-lg shadow-violet-500/25 hover:from-violet-400 hover:to-violet-500 transition-all duration-150"
          >
            <PlusIcon className="w-4 h-4" />
            Add Secret
          </button>
        }
      />

      <PageContent>
        {secretsQuery.error && (
          <div className="mb-6 rounded-xl border border-red-500/20 bg-red-500/5 p-4 text-sm text-red-200">
            {secretsQuery.error instanceof Error
              ? secretsQuery.error.message
              : "Failed to load secrets"}
          </div>
        )}

        {/* Info banner */}
        <div className="mb-6 flex items-start gap-3 p-4 rounded-xl bg-violet-500/5 border border-violet-500/20">
          <ShieldIcon className="w-5 h-5 text-violet-400 shrink-0 mt-0.5" />
          <div>
            <p className="text-sm font-medium text-violet-400">Write-only secrets</p>
            <p className="mt-1 text-xs text-zinc-400">
              Secret values are encrypted and cannot be viewed after creation. You can only update
              or delete them. Use syntax like{" "}
              <code className="px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-300">
                ${"{secret:SECRET_NAME}"}
              </code>{" "}
              in tool sources to reference them.
            </p>
          </div>
        </div>

        {/* Secrets list */}
        <SectionCard className="overflow-hidden" bodyClassName="p-0">
          <div className="divide-y divide-zinc-800/40">
            {secretsQuery.isPending ? (
              <div className="p-5 text-sm text-zinc-400">Loading…</div>
            ) : sortedSecrets.length === 0 ? (
              <div className="p-5 text-sm text-zinc-500">No secrets yet.</div>
            ) : (
              sortedSecrets.map((secret) => (
                <div key={secret.name} className="p-5 hover:bg-zinc-800/20 transition-colors">
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex items-start gap-4">
                      <div className="w-10 h-10 rounded-xl bg-zinc-800/60 flex items-center justify-center">
                        <KeyIcon className="w-5 h-5 text-zinc-400" />
                      </div>
                      <div>
                        <div className="flex items-center gap-3">
                          <code className="text-sm font-semibold text-zinc-100">{secret.name}</code>
                          <span className="px-2 py-0.5 rounded text-xs font-mono bg-zinc-800/60 text-zinc-500">
                            ••••••••
                          </span>
                        </div>
                        <div className="mt-2 text-xs text-zinc-500">
                          Write-only secret value (not readable).
                        </div>
                      </div>
                    </div>

                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => setShowUpdateModal(secret.name)}
                        className="px-3 py-1.5 rounded-lg text-xs font-medium text-zinc-300 hover:text-white hover:bg-zinc-800/60 transition-colors"
                      >
                        Update
                      </button>
                      <button
                        onClick={() => setShowDeleteModal(secret.name)}
                        className="px-3 py-1.5 rounded-lg text-xs font-medium text-red-400 hover:text-red-300 hover:bg-red-500/10 transition-colors"
                      >
                        Delete
                      </button>
                    </div>
                  </div>
                </div>
              ))
            )}
          </div>
        </SectionCard>

        {!secretsQuery.isPending && sortedSecrets.length === 0 && (
          <div className="text-center py-12">
            <ShieldIcon className="w-12 h-12 text-zinc-700 mx-auto mb-4" />
            <h3 className="text-sm font-medium text-zinc-300">No secrets yet</h3>
            <p className="mt-1 text-sm text-zinc-500">Add your first secret to get started.</p>
          </div>
        )}
      </PageContent>

      {/* Create Modal */}
      {createModal.open && (
        <CreateSecretModal
          onClose={createModal.onClose}
          onCreated={() => {
            // query invalidation happens inside modal
          }}
        />
      )}

      {/* Update Modal */}
      {showUpdateModal && (
        <CreateSecretModal
          initialName={showUpdateModal}
          onClose={() => setShowUpdateModal(null)}
          onCreated={() => {
            // query invalidation happens inside modal
          }}
        />
      )}

      {/* Delete Modal */}
      <ConfirmModal
        open={!!showDeleteModal}
        onClose={() => setShowDeleteModal(null)}
        onConfirm={() => {
          if (!showDeleteModal) return;
          deleteMutation.mutate(showDeleteModal);
        }}
        title="Delete secret?"
        description={
          showDeleteModal
            ? `This will permanently delete "${showDeleteModal}". Any tool sources using this secret will fail.`
            : "This will permanently delete the secret."
        }
        confirmLabel="Delete Secret"
        danger
        loading={deleteMutation.isPending}
      />
    </AppShell>
  );
}

const createSecretSchema = z.object({
  name: z
    .string()
    .trim()
    .min(1, "Secret name is required")
    .regex(/^[A-Z][A-Z0-9_]*$/, "Use SCREAMING_SNAKE_CASE (A-Z, 0-9, _)"),
  value: z.string().min(1, "Secret value is required"),
});

type CreateSecretForm = z.infer<typeof createSecretSchema>;

function CreateSecretModal({
  onClose,
  onCreated,
  initialName,
}: {
  onClose: () => void;
  onCreated: () => void;
  initialName?: string;
}) {
  const queryClient = useQueryClient();
  const pushToast = useToastStore((s) => s.push);
  const [createdName, setCreatedName] = useState<string | null>(null);
  const isUpdate = typeof initialName === "string" && initialName.trim().length > 0;

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<CreateSecretForm>({
    resolver: zodResolver(createSecretSchema),
    defaultValues: { name: initialName ?? "", value: "" },
  });

  const createMutation = useMutation({
    mutationFn: async (values: CreateSecretForm) => {
      await tenantApi.createSecret({ name: values.name.trim(), value: values.value });
      return values.name.trim();
    },
    onSuccess: async (name) => {
      await queryClient.invalidateQueries({ queryKey: qk.secrets() });
      setCreatedName(name);
      pushToast({
        variant: "success",
        message: isUpdate ? "Secret updated" : "Secret stored securely",
      });
      onCreated();
    },
    onError: (e) => {
      pushToast({
        variant: "error",
        message: e instanceof Error ? e.message : "Failed to save secret",
      });
    },
  });

  const close = () => {
    setCreatedName(null);
    reset();
    onClose();
  };

  return (
    <Modal
      open
      onClose={close}
      title={
        createdName
          ? isUpdate
            ? "Secret Updated"
            : "Secret Created"
          : isUpdate
            ? "Update Secret"
            : "Add Secret"
      }
      description={
        createdName
          ? "Your secret has been encrypted and stored. You can now reference it in tool sources."
          : isUpdate
            ? "Secret values are write-only. Updating will overwrite the stored value."
            : "Secret values are write-only and cannot be viewed after creation."
      }
      size="lg"
    >
      {createdName ? (
        <div>
          <div className="flex items-center gap-2 text-sm font-medium text-emerald-400 mb-4">
            <CheckCircleIcon className="w-5 h-5" />
            {isUpdate ? "Secret updated" : "Secret stored securely"}
          </div>
          <p className="text-sm text-zinc-400">Reference it in tool sources using:</p>
          <code className="mt-3 block px-4 py-3 rounded-xl bg-zinc-950/80 border border-zinc-800 text-sm font-mono text-violet-400">
            ${`{secret:${createdName}}`}
          </code>
          <div className="mt-6">
            <Button className="w-full" variant="secondary" onClick={close}>
              Done
            </Button>
          </div>
        </div>
      ) : (
        <form
          className="space-y-4"
          onSubmit={handleSubmit((values) => createMutation.mutate(values))}
        >
          <Input
            label="Secret Name"
            placeholder="e.g., API_KEY"
            {...register("name")}
            error={errors.name?.message}
            className="font-mono uppercase"
            disabled={isUpdate}
          />
          <p className="text-xs text-zinc-500">Use SCREAMING_SNAKE_CASE for consistency.</p>

          <Textarea
            label="Secret Value"
            rows={3}
            placeholder="Enter the secret value…"
            {...register("value")}
            error={errors.value?.message}
            className="font-mono"
          />
          <p className="text-xs text-zinc-500">
            This value will be encrypted and cannot be viewed again.
          </p>

          <ModalActions>
            <Button type="button" variant="ghost" onClick={close} disabled={isSubmitting}>
              Cancel
            </Button>
            <Button type="submit" loading={createMutation.isPending}>
              Save Secret
            </Button>
          </ModalActions>
        </form>
      )}
    </Modal>
  );
}
