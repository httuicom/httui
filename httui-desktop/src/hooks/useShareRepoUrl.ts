import { useCallback, useMemo } from "react";

import { useGitRemotes } from "@/hooks/useGitRemotes";
import { parseRemoteUrl } from "@/lib/share/remote-host";

export interface ShareUrlOption {
  /** "HTTPS" | "SSH" | "Web" — also the popover picker label. */
  name: string;
  url: string;
  /** Web links can be opened in the browser; clone URLs only copy. */
  openable: boolean;
}

export interface UseShareRepoUrlResult {
  options: ShareUrlOption[];
  copy: (url: string) => void;
  open: (url: string) => void;
}

export function useShareRepoUrl(
  vaultPath: string | null,
): UseShareRepoUrlResult {
  const { remotes } = useGitRemotes(vaultPath);

  const options = useMemo<ShareUrlOption[]>(() => {
    const first = remotes[0];
    if (!first) return [];
    const parsed = parseRemoteUrl(first.url);
    if (!parsed) return [];
    const slug = `${parsed.owner}/${parsed.repo}`;
    return [
      {
        name: "HTTPS",
        url: `https://${parsed.hostStr}/${slug}.git`,
        openable: false,
      },
      {
        name: "SSH",
        url: `git@${parsed.hostStr}:${slug}.git`,
        openable: false,
      },
      {
        name: "Web",
        url: `https://${parsed.hostStr}/${slug}`,
        openable: true,
      },
    ];
  }, [remotes]);

  const copy = useCallback((url: string) => {
    // No await before writeText — clipboard permission is silently denied outside a click gesture.
    void navigator.clipboard.writeText(url);
  }, []);

  const open = useCallback((url: string) => {
    void import("@tauri-apps/plugin-shell").then((m) => m.open(url));
  }, []);

  return { options, copy, open };
}
