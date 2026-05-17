import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";
import { renderWithProviders, screen } from "@/test/render";

import { OpenVaultCard } from "@/components/layout/empty-vault/OpenVaultCard";

describe("OpenVaultCard", () => {
  it("renders title, body, icon and CTA", () => {
    renderWithProviders(<OpenVaultCard onOpenClick={() => {}} />);
    expect(screen.getByTestId("open-vault-title").textContent).toBe(
      "Open vault",
    );
    expect(screen.getByTestId("open-vault-body").textContent).toContain(
      "Abra uma pasta existente",
    );
    expect(screen.getByTestId("open-vault-icon")).toBeInTheDocument();
    expect(screen.getByTestId("open-vault-cta").textContent).toContain(
      "Escolher pasta",
    );
  });

  it("dispatches onOpenClick when clicked", async () => {
    const user = userEvent.setup();
    const onOpenClick = vi.fn();
    renderWithProviders(<OpenVaultCard onOpenClick={onOpenClick} />);
    await user.click(screen.getByTestId("open-vault-card"));
    expect(onOpenClick).toHaveBeenCalledTimes(1);
  });

  it("respects busy: disables click and dims", async () => {
    const user = userEvent.setup();
    const onOpenClick = vi.fn();
    renderWithProviders(<OpenVaultCard onOpenClick={onOpenClick} busy />);
    const card = screen.getByTestId("open-vault-card") as HTMLButtonElement;
    expect(card.disabled).toBe(true);
    await user.click(card);
    expect(onOpenClick).not.toHaveBeenCalled();
  });

  it("icon is aria-hidden", () => {
    renderWithProviders(<OpenVaultCard onOpenClick={() => {}} />);
    expect(
      screen.getByTestId("open-vault-icon").getAttribute("aria-hidden"),
    ).toBe("true");
  });

  it("exposes accessible label", () => {
    renderWithProviders(<OpenVaultCard onOpenClick={() => {}} />);
    expect(screen.getByLabelText("Open existing vault")).toBeInTheDocument();
  });
});
