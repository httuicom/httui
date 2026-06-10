// Coverage for the DB block orchestrator panel. Mounts the panel with
// every heavy dependency mocked and drives the actions registered in
// the cm-db-block registry to exercise: run gating (no connection /
// empty body / ref errors / dangerous-query confirm), the streamed run
// outcomes (success + persist with alias + lsp notify, error,
// cancelled), squiggle marks from SQL error results, cache hydrate
// wiring, info-string editing and block deletion.
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { act } from "react";
import { renderWithProviders, cleanup } from "@/test/render";

// ── Mocks: cm-db-block registry (capture actions + error marks) ──
const setActionsCalls: { id: string; actions: Record<string, unknown> }[] = [];
const setErrorsCalls: unknown[][] = [];
vi.mock("@/lib/codemirror/cm-db-block", async () => {
  const actual = await vi.importActual<
    typeof import("@/lib/codemirror/cm-db-block")
  >("@/lib/codemirror/cm-db-block");
  return {
    ...actual,
    setDbBlockActions: (id: string, actions: Record<string, unknown>) => {
      setActionsCalls.push({ id, actions });
    },
    setDbBlockErrors: (...args: unknown[]) => {
      setErrorsCalls.push(args);
    },
  };
});

// ── Mocks: sibling hooks (separately tested) ──
vi.mock("../useDbLegacyMigration", () => ({
  useDbLegacyMigration: vi.fn(),
}));
const hydrateOpts: { current: Record<string, unknown> | null } = {
  current: null,
};
vi.mock("../useDbCacheHydrate", () => ({
  useDbCacheHydrate: (opts: Record<string, unknown>) => {
    hydrateOpts.current = opts;
  },
}));
const runExplainMock = vi.fn();
vi.mock("../useDbExplain", () => ({
  useDbExplain: () => runExplainMock,
}));
const loadMoreMock = vi.fn();
vi.mock("../useDbLoadMore", () => ({
  useDbLoadMore: () => loadMoreMock,
}));

// ── Mocks: IO ──
const executeDbStreamed = vi.fn();
const cancelBlockExecution = vi.fn();
vi.mock("@/lib/tauri/streamedExecution", () => ({
  executeDbStreamed: (...args: unknown[]) => executeDbStreamed(...args),
  cancelBlockExecution: (...args: unknown[]) => cancelBlockExecution(...args),
}));
vi.mock("@/lib/tauri/commands", () => ({
  saveBlockResult: vi.fn(async () => undefined),
}));
const notifyBlockRanMock = vi.fn();
vi.mock("@/lib/lsp/client", () => ({
  notifyBlockRan: () => notifyBlockRanMock(),
}));
vi.mock("@/lib/tauri/connections", () => ({
  listConnections: vi.fn(async () => [
    {
      id: "c1",
      name: "local",
      driver: "postgres",
      is_readonly: false,
    },
  ]),
}));
vi.mock("@/lib/blocks/document", () => ({
  collectBlocksAboveCM: vi.fn(async () => []),
}));
vi.mock("@/lib/blocks/hash", () => ({
  computeDbCacheHash: vi.fn(async () => "hash1"),
}));
vi.mock("@/stores/environment", () => ({
  useEnvironmentStore: {
    getState: () => ({ getActiveVariables: async () => ({}) }),
  },
}));
const ensureLoaded = vi.fn();
vi.mock("@/stores/schemaCache", () => ({
  useSchemaCacheStore: {
    getState: () => ({ ensureLoaded }),
  },
}));

// ── Mocks: sub-components — stubs that capture props ──
const toolbarProps: Record<string, unknown> = {};
vi.mock("../DbToolbar", () => ({
  DbToolbar: (props: Record<string, unknown>) => {
    Object.assign(toolbarProps, props);
    return <div data-testid="db-toolbar" />;
  },
}));
const resultProps: Record<string, unknown> = {};
vi.mock("../DbResultTabs", () => ({
  DbResult: (props: Record<string, unknown>) => {
    Object.assign(resultProps, props);
    return <div data-testid="db-result" />;
  },
}));
vi.mock("../DbStatusBar", () => ({
  DbStatusBar: () => <div data-testid="db-statusbar" />,
}));
const drawerProps: Record<string, unknown> = {};
vi.mock("../DbSettingsDrawer", () => ({
  DbSettingsDrawer: (props: Record<string, unknown>) => {
    Object.assign(drawerProps, props);
    return <div data-testid="db-settings-drawer" />;
  },
}));
const confirmProps: Record<string, unknown> = {};
vi.mock("../ConfirmRunDialog", () => ({
  ConfirmRunDialog: (props: Record<string, unknown>) => {
    Object.assign(confirmProps, props);
    return <div data-testid="db-confirm-dialog" />;
  },
}));

// ── SUT ──
import { DbFencedPanel } from "../DbFencedPanel";
import type { DbPortalEntry } from "@/lib/codemirror/cm-db-block";
import { saveBlockResult } from "@/lib/tauri/commands";
import { makeDbBlock, makeView, selectResponse } from "./helpers";
import type { DbBlockMetadata } from "@/lib/blocks/db-fence";

function mountPanel(meta: Partial<DbBlockMetadata> = {}, body = "SELECT 1;") {
  const view = makeView("");
  const block = makeDbBlock(
    view,
    { connection: "c1", alias: "q1", ...meta },
    body,
  );
  const entry = {
    blockId: "db_idx_0",
    block,
    actions: {},
    toolbar: document.createElement("div"),
    result: document.createElement("div"),
    statusbar: document.createElement("div"),
  } as DbPortalEntry;
  const utils = renderWithProviders(
    <DbFencedPanel
      blockId="db_idx_0"
      block={block}
      entry={entry}
      view={view}
      filePath="f.md"
    />,
  );
  const actions = () => setActionsCalls.at(-1)!.actions;
  // The run gate needs the connection list, which loads in an effect —
  // wait for the toolbar to receive the resolved connection.
  const connected = () =>
    vi.waitFor(() => expect(toolbarProps.activeConnection).toBeTruthy());
  return { view, block, entry, actions, connected, ...utils };
}

async function flushRun(actions: () => Record<string, unknown>) {
  await act(async () => {
    (actions().onRun as () => void)();
    await Promise.resolve();
  });
}

beforeEach(() => {
  setActionsCalls.length = 0;
  setErrorsCalls.length = 0;
  executeDbStreamed.mockReset();
  cancelBlockExecution.mockReset();
  notifyBlockRanMock.mockClear();
  vi.mocked(saveBlockResult).mockClear();
  hydrateOpts.current = null;
  for (const o of [toolbarProps, resultProps, drawerProps, confirmProps]) {
    for (const k of Object.keys(o)) delete o[k];
  }
});

afterEach(() => {
  cleanup();
});

describe("DbFencedPanel", () => {
  it("registers run/cancel/explain/settings actions and renders the portals", async () => {
    const { entry } = mountPanel();
    const a = setActionsCalls.at(-1)!;
    expect(a.id).toBe("db_idx_0");
    expect(Object.keys(a.actions)).toEqual(
      expect.arrayContaining([
        "onRun",
        "onCancel",
        "onOpenSettings",
        "onExplain",
      ]),
    );
    expect(
      entry.toolbar!.querySelector("[data-testid=db-toolbar]"),
    ).not.toBeNull();
    expect(
      entry.result!.querySelector("[data-testid=db-result]"),
    ).not.toBeNull();
    expect(
      entry.statusbar!.querySelector("[data-testid=db-statusbar]"),
    ).not.toBeNull();
  });

  it("runs the query, persists with the alias and pings the language server", async () => {
    executeDbStreamed.mockResolvedValue({
      status: "success",
      response: selectResponse([{ id: 1 }]),
    });
    const { actions, connected } = mountPanel();
    await connected();

    await flushRun(actions);

    await vi.waitFor(() => expect(resultProps.executionState).toBe("success"));
    expect(vi.mocked(saveBlockResult).mock.calls[0][6]).toBe("q1");
    expect(notifyBlockRanMock).toHaveBeenCalled();
  });

  it("does not notify the language server for an alias-less block", async () => {
    executeDbStreamed.mockResolvedValue({
      status: "success",
      response: selectResponse([{ id: 1 }]),
    });
    const { actions, connected } = mountPanel({ alias: undefined });
    await connected();

    await flushRun(actions);

    await vi.waitFor(() => expect(resultProps.executionState).toBe("success"));
    expect(vi.mocked(saveBlockResult).mock.calls[0][6]).toBeNull();
    expect(notifyBlockRanMock).not.toHaveBeenCalled();
  });

  it("surfaces execution errors and cancellation", async () => {
    executeDbStreamed.mockResolvedValue({ status: "error", message: "boom" });
    const { actions, connected } = mountPanel();
    await connected();
    await flushRun(actions);
    await vi.waitFor(() => expect(resultProps.executionState).toBe("error"));
    expect(resultProps.error).toBe("boom");

    executeDbStreamed.mockResolvedValue({ status: "cancelled" });
    await flushRun(actions);
    await vi.waitFor(() =>
      expect(resultProps.executionState).toBe("cancelled"),
    );
  });

  it("errors without running when no connection resolves", async () => {
    const { actions } = mountPanel({ connection: "ghost" });
    await vi.waitFor(() => expect(setActionsCalls.length).toBeGreaterThan(0));
    await flushRun(actions);

    await vi.waitFor(() => expect(resultProps.executionState).toBe("error"));
    expect(String(resultProps.error)).toContain("No connection");
    expect(executeDbStreamed).not.toHaveBeenCalled();
  });

  it("errors on an empty query body", async () => {
    const { actions, connected } = mountPanel({}, "   ");
    await connected();
    await flushRun(actions);

    await vi.waitFor(() =>
      expect(String(resultProps.error)).toContain("empty"),
    );
    expect(executeDbStreamed).not.toHaveBeenCalled();
  });

  it("reports unresolved references instead of dispatching", async () => {
    const { actions, connected } = mountPanel(
      {},
      "SELECT {{ghost.response.id}};",
    );
    await connected();
    await flushRun(actions);

    await vi.waitFor(() =>
      expect(String(resultProps.error)).toContain("Reference errors"),
    );
    expect(executeDbStreamed).not.toHaveBeenCalled();
  });

  it("gates a dangerous query behind the confirm dialog", async () => {
    executeDbStreamed.mockResolvedValue({
      status: "success",
      response: selectResponse([]),
    });
    const { actions, connected, getByTestId } = mountPanel(
      {},
      "UPDATE t SET x = 1;",
    );
    await connected();

    await act(async () => {
      (actions().onRun as () => void)();
    });
    expect(getByTestId("db-confirm-dialog")).toBeTruthy();
    expect(executeDbStreamed).not.toHaveBeenCalled();

    await act(async () => {
      (confirmProps.onConfirm as () => void)();
    });
    await vi.waitFor(() => expect(executeDbStreamed).toHaveBeenCalled());
  });

  it("paints squiggle marks for SQL errors with a location", async () => {
    executeDbStreamed.mockResolvedValue({
      status: "success",
      response: {
        results: [{ kind: "error", message: "bad token", line: 2, column: 5 }],
        messages: [],
        stats: { elapsed_ms: 1 },
      },
    });
    const { actions, connected } = mountPanel();
    await connected();
    await flushRun(actions);

    await vi.waitFor(() =>
      expect(
        setErrorsCalls.some(
          (call) =>
            Array.isArray(call[2]) && (call[2] as unknown[]).length === 1,
        ),
      ).toBe(true),
    );
  });

  it("cancels through the backend as a best effort", async () => {
    const { actions } = mountPanel();
    await vi.waitFor(() => expect(setActionsCalls.length).toBeGreaterThan(0));
    await act(async () => {
      (actions().onCancel as () => void)();
    });
    expect(cancelBlockExecution).toHaveBeenCalledWith("db_db_idx_0");
  });

  it("applies cached rows surfaced by the hydrate hook", async () => {
    mountPanel();
    await vi.waitFor(() => expect(hydrateOpts.current).not.toBeNull());

    await act(async () => {
      (
        hydrateOpts.current!.onHit as (hit: {
          response: unknown;
          elapsedMs: number | null;
          state: string;
        }) => void
      )({
        response: selectResponse([{ id: 7 }]),
        elapsedMs: 4,
        state: "success",
      });
    });

    expect(resultProps.cached).toBe(true);
    expect(resultProps.executionState).toBe("success");
  });

  it("rewrites the info string from the settings drawer", async () => {
    const { actions, view, getByTestId } = mountPanel();
    await vi.waitFor(() => expect(setActionsCalls.length).toBeGreaterThan(0));

    await act(async () => {
      (actions().onOpenSettings as () => void)();
    });
    expect(getByTestId("db-settings-drawer")).toBeTruthy();

    await act(async () => {
      (drawerProps.onUpdate as (p: Record<string, unknown>) => void)({
        limit: 5,
      });
    });
    expect(view.state.doc.line(1).text).toContain("limit=5");
  });

  it("deletes the whole block from the document", async () => {
    const { actions, view } = mountPanel();
    await vi.waitFor(() => expect(setActionsCalls.length).toBeGreaterThan(0));

    await act(async () => {
      (actions().onOpenSettings as () => void)();
    });
    await act(async () => {
      (drawerProps.onDelete as () => void)();
    });
    expect(view.state.doc.toString()).toBe("");
  });
});
