import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { renderWithProviders, screen } from "@/test/render";
import {
  NewConnectionSslTab,
  EMPTY_SSL_VALUE,
  SSL_MODES,
  isSslMode,
} from "@/components/layout/connections/NewConnectionSslTab";

describe("NewConnectionSslTab", () => {
  it("renders the mode select + 3 cert path inputs + hint", () => {
    renderWithProviders(
      <NewConnectionSslTab value={EMPTY_SSL_VALUE} onChange={vi.fn()} />,
    );
    expect(screen.getByTestId("new-connection-ssl-tab")).toBeInTheDocument();
    expect(screen.getByTestId("new-connection-ssl-mode")).toBeInTheDocument();
    expect(
      screen.getByTestId("new-connection-ssl-root-cert"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("new-connection-ssl-client-cert"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("new-connection-ssl-client-key"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("new-connection-ssl-hint")).toBeInTheDocument();
  });

  it("changing the mode dispatches onChange with that mode", async () => {
    const onChange = vi.fn();
    renderWithProviders(
      <NewConnectionSslTab value={EMPTY_SSL_VALUE} onChange={onChange} />,
    );
    const select = screen.getByTestId(
      "new-connection-ssl-mode",
    ) as HTMLSelectElement;
    await userEvent.setup().selectOptions(select, "verify-full");
    expect(onChange).toHaveBeenCalledWith({
      ...EMPTY_SSL_VALUE,
      mode: "verify-full",
    });
  });

  it("typing into a path patches that field", async () => {
    const onChange = vi.fn();
    renderWithProviders(
      <NewConnectionSslTab value={EMPTY_SSL_VALUE} onChange={onChange} />,
    );
    await userEvent
      .setup()
      .type(screen.getByTestId("new-connection-ssl-root-cert"), "/");
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenLastCalledWith({
      ...EMPTY_SSL_VALUE,
      rootCertPath: "/",
    });
  });

  it("typing into client cert + client key fields patches them", async () => {
    const onChange = vi.fn();
    renderWithProviders(
      <NewConnectionSslTab value={EMPTY_SSL_VALUE} onChange={onChange} />,
    );
    const user = userEvent.setup();
    await user.type(screen.getByTestId("new-connection-ssl-client-cert"), "c");
    await user.type(screen.getByTestId("new-connection-ssl-client-key"), "k");
    expect(onChange).toHaveBeenNthCalledWith(1, {
      ...EMPTY_SSL_VALUE,
      clientCertPath: "c",
    });
    expect(onChange).toHaveBeenNthCalledWith(2, {
      ...EMPTY_SSL_VALUE,
      clientKeyPath: "k",
    });
  });

  it("isSslMode accepts known modes only", () => {
    for (const mode of SSL_MODES) expect(isSslMode(mode)).toBe(true);
    expect(isSslMode("bogus")).toBe(false);
  });

  it("renders the value when controlled", () => {
    renderWithProviders(
      <NewConnectionSslTab
        value={{
          mode: "require",
          rootCertPath: "/ca",
          clientCertPath: "/cert",
          clientKeyPath: "/key",
        }}
        onChange={vi.fn()}
      />,
    );
    expect(
      (screen.getByTestId("new-connection-ssl-root-cert") as HTMLInputElement)
        .value,
    ).toBe("/ca");
    expect(
      (screen.getByTestId("new-connection-ssl-client-cert") as HTMLInputElement)
        .value,
    ).toBe("/cert");
    expect(
      (screen.getByTestId("new-connection-ssl-client-key") as HTMLInputElement)
        .value,
    ).toBe("/key");
    expect(
      (screen.getByTestId("new-connection-ssl-mode") as HTMLSelectElement)
        .value,
    ).toBe("require");
  });
});
