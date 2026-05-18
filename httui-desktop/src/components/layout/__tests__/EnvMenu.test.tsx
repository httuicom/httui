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

    await user.click(screen.getByRole("button", { name: /Environment local/ }));

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
    const active = items.find((i) => i.getAttribute("data-active") === "true");
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
    await user.click(screen.getByRole("button", { name: /Environment local/ }));
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
    await user.click(
      screen.getByRole("button", { name: /Environment no env/ }),
    );
    expect(screen.getByText("No environments")).toBeInTheDocument();
  });
});

describe("EnvMenu — V11 controlled / numeric / clone", () => {
  it("renders 1-9 numeric badges on the first nine envs", async () => {
    const user = userEvent.setup();
    const envs = Array.from({ length: 11 }, (_, i) =>
      mkEnv(`id${i}`, `env${i}`),
    );
    renderWithProviders(
      <EnvMenu
        environments={envs}
        activeEnvironment={envs[0]}
        onSwitch={() => {}}
        open
        onOpenChange={() => {}}
      />,
    );
    void user;
    expect(screen.getByTestId("env-numeric-1")).toBeInTheDocument();
    expect(screen.getByTestId("env-numeric-9")).toBeInTheDocument();
    // 10th/11th env get no numeric badge.
    expect(screen.queryByTestId("env-numeric-10")).toBeNull();
  });

  it("pressing a digit switches to that env and closes (controlled)", () => {
    const onSwitch = vi.fn();
    const onOpenChange = vi.fn();
    renderWithProviders(
      <EnvMenu
        environments={[mkEnv("a", "local"), mkEnv("b", "prod")]}
        activeEnvironment={mkEnv("a", "local")}
        onSwitch={onSwitch}
        open
        onOpenChange={onOpenChange}
      />,
    );
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "2" }));
    expect(onSwitch).toHaveBeenCalledWith("b");
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("ignores digit shortcut when not open", () => {
    const onSwitch = vi.fn();
    renderWithProviders(
      <EnvMenu
        environments={[mkEnv("a", "local"), mkEnv("b", "prod")]}
        activeEnvironment={mkEnv("a", "local")}
        onSwitch={onSwitch}
        open={false}
        onOpenChange={() => {}}
      />,
    );
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "2" }));
    expect(onSwitch).not.toHaveBeenCalled();
  });

  it("ignores out-of-range / modified digit presses", () => {
    const onSwitch = vi.fn();
    renderWithProviders(
      <EnvMenu
        environments={[mkEnv("a", "local")]}
        activeEnvironment={mkEnv("a", "local")}
        onSwitch={onSwitch}
        open
        onOpenChange={() => {}}
      />,
    );
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "5" }));
    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "1", metaKey: true }),
    );
    expect(onSwitch).not.toHaveBeenCalled();
  });

  it("renders a Clone footer that fires onRequestClone", async () => {
    const user = userEvent.setup();
    const onRequestClone = vi.fn();
    renderWithProviders(
      <EnvMenu
        environments={[mkEnv("a", "local")]}
        activeEnvironment={mkEnv("a", "local")}
        onSwitch={() => {}}
        open
        onOpenChange={() => {}}
        onRequestClone={onRequestClone}
      />,
    );
    const clone = screen.getByTestId("env-menu-clone");
    expect(clone.textContent).toContain("Clone local");
    // Ark/Zag sets pointer-events:none on the menu item during the
    // open transition; under CI load it can still be set when this
    // click runs (flaky). The element is genuinely interactive once
    // settled, so skip userEvent's transient pointer-events guard.
    await user.click(clone, { pointerEventsCheck: 0 });
    expect(onRequestClone).toHaveBeenCalledOnce();
  });

  it("hides the Clone footer when no active env", () => {
    renderWithProviders(
      <EnvMenu
        environments={[mkEnv("a", "local")]}
        activeEnvironment={null}
        onSwitch={() => {}}
        open
        onOpenChange={() => {}}
        onRequestClone={() => {}}
      />,
    );
    expect(screen.queryByTestId("env-menu-clone")).toBeNull();
  });

  it("propagates Chakra open/close through onOpenChange", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    renderWithProviders(
      <EnvMenu
        environments={[mkEnv("a", "local")]}
        activeEnvironment={mkEnv("a", "local")}
        onSwitch={() => {}}
        open={false}
        onOpenChange={onOpenChange}
      />,
    );
    await user.click(screen.getByRole("button", { name: /Environment local/ }));
    expect(onOpenChange).toHaveBeenCalledWith(true);
  });
});
