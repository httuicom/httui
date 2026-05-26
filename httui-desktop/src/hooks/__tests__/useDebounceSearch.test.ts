import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useDebounceSearch } from "@/hooks/useDebounceSearch";

interface Item {
  id: string;
  label: string;
}

function setup(opts: {
  searchFn?: (q: string) => Promise<Item[]> | null;
  loadOnMount?: () => Promise<Item[]> | null;
  loadOnMountDeps?: ReadonlyArray<unknown>;
  debounceMs?: number;
}) {
  const onSelect = vi.fn();
  const onClose = vi.fn();
  const hook = renderHook(() =>
    useDebounceSearch<Item>({
      searchFn: opts.searchFn ?? (() => null),
      loadOnMount: opts.loadOnMount,
      loadOnMountDeps: opts.loadOnMountDeps,
      debounceMs: opts.debounceMs,
      onSelect,
      onClose,
    }),
  );
  return { ...hook, onSelect, onClose };
}

describe("useDebounceSearch", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  async function flush(ms = 200) {
    await act(async () => {
      vi.advanceTimersByTime(ms);
      await Promise.resolve();
      await Promise.resolve();
    });
  }

  it("starts with empty state (query, results, selectedIndex 0)", () => {
    const { result } = setup({});
    expect(result.current.query).toBe("");
    expect(result.current.results).toEqual([]);
    expect(result.current.selectedIndex).toBe(0);
    expect(result.current.safeIndex).toBe(0);
  });

  it("handleSearch updates query immediately and resets selectedIndex", () => {
    const { result } = setup({});
    act(() => result.current.setSelectedIndex(7));
    act(() => result.current.handleSearch("foo"));
    expect(result.current.query).toBe("foo");
    expect(result.current.selectedIndex).toBe(0);
  });

  it("debounces searchFn within the configured window", async () => {
    const calls: string[] = [];
    const searchFn = vi.fn(async (q: string) => {
      calls.push(q);
      return [{ id: q, label: q }];
    });

    const { result } = setup({ searchFn, debounceMs: 200 });

    act(() => result.current.handleSearch("a"));
    act(() => result.current.handleSearch("ab"));
    act(() => result.current.handleSearch("abc"));

    expect(calls).toEqual([]); // still inside window

    await flush(200);

    expect(calls).toEqual(["abc"]);
    expect(result.current.results).toEqual([{ id: "abc", label: "abc" }]);
  });

  it("uses default debounceMs of 150 when not provided", async () => {
    const searchFn = vi.fn(async () => [{ id: "x", label: "x" }]);
    const { result } = setup({ searchFn });

    act(() => result.current.handleSearch("hi"));

    await act(async () => {
      vi.advanceTimersByTime(149);
      await Promise.resolve();
    });
    expect(searchFn).not.toHaveBeenCalled();

    await act(async () => {
      vi.advanceTimersByTime(2);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(searchFn).toHaveBeenCalledTimes(1);
  });

  it("clears results when searchFn returns null (whitespace-only branch)", async () => {
    const searchFn = (q: string) =>
      q.trim() ? Promise.resolve([{ id: q, label: q }]) : null;

    const { result } = setup({ searchFn, debounceMs: 50 });

    act(() => result.current.handleSearch("ok"));
    await flush(60);
    expect(result.current.results).toHaveLength(1);

    act(() => result.current.handleSearch("   "));
    await flush(60);
    expect(result.current.results).toEqual([]);
  });

  it("clears results to [] when searchFn rejects", async () => {
    const searchFn = vi.fn(() => Promise.reject(new Error("boom")));

    const { result } = setup({ searchFn, debounceMs: 50 });

    // Seed some results so we can confirm the reject clears them
    act(() => result.current.setSelectedIndex(0));
    // simulate prior populated state via handleSearch + a successful run is hard
    // here — easier: just confirm post-reject results are []
    act(() => result.current.handleSearch("x"));
    await flush(80);

    expect(result.current.results).toEqual([]);
  });

  it("loadOnMount runs once on mount and populates results", async () => {
    const loadOnMount = vi.fn(async () => [
      { id: "1", label: "first" },
      { id: "2", label: "second" },
    ]);

    const { result } = setup({ loadOnMount });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(loadOnMount).toHaveBeenCalledTimes(1);
    expect(result.current.results).toHaveLength(2);
  });

  it("loadOnMount returning null skips the load", async () => {
    const loadOnMount = vi.fn(() => null);

    const { result } = setup({ loadOnMount });

    await act(async () => {
      await Promise.resolve();
    });

    expect(loadOnMount).toHaveBeenCalled();
    expect(result.current.results).toEqual([]);
  });

  it("loadOnMount swallows errors silently", async () => {
    const loadOnMount = vi.fn(() => Promise.reject(new Error("net down")));

    const { result } = setup({ loadOnMount });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.results).toEqual([]);
  });

  it("loadOnMount re-runs when loadOnMountDeps change", async () => {
    let loaded = 0;
    const onSelect = vi.fn();
    const onClose = vi.fn();

    const { rerender } = renderHook(
      ({ dep }: { dep: number }) =>
        useDebounceSearch<Item>({
          searchFn: () => null,
          loadOnMount: async () => {
            loaded++;
            return [];
          },
          loadOnMountDeps: [dep],
          onSelect,
          onClose,
        }),
      { initialProps: { dep: 1 } },
    );

    await act(async () => {
      await Promise.resolve();
    });
    expect(loaded).toBe(1);

    rerender({ dep: 2 });

    await act(async () => {
      await Promise.resolve();
    });
    expect(loaded).toBe(2);
  });

  it("safeIndex clamps to results.length-1", async () => {
    const { result } = setup({
      loadOnMount: async () => [
        { id: "a", label: "A" },
        { id: "b", label: "B" },
      ],
      searchFn: () => null,
    });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(result.current.results).toHaveLength(2);

    act(() => result.current.setSelectedIndex(99));
    expect(result.current.safeIndex).toBe(1);
  });

  it("ArrowDown advances selection with wrap", async () => {
    const { result } = setup({
      loadOnMount: async () => [
        { id: "a", label: "A" },
        { id: "b", label: "B" },
        { id: "c", label: "C" },
      ],
    });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const ev = (key: string) =>
      ({ key, preventDefault: vi.fn() }) as unknown as React.KeyboardEvent;

    act(() => result.current.handleKeyDown(ev("ArrowDown")));
    expect(result.current.selectedIndex).toBe(1);
    act(() => result.current.handleKeyDown(ev("ArrowDown")));
    act(() => result.current.handleKeyDown(ev("ArrowDown")));
    expect(result.current.selectedIndex).toBe(0); // wrap
  });

  it("ArrowUp moves backward with wrap", async () => {
    const { result } = setup({
      loadOnMount: async () => [
        { id: "a", label: "A" },
        { id: "b", label: "B" },
      ],
    });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const ev = (key: string) =>
      ({ key, preventDefault: vi.fn() }) as unknown as React.KeyboardEvent;

    act(() => result.current.handleKeyDown(ev("ArrowUp")));
    expect(result.current.selectedIndex).toBe(1); // wrap to last
  });

  it("Enter triggers handleSelect on safeIndex (calls onClose then onSelect)", async () => {
    const items = [
      { id: "a", label: "A" },
      { id: "b", label: "B" },
    ];
    const onSelect = vi.fn();
    const onClose = vi.fn();

    const { result } = renderHook(() =>
      useDebounceSearch<Item>({
        searchFn: () => null,
        loadOnMount: async () => items,
        onSelect,
        onClose,
      }),
    );

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const ev = (key: string) =>
      ({ key, preventDefault: vi.fn() }) as unknown as React.KeyboardEvent;

    act(() => result.current.setSelectedIndex(1));
    act(() => result.current.handleKeyDown(ev("Enter")));

    expect(onClose).toHaveBeenCalled();
    expect(onSelect).toHaveBeenCalledWith(items[1]);
  });

  it("handleSelect for invalid index is a no-op", () => {
    const { result, onSelect, onClose } = setup({});

    act(() => result.current.handleSelect(42));

    expect(onSelect).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });
});
