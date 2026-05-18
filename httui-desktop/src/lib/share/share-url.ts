// share URL composers.
//
// `composeBlobUrl` — `<origin>/blob/<sha>/<path>` (GitHub
// shape) or `<origin>/-/blob/<sha>/<path>` (GitLab shape). Optional
// `#L<line>` anchor.
// `composeTreeUrl` — fallback: `<origin>/tree/<sha>` when no
// active file is open.
// `composeCompareUrl` — `<origin>/compare/<base>..<current>`.
//
// All composers return either a string URL or a structured
// `UnsupportedResult` that the consumer can show as a "Manual: open
// <origin>" hint with a copy-URL fallback (per epic spec for
// Bitbucket / Gitea / Other).

import type { ParsedRemote, RemoteHost } from "./remote-host";

export interface UnsupportedResult {
  ok: false;
  reason: "unsupported-host";
  /** Pre-formatted hint copy the consumer can render verbatim. */
  hint: string;
  /** Always points at the bare origin so the consumer can offer a
   *  copy-URL or "open in browser" fallback. */
  fallback: string;
}

export interface SupportedResult {
  ok: true;
  url: string;
}

export type ComposeResult = SupportedResult | UnsupportedResult;

export function composeBlobUrl(
  remote: ParsedRemote,
  sha: string,
  path: string,
  line?: number,
): ComposeResult {
  const base = baseUrl(remote);
  const cleanedPath = path.replace(/^\/+/u, "");
  const anchor = line && line > 0 ? `#L${line}` : "";
  switch (remote.host.kind) {
    case "github":
    case "bitbucket":
    case "gitea":
    case "other":
      // GitHub-shaped path; Bitbucket/Gitea/Other still get the
      // "manual" hint below.
      if (
        remote.host.kind === "bitbucket" ||
        remote.host.kind === "gitea" ||
        remote.host.kind === "other"
      ) {
        return manualHint(remote);
      }
      return { ok: true, url: `${base}/blob/${sha}/${cleanedPath}${anchor}` };
    case "gitlab":
    case "gitlab_self_hosted":
      return { ok: true, url: `${base}/-/blob/${sha}/${cleanedPath}${anchor}` };
  }
}

export function composeTreeUrl(
  remote: ParsedRemote,
  sha: string,
): ComposeResult {
  const base = baseUrl(remote);
  switch (remote.host.kind) {
    case "github":
      return { ok: true, url: `${base}/tree/${sha}` };
    case "gitlab":
    case "gitlab_self_hosted":
      return { ok: true, url: `${base}/-/tree/${sha}` };
    case "bitbucket":
    case "gitea":
    case "other":
      return manualHint(remote);
  }
}

export function composeCompareUrl(
  remote: ParsedRemote,
  base: string,
  current: string,
): ComposeResult {
  const baseHttp = baseUrl(remote);
  switch (remote.host.kind) {
    case "github":
      return {
        ok: true,
        url: `${baseHttp}/compare/${base}...${current}`,
      };
    case "gitlab":
    case "gitlab_self_hosted":
      return {
        ok: true,
        url: `${baseHttp}/-/compare/${base}...${current}`,
      };
    case "bitbucket":
    case "gitea":
    case "other":
      return manualHint(remote);
  }
}

/**
 * Re-derives an HTTPS base URL from a `ParsedRemote`. Always returns
 * the canonical `https://<host>/<owner>/<repo>` form regardless of
 * whether the user typed an SSH URL — so the rendered share URL is
 * always something a browser can open.
 */
function baseUrl(remote: ParsedRemote): string {
  const host = remote.hostStr;
  return `https://${host}/${remote.owner}/${remote.repo}`;
}

function manualHint(remote: ParsedRemote): UnsupportedResult {
  const label = humanLabel(remote.host);
  return {
    ok: false,
    reason: "unsupported-host",
    hint: `Manual: open ${remote.hostStr} in browser (${label} share URLs are not yet auto-composed).`,
    fallback: baseUrl(remote),
  };
}

function humanLabel(host: RemoteHost): string {
  switch (host.kind) {
    case "github":
      return "GitHub";
    case "gitlab":
    case "gitlab_self_hosted":
      return "GitLab";
    case "bitbucket":
      return "Bitbucket";
    case "gitea":
      return "Gitea";
    case "other":
      return "this forge";
  }
}
