import { invoke } from "@tauri-apps/api/core";

/**
 * Compute block hash server-side, including environment + connection context.
 * The hash covers the active environment ID and connection ID for cache
 * isolation, and is computed server-side so the frontend cannot spoof it.
 */
export async function hashBlockContent(
  content: string,
  connectionId?: string | null,
): Promise<string> {
  return invoke("compute_block_hash", {
    content,
    connectionId: connectionId ?? null,
  });
}

/**
 * Build the cache hash key for a db block run. Keeps cache entries isolated
 * across active environments by folding in a snapshot of *only* the env vars
 * referenced by the body — so a query that doesn't use any envs has a stable
 * hash regardless of which environment is active.
 *
 * Shared between `DbFencedPanel` (writes the cache on run) and
 * `document.ts#populateCachedResults` (reads the cache when rebuilding the
 * block context graph for `{{ref}}` autocomplete / resolution). Both sides
 * MUST stay in lockstep or reads will miss valid cache entries.
 */
export async function computeDbCacheHash(
  body: string,
  connectionId: string,
  envVars: Record<string, string>,
): Promise<string> {
  const usedEnvEntries = Object.entries(envVars)
    .filter(([k]) => body.includes(`{{${k}}}`))
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([k, v]) => `${k}=${v}`)
    .join("\n");
  const keyed = usedEnvEntries ? `${body}\n__ENV__\n${usedEnvEntries}` : body;
  return hashBlockContent(keyed, connectionId);
}

/**
 * Build the cache hash key for an http block run.
 *
 * Inputs canonicalized:
 *   - method (uppercase)
 *   - URL (path + canonical-merged query string from params, sorted)
 *   - headers (sorted by lowercased key)
 *   - body (verbatim)
 *   - env-var snapshot of *only* the env keys referenced anywhere in the
 *     hashed text — same shape as the DB cache so different active
 *     environments don't share cache entries when they actually differ.
 *
 * Mutation methods (POST/PUT/PATCH/DELETE) should still be hashed for
 * deterministic equality checks, but callers are expected to skip cache
 * reads/writes for them. The hash itself is method-aware so a `GET` and a
 * `POST` to the same URL never collide.
 */
export async function computeHttpCacheHash(
  parts: {
    method: string;
    url: string;
    params: Array<{ key: string; value: string }>;
    headers: Array<{ key: string; value: string }>;
    body: string;
  },
  envVars: Record<string, string>,
): Promise<string> {
  const method = parts.method.toUpperCase();
  const sortedParams = [...parts.params]
    .sort((a, b) => a.key.localeCompare(b.key))
    .map((p) => `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`)
    .join("&");
  const canonicalUrl = sortedParams
    ? `${parts.url}?${sortedParams}`
    : parts.url;
  const sortedHeaders = [...parts.headers]
    .sort((a, b) => a.key.toLowerCase().localeCompare(b.key.toLowerCase()))
    .map((h) => `${h.key.toLowerCase()}: ${h.value}`)
    .join("\n");

  const fullText = [method, canonicalUrl, sortedHeaders, parts.body].join(
    "\n__SEP__\n",
  );

  const usedEnvEntries = Object.entries(envVars)
    .filter(([k]) => fullText.includes(`{{${k}}}`))
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([k, v]) => `${k}=${v}`)
    .join("\n");
  const keyed = usedEnvEntries
    ? `${fullText}\n__ENV__\n${usedEnvEntries}`
    : fullText;
  return hashBlockContent(keyed, null);
}
