import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { UnlockTenantCard } from "./_components/UnlockTenantCard";
import { ServerIconStack } from "@/components/icons";
import { GATEWAY_DATA_BASE } from "@/src/lib/env";

export const dynamic = "force-dynamic";

export default async function Home() {
  const cookieStore = await cookies();
  const hasSession = cookieStore.has("ugw_tenant_token");
  if (hasSession) {
    redirect("/profiles");
  }
  const dataBase = GATEWAY_DATA_BASE;

  return (
    <div className="min-h-full bg-zinc-950">
      {/* Hero Section */}
      <div className="relative overflow-hidden">
        {/* Background gradient */}
        <div className="absolute inset-0 bg-gradient-to-br from-violet-500/10 via-transparent to-emerald-500/5" />
        <div className="absolute top-0 left-1/2 -translate-x-1/2 w-[800px] h-[600px] bg-violet-500/20 blur-[120px] rounded-full" />

        <div className="relative mx-auto max-w-5xl px-6 pt-20 pb-16">
          <div className="flex items-center gap-2 text-sm text-zinc-400 mb-4">
            <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-500/10 text-emerald-400 text-xs font-medium">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
              Gateway Online
            </span>
          </div>

          <h1 className="text-4xl sm:text-5xl font-bold text-white tracking-tight">MCP Gateway</h1>
          <p className="mt-4 text-lg text-zinc-400 max-w-2xl">
            Transform any HTTP endpoint into an MCP server. Create profiles, manage API keys, and
            connect your AI agents in seconds.
          </p>
        </div>
      </div>

      <div className="mx-auto max-w-5xl px-6 pb-20">
        <div className="grid gap-6 lg:grid-cols-[1.3fr_0.7fr]">
          <UnlockTenantCard />

          <div className="rounded-2xl border border-zinc-800/80 bg-zinc-900/40 p-6">
            <h2 className="text-sm font-semibold text-zinc-100 flex items-center gap-2">
              <ServerIconStack className="w-5 h-5 text-violet-400" />
              Gateway configuration
            </h2>
            <div className="mt-4 space-y-3">
              <ConfigRow label="Data plane" value={dataBase} />
            </div>
            <p className="mt-4 text-xs text-zinc-500">
              After unlocking, you’ll be able to create profiles and copy MCP client configs.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}

function ConfigRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 py-2 border-b border-zinc-800/60 last:border-0">
      <span className="text-sm text-zinc-400">{label}</span>
      <span className="text-sm font-mono text-zinc-200 truncate max-w-[200px]">{value}</span>
    </div>
  );
}
