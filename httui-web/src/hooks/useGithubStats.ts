import { useEffect, useState } from "react";

const REPO = "httuicom/httui";
const CACHE_KEY = `httui:gh-stats:${REPO}:v1`;
const CACHE_TTL_MS = 60 * 60 * 1000; // 1h

export type GithubStats = {
  stars: string;
  starsRaw: number | null;
  contributors: string;
  contributorsRaw: number | null;
  license: string;
  version: string;
  versionDate: string;
  repoUrl: string;
};

const FALLBACK: GithubStats = {
  stars: "—",
  starsRaw: null,
  contributors: "—",
  contributorsRaw: null,
  license: "MIT",
  version: "—",
  versionDate: "",
  repoUrl: `https://github.com/${REPO}`,
};

function formatStars(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1).replace(/\.0$/, "")}k`;
  return String(n);
}

function formatDate(iso: string): string {
  if (!iso) return "";
  const d = new Date(iso);
  return d.toLocaleDateString("en-US", {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}

function parseLastPage(link: string | null): number | null {
  if (!link) return null;
  const match = link.match(/<[^>]*[?&]page=(\d+)[^>]*>;\s*rel="last"/);
  return match ? Number(match[1]) : null;
}

async function fetchStats(): Promise<GithubStats> {
  const repoRes = await fetch(`https://api.github.com/repos/${REPO}`);
  if (!repoRes.ok) throw new Error(`repo ${repoRes.status}`);
  const repo = await repoRes.json();

  const stars: number = repo.stargazers_count ?? 0;
  const license: string = repo.license?.spdx_id ?? "MIT";

  // Latest release (tolerate 404 — repo may not have releases yet)
  let version = "main";
  let versionDate = "";
  try {
    const relRes = await fetch(
      `https://api.github.com/repos/${REPO}/releases/latest`,
    );
    if (relRes.ok) {
      const rel = await relRes.json();
      version = rel.tag_name ?? version;
      versionDate = formatDate(rel.published_at);
    }
  } catch {
    // ignore
  }

  // Contributor count via Link header (per_page=1)
  let contribCount: number | null = null;
  try {
    const cRes = await fetch(
      `https://api.github.com/repos/${REPO}/contributors?per_page=1&anon=true`,
    );
    if (cRes.ok) {
      const last = parseLastPage(cRes.headers.get("Link"));
      if (last) contribCount = last;
      else {
        const list = await cRes.json();
        if (Array.isArray(list)) contribCount = list.length;
      }
    }
  } catch {
    // ignore
  }

  return {
    stars: formatStars(stars),
    starsRaw: stars,
    contributors: contribCount === null ? "—" : String(contribCount),
    contributorsRaw: contribCount,
    license,
    version,
    versionDate,
    repoUrl: repo.html_url ?? FALLBACK.repoUrl,
  };
}

function readCache(): GithubStats | null {
  if (typeof localStorage === "undefined") return null;
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { ts: number; data: GithubStats };
    if (Date.now() - parsed.ts > CACHE_TTL_MS) return null;
    return parsed.data;
  } catch {
    return null;
  }
}

function writeCache(data: GithubStats) {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify({ ts: Date.now(), data }));
  } catch {
    // ignore
  }
}

export function useGithubStats(): GithubStats {
  const [stats, setStats] = useState<GithubStats>(
    () => readCache() ?? FALLBACK,
  );

  useEffect(() => {
    let cancelled = false;
    if (readCache()) return;
    fetchStats()
      .then((data) => {
        if (cancelled) return;
        setStats(data);
        writeCache(data);
      })
      .catch(() => {
        // keep fallback
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return stats;
}
