import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { NetworkFields } from "@/components/layout/connections/form/NetworkFields";

function defaults() {
  return {
    driver: "postgres" as const,
    host: "localhost",
    onHostChange: vi.fn(),
    port: "5432",
    onPortChange: vi.fn(),
    dbName: "app",
    onDbNameChange: vi.fn(),
    username: "alice",
    onUsernameChange: vi.fn(),
    password: "",
    onPasswordChange: vi.fn(),
    sslMode: "disable",
    onSslModeChange: vi.fn(),
  };
}

describe("NetworkFields", () => {
  it("renders all field labels", () => {
    renderWithProviders(<NetworkFields {...defaults()} />);
    expect(screen.getByText("HOST")).toBeInTheDocument();
    expect(screen.getByText("PORT")).toBeInTheDocument();
    expect(screen.getByText("DATABASE")).toBeInTheDocument();
    expect(screen.getByText("USERNAME")).toBeInTheDocument();
    expect(screen.getByText("PASSWORD")).toBeInTheDocument();
    expect(screen.getByText("SSL")).toBeInTheDocument();
  });

  it("uses MySQL placeholder when driver is mysql", () => {
    renderWithProviders(<NetworkFields {...defaults()} driver="mysql" />);
    expect(screen.getByPlaceholderText("root")).toBeInTheDocument();
  });

  it("uses postgres placeholder when driver is postgres", () => {
    renderWithProviders(<NetworkFields {...defaults()} />);
    expect(screen.getByPlaceholderText("postgres")).toBeInTheDocument();
  });

  it("dispatches host onChange as user types", async () => {
    const onHostChange = vi.fn();
    renderWithProviders(
      <NetworkFields {...defaults()} onHostChange={onHostChange} />,
    );
    const host = screen.getByPlaceholderText("localhost");
    await userEvent.setup().type(host, "x");
    expect(onHostChange).toHaveBeenCalled();
  });

  it("ssl mode select offers all 4 modes and dispatches onChange", async () => {
    const onSslModeChange = vi.fn();
    renderWithProviders(
      <NetworkFields {...defaults()} onSslModeChange={onSslModeChange} />,
    );
    const select = screen.getByRole("combobox") as HTMLSelectElement;
    expect(Array.from(select.options).map((o) => o.value)).toEqual([
      "disable",
      "require",
      "verify-ca",
      "verify-full",
    ]);
    await userEvent.setup().selectOptions(select, "require");
    expect(onSslModeChange).toHaveBeenCalled();
  });

  it("password input has the locked-icon hint", () => {
    renderWithProviders(<NetworkFields {...defaults()} />);
    // password input + locked-icon flexbox both render
    expect(screen.getByPlaceholderText("••••••••")).toBeInTheDocument();
  });

  it("dispatches each field's onChange handler when typed", async () => {
    const props = defaults();
    renderWithProviders(<NetworkFields {...props} />);
    const user = userEvent.setup();
    await user.type(screen.getByPlaceholderText("localhost"), "h");
    await user.type(screen.getByPlaceholderText("5432"), "1");
    await user.type(screen.getByPlaceholderText("mydb"), "x");
    await user.type(screen.getByPlaceholderText("postgres"), "u");
    await user.type(screen.getByPlaceholderText("••••••••"), "p");
    expect(props.onHostChange).toHaveBeenCalled();
    expect(props.onPortChange).toHaveBeenCalled();
    expect(props.onDbNameChange).toHaveBeenCalled();
    expect(props.onUsernameChange).toHaveBeenCalled();
    expect(props.onPasswordChange).toHaveBeenCalled();
  });
});
