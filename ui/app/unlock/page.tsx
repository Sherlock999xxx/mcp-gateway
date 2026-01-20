"use client";

import Link from "next/link";
import { UnlockTenantCard } from "../_components/UnlockTenantCard";
import { ArrowLeftIcon, WarningIcon } from "@/components/icons";

export default function UnlockPage() {
  return (
    <div className="min-h-full bg-zinc-950 flex items-center justify-center p-6">
      {/* Background effects */}
      <div className="fixed inset-0 bg-gradient-to-br from-violet-500/5 via-transparent to-emerald-500/5" />
      <div className="fixed top-1/4 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[400px] bg-violet-500/10 blur-[100px] rounded-full" />

      <div className="relative w-full max-w-2xl">
        {/* Header */}
        <div className="text-center mb-8">
          <Link
            href="/"
            className="inline-flex items-center gap-2 text-sm text-zinc-500 hover:text-zinc-300 transition-colors mb-6"
          >
            <ArrowLeftIcon className="w-4 h-4" />
            Back to home
          </Link>

          <h1 className="text-2xl font-bold text-white">Unlock Tenant</h1>
          <p className="mt-2 text-sm text-zinc-400 max-w-sm mx-auto">
            Paste your tenant token to access the Gateway dashboard.
          </p>
        </div>

        <UnlockTenantCard />

        {/* Warning */}
        <div className="mt-6 flex items-start gap-3 p-4 rounded-xl bg-amber-500/5 border border-amber-500/20">
          <WarningIcon className="w-5 h-5 text-amber-500 shrink-0 mt-0.5" />
          <div>
            <p className="text-sm font-medium text-amber-400">Full Tenant Access</p>
            <p className="mt-1 text-xs text-zinc-400">
              This token grants administrative privileges to the tenant. Keep it secure and avoid
              sharing it. This UI stores the session in this browser.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
