import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { renderHook, act } from "@testing-library/react";

import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { resetGitStore } from "@/stores/git";

import { useGitRemotes } from "@/hooks/useGitRemotes";

beforeEach(() => {
  clearTauriMocks();
  resetGitStore();
  // The store-backed shim also polls status; keep it quiet so these
  // remotes-only specs stay deterministic.
  mockTauriCommand("git_status_cmd", () => ({
    branch: "main",
    upstream: null,
    ahead: 0,
    behind: 0,
    changed: [],
    clean: true,
  }));
});

afterEach(() => {
  clearTauriMocks();
  resetGitStore();
});

describe("useGitRemotes", () => {
  it("idles when vaultPath is null", async () => {
    const { result } = renderHook(() => useGitRemotes(null));
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.remotes).toEqual([]);
    expect(result.current.loaded).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("loads remotes on vault change", async () => {
    mockTauriCommand("git_remote_list_cmd", () => [
      { name: "origin", url: "git@github.com:foo/bar.git" },
    ]);
    const { result } = renderHook(() => useGitRemotes("/v"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.remotes).toEqual([
      { name: "origin", url: "git@github.com:foo/bar.git" },
    ]);
    expect(result.current.loaded).toBe(true);
    expect(result.current.error).toBeNull();
  });

  it("treats empty-list as a successful load (no remotes configured)", async () => {
    // Empty array isn't an error — the popover renders the empty
    // state with a "configure a remote" link. `loaded` stays true
    // so the consumer can distinguish "haven't fetched" from
    // "fetched and got nothing".
    mockTauriCommand("git_remote_list_cmd", () => []);
    const { result } = renderHook(() => useGitRemotes("/v"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.remotes).toEqual([]);
    expect(result.current.loaded).toBe(true);
    expect(result.current.error).toBeNull();
  });

  it("surfaces Tauri errors via the error field", async () => {
    mockTauriCommand("git_remote_list_cmd", () => {
      throw new Error("not a git repository");
    });
    const { result } = renderHook(() => useGitRemotes("/v"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.remotes).toEqual([]);
    expect(result.current.loaded).toBe(false);
    expect(result.current.error).toBe("not a git repository");
  });

  it("non-Error throws stringify into the error field", async () => {
    mockTauriCommand("git_remote_list_cmd", () => {
      throw "string error";
    });
    const { result } = renderHook(() => useGitRemotes("/v"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.error).toBe("string error");
  });

  it("refresh() re-fetches manually", async () => {
    let count = 0;
    mockTauriCommand("git_remote_list_cmd", () => {
      count += 1;
      return count === 1
        ? [{ name: "origin", url: "first" }]
        : [
            { name: "origin", url: "first" },
            { name: "upstream", url: "second" },
          ];
    });
    const { result } = renderHook(() => useGitRemotes("/v"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.remotes).toHaveLength(1);

    await act(async () => {
      result.current.refresh();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.remotes).toHaveLength(2);
  });

  it("clears state when vault is set to null", async () => {
    mockTauriCommand("git_remote_list_cmd", () => [
      { name: "origin", url: "u" },
    ]);
    const { result, rerender } = renderHook(
      ({ vault }) => useGitRemotes(vault),
      { initialProps: { vault: "/v" as string | null } },
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.loaded).toBe(true);

    rerender({ vault: null });
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.remotes).toEqual([]);
    expect(result.current.loaded).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("clears the error on a successful refresh", async () => {
    let firstCall = true;
    mockTauriCommand("git_remote_list_cmd", () => {
      if (firstCall) {
        firstCall = false;
        throw new Error("boom");
      }
      return [{ name: "origin", url: "ok" }];
    });
    const { result } = renderHook(() => useGitRemotes("/v"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.error).toBe("boom");

    await act(async () => {
      result.current.refresh();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.remotes).toEqual([{ name: "origin", url: "ok" }]);
    expect(result.current.error).toBeNull();
    expect(result.current.loaded).toBe(true);
  });
});
