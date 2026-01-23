"use client";

import { useState } from "react";
import type { Profile } from "@/src/lib/types";
import { Button, Input, Modal, ModalActions, Textarea } from "@/components/ui";

export function EditProfilePanel({
  open,
  profile,
  saving,
  saveError,
  onSave,
  onClose,
}: {
  open: boolean;
  profile: Profile;
  saving: boolean;
  saveError: string | null;
  onSave: (next: { name: string; description: string }) => void;
  onClose: () => void;
}) {
  // Mount a fresh draft per open to make Cancel/outside-click discard automatically.
  if (!open) return null;
  return (
    <EditProfilePanelOpen
      profile={profile}
      saving={saving}
      saveError={saveError}
      onSave={onSave}
      onClose={onClose}
    />
  );
}

function EditProfilePanelOpen({
  profile,
  saving,
  saveError,
  onSave,
  onClose,
}: Omit<Parameters<typeof EditProfilePanel>[0], "open">) {
  const [draft, setDraft] = useState<{ name: string; description: string }>(() => ({
    name: profile.name ?? "",
    description: profile.description ?? "",
  }));

  return (
    <Modal
      open
      onClose={onClose}
      title="Edit profile"
      description="Update name and description without leaving the page."
      size="lg"
    >
      {saveError ? (
        <div className="rounded-xl border border-red-500/20 bg-red-500/5 p-3 text-sm text-red-200">
          {saveError}
        </div>
      ) : null}

      <div className="mt-5 grid gap-4 md:grid-cols-2">
        <div>
          <div className="text-xs font-medium text-zinc-400 mb-2">Name</div>
          <Input
            value={draft.name}
            onChange={(e) => setDraft((p) => ({ ...p, name: e.target.value }))}
            placeholder="Profile name"
          />
        </div>
        <div>
          <div className="text-xs font-medium text-zinc-400 mb-2">Description</div>
          <Textarea
            value={draft.description}
            onChange={(e) => setDraft((p) => ({ ...p, description: e.target.value }))}
            placeholder="Optional description"
            rows={3}
          />
        </div>
      </div>

      <ModalActions>
        <Button type="button" variant="ghost" onClick={onClose} disabled={saving}>
          Cancel
        </Button>
        <Button type="button" variant="secondary" onClick={() => onSave(draft)} disabled={saving}>
          {saving ? "Saving…" : "Save"}
        </Button>
      </ModalActions>
    </Modal>
  );
}
