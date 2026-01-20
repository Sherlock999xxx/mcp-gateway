import { NextResponse } from "next/server";

export const dynamic = "force-dynamic";

export async function GET() {
  const base = process.env.GATEWAY_ADMIN_BASE;
  if (!base) {
    return NextResponse.json(
      { ok: false, error: "GATEWAY_ADMIN_BASE is not set" },
      { status: 500 },
    );
  }

  // Control plane has /status (AppState) and /health endpoints.
  const url = `${base.replace(/\/+$/, "")}/status`;
  const res = await fetch(url, { cache: "no-store" });
  const text = await res.text();

  if (!res.ok) {
    return NextResponse.json({ ok: false, status: res.status, body: text }, { status: 502 });
  }

  try {
    const json = JSON.parse(text) as unknown;
    return NextResponse.json({ ok: true, status: json });
  } catch {
    return NextResponse.json(
      { ok: false, error: "invalid JSON from gateway", body: text },
      { status: 502 },
    );
  }
}
