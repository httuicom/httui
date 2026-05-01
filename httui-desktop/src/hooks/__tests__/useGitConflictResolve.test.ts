import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { act, renderHook } from "@testing-library/react";

import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { useGitConflictResolve } from "@/hooks/useGitConflictResolve";

const VAULT = "/v";

beforeEach(() => clearTauriMocks());
afterEach(() => clearTauriMocks());

describe("useGitConflictResolve", () => {
  it("acceptOurs runs checkout + stage in order", async () => {
    const calls: string[] = [];
    mockTauriCommand("git_checkout_conflict_path_cmd", (args) => {
      const a = args as { side: string };
      calls.push(`checkout:${a.side}`);
    });
    mockTauriCommand("stage_path_cmd", () => {
      calls.push("stage");
    });

    const { result } = renderHook(() => useGitConflictResolve(VAULT));
    await act(async () => {
      await result.current.acceptOurs("conflict.md");
    });

    expect(calls).toEqual(["checkout:ours", "stage"]);
    expect(result.current.busy).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("acceptTheirs sets side='theirs' on the checkout call", async () => {
    let lastSide: string | null = null;
    mockTauriCommand("git_checkout_conflict_path_cmd", (args) => {
      lastSide = (args as { side: string }).side;
    });
    mockTauriCommand("stage_path_cmd", () => {});

    const { result } = renderHook(() => useGitConflictResolve(VAULT));
    await act(async () => {
      await result.current.acceptTheirs("conflict.md");
    });

    expect(lastSide).toBe("theirs");
  });

  it("captures the checkout error and skips the stage step", async () => {
    const stage = vi.fn();
    mockTauriCommand("git_checkout_conflict_path_cmd", () => {
      throw new Error("path is empty");
    });
    mockTauriCommand("stage_path_cmd", () => {
      stage();
    });

    const { result } = renderHook(() => useGitConflictResolve(VAULT));
    await act(async () => {
      await result.current.acceptOurs("");
    });

    expect(result.current.error).toBe("path is empty");
    expect(stage).not.toHaveBeenCalled();
  });

  it("captures stage errors when checkout succeeds but stage fails", async () => {
    mockTauriCommand("git_checkout_conflict_path_cmd", () => {});
    mockTauriCommand("stage_path_cmd", () => {
      throw new Error("index locked");
    });

    const { result } = renderHook(() => useGitConflictResolve(VAULT));
    await act(async () => {
      await result.current.acceptTheirs("conflict.md");
    });

    expect(result.current.error).toBe("index locked");
  });

  it("is a no-op when vaultPath is null", async () => {
    const checkout = vi.fn();
    mockTauriCommand("git_checkout_conflict_path_cmd", () => {
      checkout();
    });

    const { result } = renderHook(() => useGitConflictResolve(null));
    await act(async () => {
      await result.current.acceptOurs("conflict.md");
    });

    expect(checkout).not.toHaveBeenCalled();
    expect(result.current.error).toBeNull();
  });

  it("clearError resets the error to null", async () => {
    mockTauriCommand("git_checkout_conflict_path_cmd", () => {
      throw new Error("nope");
    });
    mockTauriCommand("stage_path_cmd", () => {});

    const { result } = renderHook(() => useGitConflictResolve(VAULT));
    await act(async () => {
      await result.current.acceptOurs("c.md");
    });
    expect(result.current.error).toBe("nope");

    act(() => result.current.clearError());
    expect(result.current.error).toBeNull();
  });

  it("stringifies non-Error throws", async () => {
    mockTauriCommand("git_checkout_conflict_path_cmd", () => {
      throw "boom";
    });
    mockTauriCommand("stage_path_cmd", () => {});

    const { result } = renderHook(() => useGitConflictResolve(VAULT));
    await act(async () => {
      await result.current.acceptOurs("c.md");
    });

    expect(result.current.error).toBe("boom");
  });
});
