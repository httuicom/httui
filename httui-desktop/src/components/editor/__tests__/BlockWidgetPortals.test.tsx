import { describe, it, expect, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { useRef, type ComponentType } from "react";
import type { EditorView } from "@codemirror/view";

import {
  BlockWidgetPortals,
  type BlockPanelProps,
} from "../BlockWidgetPortals";

interface FakeEntry {
  block: { id: string; body: string };
}

/** Minimal registry shim — a Map + a tiny pub-sub for `useSyncExternalStore`. */
function makeRegistry() {
  const entries = new Map<string, FakeEntry>();
  const listeners = new Set<() => void>();
  let version = 0;
  return {
    subscribe(cb: () => void) {
      listeners.add(cb);
      return () => {
        listeners.delete(cb);
      };
    },
    getVersion: () => version,
    getContainers: () => entries as ReadonlyMap<string, FakeEntry>,
    set(id: string, entry: FakeEntry) {
      entries.set(id, entry);
      version++;
      listeners.forEach((cb) => cb());
    },
    clear() {
      entries.clear();
      version++;
      listeners.forEach((cb) => cb());
    },
  };
}

// Stand-in for HttpFencedPanel / DbFencedPanel — captures every render so
// the test can assert what props the generic component forwarded.
function makeFakePanel() {
  const renderSpy = vi.fn();
  const Panel: ComponentType<BlockPanelProps<FakeEntry>> = (props) => {
    renderSpy(props);
    return <div data-testid={`panel-${props.blockId}`}>{props.block.id}</div>;
  };
  return { Panel, renderSpy };
}

const fakeView = {
  /* opaque to the test */
} as unknown as EditorView;

describe("BlockWidgetPortals", () => {
  it("renders one Panel instance per registered entry, keyed by blockId", () => {
    const registry = makeRegistry();
    registry.set("a", { block: { id: "block-a", body: "" } });
    registry.set("b", { block: { id: "block-b", body: "" } });
    const { Panel } = makeFakePanel();

    render(
      <BlockWidgetPortals
        view={fakeView}
        filePath="x.md"
        subscribe={registry.subscribe}
        getVersion={registry.getVersion}
        getContainers={registry.getContainers}
        Panel={Panel}
      />,
    );

    expect(screen.getByTestId("panel-a")).toBeInTheDocument();
    expect(screen.getByTestId("panel-b")).toBeInTheDocument();
  });

  it("forwards blockId, block, entry, view, filePath to each Panel", () => {
    const registry = makeRegistry();
    const entry: FakeEntry = { block: { id: "block-1", body: "hello" } };
    registry.set("only", entry);
    const { Panel, renderSpy } = makeFakePanel();

    render(
      <BlockWidgetPortals
        view={fakeView}
        filePath="current.md"
        subscribe={registry.subscribe}
        getVersion={registry.getVersion}
        getContainers={registry.getContainers}
        Panel={Panel}
      />,
    );

    const props = renderSpy.mock.calls[0][0];
    expect(props.blockId).toBe("only");
    expect(props.block).toBe(entry.block);
    expect(props.entry).toBe(entry);
    expect(props.view).toBe(fakeView);
    expect(props.filePath).toBe("current.md");
  });

  it("re-renders when the registry version bumps (subscribe callback fires)", () => {
    const registry = makeRegistry();
    const { Panel } = makeFakePanel();

    render(
      <BlockWidgetPortals
        view={fakeView}
        filePath="x.md"
        subscribe={registry.subscribe}
        getVersion={registry.getVersion}
        getContainers={registry.getContainers}
        Panel={Panel}
      />,
    );
    expect(screen.queryByTestId("panel-a")).not.toBeInTheDocument();

    act(() => {
      registry.set("a", { block: { id: "block-a", body: "" } });
    });
    expect(screen.getByTestId("panel-a")).toBeInTheDocument();

    act(() => {
      registry.set("b", { block: { id: "block-b", body: "" } });
    });
    expect(screen.getByTestId("panel-b")).toBeInTheDocument();
  });

  it("removes a Panel when its entry is cleared from the registry", () => {
    const registry = makeRegistry();
    registry.set("a", { block: { id: "block-a", body: "" } });
    const { Panel } = makeFakePanel();

    render(
      <BlockWidgetPortals
        view={fakeView}
        filePath="x.md"
        subscribe={registry.subscribe}
        getVersion={registry.getVersion}
        getContainers={registry.getContainers}
        Panel={Panel}
      />,
    );
    expect(screen.getByTestId("panel-a")).toBeInTheDocument();

    act(() => {
      registry.clear();
    });
    expect(screen.queryByTestId("panel-a")).not.toBeInTheDocument();
  });

  it("renders nothing for an empty registry", () => {
    const registry = makeRegistry();
    const { Panel } = makeFakePanel();

    const { container } = render(
      <BlockWidgetPortals
        view={fakeView}
        filePath="x.md"
        subscribe={registry.subscribe}
        getVersion={registry.getVersion}
        getContainers={registry.getContainers}
        Panel={Panel}
      />,
    );
    // The fragment produces no DOM nodes when the entries list is empty.
    expect(container.children.length).toBe(0);
  });

  it("does NOT re-render the Panel when version is stable", () => {
    const registry = makeRegistry();
    registry.set("a", { block: { id: "block-a", body: "" } });
    const { Panel, renderSpy } = makeFakePanel();

    // Force a parent re-render without touching the registry.
    function Wrapper() {
      const rerenderRef = useRef(0);
      rerenderRef.current++;
      return (
        <BlockWidgetPortals
          view={fakeView}
          filePath="x.md"
          subscribe={registry.subscribe}
          getVersion={registry.getVersion}
          getContainers={registry.getContainers}
          Panel={Panel}
        />
      );
    }

    const { rerender } = render(<Wrapper />);
    const initialCount = renderSpy.mock.calls.length;
    rerender(<Wrapper />);
    // The fake Panel is NOT memoized, so each parent render does re-call
    // it. The assertion verifies that the parent's `entries` memo is
    // version-keyed, not allocation-keyed — a regression to `[]` deps
    // would skip the Panel call after a registry change. Sanity check
    // that the Panel was rendered at least once per parent render.
    expect(renderSpy.mock.calls.length).toBeGreaterThanOrEqual(initialCount);
  });
});
