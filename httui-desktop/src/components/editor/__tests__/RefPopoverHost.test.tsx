import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { RefPopoverHost } from "@/components/editor/RefPopoverHost";
import {
  openRefPopover,
  getRefPopoverState,
  resetRefPopover,
} from "@/lib/blocks/cm-ref-popover";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { useEnvironmentStore } from "@/stores/environment";
import { useWorkspaceStore } from "@/stores/workspace";

function fakeView() {
  return {
    state: { doc: { length: 10 }, selection: { main: { head: 0 } } },
    dispatch: vi.fn(),
    focus: vi.fn(),
  };
}

beforeEach(() => {
  resetRefPopover();
  clearTauriMocks();
  mockTauriCommand("grep_var_uses", () => []);
  useWorkspaceStore.setState({ vaultPath: "/v" } as never);
  useEnvironmentStore.setState({
    activeEnvironment: { id: "e1", name: "local", is_active: true },
    getActiveVariables: async () => ({ api_base: "x" }),
  } as never);
});
afterEach(() => {
  resetRefPopover();
  clearTauriMocks();
});

describe("RefPopoverHost", () => {
  it("renders nothing when no chip is active", () => {
    renderWithProviders(<RefPopoverHost />);
    expect(screen.queryByTestId("ref-popover-host")).toBeNull();
  });

  it("mounts the popover when a chip is clicked", async () => {
    renderWithProviders(<RefPopoverHost />);
    openRefPopover({
      rawKey: "api_base",
      rect: { left: 30, top: 40, right: 80, bottom: 52 },
      view: fakeView() as never,
      caret: 3,
    });
    expect(await screen.findByTestId("ref-popover-host")).toBeInTheDocument();
    expect(screen.getByTestId("ref-popover")).toBeInTheDocument();
  });

  it("Escape closes the popover and clears state", async () => {
    const user = userEvent.setup();
    renderWithProviders(<RefPopoverHost />);
    const view = fakeView();
    openRefPopover({
      rawKey: "api_base",
      rect: { left: 0, top: 0, right: 0, bottom: 0 },
      view: view as never,
      caret: 2,
    });
    await screen.findByTestId("ref-popover-host");
    await user.keyboard("{Escape}");
    expect(getRefPopoverState()).toBeNull();
    expect(view.focus).toHaveBeenCalled();
  });

  it("an outside mousedown closes the popover", async () => {
    renderWithProviders(<RefPopoverHost />);
    openRefPopover({
      rawKey: "api_base",
      rect: { left: 0, top: 0, right: 0, bottom: 0 },
      view: fakeView() as never,
      caret: 0,
    });
    await screen.findByTestId("ref-popover-host");
    // Let the deferred arm fire, then click outside.
    await new Promise((r) => setTimeout(r, 5));
    document.body.dispatchEvent(
      new MouseEvent("mousedown", { bubbles: true }),
    );
    await vi.waitFor(() => expect(getRefPopoverState()).toBeNull());
  });
});
