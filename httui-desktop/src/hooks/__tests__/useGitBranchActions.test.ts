import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

import { useGitBranchActions } from "@/hooks/useGitBranchActions";
import { useWorkspaceStore } from "@/stores/workspace";
import type { BranchInfo } from "@/lib/tauri/git";

const BRANCHES: BranchInfo[] = [
  { name: "main", current: true, remote: false },
  { name: "feat/x", current: false, remote: false },
  { name: "origin/main", current: false, remote: true },
];

let refreshCalls = 0;

beforeEach(() => {
  refreshCalls = 0;
  clearTauriMocks();
  mockTauriCommand("git_branch_list_cmd", () => BRANCHES);
  mockTauriCommand("git_checkout_cmd", () => undefined);
  mockTauriCommand("git_checkout_b_cmd", () => undefined);
  useWorkspaceStore.setState({
    refreshFileTree: vi.fn(async () => {
      refreshCalls += 1;
    }),
  });
});

afterEach(() => {
  clearTauriMocks();
});

describe("useGitBranchActions", () => {
  it("is idle when vaultPath is null", () => {
    const { result } = renderHook(() => useGitBranchActions(null));
    expect(result.current.branches).toEqual([]);
    act(() => result.current.loadBranches());
    expect(result.current.branches).toEqual([]);
  });

  it("loads branches on demand", async () => {
    const { result } = renderHook(() => useGitBranchActions("/v"));
    act(() => result.current.loadBranches());
    await waitFor(() =>
      expect(result.current.branches).toHaveLength(3),
    );
    expect(result.current.error).toBeNull();
  });

  it("captures an error when branch list fails", async () => {
    mockTauriCommand("git_branch_list_cmd", () => {
      throw new Error("not a git repo");
    });
    const { result } = renderHook(() => useGitBranchActions("/v"));
    act(() => result.current.loadBranches());
    await waitFor(() =>
      expect(result.current.error).toBe("not a git repo"),
    );
    expect(result.current.branches).toEqual([]);
  });

  it("selectBranch checks out, refreshes the tree and reloads", async () => {
    let checkedOut = "";
    mockTauriCommand("git_checkout_cmd", (a) => {
      checkedOut = (a as { branch: string }).branch;
      return undefined;
    });
    const { result } = renderHook(() => useGitBranchActions("/v"));
    await act(async () => {
      await result.current.selectBranch(BRANCHES[1]);
    });
    expect(checkedOut).toBe("feat/x");
    expect(refreshCalls).toBe(1);
    expect(result.current.branches).toHaveLength(3);
  });

  it("createBranch routes to git_checkout_b", async () => {
    let created = "";
    mockTauriCommand("git_checkout_b_cmd", (a) => {
      created = (a as { newBranch: string }).newBranch;
      return undefined;
    });
    const { result } = renderHook(() => useGitBranchActions("/v"));
    await act(async () => {
      await result.current.createBranch("feat/new");
    });
    expect(created).toBe("feat/new");
    expect(refreshCalls).toBe(1);
  });

  it("surfaces a checkout error without throwing", async () => {
    mockTauriCommand("git_checkout_cmd", () => {
      throw new Error("would overwrite");
    });
    const { result } = renderHook(() => useGitBranchActions("/v"));
    await act(async () => {
      await result.current.selectBranch(BRANCHES[1]);
    });
    expect(result.current.error).toBe("would overwrite");
    expect(result.current.busy).toBe(false);
  });
});
