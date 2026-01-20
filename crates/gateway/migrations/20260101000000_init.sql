-- migrate:up
-- Mode 3 schema (Postgres).
--
-- Flattened baseline schema snapshot for initial public release.
-- This migration is intended to be applied to an empty database.

create table tenants (
    id text primary key,
    enabled boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table upstreams (
    id text primary key,
    enabled boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table upstream_endpoints (
    upstream_id text not null references upstreams(id) on delete cascade,
    id text not null,
    url text not null,
    -- Optional auth config for connecting to the upstream MCP endpoint.
    auth jsonb null,
    enabled boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (upstream_id, id)
);

create table profiles (
    -- Public identifier exposed in the data-plane path: /{profile_id}/mcp.
    -- Must be a UUIDv4 (random) to avoid easy endpoint enumeration.
    id uuid primary key,
    tenant_id text not null references tenants(id) on delete cascade,
    -- Human-friendly profile name (unique per tenant, case-insensitive).
    name text not null,
    -- Optional human-friendly description.
    description text null,
    enabled boolean not null default true,
    allow_partial_upstreams boolean not null default true,
    -- Tool allowlist (optional; empty list means allow all).
    enabled_tools text[] not null default '{}'::text[],
    -- Per-profile tool transforms.
    transforms jsonb not null default '{}'::jsonb,
    -- Per-profile data-plane auth policy (Mode 3).
    data_plane_auth_mode text not null default 'api_key_initialize_only',
    accept_x_api_key boolean not null default true,
    -- Optional per-profile data-plane limits (Mode 3, disabled by default).
    rate_limit_enabled boolean not null default false,
    rate_limit_tool_calls_per_minute integer null,
    quota_enabled boolean not null default false,
    quota_tool_calls bigint null,
    -- Tool call timeout and retry policy (Mode 3).
    tool_call_timeout_secs integer null,
    tool_policies jsonb not null default '[]'::jsonb,
    -- Per-profile MCP proxy settings (capabilities allow/deny, namespacing, notification filters).
    mcp_settings jsonb not null default '{}'::jsonb,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create table profile_upstreams (
    profile_id uuid not null references profiles(id) on delete cascade,
    upstream_id text not null references upstreams(id) on delete cascade,
    ordinal integer not null default 0,
    created_at timestamptz not null default now(),
    primary key (profile_id, upstream_id)
);

create index idx_profiles_tenant_id on profiles(tenant_id);
create unique index profiles_tenant_name_ci_uq on profiles (tenant_id, lower(name));
create index idx_profile_upstreams_upstream_id on profile_upstreams(upstream_id);

-- Mode 3 overlay: tenant-owned local tool sources (HTTP/OpenAPI).
create table tool_sources (
    tenant_id text not null references tenants(id) on delete cascade,
    id text not null,
    kind text not null, -- 'http' | 'openapi'
    enabled boolean not null default true,
    spec jsonb not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (tenant_id, id)
);

-- Profile ↔ local tool source attachments (shared + tenant-owned).
create table profile_sources (
    profile_id uuid not null references profiles(id) on delete cascade,
    source_id text not null,
    ordinal integer not null default 0,
    created_at timestamptz not null default now(),
    primary key (profile_id, source_id)
);

create index idx_profile_sources_source_id on profile_sources(source_id);

-- Tenant secrets (write-only via APIs, never returned). Encrypted at rest by the Gateway.
create table secrets (
    tenant_id text not null references tenants(id) on delete cascade,
    name text not null,
    -- Legacy plaintext (nullable). New writes store ciphertext and clear `value`.
    value text,
    kid text,
    nonce bytea,
    ciphertext bytea,
    algo text,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (tenant_id, name),
    constraint secrets_value_or_ciphertext_present
    check (
        value is not null
        or (kid is not null and nonce is not null and ciphertext is not null)
    )
);

-- Durable contract event log for `notifications/*/list_changed` replay.
create table contract_events (
    id bigserial primary key,
    profile_id uuid not null,
    kind text not null,
    contract_hash text not null,
    created_at timestamptz not null default now()
);

create index contract_events_profile_id_id_idx
    on contract_events (profile_id, id);

-- Tenant-issued API keys for data-plane authentication.
create table api_keys (
    id uuid primary key,
    tenant_id text not null references tenants(id) on delete cascade,
    -- If set, key is scoped to a specific profile. If NULL, key is tenant-wide.
    profile_id uuid null references profiles(id) on delete cascade,
    name text not null,
    -- Short prefix for UX/debugging (not sensitive).
    prefix text not null,
    -- SHA-256 hex string of the secret (do NOT store the secret itself).
    secret_hash text not null,
    revoked_at timestamptz null,
    last_used_at timestamptz null,
    total_tool_calls_attempted bigint not null default 0,
    total_requests_attempted bigint not null default 0,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index api_keys_secret_hash_uq on api_keys(secret_hash);
create index api_keys_tenant_id_idx on api_keys(tenant_id);
create index api_keys_tenant_profile_idx on api_keys(tenant_id, profile_id);
create index api_keys_tenant_revoked_idx on api_keys(tenant_id, revoked_at);

-- Per API key and profile state for enforcement in HA (rate limits and quotas).
create table api_key_profile_state (
    api_key_id uuid not null references api_keys(id) on delete cascade,
    profile_id uuid not null references profiles(id) on delete cascade,
    -- Rate limiting state (fixed window per minute).
    rate_window_start timestamptz not null,
    rate_window_count integer not null default 0,
    -- Quota state (remaining tool calls). Null means not initialized or not used.
    quota_remaining bigint null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    primary key (api_key_id, profile_id)
);

-- OIDC principal bindings (issuer + subject) -> tenant/profile authorization.
create table oidc_principals (
    id uuid primary key,
    issuer text not null,
    subject text not null,
    tenant_id text not null references tenants(id) on delete cascade,
    -- If set, principal is scoped to a single profile. If NULL, principal is tenant-wide.
    profile_id uuid null references profiles(id) on delete cascade,
    enabled boolean not null default true,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- Enforce at most one tenant-wide binding per (issuer, subject, tenant).
create unique index oidc_principals_tenant_wide_uq
    on oidc_principals(issuer, subject, tenant_id)
    where profile_id is null;

-- Enforce at most one profile-scoped binding per (issuer, subject, tenant, profile).
create unique index oidc_principals_profile_scoped_uq
    on oidc_principals(issuer, subject, tenant_id, profile_id)
    where profile_id is not null;

create index oidc_principals_tenant_id_idx on oidc_principals(tenant_id);
create index oidc_principals_issuer_subject_idx on oidc_principals(issuer, subject);

-- migrate:down
drop index if exists oidc_principals_issuer_subject_idx;
drop index if exists oidc_principals_tenant_id_idx;
drop index if exists oidc_principals_profile_scoped_uq;
drop index if exists oidc_principals_tenant_wide_uq;

drop index if exists api_keys_tenant_revoked_idx;
drop index if exists api_keys_tenant_profile_idx;
drop index if exists api_keys_tenant_id_idx;
drop index if exists api_keys_secret_hash_uq;

drop index if exists contract_events_profile_id_id_idx;

drop index if exists idx_profile_sources_source_id;
drop index if exists idx_profile_upstreams_upstream_id;
drop index if exists profiles_tenant_name_ci_uq;
drop index if exists idx_profiles_tenant_id;

drop table if exists oidc_principals;
drop table if exists api_key_profile_state;
drop table if exists api_keys;
drop table if exists contract_events;
drop table if exists secrets;
drop table if exists profile_sources;
drop table if exists tool_sources;
drop table if exists profile_upstreams;
drop table if exists profiles;
drop table if exists upstream_endpoints;
drop table if exists upstreams;
drop table if exists tenants;
