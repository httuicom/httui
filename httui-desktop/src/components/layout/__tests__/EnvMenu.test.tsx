import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { EnvMenu } from "@/components/layout/EnvMenu";
import { renderWithProviders, screen } from "@/test/render";

const mkEnv = (id: string, name: string) => ({
  id,
  name,
  is_active: false,
  created_at: "2026-01-01T00:00:00Z",
});

describe("EnvMenu", () => {
  it('renders "no env" when no active env', () => {
    renderWithProviders(
      <EnvMenu
        environments={[]}
        activeEnvironment={null}
        onSwitch={() => {}}
      />,
    );
    expect(
      screen.getByRole("button", { name: /Environment no env/ }),
    ).toBeInTheDocument();
  });

  it("renders the active env name on the trigger", () => {
    renderWithProviders(
      <EnvMenu
        environments={[mkEnv("a", "local"), mkEnv("b", "prod")]}
        activeEnvironment={mkEnv("a", "local")}
        onSwitch={() => {}}
      />,
    );
    expect(
      screen.getByRole("button", { name: /Environment local/ }),
    ).toBeInTheDocument();
  });

  it("opens a menu listing every env on click", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <EnvMenu
        environments={[mkEnv("a", "local"), mkEnv("b", "prod")]}
        activeEnvironment={mkEnv("a", "local")}
        onSwitch={() => {}}
      />,
    );

    await user.click(
      screen.getByRole("button", { name: /Environment local/ }),
    );

    const items = screen.getAllByRole("menuitem");
    expect(items).toHaveLength(2);
    expect(items[0].getAttribute("data-env-id")).toBe("a");
    expect(items[1].getAttribute("data-env-id")).toBe("b");
  });

  it("marks the active env item with data-active=true", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <EnvMenu
        environments={[mkEnv("a", "local"), mkEnv("b", "prod")]}
        activeEnvironment={mkEnv("b", "prod")}
        onSwitch={() => {}}
      />,
    );
    await user.click(screen.getByRole("button", { name: /Environment prod/ }));
    const items = screen.getAllByRole("menuitem");
    const active = items.find(
      (i) => i.getAttribute("data-active") === "true",
    );
    expect(active?.getAttribute("data-env-id")).toBe("b");
  });

  it("clicking a non-active item calls onSwitch with its id", async () => {
    const user = userEvent.setup();
    const onSwitch = vi.fn();
    renderWithProviders(
      <EnvMenu
        environments={[mkEnv("a", "local"), mkEnv("b", "prod")]}
        activeEnvironment={mkEnv("a", "local")}
        onSwitch={onSwitch}
      />,
    );
    await user.click(
      screen.getByRole("button", { name: /Environment local/ }),
    );
    await user.click(screen.getByText("prod"));
    expect(onSwitch).toHaveBeenCalledWith("b");
  });

  it('shows "No environments" when the list is empty', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <EnvMenu
        environments={[]}
        activeEnvironment={null}
        onSwitch={() => {}}
      />,
    );
    await user.click(screen.getByRole("button", { name: /Environment no env/ }));
    expect(screen.getByText("No environments")).toBeInTheDocument();
  });
});
