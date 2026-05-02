import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";
import { renderWithProviders, screen, waitFor } from "@/test/render";

import { CreateVaultCard } from "@/components/layout/empty-vault/CreateVaultCard";

function setup(over: Partial<Parameters<typeof CreateVaultCard>[0]> = {}) {
  const onCreate = vi.fn(async () => {});
  const onPickParent = vi.fn(async () => null as string | null);
  const utils = renderWithProviders(
    <CreateVaultCard
      onCreate={onCreate}
      onPickParent={onPickParent}
      {...over}
    />,
  );
  return { ...utils, onCreate, onPickParent };
}

describe("CreateVaultCard — collapsed state", () => {
  it("renders title, body, icon and expand CTA", () => {
    setup();
    expect(screen.getByTestId("create-vault-title").textContent).toBe(
      "Create vault",
    );
    expect(screen.getByTestId("create-vault-body").textContent).toContain(
      "do zero",
    );
    expect(screen.getByTestId("create-vault-expand")).toBeInTheDocument();
    expect(screen.queryByTestId("create-vault-form")).toBeNull();
  });

  it("expanding reveals form, hides CTA", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getByTestId("create-vault-expand"));
    expect(screen.getByTestId("create-vault-form")).toBeInTheDocument();
    expect(screen.queryByTestId("create-vault-expand")).toBeNull();
  });
});

describe("CreateVaultCard — expanded state", () => {
  async function expand() {
    const user = userEvent.setup();
    const utils = setup();
    await user.click(screen.getByTestId("create-vault-expand"));
    return { ...utils, user };
  }

  it("requires parent: submitting without picker surfaces inline error", async () => {
    const { user, onCreate } = await expand();
    await user.type(screen.getByTestId("create-vault-name"), "v1");
    await user.click(screen.getByTestId("create-vault-submit"));
    expect(screen.getByTestId("create-vault-error").textContent).toContain(
      "pasta pai",
    );
    expect(onCreate).not.toHaveBeenCalled();
  });

  it("requires non-empty trimmed name", async () => {
    const { user, onCreate, onPickParent } = await expand();
    onPickParent.mockResolvedValueOnce("/tmp");
    await user.click(screen.getByTestId("create-vault-pick-parent"));
    await waitFor(() =>
      expect(screen.getByTestId("create-vault-parent").textContent).toContain(
        "/tmp",
      ),
    );
    await user.type(screen.getByTestId("create-vault-name"), "   ");
    await user.click(screen.getByTestId("create-vault-submit"));
    expect(screen.getByTestId("create-vault-error").textContent).toContain(
      "nome",
    );
    expect(onCreate).not.toHaveBeenCalled();
  });

  it("submits trimmed name with picked parent", async () => {
    const { user, onCreate, onPickParent } = await expand();
    onPickParent.mockResolvedValueOnce("/tmp");
    await user.click(screen.getByTestId("create-vault-pick-parent"));
    await waitFor(() =>
      expect(screen.getByTestId("create-vault-parent").textContent).toContain(
        "/tmp",
      ),
    );
    await user.type(screen.getByTestId("create-vault-name"), "  meu-vault  ");
    await user.click(screen.getByTestId("create-vault-submit"));
    await waitFor(() => expect(onCreate).toHaveBeenCalledTimes(1));
    expect(onCreate).toHaveBeenCalledWith("/tmp", "meu-vault");
  });

  it("surfaces onCreate rejection inline", async () => {
    const { user, onCreate, onPickParent } = await expand();
    onPickParent.mockResolvedValueOnce("/tmp");
    await user.click(screen.getByTestId("create-vault-pick-parent"));
    onCreate.mockRejectedValueOnce(new Error("permission denied"));
    await user.type(screen.getByTestId("create-vault-name"), "x");
    await user.click(screen.getByTestId("create-vault-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("create-vault-error").textContent).toContain(
        "permission denied",
      ),
    );
  });

  it("surfaces onPickParent rejection inline", async () => {
    const { user, onPickParent } = await expand();
    onPickParent.mockRejectedValueOnce(new Error("dialog crashed"));
    await user.click(screen.getByTestId("create-vault-pick-parent"));
    await waitFor(() =>
      expect(screen.getByTestId("create-vault-error").textContent).toContain(
        "dialog crashed",
      ),
    );
  });

  it("busy disables the expand CTA", async () => {
    const user = userEvent.setup();
    const onCreate = vi.fn(async () => {});
    const onPickParent = vi.fn(async () => null as string | null);
    renderWithProviders(
      <CreateVaultCard
        onCreate={onCreate}
        onPickParent={onPickParent}
        busy
      />,
    );
    const cta = screen.getByTestId("create-vault-expand") as HTMLButtonElement;
    expect(cta.disabled).toBe(true);
    await user.click(cta);
    expect(screen.queryByTestId("create-vault-form")).toBeNull();
  });
});
