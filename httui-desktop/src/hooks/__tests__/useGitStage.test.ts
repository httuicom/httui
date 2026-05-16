import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { renderHook, act } from "@testing-library/react";

import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { resetGitStore, useGitStore } from "@/stores/git";
import { useGitStage } from "@/hooks/useGitStage";

const CLEAN = {
  branch: "main",
  upstream: null,
  ahead: 0,
  behind: 0,
  changed: [],
  clean: true,
};

const file = (over: Partial<Record<string, unknown>> = {}) => ({
  path: "note.md",
  status: " M",
  staged: false,
  untracked: false,
  ...over,
});

beforeEach(() => {
  clearTauriMocks();
  resetGitStore();
  mockTauriCommand("git_status_cmd", () => CLEAN);
});

afterEach(() => {
  clearTauriMocks();
  resetGitStore();
});

describe("useGitStage", () => {
  it("no-ops without a vault", async () => {
    let staged = false;
    mockTauriCommand("stage_path_cmd", () => {
      staged = true;
    });
    const { result } = renderHook(() => useGitStage(null));
    await act(async () => {
      await result.current.toggleStage(file());
    });
    expect(staged).toBe(false);
  });

  it("stages an unstaged path then refreshes status", async () => {
    const calls: string[] = [];
    mockTauriCommand("stage_path_cmd", (a) => {
      calls.push(`stage:${(a as { path: string }).path}`);
    });
    let statusCalls = 0;
    mockTauriCommand("git_status_cmd", () => {
      statusCalls += 1;
      return CLEAN;
    });
    useGitStore.setState({ vaultPath: "/v" });

    const { result } = renderHook(() => useGitStage("/v"));
    await act(async () => {
      await result.current.toggleStage(file({ staged: false }));
    });

    expect(calls).toEqual(["stage:note.md"]);
    expect(statusCalls).toBeGreaterThan(0);
  });

  it("unstages a staged path", async () => {
    const calls: string[] = [];
    mockTauriCommand("unstage_path_cmd", (a) => {
      calls.push(`unstage:${(a as { path: string }).path}`);
    });
    useGitStore.setState({ vaultPath: "/v" });

    const { result } = renderHook(() => useGitStage("/v"));
    await act(async () => {
      await result.current.toggleStage(file({ staged: true }));
    });

    expect(calls).toEqual(["unstage:note.md"]);
  });
});
