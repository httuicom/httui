import { describe, it, expect, vi, afterEach } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { ConnectionDetailFooter } from "@/components/layout/connections/ConnectionDetailFooter";

afterEach(() => {
  vi.useRealTimers();
});

async function flush() {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

describe("ConnectionDetailFooter — Test action", () => {
  it("renders the three action buttons", () => {
    renderWithProviders(
      <ConnectionDetailFooter
        onTest={() => Promise.resolve(0)}
        onDuplicate={() => {}}
        onDelete={() => {}}
      />,
    );
    expect(screen.getByTestId("footer-test")).toBeInTheDocument();
    expect(screen.getByTestId("footer-duplicate")).toBeInTheDocument();
    expect(screen.getByTestId("footer-delete")).toBeInTheDocument();
  });

  it("Test → ok banner with the latency from onTest", async () => {
    const onTest = vi.fn().mockResolvedValue(47);
    renderWithProviders(
      <ConnectionDetailFooter
        onTest={onTest}
        onDuplicate={() => {}}
        onDelete={() => {}}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("footer-test"));
    await flush();
    expect(onTest).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("footer-test-ok").textContent).toContain("47ms");
  });

  it("Test → err banner with the message from onTest", async () => {
    const onTest = vi.fn().mockRejectedValue(new Error("auth failed"));
    renderWithProviders(
      <ConnectionDetailFooter
        onTest={onTest}
        onDuplicate={() => {}}
        onDelete={() => {}}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("footer-test"));
    await flush();
    expect(screen.getByTestId("footer-test-err").textContent).toContain(
      "auth failed",
    );
  });
});

describe("ConnectionDetailFooter — Duplicate action", () => {
  it("dispatches onDuplicate when clicked", async () => {
    const onDuplicate = vi.fn().mockResolvedValue(undefined);
    renderWithProviders(
      <ConnectionDetailFooter
        onTest={() => Promise.resolve(0)}
        onDuplicate={onDuplicate}
        onDelete={() => {}}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("footer-duplicate"));
    await flush();
    expect(onDuplicate).toHaveBeenCalledTimes(1);
  });

  it("surfaces duplicate error inline when onDuplicate rejects", async () => {
    const onDuplicate = vi.fn().mockRejectedValue(new Error("name taken"));
    renderWithProviders(
      <ConnectionDetailFooter
        onTest={() => Promise.resolve(0)}
        onDuplicate={onDuplicate}
        onDelete={() => {}}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("footer-duplicate"));
    await flush();
    expect(screen.getByTestId("footer-duplicate-error").textContent).toContain(
      "name taken",
    );
  });
});

describe("ConnectionDetailFooter — Delete action (two-step confirm)", () => {
  it("first click flips the button label to 'Click again to confirm'", async () => {
    const onDelete = vi.fn();
    renderWithProviders(
      <ConnectionDetailFooter
        onTest={() => Promise.resolve(0)}
        onDuplicate={() => {}}
        onDelete={onDelete}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("footer-delete"));
    expect(screen.getByTestId("footer-delete").textContent).toContain(
      "Click again",
    );
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("second click within timeout dispatches onDelete", async () => {
    const onDelete = vi.fn().mockResolvedValue(undefined);
    renderWithProviders(
      <ConnectionDetailFooter
        onTest={() => Promise.resolve(0)}
        onDuplicate={() => {}}
        onDelete={onDelete}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("footer-delete"));
    await user.click(screen.getByTestId("footer-delete"));
    await flush();
    expect(onDelete).toHaveBeenCalledTimes(1);
  });

  it("second click after timeout reverts to fresh confirm", async () => {
    const onDelete = vi.fn();
    // Use a tiny real timeout — wait it out instead of fake-timer
    // gymnastics that conflict with userEvent's internal setTimeouts.
    renderWithProviders(
      <ConnectionDetailFooter
        onTest={() => Promise.resolve(0)}
        onDuplicate={() => {}}
        onDelete={onDelete}
        deleteConfirmTimeoutMs={50}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("footer-delete"));
    expect(screen.getByTestId("footer-delete").textContent).toContain(
      "Click again",
    );
    // Wait past the reset window
    await new Promise((r) => setTimeout(r, 120));
    await flush();
    expect(screen.getByTestId("footer-delete").textContent).toContain("Delete");
    expect(screen.getByTestId("footer-delete").textContent).not.toContain(
      "Click again",
    );
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("surfaces delete error when onDelete rejects", async () => {
    const onDelete = vi.fn().mockRejectedValue(new Error("in use"));
    renderWithProviders(
      <ConnectionDetailFooter
        onTest={() => Promise.resolve(0)}
        onDuplicate={() => {}}
        onDelete={onDelete}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("footer-delete"));
    await user.click(screen.getByTestId("footer-delete"));
    await flush();
    expect(screen.getByTestId("footer-delete-error").textContent).toContain(
      "in use",
    );
  });
});
