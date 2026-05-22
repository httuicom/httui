import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { resetGitStore } from "@/stores/git";

import { useGitStatus, GIT_STATUS_POLL_MS } from "@/hooks/useGitStatus";

const SAMPLE = {
  branch: "main",
  upstream: "origin/main",
  ahead: 1,
  behind: 0,
  changed: [],
  clean: true,
};

beforeEach(() => {
  clearTauriMocks();
  resetGitStore();
  vi.useFakeTimers();
  mockTauriCommand("git_remote_list_cmd", () => []);
});

afterEach(() => {
  clearTauriMocks();
  resetGitStore();
  vi.useRealTimers();
});

describe("useGitStatus", () => {
  it("returns null status when vaultPath is null", () => {
    const { result } = renderHook(() => useGitStatus(null));
    expect(result.current.status).toBeNull();
    expect(result.current.error).toBeNull();
  });

  it("populates status after the initial poll resolves", async () => {
    mockTauriCommand("git_status_cmd", () => SAMPLE);

    const { result } = renderHook(() => useGitStatus("/v"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.status).toEqual(SAMPLE);
    expect(result.current.error).toBeNull();
  });

  it("polls again after GIT_STATUS_POLL_MS", async () => {
    let calls = 0;
    mockTauriCommand("git_status_cmd", () => {
      calls += 1;
      return SAMPLE;
    });

    renderHook(() => useGitStatus("/v"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(calls).toBe(1);

    await act(async () => {
      vi.advanceTimersByTime(GIT_STATUS_POLL_MS);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(calls).toBe(2);
  });

  it("captures errors and keeps the previous status visible", async () => {
    let throwNext = false;
    mockTauriCommand("git_status_cmd", () => {
      if (throwNext) throw new Error("boom");
      return SAMPLE;
    });

    const { result } = renderHook(() => useGitStatus("/v"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.status).toEqual(SAMPLE);

    throwNext = true;
    await act(async () => {
      vi.advanceTimersByTime(GIT_STATUS_POLL_MS);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.error).toBe("boom");
    expect(result.current.status).toEqual(SAMPLE);
  });

  it("refresh() forces an immediate refetch", async () => {
    let calls = 0;
    mockTauriCommand("git_status_cmd", () => {
      calls += 1;
      return SAMPLE;
    });

    const { result } = renderHook(() => useGitStatus("/v"));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(calls).toBe(1);

    await act(async () => {
      result.current.refresh();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(calls).toBe(2);
  });

  it("resets to null when vaultPath flips back to null", async () => {
    mockTauriCommand("git_status_cmd", () => SAMPLE);

    const { result, rerender } = renderHook(
      ({ p }: { p: string | null }) => useGitStatus(p),
      { initialProps: { p: "/v" as string | null } },
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.status).toEqual(SAMPLE);

    rerender({ p: null });
    expect(result.current.status).toBeNull();
  });
});
