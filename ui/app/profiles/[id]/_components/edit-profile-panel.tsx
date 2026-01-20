"use client";

import { useState } from "react";
import type { Profile } from "@/src/lib/types";
import { Button, Input, Textarea } from "@/components/ui";

export function EditProfilePanel({
  profile,
  saving,
  saveError,
  onSave,
  onCancel,
}: {
  profile: Profile;
  saving: boolean;
  saveError: string | null;
  onSave: (next: { name: string; description: string }) => void;
  onCancel: () => void;
}) {
  const [draft, setDraft] = useState<{ name: string; description: string }>(() => ({
    name: profile.name ?? "",
    description: profile.description ?? "",
  }));

  return (
    <div className="mb-6 rounded-2xl border border-zinc-800/60 bg-zinc-900/40 p-6">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="text-sm font-semibold text-zinc-100">Edit profile</div>
          <div className="mt-1 text-xs text-zinc-500">
            Update name and description without leaving the page.
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button type="button" variant="secondary" onClick={() => onSave(draft)} disabled={saving}>
            {saving ? "Saving…" : "Save"}
          </Button>
          <Button type="button" variant="ghost" onClick={onCancel} disabled={saving}>
            Cancel
          </Button>
        </div>
      </div>
      {saveError ? (
        <div className="mt-4 rounded-xl border border-red-500/20 bg-red-500/5 p-3 text-sm text-red-200">
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
    </div>
  );
}
