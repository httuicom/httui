import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { EmptyVaultSidebar } from "@/components/layout/empty-vault/EmptyVaultSidebar";

describe("EmptyVaultSidebar", () => {
  it("renders the WORKSPACE / RECENTES / EXPLORAR section labels", () => {
    renderWithProviders(<EmptyVaultSidebar onCreateRunbook={() => {}} />);
    expect(screen.getByText("WORKSPACE")).toBeInTheDocument();
    expect(screen.getByText("RECENTES")).toBeInTheDocument();
    expect(screen.getByText("EXPLORAR")).toBeInTheDocument();
  });

  it("workspace pill defaults to 'default' label + uppercase initial", () => {
    renderWithProviders(<EmptyVaultSidebar onCreateRunbook={() => {}} />);
    const pill = screen.getByTestId("workspace-pill");
    expect(pill.textContent).toContain("default");
    expect(pill.textContent).toContain("D");
  });

  it("workspace pill respects custom workspaceName + initial", () => {
    renderWithProviders(
      <EmptyVaultSidebar
        workspaceName="acme-payments"
        onCreateRunbook={() => {}}
      />,
    );
    const pill = screen.getByTestId("workspace-pill");
    expect(pill.textContent).toContain("acme-payments");
    expect(pill.textContent).toContain("A");
  });

  it("'Novo runbook' button dispatches onCreateRunbook", async () => {
    const user = userEvent.setup();
    const onCreateRunbook = vi.fn();
    renderWithProviders(
      <EmptyVaultSidebar onCreateRunbook={onCreateRunbook} />,
    );
    await user.click(screen.getByTestId("create-runbook-btn"));
    expect(onCreateRunbook).toHaveBeenCalledTimes(1);
  });

  it("workspace pill click dispatches onWorkspaceClick when supplied", async () => {
    const user = userEvent.setup();
    const onWorkspaceClick = vi.fn();
    renderWithProviders(
      <EmptyVaultSidebar
        onCreateRunbook={() => {}}
        onWorkspaceClick={onWorkspaceClick}
      />,
    );
    await user.click(screen.getByTestId("workspace-pill"));
    expect(onWorkspaceClick).toHaveBeenCalledTimes(1);
  });

  it("RECENTES shows the empty-state copy", () => {
    renderWithProviders(<EmptyVaultSidebar onCreateRunbook={() => {}} />);
    expect(screen.getByTestId("recentes-empty").textContent).toBe(
      "Vazio. Quando você criar runbooks, eles aparecerão aqui.",
    );
  });

  it("EXPLORAR section lists Connections / Variables / Members (Templates is out of V1 scope)", () => {
    renderWithProviders(<EmptyVaultSidebar onCreateRunbook={() => {}} />);
    expect(screen.getByTestId("explore-connections")).toBeInTheDocument();
    expect(screen.getByTestId("explore-variables")).toBeInTheDocument();
    expect(screen.getByTestId("explore-members")).toBeInTheDocument();
    expect(screen.queryByTestId("explore-templates")).toBeNull();
  });

  it("Connections / Variables / Members render a (count) suffix", () => {
    renderWithProviders(<EmptyVaultSidebar onCreateRunbook={() => {}} />);
    expect(screen.getByTestId("explore-connections").textContent).toContain(
      "(0)",
    );
    expect(screen.getByTestId("explore-variables").textContent).toContain(
      "(0)",
    );
    expect(screen.getByTestId("explore-members").textContent).toContain("(1)");
  });

  it("tags the wrapper with data-atom='empty-vault-sidebar'", () => {
    const { container } = renderWithProviders(
      <EmptyVaultSidebar onCreateRunbook={() => {}} />,
    );
    expect(
      container.querySelector('[data-atom="empty-vault-sidebar"]'),
    ).toBeTruthy();
  });
});
