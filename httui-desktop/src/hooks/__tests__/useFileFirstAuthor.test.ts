import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { useFileFirstAuthor } from "@/hooks/useFileFirstAuthor";
import type { CommitInfo } from "@/lib/tauri/git";

const ALICE: CommitInfo = {
  sha: "deadbeefcafe1234567890abcdef0123456789ab",
  short_sha: "deadbee",
  author_name: "Alice",
  author_email: "alice@example.com",
  timestamp: 1_700_000_000,
  subject: "add runbook",
};

beforeEach(() => clearTauriMocks());
afterEach(() => clearTauriMocks());

describe("useFileFirstAuthor", () => {
  it("idles when vaultPath is null", async () => {
    const { result } = renderHook(() => useFileFirstAuthor(null, "runbook.md"));
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.author).toBeNull();
    expect(result.current.loaded).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("idles when filePath is null", async () => {
    const { result } = renderHook(() => useFileFirstAuthor("/v", null));
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.author).toBeNull();
    expect(result.current.loaded).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("populates the author after the initial fetch", async () => {
    mockTauriCommand("git_first_commit_author_cmd", () => ALICE);
    const { result } = renderHook(() => useFileFirstAuthor("/v", "runbook.md"));
    await waitFor(() => expect(result.current.loaded).toBe(true));
    expect(result.current.author).toEqual(ALICE);
    expect(result.current.error).toBeNull();
  });

  it("loaded=true with author=null is the not-in-history signal", async () => {
    mockTauriCommand("git_first_commit_author_cmd", () => null);
    const { result } = renderHook(() =>
      useFileFirstAuthor("/v", "untracked.md"),
    );
    await waitFor(() => expect(result.current.loaded).toBe(true));
    expect(result.current.author).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it("captures the IPC error on failure", async () => {
    mockTauriCommand("git_first_commit_author_cmd", () => {
      throw new Error("not a git repo");
    });
    const { result } = renderHook(() => useFileFirstAuthor("/v", "runbook.md"));
    await waitFor(() => expect(result.current.error).toBe("not a git repo"));
    expect(result.current.author).toBeNull();
    expect(result.current.loaded).toBe(false);
  });

  it("stringifies non-Error throws", async () => {
    mockTauriCommand("git_first_commit_author_cmd", () => {
      throw "boom";
    });
    const { result } = renderHook(() => useFileFirstAuthor("/v", "runbook.md"));
    await waitFor(() => expect(result.current.error).toBe("boom"));
  });

  it("refresh re-fetches and replaces stale data", async () => {
    let next: CommitInfo | null = ALICE;
    mockTauriCommand("git_first_commit_author_cmd", () => next);
    const { result } = renderHook(() => useFileFirstAuthor("/v", "runbook.md"));
    await waitFor(() => expect(result.current.author).toEqual(ALICE));

    next = null;
    await act(async () => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.author).toBeNull());
    expect(result.current.loaded).toBe(true);
  });

  it("clears state when vaultPath transitions to null", async () => {
    mockTauriCommand("git_first_commit_author_cmd", () => ALICE);
    const { result, rerender } = renderHook(
      ({ vault }: { vault: string | null }) =>
        useFileFirstAuthor(vault, "runbook.md"),
      { initialProps: { vault: "/v" as string | null } },
    );
    await waitFor(() => expect(result.current.author).toEqual(ALICE));

    rerender({ vault: null });
    await waitFor(() => expect(result.current.author).toBeNull());
    expect(result.current.loaded).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("error clears on successful refresh", async () => {
    let mode: "throw" | "ok" = "throw";
    mockTauriCommand("git_first_commit_author_cmd", () => {
      if (mode === "throw") throw new Error("transient");
      return ALICE;
    });
    const { result } = renderHook(() => useFileFirstAuthor("/v", "runbook.md"));
    await waitFor(() => expect(result.current.error).toBe("transient"));

    mode = "ok";
    await act(async () => {
      result.current.refresh();
    });
    await waitFor(() => expect(result.current.error).toBeNull());
    expect(result.current.author).toEqual(ALICE);
  });
});
