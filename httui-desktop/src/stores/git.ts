// V10.1 — single source of truth for git state.
//
// V10 polled git status/remotes from per-component hooks and kept
// the commit-message draft + log list in GitPanelContainer's local
// state. V10.1 adds a second consumer (the GitSidePanel) that must
// stay in lockstep with the pane-tab (cenário 7). This store owns
// the *polled data* (status, remotes, commits) and the commit
// draft; the pure action hooks (branch actions, conflict resolve,
// share URL) and every presentational sub-component are carry from
// V10 and stay untouched.
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
   *  prefill (cenário 2) must not clobber a hand-edited draft. */
  commitMessageDirty: boolean;
  /** Epoch ms of the last successful fetch/pull/push, or null.
   *  Drives the pane-tab "last sync" metric (cenário 6). Session-
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
   *  draft non-dirty so a later user edit still wins (cenário 2). */
  setCommitMessageFromTemplate: (msg: string) => void;
  /** Stamp a successful sync op (cenário 6 "last sync" metric). */
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
          // Coerce at the IPC boundary — consumers `.slice`/`.map`
          // over this, so a non-array result must never land in state.
          set({ commits: Array.isArray(list) ? list : [] });
        } catch {
          // Transient (not a repo yet, IPC dead) — the status poll
          // surfaces real errors; the log list just stays empty.
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
