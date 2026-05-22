// Single source of truth for git state. Owns polled data (status,
// remotes, commits) and commit draft so the GitSidePanel and pane-tab
// stay in lockstep.
//
// Polling is refcounted: each `useGitStatus`/`useGitRemotes` mount
// `acquire`s and unmount `release`s. A single 2s interval runs
// while at least one consumer is mounted and a vault is open.

import { create } from "zustand";
import { devtools } from "zustand/middleware";

import {
  gitLog,
  gitRemoteList,
  gitStatus,
  type CommitInfo,
  type GitStatus,
  type Remote,
} from "@/lib/tauri/git";

export const GIT_STATUS_POLL_MS = 2000;
const LOG_LIMIT = 50;

interface GitState {
  vaultPath: string | null;
  status: GitStatus | null;
  statusError: string | null;
  remotes: Remote[];
  remotesLoaded: boolean;
  remotesError: string | null;
  commits: CommitInfo[];
  commitMessage: string;
  /** True once the user typed into the commit field — template
   * prefill must not clobber a hand-edited draft. */
  commitMessageDirty: boolean;
  /** Epoch ms of the last successful fetch/pull/push, or null.
   * Drives the pane-tab "last sync" metric. Session-
   *  only — a freshness hint, not worth persisting. */
  lastSyncAt: number | null;

  _subscribers: number;
  _timer: ReturnType<typeof setInterval> | null;

  acquire: (vaultPath: string | null) => void;
  release: () => void;

  refreshStatus: () => Promise<void>;
  refreshRemotes: () => Promise<void>;
  reloadLog: (pathFilter?: string | null) => Promise<void>;

  setCommitMessage: (msg: string) => void;
  resetCommitMessage: () => void;
  /** Prefill from the commit template — sets the text but keeps the
   * draft non-dirty so a later user edit still wins. */
  setCommitMessageFromTemplate: (msg: string) => void;
  /** Stamp a successful sync op ("last sync" metric). */
  markSynced: () => void;
}

const INITIAL = {
  status: null as GitStatus | null,
  statusError: null as string | null,
  remotes: [] as Remote[],
  remotesLoaded: false,
  remotesError: null as string | null,
  commits: [] as CommitInfo[],
  commitMessage: "",
  commitMessageDirty: false,
  lastSyncAt: null as number | null,
};

function stopTimer(timer: ReturnType<typeof setInterval> | null) {
  if (timer) clearInterval(timer);
}

/**
 * Structural equality for the polled IPC payloads. The 2s poll calls
 * `gitStatus`/`gitRemoteList` which deserialize a *fresh* object every
 * tick — `set({status:next})` then changes the store ref even when git
 * is byte-identical, re-rendering every subscriber (GitPanelContainer
 * + GitSidePanel) every 2s. Skipping the `set()` when the payload is
 * unchanged keeps the old reference so subscribers don't re-render on
 * a no-op poll. `JSON.stringify` is safe here: these are small plain
 * structs from serde with a stable field order across polls.
 */
function sameJson(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export const useGitStore = create<GitState>()(
  devtools(
    (set, get) => ({
      vaultPath: null,
      ...INITIAL,
      _subscribers: 0,
      _timer: null,

      acquire: (vaultPath) => {
        const prev = get();
        const subscribers = prev._subscribers + 1;
        if (prev.vaultPath !== vaultPath) {
          set({ vaultPath, ...INITIAL, _subscribers: subscribers });
        } else {
          set({ _subscribers: subscribers });
        }

        if (!vaultPath) {
          stopTimer(get()._timer);
          set({ _timer: null });
          return;
        }

        void get().refreshStatus();
        void get().refreshRemotes();

        if (!get()._timer) {
          const timer = setInterval(() => {
            void get().refreshStatus();
            void get().refreshRemotes();
          }, GIT_STATUS_POLL_MS);
          set({ _timer: timer });
        }
      },

      release: () => {
        const subscribers = Math.max(0, get()._subscribers - 1);
        set({ _subscribers: subscribers });
        if (subscribers === 0) {
          stopTimer(get()._timer);
          set({ _timer: null });
        }
      },

      refreshStatus: async () => {
        const vp = get().vaultPath;
        if (!vp) {
          set({ status: null, statusError: null });
          return;
        }
        try {
          const next = await gitStatus(vp);
          if (get().vaultPath !== vp) return;
          const prev = get();
          if (prev.status !== null && sameJson(prev.status, next)) {
            // Unchanged poll — keep the ref; only clear a stale error.
            if (prev.statusError !== null) set({ statusError: null });
            return;
          }
          set({ status: next, statusError: null });
        } catch (e) {
          if (get().vaultPath !== vp) return;
          set({ statusError: e instanceof Error ? e.message : String(e) });
        }
      },

      refreshRemotes: async () => {
        const vp = get().vaultPath;
        if (!vp) {
          set({ remotes: [], remotesLoaded: false, remotesError: null });
          return;
        }
        try {
          const list = await gitRemoteList(vp);
          if (get().vaultPath !== vp) return;
          const prev = get();
          if (
            prev.remotesLoaded &&
            prev.remotesError === null &&
            sameJson(prev.remotes, list)
          ) {
            // Unchanged poll — keep the ref so subscribers don't churn.
            return;
          }
          set({ remotes: list, remotesLoaded: true, remotesError: null });
        } catch (e) {
          if (get().vaultPath !== vp) return;
          set({
            remotes: [],
            remotesLoaded: false,
            remotesError: e instanceof Error ? e.message : String(e),
          });
        }
      },

      reloadLog: async (pathFilter) => {
        const vp = get().vaultPath;
        if (!vp) {
          set({ commits: [] });
          return;
        }
        try {
          const list = await gitLog(vp, LOG_LIMIT, pathFilter ?? undefined);
          if (get().vaultPath !== vp) return;
          // Coerce at the IPC boundary — consumers iterate over this array.
          set({ commits: Array.isArray(list) ? list : [] });
        } catch {
          set({ commits: [] });
        }
      },

      setCommitMessage: (msg) =>
        set({ commitMessage: msg, commitMessageDirty: true }),
      resetCommitMessage: () =>
        set({ commitMessage: "", commitMessageDirty: false }),
      setCommitMessageFromTemplate: (msg) =>
        set({ commitMessage: msg, commitMessageDirty: false }),
      markSynced: () => set({ lastSyncAt: Date.now() }),
    }),
    { name: "git-store" },
  ),
);

/** Test-only: reset the singleton between specs (mirrors the
 *  workspace store test helper). Clears any live poll timer. */
export function resetGitStore() {
  stopTimer(useGitStore.getState()._timer);
  useGitStore.setState({
    vaultPath: null,
    ...INITIAL,
    _subscribers: 0,
    _timer: null,
  });
}
