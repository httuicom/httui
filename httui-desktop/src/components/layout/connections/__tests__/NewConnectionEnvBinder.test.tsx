import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { renderWithProviders, screen } from "@/test/render";
import { NewConnectionEnvBinder } from "@/components/layout/connections/NewConnectionEnvBinder";

const ENVS = [
  { id: "local", name: "local" },
  { id: "staging", name: "staging" },
  { id: "qa-eu", name: "qa-eu" },
  { id: "prod", name: "prod", readOnly: true },
];

describe("NewConnectionEnvBinder", () => {
  it("renders one pill per env in order", () => {
    renderWithProviders(
      <NewConnectionEnvBinder
        envs={ENVS}
        selectedIds={[]}
        onToggle={vi.fn()}
      />,
    );
    for (const env of ENVS) {
      expect(
        screen.getByTestId(`new-connection-env-pill-${env.id}`),
      ).toBeInTheDocument();
    }
  });

  it("marks selected pills via data-active + aria-pressed", () => {
    renderWithProviders(
      <NewConnectionEnvBinder
        envs={ENVS}
        selectedIds={["staging"]}
        onToggle={vi.fn()}
      />,
    );
    const staging = screen.getByTestId("new-connection-env-pill-staging");
    expect(staging.getAttribute("data-active")).toBe("true");
    expect(staging.getAttribute("aria-pressed")).toBe("true");
    const local = screen.getByTestId("new-connection-env-pill-local");
    expect(local.getAttribute("data-active")).toBe("false");
  });

  it("flags the read-only env via data-readonly + the inline (read-only) tag", () => {
    renderWithProviders(
      <NewConnectionEnvBinder
        envs={ENVS}
        selectedIds={[]}
        onToggle={vi.fn()}
      />,
    );
    const prod = screen.getByTestId("new-connection-env-pill-prod");
    expect(prod.getAttribute("data-readonly")).toBe("true");
    expect(prod.textContent).toContain("(read-only)");
  });

  it("clicking a pill dispatches onToggle with that env id", async () => {
    const onToggle = vi.fn();
    renderWithProviders(
      <NewConnectionEnvBinder
        envs={ENVS}
        selectedIds={[]}
        onToggle={onToggle}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("new-connection-env-pill-local"));
    expect(onToggle).toHaveBeenCalledWith("local");
  });

  it("renders the dashed '+ novo' pill only when onCreateNew is supplied", () => {
    const { rerender } = renderWithProviders(
      <NewConnectionEnvBinder
        envs={ENVS}
        selectedIds={[]}
        onToggle={vi.fn()}
      />,
    );
    expect(
      screen.queryByTestId("new-connection-env-pill-new"),
    ).not.toBeInTheDocument();

    rerender(
      <NewConnectionEnvBinder
        envs={ENVS}
        selectedIds={[]}
        onToggle={vi.fn()}
        onCreateNew={vi.fn()}
      />,
    );
    expect(
      screen.getByTestId("new-connection-env-pill-new"),
    ).toBeInTheDocument();
  });

  it("clicking '+ novo' dispatches onCreateNew", async () => {
    const onCreateNew = vi.fn();
    renderWithProviders(
      <NewConnectionEnvBinder
        envs={ENVS}
        selectedIds={[]}
        onToggle={vi.fn()}
        onCreateNew={onCreateNew}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("new-connection-env-pill-new"));
    expect(onCreateNew).toHaveBeenCalledTimes(1);
  });

  it("renders the section heading from the canvas spec", () => {
    renderWithProviders(
      <NewConnectionEnvBinder
        envs={ENVS}
        selectedIds={[]}
        onToggle={vi.fn()}
      />,
    );
    expect(screen.getByText("Vincular ao ambiente")).toBeInTheDocument();
  });
});
