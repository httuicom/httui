import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { renderHook, act } from "@testing-library/react";

import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { resetGitStore, useGitStore } from "@/stores/git";
import { useGitSync } from "@/hooks/useGitSync";
import type { GitStatus } from "@/lib/tauri/git";

const CLEAN: GitStatus = {
  branch: "main",
  upstream: "origin/main",
  ahead: 0,
  behind: 0,
  changed: [],
  clean: true,
};
const changedStatus = (over: Partial<typeof CLEAN> = {}) => ({
  ...CLEAN,
  clean: false,
  changed: [{ path: "a.md", status: " M", staged: false, untracked: false }],
  ...over,
});

function mockGit(record: string[]) {
  mockTauriCommand("stage_path_cmd", (a) =>
    record.push(`stage:${(a as { path: string }).path}`),
  );
  mockTauriCommand("git_commit_cmd", (a) =>
    record.push(`commit:${(a as { message: string }).message}`),
  );
  mockTauriCommand("git_pull_cmd", (a) =>
    record.push(`pull:ffOnly=${(a as { ffOnly: boolean }).ffOnly}`),
  );
  mockTauriCommand("git_push_cmd", (a) =>
    record.push(`push:u=${(a as { setUpstream: boolean }).setUpstream}`),
  );
  mockTauriCommand("git_status_cmd", () => useGitStore.getState().status);
  mockTauriCommand("git_log_cmd", () => []);
}

beforeEach(() => {
  clearTauriMocks();
  resetGitStore();
});
afterEach(() => {
  clearTauriMocks();
  resetGitStore();
});

describe("useGitSync", () => {
  it("runs stage-all → commit → pull --ff-only → push in order", async () => {
    const rec: string[] = [];
    mockGit(rec);
    useGitStore.setState({
      vaultPath: "/v",
      status: changedStatus(),
      commitMessage: "Update a",
    });

    const { result } = renderHook(() => useGitSync("/v"));
    await act(async () => {
      await result.current.sync();
    });

    expect(rec).toEqual([
      "stage:a.md",
      "commit:Update a",
      "pull:ffOnly=true",
      "push:u=false",
    ]);
    expect(result.current.step).toBe("done");
    expect(result.current.error).toBeNull();
  });

  it("stops at the failing step and reports it", async () => {
    const rec: string[] = [];
    mockGit(rec);
    mockTauriCommand("git_pull_cmd", () => {
      throw new Error("not fast-forward");
    });
    useGitStore.setState({
      vaultPath: "/v",
      status: changedStatus(),
      commitMessage: "Update a",
    });

    const { result } = renderHook(() => useGitSync("/v"));
    await act(async () => {
      await result.current.sync();
    });

    expect(result.current.failedStep).toBe("pulling");
    expect(result.current.error).toBe("not fast-forward");
    expect(rec).not.toContain("push:u=false");
  });

  it("blocks an empty commit message", async () => {
    const rec: string[] = [];
    mockGit(rec);
    useGitStore.setState({
      vaultPath: "/v",
      status: changedStatus(),
      commitMessage: "   ",
    });

    const { result } = renderHook(() => useGitSync("/v"));
    await act(async () => {
      await result.current.sync();
    });

    expect(result.current.failedStep).toBe("committing");
    expect(result.current.error).toMatch(/empty/i);
    expect(rec).not.toContain("commit:");
  });

  it("pauses for the set-upstream confirm, then pushes with -u", async () => {
    const rec: string[] = [];
    mockGit(rec);
    useGitStore.setState({
      vaultPath: "/v",
      status: changedStatus({ upstream: null }),
      commitMessage: "Update a",
    });

    const { result } = renderHook(() => useGitSync("/v"));
    await act(async () => {
      await result.current.sync();
    });
    expect(result.current.upstreamPrompt).toEqual({
      branch: "main",
      remote: "origin",
    });
    expect(rec).not.toContain("push:u=false");

    await act(async () => {
      await result.current.confirmSetUpstream();
    });
    expect(rec).toContain("push:u=true");
    expect(result.current.step).toBe("done");
  });

  it("cancel set-upstream returns to idle without pushing", async () => {
    const rec: string[] = [];
    mockGit(rec);
    useGitStore.setState({
      vaultPath: "/v",
      status: changedStatus({ upstream: null }),
      commitMessage: "Update a",
    });

    const { result } = renderHook(() => useGitSync("/v"));
    await act(async () => {
      await result.current.sync();
    });
    act(() => {
      result.current.cancelSetUpstream();
    });
    expect(result.current.upstreamPrompt).toBeNull();
    expect(result.current.step).toBe("idle");
    expect(rec.some((r) => r.startsWith("push:"))).toBe(false);
  });

  it("skips stage/commit when the tree is clean (push-only sync)", async () => {
    const rec: string[] = [];
    mockGit(rec);
    useGitStore.setState({
      vaultPath: "/v",
      status: { ...CLEAN, ahead: 1 },
      commitMessage: "",
    });

    const { result } = renderHook(() => useGitSync("/v"));
    await act(async () => {
      await result.current.sync();
    });

    expect(rec).toEqual(["pull:ffOnly=true", "push:u=false"]);
    expect(result.current.step).toBe("done");
  });
});
