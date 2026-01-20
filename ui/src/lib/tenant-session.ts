export const TENANT_TOKEN_COOKIE = "ugw_tenant_token";
export const TENANT_ID_COOKIE = "ugw_tenant_id";
export const TENANT_EXP_COOKIE = "ugw_tenant_exp_unix";

export type TenantTokenPayloadV1 = {
  tenant_id: string;
  exp_unix_secs: number;
};

type RawTenantTokenPayloadV1 =
  | { tenant_id: string; exp_unix_secs: number }
  | { tenantId: string; expUnixSecs: number };

function base64UrlToBase64(input: string): string {
  const padded = input.replace(/-/g, "+").replace(/_/g, "/");
  const padLen = (4 - (padded.length % 4)) % 4;
  return padded + "=".repeat(padLen);
}

export function decodeTenantTokenPayload(token: string): TenantTokenPayloadV1 {
  const raw = token.trim().startsWith("Bearer ")
    ? token.trim().slice("Bearer ".length).trim()
    : token.trim();

  const parts = raw.split(".");
  if (parts.length !== 3 || parts[0] !== "tv1") {
    throw new Error("Invalid token format (expected tv1.<payload_b64>.<sig_b64>)");
  }

  const payloadB64Url = parts[1];
  const payloadB64 = base64UrlToBase64(payloadB64Url);
  const json = atob(payloadB64);
  const payload = JSON.parse(json) as RawTenantTokenPayloadV1;

  const tenantId =
    typeof (payload as { tenant_id?: unknown }).tenant_id === "string"
      ? (payload as { tenant_id: string }).tenant_id
      : typeof (payload as { tenantId?: unknown }).tenantId === "string"
        ? (payload as { tenantId: string }).tenantId
        : null;

  const expUnixSecs =
    typeof (payload as { exp_unix_secs?: unknown }).exp_unix_secs === "number"
      ? (payload as { exp_unix_secs: number }).exp_unix_secs
      : typeof (payload as { expUnixSecs?: unknown }).expUnixSecs === "number"
        ? (payload as { expUnixSecs: number }).expUnixSecs
        : null;

  if (!tenantId) {
    throw new Error("Invalid token payload (missing tenantId)");
  }
  if (expUnixSecs == null || typeof expUnixSecs !== "number" || !Number.isFinite(expUnixSecs)) {
    throw new Error("Invalid token payload (missing expUnixSecs)");
  }
  return { tenant_id: tenantId, exp_unix_secs: expUnixSecs };
}

export function setTenantSessionCookies(token: string, payload: TenantTokenPayloadV1): void {
  if (typeof document === "undefined") return;
  const now = Math.floor(Date.now() / 1000);
  const maxAge = Math.max(1, payload.exp_unix_secs - now);

  // v0 UI-only session: cookie is not httpOnly (set from browser JS).
  // Backend integration can migrate this to httpOnly cookies later.
  document.cookie = `${TENANT_TOKEN_COOKIE}=${encodeURIComponent(token)}; Path=/; Max-Age=${maxAge}; SameSite=Lax`;
  document.cookie = `${TENANT_ID_COOKIE}=${encodeURIComponent(payload.tenant_id)}; Path=/; Max-Age=${maxAge}; SameSite=Lax`;
  document.cookie = `${TENANT_EXP_COOKIE}=${encodeURIComponent(String(payload.exp_unix_secs))}; Path=/; Max-Age=${maxAge}; SameSite=Lax`;
}

export function clearTenantSessionCookies(): void {
  if (typeof document === "undefined") return;
  const past = "Thu, 01 Jan 1970 00:00:00 GMT";
  document.cookie = `${TENANT_TOKEN_COOKIE}=; Path=/; Expires=${past}; SameSite=Lax`;
  document.cookie = `${TENANT_ID_COOKIE}=; Path=/; Expires=${past}; SameSite=Lax`;
  document.cookie = `${TENANT_EXP_COOKIE}=; Path=/; Expires=${past}; SameSite=Lax`;
}

export function forceReunlock(nextPath?: string): void {
  if (typeof window === "undefined") return;
  clearTenantSessionCookies();
  const next = nextPath ?? window.location.pathname;
  window.location.href = `/unlock?next=${encodeURIComponent(next)}`;
}

export function readCookie(name: string): string | null {
  if (typeof document === "undefined") return null;
  const cookie = document.cookie
    .split(";")
    .map((c) => c.trim())
    .find((c) => c.startsWith(`${name}=`));
  if (!cookie) return null;
  return decodeURIComponent(cookie.slice(name.length + 1));
}

export function getTenantIdFromCookies(): string | null {
  return readCookie(TENANT_ID_COOKIE);
}

export function getTenantExpFromCookies(): number | null {
  const raw = readCookie(TENANT_EXP_COOKIE);
  if (!raw) return null;
  const n = Number(raw);
  return Number.isFinite(n) ? n : null;
}
