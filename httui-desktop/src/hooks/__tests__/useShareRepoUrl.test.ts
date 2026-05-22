import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

const { shellOpen } = vi.hoisted(() => ({ shellOpen: vi.fn() }));
vi.mock("@tauri-apps/plugin-shell", () => ({
  open: shellOpen,
}));

import { useShareRepoUrl } from "@/hooks/useShareRepoUrl";

const writeText = vi.fn();

beforeEach(() => {
  clearTauriMocks();
  shellOpen.mockClear();
  writeText.mockClear();
  Object.assign(navigator, {
    clipboard: { writeText: (t: string) => writeText(t) },
  });
});

afterEach(() => clearTauriMocks());

describe("useShareRepoUrl", () => {
  it("returns no options when vaultPath is null", () => {
    const { result } = renderHook(() => useShareRepoUrl(null));
    expect(result.current.options).toEqual([]);
  });

  it("derives HTTPS / SSH / Web from a GitHub SSH remote", async () => {
    mockTauriCommand("git_remote_list_cmd", () => [
      { name: "origin", url: "git@github.com:acme/widgets.git" },
    ]);
    const { result } = renderHook(() => useShareRepoUrl("/v"));
    await waitFor(() => expect(result.current.options).toHaveLength(3));
    const byName = Object.fromEntries(
      result.current.options.map((o) => [o.name, o]),
    );
    expect(byName.HTTPS.url).toBe("https://github.com/acme/widgets.git");
    expect(byName.SSH.url).toBe("git@github.com:acme/widgets.git");
    expect(byName.Web.url).toBe("https://github.com/acme/widgets");
    expect(byName.Web.openable).toBe(true);
    expect(byName.HTTPS.openable).toBe(false);
  });

  it("returns no options when the remote URL doesn't parse", async () => {
    mockTauriCommand("git_remote_list_cmd", () => [
      { name: "origin", url: "not-a-url" },
    ]);
    const { result } = renderHook(() => useShareRepoUrl("/v"));
    await waitFor(() => expect(result.current.options).toEqual([]));
  });

  it("copy writes to the clipboard", () => {
    const { result } = renderHook(() => useShareRepoUrl(null));
    result.current.copy("https://x/y.git");
    expect(writeText).toHaveBeenCalledWith("https://x/y.git");
  });

  it("open routes to the tauri shell opener", async () => {
    const { result } = renderHook(() => useShareRepoUrl(null));
    result.current.open("https://github.com/acme/widgets");
    // Lazy import lands on a microtask — wait for it.
    await waitFor(() =>
      expect(shellOpen).toHaveBeenCalledWith("https://github.com/acme/widgets"),
    );
  });
});
