import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useContentSearch } from "@/hooks/useContentSearch";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import type { ContentSearchResult } from "@/lib/tauri/commands";

const mkHit = (path: string, snippet: string): ContentSearchResult => ({
  file_path: path,
  snippet,
});

describe("useContentSearch", () => {
  beforeEach(() => {
    clearTauriMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    clearTauriMocks();
    vi.useRealTimers();
  });

  async function flush() {
    await act(async () => {
      vi.advanceTimersByTime(200);
      await Promise.resolve();
      await Promise.resolve();
    });
  }

  it("starts with empty state", () => {
    const { result } = renderHook(() =>
      useContentSearch({ onSelect: vi.fn(), onClose: vi.fn() }),
    );
    expect(result.current.query).toBe("");
    expect(result.current.results).toEqual([]);
    expect(result.current.grouped).toEqual({});
  });

  it("debounces search_content (150ms) and uses trimmed query", async () => {
    let calls = 0;
    let lastQuery: string | null = null;
    mockTauriCommand("search_content", (args) => {
      calls++;
      lastQuery = (args as { query: string }).query;
      return [mkHit("a.md", "hello")];
    });

    const { result } = renderHook(() =>
      useContentSearch({ onSelect: vi.fn(), onClose: vi.fn() }),
    );

    act(() => result.current.handleSearch("  hel  "));
    expect(calls).toBe(0); // debounce open

    await act(async () => {
      vi.advanceTimersByTime(150);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(calls).toBe(1);
    expect(lastQuery).toBe("hel");
  });

  it("clears results when query is whitespace only", async () => {
    mockTauriCommand("search_content", () => [mkHit("a.md", "x")]);
    const { result } = renderHook(() =>
      useContentSearch({ onSelect: vi.fn(), onClose: vi.fn() }),
    );

    act(() => result.current.handleSearch("ab"));
    await flush();
    expect(result.current.results).toHaveLength(1);

    act(() => result.current.handleSearch("   "));
    await flush();
    expect(result.current.results).toEqual([]);
  });

  it("groups results by file_path", async () => {
    mockTauriCommand("search_content", () => [
      mkHit("a.md", "hit1"),
      mkHit("a.md", "hit2"),
      mkHit("b.md", "hit3"),
    ]);

    const { result } = renderHook(() =>
      useContentSearch({ onSelect: vi.fn(), onClose: vi.fn() }),
    );
    act(() => result.current.handleSearch("x"));
    await flush();

    expect(result.current.grouped).toEqual({
      "a.md": [mkHit("a.md", "hit1"), mkHit("a.md", "hit2")],
      "b.md": [mkHit("b.md", "hit3")],
    });
  });

  it("handleSelect calls onClose then onSelect with file_path", async () => {
    mockTauriCommand("search_content", () => [mkHit("found.md", "snippet")]);
    const onSelect = vi.fn();
    const onClose = vi.fn();

    const { result } = renderHook(() =>
      useContentSearch({ onSelect, onClose }),
    );
    act(() => result.current.handleSearch("x"));
    await flush();

    act(() => result.current.handleSelect(0));

    expect(onClose).toHaveBeenCalled();
    expect(onSelect).toHaveBeenCalledWith("found.md");
  });

  it("handleSelect is no-op for invalid index", () => {
    const onSelect = vi.fn();
    const onClose = vi.fn();
    const { result } = renderHook(() =>
      useContentSearch({ onSelect, onClose }),
    );

    act(() => result.current.handleSelect(0));
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("ArrowDown / ArrowUp navigate with wrap", async () => {
    mockTauriCommand("search_content", () => [
      mkHit("a.md", "1"),
      mkHit("b.md", "2"),
      mkHit("c.md", "3"),
    ]);

    const { result } = renderHook(() =>
      useContentSearch({ onSelect: vi.fn(), onClose: vi.fn() }),
    );
    act(() => result.current.handleSearch("x"));
    await flush();

    const ev = (key: string) =>
      ({ key, preventDefault: vi.fn() }) as unknown as React.KeyboardEvent;

    act(() => result.current.handleKeyDown(ev("ArrowDown")));
    expect(result.current.selectedIndex).toBe(1);
    act(() => result.current.handleKeyDown(ev("ArrowUp")));
    act(() => result.current.handleKeyDown(ev("ArrowUp")));
    expect(result.current.selectedIndex).toBe(2); // wrapped to last
  });

  it("Enter triggers handleSelect on safeIndex", async () => {
    mockTauriCommand("search_content", () => [
      mkHit("first.md", "1"),
      mkHit("second.md", "2"),
    ]);
    const onSelect = vi.fn();

    const { result } = renderHook(() =>
      useContentSearch({ onSelect, onClose: vi.fn() }),
    );
    act(() => result.current.handleSearch("x"));
    await flush();

    const ev = (key: string) =>
      ({ key, preventDefault: vi.fn() }) as unknown as React.KeyboardEvent;

    act(() => result.current.setSelectedIndex(1));
    act(() => result.current.handleKeyDown(ev("Enter")));

    expect(onSelect).toHaveBeenCalledWith("second.md");
  });

  it("clears results to [] on search_content error", async () => {
    mockTauriCommand("search_content", () => {
      throw new Error("backend down");
    });

    const { result } = renderHook(() =>
      useContentSearch({ onSelect: vi.fn(), onClose: vi.fn() }),
    );
    act(() => result.current.handleSearch("anything"));
    await flush();

    expect(result.current.results).toEqual([]);
  });
});
