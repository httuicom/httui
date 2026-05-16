import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { renderHook, act } from "@testing-library/react";

import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { resetGitStore, useGitStore } from "@/stores/git";
import { useGitCommit } from "@/hooks/useGitCommit";

beforeEach(() => {
  clearTauriMocks();
  resetGitStore();
});

afterEach(() => {
  clearTauriMocks();
  resetGitStore();
});

describe("useGitCommit", () => {
  it("no-ops when there is no vault", async () => {
    let called = false;
    mockTauriCommand("git_commit_cmd", () => {
      called = true;
    });
    const { result } = renderHook(() => useGitCommit(null));
    await act(async () => {
      await result.current.commit({ message: "x", amend: false });
    });
    expect(called).toBe(false);
  });

  it("commits, resets the draft and refreshes status", async () => {
    const calls: Array<Record<string, unknown>> = [];
    mockTauriCommand("git_commit_cmd", (args) => {
      calls.push(args as Record<string, unknown>);
    });
    mockTauriCommand("git_status_cmd", () => ({
      branch: "main",
      upstream: null,
      ahead: 0,
      behind: 0,
      changed: [],
      clean: true,
    }));
    useGitStore.setState({
      vaultPath: "/v",
      commitMessage: "draft",
      commitMessageDirty: true,
    });

    const { result } = renderHook(() => useGitCommit("/v"));
    await act(async () => {
      await result.current.commit({ message: "feat: x", amend: false });
    });

    expect(calls).toHaveLength(1);
    expect(calls[0]).toMatchObject({
      vaultPath: "/v",
      message: "feat: x",
      amend: false,
    });
    expect(useGitStore.getState().commitMessage).toBe("");
    expect(useGitStore.getState().commitMessageDirty).toBe(false);
    expect(useGitStore.getState().status?.branch).toBe("main");
  });

  it("guards against a double-submit while in flight", async () => {
    let resolve: (() => void) | null = null;
    let count = 0;
    mockTauriCommand("git_commit_cmd", () => {
      count += 1;
      return new Promise<void>((res) => (resolve = res));
    });
    mockTauriCommand("git_status_cmd", () => ({
      branch: "main",
      upstream: null,
      ahead: 0,
      behind: 0,
      changed: [],
      clean: true,
    }));
    useGitStore.setState({ vaultPath: "/v" });

    const { result } = renderHook(() => useGitCommit("/v"));
    let first!: Promise<void>;
    act(() => {
      first = result.current.commit({ message: "a", amend: false });
    });
    await act(async () => {
      await result.current.commit({ message: "b", amend: false });
    });
    expect(count).toBe(1);
    resolve!();
    await act(async () => {
      await first;
    });
  });
});
