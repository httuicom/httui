import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { act } from "@testing-library/react";
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

function open(view = fakeView()) {
  act(() => {
    openRefPopover({
      rawKey: "api_base",
      rect: { left: 30, top: 40, right: 80, bottom: 52 },
      view: view as never,
      caret: 3,
    });
  });
  return view;
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

  it("mounts the Chakra popover when a chip is clicked", async () => {
    renderWithProviders(<RefPopoverHost />);
    open();
    expect(await screen.findByTestId("ref-popover-host")).toBeInTheDocument();
    expect(screen.getByTestId("ref-popover")).toBeInTheDocument();
  });

  it("Escape (Chakra onOpenChange) closes + restores editor focus", async () => {
    const user = userEvent.setup();
    renderWithProviders(<RefPopoverHost />);
    const view = open();
    await screen.findByTestId("ref-popover-host");
    await user.keyboard("{Escape}");
    await vi.waitFor(() => expect(getRefPopoverState()).toBeNull());
    expect(view.focus).toHaveBeenCalled();
  });

  it("the Close button closes + restores editor focus", async () => {
    const user = userEvent.setup();
    renderWithProviders(<RefPopoverHost />);
    const view = open();
    await user.click(await screen.findByTestId("ref-popover-close"));
    expect(getRefPopoverState()).toBeNull();
    expect(view.focus).toHaveBeenCalled();
  });
});
