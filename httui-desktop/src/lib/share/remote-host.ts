// frontend port of `httui-core::git::remote_host`.
//
// Mirrors the Rust parser shape so the popover can compose URLs
// without an IPC roundtrip on every keystroke / menu open. Rust is
// canonical for any backend-driven flow; this TS port stays in sync
// via a parallel test suite that mirrors the Rust cases.

export type RemoteHost =
  | { kind: "github" }
  | { kind: "gitlab" }
  | { kind: "gitlab_self_hosted"; host: string }
  | { kind: "bitbucket" }
  | { kind: "gitea" }
  | { kind: "other"; host: string };

export interface ParsedRemote {
  host: RemoteHost;
  /** Original-cased host string. */
  hostStr: string;
  owner: string;
  repo: string;
  /** Untouched original URL — used for the "open <origin>" fallback. */
  original: string;
}

export function parseRemoteUrl(url: string): ParsedRemote | null {
  const trimmed = url.trim();
  if (trimmed.length === 0) return null;

  let host: string;
  let path: string;

  if (trimmed.startsWith("git@")) {
    // SSH: git@host:owner/repo[.git]
    const stripped = trimmed.slice("git@".length);
    const colonIdx = stripped.indexOf(":");
    if (colonIdx === -1) return null;
    host = stripped.slice(0, colonIdx);
    path = stripped.slice(colonIdx + 1);
  } else {
    const stripped = stripScheme(trimmed);
    if (stripped === null) return null;
    const slashIdx = stripped.indexOf("/");
    if (slashIdx === -1) return null;
    host = stripped.slice(0, slashIdx);
    path = stripped.slice(slashIdx + 1);
    const atIdx = host.lastIndexOf("@");
    if (atIdx !== -1) host = host.slice(atIdx + 1);
    const portIdx = host.indexOf(":");
    if (portIdx !== -1) host = host.slice(0, portIdx);
  }

  let cleaned = path.replace(/^\/+/u, "").replace(/\/+$/u, "");
  if (cleaned.endsWith(".git")) {
    cleaned = cleaned.slice(0, -".git".length);
  }
  const segments = cleaned.split("/");
  if (segments.length < 2) return null;
  const owner = segments[0]!;
  const repo = segments[segments.length - 1]!;
  if (owner.length === 0 || repo.length === 0) return null;

  return {
    host: classifyHost(host.toLowerCase()),
    hostStr: host,
    owner,
    repo,
    original: trimmed,
  };
}

function stripScheme(url: string): string | null {
  const schemes = ["https://", "http://", "ssh://", "git://"];
  for (const s of schemes) {
    if (url.startsWith(s)) return url.slice(s.length);
  }
  return null;
}

function classifyHost(host: string): RemoteHost {
  if (host === "github.com") return { kind: "github" };
  if (host === "gitlab.com") return { kind: "gitlab" };
  if (host.startsWith("gitlab.")) {
    return { kind: "gitlab_self_hosted", host };
  }
  if (host === "bitbucket.org" || host.endsWith(".bitbucket.org")) {
    return { kind: "bitbucket" };
  }
  if (host === "gitea.com" || host.startsWith("gitea.")) {
    return { kind: "gitea" };
  }
  return { kind: "other", host };
}
