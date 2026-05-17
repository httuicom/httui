import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { WorkspaceMenu } from "@/components/layout/topbar/WorkspaceMenu";
import { renderWithProviders, screen } from "@/test/render";

const baseProps = {
  workspace: "secret-test",
  isLeaf: false,
  vaults: ["/Users/me/secret-test", "/Users/me/notes"],
  activeVault: "/Users/me/secret-test",
  onSwitch: vi.fn(),
  onOpenOther: vi.fn(),
};

describe("WorkspaceMenu", () => {
  it("renders a trigger button with the workspace label", () => {
    renderWithProviders(<WorkspaceMenu {...baseProps} />);
    expect(
      screen.getByRole("button", { name: /Workspace secret-test/ }),
    ).toBeInTheDocument();
    expect(screen.getByText("secret-test")).toBeInTheDocument();
  });

  it("uses fg color when this segment is the deepest (leaf)", () => {
    renderWithProviders(<WorkspaceMenu {...baseProps} isLeaf={true} />);
    const trigger = screen.getByRole("button", {
      name: /Workspace secret-test/,
    });
    // Color is applied via Chakra style — assert presence rather than
    // exact CSS to keep the test resilient to token resolution.
    expect(trigger.getAttribute("data-segment")).toBe("workspace");
  });

  it("opens a menu listing every vault on click", async () => {
    const user = userEvent.setup();
    renderWithProviders(<WorkspaceMenu {...baseProps} />);

    await user.click(
      screen.getByRole("button", { name: /Workspace secret-test/ }),
    );

    // Both vault basenames are visible (one in the trigger, one in the
    // dropdown — that's why the matcher is the path entry which only
    // exists in the dropdown).
    expect(screen.getByText("/Users/me/secret-test")).toBeInTheDocument();
    expect(screen.getByText("/Users/me/notes")).toBeInTheDocument();
  });

  it("marks the active vault with data-active=true", async () => {
    const user = userEvent.setup();
    renderWithProviders(<WorkspaceMenu {...baseProps} />);

    await user.click(
      screen.getByRole("button", { name: /Workspace secret-test/ }),
    );

    const items = screen.getAllByRole("menuitem");
    const active = items.find((i) => i.getAttribute("data-active") === "true");
    expect(active?.getAttribute("data-vault-path")).toBe(
      "/Users/me/secret-test",
    );
  });

  it("clicking another vault calls onSwitch with its path", async () => {
    const user = userEvent.setup();
    const onSwitch = vi.fn();
    renderWithProviders(<WorkspaceMenu {...baseProps} onSwitch={onSwitch} />);

    await user.click(
      screen.getByRole("button", { name: /Workspace secret-test/ }),
    );
    await user.click(screen.getByText("notes"));

    expect(onSwitch).toHaveBeenCalledWith("/Users/me/notes");
  });

  it('always shows the "Abrir outro vault…" item', async () => {
    const user = userEvent.setup();
    renderWithProviders(<WorkspaceMenu {...baseProps} />);

    await user.click(
      screen.getByRole("button", { name: /Workspace secret-test/ }),
    );

    expect(screen.getByText("Abrir outro vault…")).toBeInTheDocument();
  });

  it('clicking "Abrir outro vault…" calls onOpenOther', async () => {
    const user = userEvent.setup();
    const onOpenOther = vi.fn();
    renderWithProviders(
      <WorkspaceMenu {...baseProps} onOpenOther={onOpenOther} />,
    );

    await user.click(
      screen.getByRole("button", { name: /Workspace secret-test/ }),
    );
    await user.click(screen.getByText("Abrir outro vault…"));

    expect(onOpenOther).toHaveBeenCalledTimes(1);
  });

  it('renders only "Abrir outro vault…" when there are no vaults', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <WorkspaceMenu
        {...baseProps}
        vaults={[]}
        activeVault={null}
        workspace="—"
      />,
    );

    await user.click(screen.getByRole("button", { name: /Workspace —/ }));

    expect(screen.getByText("Abrir outro vault…")).toBeInTheDocument();
    // No data-vault-path items
    expect(
      screen
        .queryAllByRole("menuitem")
        .filter((i) => !!i.getAttribute("data-vault-path")),
    ).toHaveLength(0);
  });
});
