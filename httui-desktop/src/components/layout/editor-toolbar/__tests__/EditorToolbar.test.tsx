import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import {
  EditorToolbar,
  formatRelativeTime,
  shortenPath,
} from "@/components/layout/editor-toolbar/EditorToolbar";

describe("shortenPath", () => {
  it("returns the input when path is 1 segment", () => {
    expect(shortenPath("notes.md")).toBe("notes.md");
  });

  it("returns full path when 2 segments", () => {
    expect(shortenPath("auth/login.md")).toBe("auth/login.md");
  });

  it("keeps last 2 segments when deeper", () => {
    expect(shortenPath("/Users/me/vault/runbooks/auth/login.md")).toBe(
      "auth/login.md",
    );
  });
});

describe("formatRelativeTime", () => {
  const NOW = new Date("2026-05-01T12:00:00Z");

  it("returns '—' for null", () => {
    expect(formatRelativeTime(null, NOW)).toBe("—");
  });

  it("returns 'just now' for under 30s", () => {
    const date = new Date(NOW.getTime() - 5_000);
    expect(formatRelativeTime(date, NOW)).toBe("just now");
  });

  it("returns minutes for < 1h", () => {
    const date = new Date(NOW.getTime() - 5 * 60_000);
    expect(formatRelativeTime(date, NOW)).toBe("5m ago");
  });

  it("returns hours for < 24h", () => {
    const date = new Date(NOW.getTime() - 3 * 60 * 60_000);
    expect(formatRelativeTime(date, NOW)).toBe("3h ago");
  });

  it("returns days for < 30 days", () => {
    const date = new Date(NOW.getTime() - 5 * 24 * 60 * 60_000);
    expect(formatRelativeTime(date, NOW)).toBe("5d ago");
  });

  it("falls back to a localised month/day for older", () => {
    const date = new Date(NOW.getTime() - 60 * 24 * 60 * 60_000);
    const result = formatRelativeTime(date, NOW);
    // Should NOT contain "ago"
    expect(result).not.toMatch(/ago$/);
    expect(result.length).toBeGreaterThan(0);
  });
});

describe("EditorToolbar", () => {
  const baseProps = {
    filePath: "/v/runbooks/auth/login.md",
    editedAt: new Date("2026-05-01T11:55:00Z"),
    unsaved: false,
    blockCount: 3,
    autoCapture: false,
    onAutoCaptureChange: vi.fn(),
  };

  it("renders the shortened file path", () => {
    renderWithProviders(<EditorToolbar {...baseProps} />);
    expect(screen.getByTestId("editor-toolbar-path").textContent).toBe(
      "auth/login.md",
    );
  });

  it("shows the full path on the title attribute (hover)", () => {
    renderWithProviders(<EditorToolbar {...baseProps} />);
    expect(
      screen.getByTestId("editor-toolbar-path").getAttribute("title"),
    ).toBe("/v/runbooks/auth/login.md");
  });

  it("renders 'edited Xm ago' for clean tab", () => {
    renderWithProviders(<EditorToolbar {...baseProps} />);
    expect(screen.getByTestId("editor-toolbar-edited").textContent).toMatch(
      /edited /,
    );
  });

  it("renders the unsaved suffix when dirty", () => {
    renderWithProviders(<EditorToolbar {...baseProps} unsaved={true} />);
    expect(screen.getByTestId("editor-toolbar-edited").textContent).toContain(
      "(unsaved)",
    );
  });

  it("singularises block count for 1", () => {
    renderWithProviders(<EditorToolbar {...baseProps} blockCount={1} />);
    expect(screen.getByTestId("editor-toolbar-blocks").textContent).toBe(
      "1 block",
    );
  });

  it("pluralises block count for 0/N", () => {
    renderWithProviders(<EditorToolbar {...baseProps} blockCount={0} />);
    expect(screen.getByTestId("editor-toolbar-blocks").textContent).toBe(
      "0 blocks",
    );
  });

  it("auto-capture toggle: aria-pressed + data-active reflect state", () => {
    renderWithProviders(<EditorToolbar {...baseProps} autoCapture={true} />);
    const btn = screen.getByTestId("editor-toolbar-autocapture");
    expect(btn.getAttribute("aria-pressed")).toBe("true");
    expect(btn.getAttribute("data-active")).toBe("true");
  });

  it("auto-capture toggle dispatches onAutoCaptureChange with the inverted value", async () => {
    const onAutoCaptureChange = vi.fn();
    renderWithProviders(
      <EditorToolbar
        {...baseProps}
        autoCapture={false}
        onAutoCaptureChange={onAutoCaptureChange}
      />,
    );
    await userEvent
      .setup()
      .click(screen.getByTestId("editor-toolbar-autocapture"));
    expect(onAutoCaptureChange).toHaveBeenCalledWith(true);
  });

  it("toolbar exposes data-atom='editor-toolbar' for testing/styling hooks", () => {
    const { container } = renderWithProviders(<EditorToolbar {...baseProps} />);
    expect(
      container.querySelector('[data-atom="editor-toolbar"]'),
    ).toBeTruthy();
  });
});
