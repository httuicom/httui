import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";
import { renderWithProviders, screen, waitFor } from "@/test/render";

import { CloneVaultCard } from "@/components/layout/empty-vault/CloneVaultCard";

function setup(over: Partial<Parameters<typeof CloneVaultCard>[0]> = {}) {
  const onClone = vi.fn(async () => {});
  const onPickDestination = vi.fn(async () => null as string | null);
  const utils = renderWithProviders(
    <CloneVaultCard
      onClone={onClone}
      onPickDestination={onPickDestination}
      {...over}
    />,
  );
  return { ...utils, onClone, onPickDestination };
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

  it("submits trimmed URL with no destination by default", async () => {
    const { user, onClone } = await expand();
    await user.type(
      screen.getByTestId("clone-vault-url"),
      "  https://github.com/x/y.git  ",
    );
    await user.click(screen.getByTestId("clone-vault-submit"));
    await waitFor(() => expect(onClone).toHaveBeenCalledTimes(1));
    expect(onClone).toHaveBeenCalledWith("https://github.com/x/y.git", null);
  });

  it("picks destination via onPickDestination and forwards it", async () => {
    const { user, onClone, onPickDestination } = await expand();
    onPickDestination.mockResolvedValueOnce("/tmp/clone-here");
    await user.click(screen.getByTestId("clone-vault-pick-destination"));
    await waitFor(() =>
      expect(screen.getByTestId("clone-vault-destination").textContent).toContain(
        "/tmp/clone-here",
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
      "/tmp/clone-here",
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

  it("surfaces onPickDestination rejection inline", async () => {
    const { user, onPickDestination } = await expand();
    onPickDestination.mockRejectedValueOnce(new Error("picker boom"));
    await user.click(screen.getByTestId("clone-vault-pick-destination"));
    await waitFor(() =>
      expect(screen.getByTestId("clone-vault-error").textContent).toContain(
        "picker boom",
      ),
    );
  });

  it("busy disables the expand CTA", async () => {
    const user = userEvent.setup();
    const onClone = vi.fn(async () => {});
    const onPickDestination = vi.fn(async () => null as string | null);
    renderWithProviders(
      <CloneVaultCard
        onClone={onClone}
        onPickDestination={onPickDestination}
        busy
      />,
    );
    const cta = screen.getByTestId("clone-vault-expand") as HTMLButtonElement;
    expect(cta.disabled).toBe(true);
    await user.click(cta);
    expect(screen.queryByTestId("clone-vault-form")).toBeNull();
  });
});
