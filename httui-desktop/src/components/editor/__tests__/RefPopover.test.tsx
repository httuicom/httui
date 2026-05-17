import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { RefPopover } from "@/components/editor/RefPopover";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { useEnvironmentStore } from "@/stores/environment";
import { useSessionOverrideStore } from "@/stores/sessionOverride";
import type { RefPopoverState } from "@/lib/blocks/cm-ref-popover";

function mkState(rawKey: string): RefPopoverState {
  return {
    rawKey,
    rect: { left: 0, top: 0, right: 0, bottom: 0 },
    view: {} as never,
    caret: 0,
  };
}

beforeEach(() => {
  clearTauriMocks();
  useSessionOverrideStore.setState({ overrides: {} });
  useEnvironmentStore.setState({
    activeEnvironment: { id: "e1", name: "local", is_active: true },
    getActiveVariables: async () => ({ api_base: "http://localhost:3000" }),
  } as never);
  mockTauriCommand("grep_var_uses", () => [
    { file_path: "runbook.md", line: 7, snippet: "{{api_base}}" },
    { file_path: "ops.md", line: 3, snippet: "{{api_base}}/x" },
  ]);
});
afterEach(() => clearTauriMocks());

describe("RefPopover — env variable", () => {
  it("shows the per-env value", async () => {
    renderWithProviders(
      <RefPopover
        state={mkState("api_base")}
        vaultPath="/v"
        onClose={() => {}}
      />,
    );
    expect(
      (await screen.findByTestId("ref-popover-value")).textContent,
    ).toContain("http://localhost:3000");
  });

  it("Set writes a session override and renders the TEMPORARY chip", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <RefPopover
        state={mkState("api_base")}
        vaultPath="/v"
        onClose={() => {}}
      />,
    );
    await user.type(
      screen.getByTestId("ref-popover-override-input"),
      "http://staging",
    );
    await user.click(screen.getByTestId("ref-popover-override-set"));
    expect(
      useSessionOverrideStore.getState().getOverride("local", "api_base"),
    ).toBe("http://staging");
    expect(await screen.findByTestId("temporary-chip")).toBeInTheDocument();
  });

  it("clears the override via the TEMPORARY chip", async () => {
    const user = userEvent.setup();
    useSessionOverrideStore.getState().setOverride("local", "api_base", "ov");
    renderWithProviders(
      <RefPopover
        state={mkState("api_base")}
        vaultPath="/v"
        onClose={() => {}}
      />,
    );
    await user.click(screen.getByTestId("temporary-chip"));
    expect(
      useSessionOverrideStore.getState().getOverride("local", "api_base"),
    ).toBeUndefined();
  });

  it("lists the blocks the variable is used in", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <RefPopover
        state={mkState("api_base")}
        vaultPath="/v"
        onClose={() => {}}
      />,
    );
    const usesBtn = await screen.findByTestId("ref-popover-uses");
    expect(usesBtn.textContent).toContain("Used in 2 blocks");
    await user.click(usesBtn);
    expect(screen.getAllByTestId("ref-popover-use-row")).toHaveLength(2);
  });

  it("Close fires onClose", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <RefPopover
        state={mkState("api_base")}
        vaultPath="/v"
        onClose={onClose}
      />,
    );
    await user.click(screen.getByTestId("ref-popover-close"));
    expect(onClose).toHaveBeenCalled();
  });
});

describe("RefPopover — block reference", () => {
  it("shows a read-only note and no override controls", () => {
    renderWithProviders(
      <RefPopover
        state={mkState("login.response.id")}
        vaultPath="/v"
        onClose={() => {}}
      />,
    );
    expect(screen.getByTestId("ref-popover-blockref")).toBeInTheDocument();
    expect(screen.queryByTestId("ref-popover-override-input")).toBeNull();
  });
});
