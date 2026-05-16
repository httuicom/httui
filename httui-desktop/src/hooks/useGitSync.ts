// V10.1 cenário 3 — one-click Sync.
//
// Orchestrates stage-all → commit → pull --ff-only → push as a
// single button. Per-step state drives the progress feedback. Any
// step that fails STOPS the flow and surfaces which step failed —
// never a silent inconsistent state (a local commit that didn't
// push still shows as "ahead" in the status header).
//
// No upstream → reuse the V10 set-upstream confirm: the flow pauses
// before push, the panel renders the confirm, and confirm/cancel
// resumes/aborts the push only (stage/commit/pull already done).
//
// Decisions (locked with the user 2026-05-16):
//   stage-all (not just staged) · pull --ff-only (no merge commit) ·
//   pull before push.

import { useCallback, useState } from "react";

import {
  gitCommit,
  gitPull,
  gitPush,
  stagePath,
  type GitStatus,
} from "@/lib/tauri/git";
import { useGitStore } from "@/stores/git";

export type SyncStep =
  | "idle"
  | "staging"
  | "committing"
  | "pulling"
  | "pushing"
  | "done";

export interface UseGitSyncResult {
  step: SyncStep;
  error: string | null;
  /** Which step failed (set alongside `error`). */
  failedStep: SyncStep | null;
  /** Non-null while the no-upstream confirm is pending. */
  upstreamPrompt: { branch: string; remote: string } | null;
  busy: boolean;
  sync: () => Promise<void>;
  confirmSetUpstream: () => Promise<void>;
  cancelSetUpstream: () => void;
}

const msgOf = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function useGitSync(vaultPath: string | null): UseGitSyncResult {
  const [step, setStep] = useState<SyncStep>("idle");
  const [error, setError] = useState<string | null>(null);
  const [failedStep, setFailedStep] = useState<SyncStep | null>(null);
  const [upstreamPrompt, setUpstreamPrompt] = useState<{
    branch: string;
    remote: string;
  } | null>(null);

  const busy =
    step !== "idle" && step !== "done" && error === null && !upstreamPrompt;

  const finish = useCallback(async () => {
    setStep("done");
    useGitStore.getState().markSynced();
    await useGitStore.getState().refreshStatus();
    await useGitStore.getState().reloadLog();
  }, []);

  const runPush = useCallback(
    async (status: GitStatus) => {
      if (status.upstream === null && status.branch) {
        setUpstreamPrompt({ branch: status.branch, remote: "origin" });
        return;
      }
      setStep("pushing");
      await gitPush(vaultPath!);
      await finish();
    },
    [vaultPath, finish],
  );

  const sync = useCallback(async () => {
    if (!vaultPath || busy) return;
    const status = useGitStore.getState().status;
    if (!status) return;

    setError(null);
    setFailedStep(null);
    setUpstreamPrompt(null);

    const changed = status.changed;
    const message = useGitStore.getState().commitMessage.trim();
    let current: SyncStep = "idle";
    try {
      if (changed.length > 0) {
        current = "staging";
        setStep("staging");
        for (const f of changed) {
          await stagePath(vaultPath, f.path);
        }
        current = "committing";
        setStep("committing");
        if (!message) throw new Error("Commit message is empty");
        await gitCommit(vaultPath, message, false);
        useGitStore.getState().resetCommitMessage();
      }
      current = "pulling";
      setStep("pulling");
      await gitPull(vaultPath, undefined, undefined, true);
      current = "pushing";
      await runPush(status);
    } catch (e) {
      setFailedStep(current);
      setError(msgOf(e));
    }
  }, [vaultPath, busy, runPush]);

  const confirmSetUpstream = useCallback(async () => {
    const prompt = upstreamPrompt;
    if (!vaultPath || !prompt) return;
    setUpstreamPrompt(null);
    setStep("pushing");
    try {
      await gitPush(vaultPath, prompt.remote, prompt.branch, true);
      await finish();
    } catch (e) {
      setFailedStep("pushing");
      setError(msgOf(e));
    }
  }, [vaultPath, upstreamPrompt, finish]);

  const cancelSetUpstream = useCallback(() => {
    setUpstreamPrompt(null);
    setStep("idle");
  }, []);

  return {
    step,
    error,
    failedStep,
    upstreamPrompt,
    busy,
    sync,
    confirmSetUpstream,
    cancelSetUpstream,
  };
}
