import { describe, it, expect, vi } from "vitest";
import userEvent from "@testing-library/user-event";

import { renderWithProviders, screen } from "@/test/render";
import { NewConnectionTestBanner } from "@/components/layout/connections/NewConnectionTestBanner";

describe("NewConnectionTestBanner", () => {
  it("renders nothing when idle", () => {
    const { container } = renderWithProviders(
      <NewConnectionTestBanner state={{ kind: "idle" }} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders the running banner with neutral dot", () => {
    renderWithProviders(
      <NewConnectionTestBanner state={{ kind: "running" }} />,
    );
    expect(
      screen.getByTestId("new-connection-test-banner-running"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("dot-running")).toBeInTheDocument();
    expect(screen.getByText("Testing…")).toBeInTheDocument();
  });

  it("renders the ok banner with detail line + latency", () => {
    renderWithProviders(
      <NewConnectionTestBanner
        state={{
          kind: "ok",
          detail: "postgres 15.4 · 47 tables",
          latencyMs: 18,
        }}
      />,
    );
    const banner = screen.getByTestId("new-connection-test-banner-ok");
    expect(banner).toBeInTheDocument();
    expect(banner.textContent).toContain("Connection OK");
    expect(banner.textContent).toContain("postgres 15.4 · 47 tables");
    expect(banner.textContent).toContain("18ms");
    expect(screen.getByTestId("dot-ok")).toBeInTheDocument();
  });

  it("renders the err banner with the message", () => {
    renderWithProviders(
      <NewConnectionTestBanner
        state={{ kind: "err", message: "ECONNREFUSED" }}
      />,
    );
    const banner = screen.getByTestId("new-connection-test-banner-err");
    expect(banner).toBeInTheDocument();
    expect(banner.textContent).toContain("Failed");
    expect(banner.textContent).toContain("ECONNREFUSED");
    expect(screen.getByTestId("dot-err")).toBeInTheDocument();
  });

  it("renders the Re-testar button on ok + dispatches onRetry", async () => {
    const onRetry = vi.fn();
    renderWithProviders(
      <NewConnectionTestBanner
        state={{ kind: "ok", detail: "postgres", latencyMs: 5 }}
        onRetry={onRetry}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("new-connection-test-retry"));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("renders the Re-testar button on err + dispatches onRetry", async () => {
    const onRetry = vi.fn();
    renderWithProviders(
      <NewConnectionTestBanner
        state={{ kind: "err", message: "boom" }}
        onRetry={onRetry}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("new-connection-test-retry"));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("hides Re-testar when onRetry is not provided", () => {
    renderWithProviders(
      <NewConnectionTestBanner
        state={{ kind: "ok", detail: "postgres", latencyMs: 5 }}
      />,
    );
    expect(
      screen.queryByTestId("new-connection-test-retry"),
    ).not.toBeInTheDocument();
  });
});
