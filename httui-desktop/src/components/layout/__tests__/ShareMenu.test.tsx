import { describe, it, expect, beforeEach, afterEach } from "vitest";
import userEvent from "@testing-library/user-event";
import { renderWithProviders, screen, waitFor } from "@/test/render";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";

import { ShareMenu } from "@/components/layout/ShareMenu";

beforeEach(() => {
  clearTauriMocks();
  mockTauriCommand("git_remote_list_cmd", () => [
    { name: "origin", url: "git@github.com:acme/widgets.git" },
  ]);
});

afterEach(() => clearTauriMocks());

describe("ShareMenu", () => {
  it("renders a compact trigger by default (no label)", () => {
    renderWithProviders(<ShareMenu vaultPath="/v" />);
    const trigger = screen.getByTestId("share-menu-trigger");
    expect(trigger.getAttribute("data-variant")).toBe("statusbar");
    expect(trigger.textContent).toBe("");
  });

  it("renders a labelled trigger in toolbar variant", () => {
    renderWithProviders(<ShareMenu vaultPath="/v" variant="toolbar" />);
    const trigger = screen.getByTestId("share-menu-trigger");
    expect(trigger.getAttribute("data-variant")).toBe("toolbar");
    expect(trigger.textContent).toContain("Share");
  });

  it("opens the SharePopover with the derived URLs", async () => {
    const user = userEvent.setup();
    renderWithProviders(<ShareMenu vaultPath="/v" />);
    await user.click(screen.getByTestId("share-menu-trigger"));
    await waitFor(() => {
      expect(screen.getByTestId("share-popover")).toBeInTheDocument();
    });
    // origin parsed → HTTPS / SSH / Web picker present.
    expect(
      screen.getByTestId("share-popover-remote-HTTPS"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("share-popover-remote-Web"),
    ).toBeInTheDocument();
  });

  it("shows the empty state when no remote is configured", async () => {
    mockTauriCommand("git_remote_list_cmd", () => []);
    const user = userEvent.setup();
    renderWithProviders(<ShareMenu vaultPath="/v" />);
    await user.click(screen.getByTestId("share-menu-trigger"));
    await waitFor(() => {
      expect(
        screen.getByTestId("share-popover").getAttribute("data-state"),
      ).toBe("empty");
    });
  });
});
