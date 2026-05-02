import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";
import { renderWithProviders, screen, waitFor } from "@/test/render";

import { CloneVaultCard } from "@/components/layout/empty-vault/CloneVaultCard";

function setup(over: Partial<Parameters<typeof CloneVaultCard>[0]> = {}) {
  const onClone = vi.fn(async () => {});
  const onPickParent = vi.fn(async () => null as string | null);
  const utils = renderWithProviders(
    <CloneVaultCard
      onClone={onClone}
      onPickParent={onPickParent}
      {...over}
    />,
  );
  return { ...utils, onClone, onPickParent };
}

describe("CloneVaultCard — collapsed state", () => {
  it("renders title, body, icon and expand CTA", () => {
    setup();
    expect(screen.getByTestId("clone-vault-title").textContent).toBe(
      "Clone vault",
    );
    expect(screen.getByTestId("clone-vault-body").textContent).toContain(
      "Clone um repositório git",
    );
    expect(screen.getByTestId("clone-vault-expand")).toBeInTheDocument();
    expect(screen.queryByTestId("clone-vault-form")).toBeNull();
  });

  it("expanding reveals the form and hides the CTA", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getByTestId("clone-vault-expand"));
    expect(screen.getByTestId("clone-vault-form")).toBeInTheDocument();
    expect(screen.queryByTestId("clone-vault-expand")).toBeNull();
  });
});

describe("CloneVaultCard — expanded state", () => {
  async function expand() {
    const user = userEvent.setup();
    const utils = setup();
    await user.click(screen.getByTestId("clone-vault-expand"));
    return { ...utils, user };
  }

  it("requires URL — submitting empty surfaces inline error", async () => {
    const { user, onClone } = await expand();
    await user.click(screen.getByTestId("clone-vault-submit"));
    expect(screen.getByTestId("clone-vault-error").textContent).toContain(
      "URL",
    );
    expect(onClone).not.toHaveBeenCalled();
  });

  it("submits trimmed URL with null parent by default (~/Documents)", async () => {
    const { user, onClone } = await expand();
    await user.type(
      screen.getByTestId("clone-vault-url"),
      "  https://github.com/x/y.git  ",
    );
    await user.click(screen.getByTestId("clone-vault-submit"));
    await waitFor(() => expect(onClone).toHaveBeenCalledTimes(1));
    expect(onClone).toHaveBeenCalledWith("https://github.com/x/y.git", null);
  });

  it("picks parent via onPickParent and forwards it as the container", async () => {
    const { user, onClone, onPickParent } = await expand();
    onPickParent.mockResolvedValueOnce("/tmp/parent-here");
    await user.click(screen.getByTestId("clone-vault-pick-parent"));
    await waitFor(() =>
      expect(screen.getByTestId("clone-vault-parent").textContent).toContain(
        "/tmp/parent-here",
      ),
    );
    await user.type(
      screen.getByTestId("clone-vault-url"),
      "git@github.com:x/y.git",
    );
    await user.click(screen.getByTestId("clone-vault-submit"));
    await waitFor(() => expect(onClone).toHaveBeenCalledTimes(1));
    expect(onClone).toHaveBeenCalledWith(
      "git@github.com:x/y.git",
      "/tmp/parent-here",
    );
  });

  it("surfaces onClone rejection inline without crashing", async () => {
    const { user, onClone } = await expand();
    onClone.mockRejectedValueOnce(new Error("network unreachable"));
    await user.type(
      screen.getByTestId("clone-vault-url"),
      "https://nope.invalid/x.git",
    );
    await user.click(screen.getByTestId("clone-vault-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("clone-vault-error").textContent).toContain(
        "network unreachable",
      ),
    );
  });

  it("surfaces onPickParent rejection inline", async () => {
    const { user, onPickParent } = await expand();
    onPickParent.mockRejectedValueOnce(new Error("picker boom"));
    await user.click(screen.getByTestId("clone-vault-pick-parent"));
    await waitFor(() =>
      expect(screen.getByTestId("clone-vault-error").textContent).toContain(
        "picker boom",
      ),
    );
  });

  it("busy disables the expand CTA", async () => {
    const user = userEvent.setup();
    const onClone = vi.fn(async () => {});
    const onPickParent = vi.fn(async () => null as string | null);
    renderWithProviders(
      <CloneVaultCard
        onClone={onClone}
        onPickParent={onPickParent}
        busy
      />,
    );
    const cta = screen.getByTestId("clone-vault-expand") as HTMLButtonElement;
    expect(cta.disabled).toBe(true);
    await user.click(cta);
    expect(screen.queryByTestId("clone-vault-form")).toBeNull();
  });

  it("default parent label reads '~/Documents'", async () => {
    const utils = setup();
    const user = userEvent.setup();
    await user.click(screen.getByTestId("clone-vault-expand"));
    expect(screen.getByTestId("clone-vault-parent").textContent).toContain(
      "~/Documents",
    );
    expect(utils.onClone).not.toHaveBeenCalled();
  });
});
