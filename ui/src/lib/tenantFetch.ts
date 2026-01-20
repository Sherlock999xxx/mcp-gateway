import { forceReunlock, getTenantExpFromCookies } from "@/src/lib/tenant-session";

export class MissingTenantSessionError extends Error {
  constructor(message = "missing tenant session") {
    super(message);
    this.name = "MissingTenantSessionError";
  }
}

export function isMissingTenantSessionError(e: unknown): e is MissingTenantSessionError {
  if (e instanceof MissingTenantSessionError) return true;
  if (
    e &&
    typeof e === "object" &&
    "name" in e &&
    (e as { name: unknown }).name === "MissingTenantSessionError"
  ) {
    return true;
  }
  return false;
}

function coerceErrorMessage(bodyText: string): string {
  const t = bodyText.trim();
  if (!t) return "Request failed";
  try {
    const v = JSON.parse(t) as unknown;
    if (v && typeof v === "object") {
      const o = v as Record<string, unknown>;
      if (typeof o.error === "string" && o.error.trim()) return o.error;
      if (typeof o.body === "string" && o.body.trim()) return o.body;
    }
  } catch {
    // ignore: not JSON
  }
  return t;
}

/**
 * Fetch helper for tenant-scoped UI API routes.
 *
 * If the UI session is missing/expired, our BFF routes return 401 and we
 * immediately redirect the user back to /unlock.
 */
export async function tenantFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  const exp = getTenantExpFromCookies();
  if (exp != null) {
    const now = Math.floor(Date.now() / 1000);
    if (exp <= now) {
      forceReunlock();
      throw new MissingTenantSessionError("tenant session expired");
    }
  }
  const res = await fetch(input, init);
  if (res.status === 401) {
    // Current page is already the best "next" value.
    forceReunlock();
    throw new MissingTenantSessionError();
  }
  return res;
}

export async function tenantFetchJson<T>(input: RequestInfo | URL, init?: RequestInit): Promise<T> {
  const res = await tenantFetch(input, init);
  const text = await res.text();
  if (!res.ok) {
    throw new Error(coerceErrorMessage(text));
  }
  try {
    return JSON.parse(text) as T;
  } catch {
    throw new Error(`Invalid JSON response: ${text.slice(0, 2000)}`);
  }
}
