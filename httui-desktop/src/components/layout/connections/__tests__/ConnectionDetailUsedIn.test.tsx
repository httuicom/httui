import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { ConnectionDetailUsedIn } from "@/components/layout/connections/ConnectionDetailUsedIn";
import type { RunbookUsage } from "@/components/layout/connections/connection-usages";

function usage(
  filePath: string,
  line: number,
  preview: string | null = "SELECT 1;",
): RunbookUsage {
  return { filePath, line, preview };
}

describe("ConnectionDetailUsedIn", () => {
  it("shows the empty state when there are no usages", () => {
    renderWithProviders(<ConnectionDetailUsedIn usages={[]} />);
    expect(screen.getByTestId("used-in-empty")).toBeInTheDocument();
    expect(screen.getByTestId("used-in-count").textContent).toBe("0");
  });

  it("shows the loading state when no usages yet but loading", () => {
    renderWithProviders(<ConnectionDetailUsedIn usages={[]} loading />);
    expect(screen.getByTestId("used-in-loading")).toBeInTheDocument();
    expect(screen.queryByTestId("used-in-empty")).toBeNull();
  });

  it("renders one row per usage with file path and :line", () => {
    renderWithProviders(
      <ConnectionDetailUsedIn
        usages={[usage("runbooks/a.md", 12), usage("runbooks/b.md", 7, null)]}
      />,
    );
    expect(screen.getByTestId("used-in-count").textContent).toBe("2");
    expect(screen.getByTestId("used-in-row-0").textContent).toContain(
      "runbooks/a.md",
    );
    expect(screen.getByTestId("used-in-row-0").textContent).toContain(":12");
    expect(screen.getByTestId("used-in-row-1").textContent).toContain(":7");
  });

  it("renders the preview when present", () => {
    renderWithProviders(
      <ConnectionDetailUsedIn
        usages={[usage("a.md", 1, "SELECT count(*) FROM orders;")]}
      />,
    );
    expect(screen.getByTestId("used-in-row-0-preview").textContent).toContain(
      "orders",
    );
  });

  it("hides the preview slot when null", () => {
    renderWithProviders(
      <ConnectionDetailUsedIn usages={[usage("a.md", 1, null)]} />,
    );
    expect(screen.queryByTestId("used-in-row-0-preview")).toBeNull();
  });

  it("dispatches onOpen with (filePath, line) when a row is clicked", async () => {
    const onOpen = vi.fn();
    renderWithProviders(
      <ConnectionDetailUsedIn
        usages={[usage("notes/x.md", 42)]}
        onOpen={onOpen}
      />,
    );
    await userEvent.setup().click(screen.getByTestId("used-in-row-0"));
    expect(onOpen).toHaveBeenCalledWith("notes/x.md", 42);
  });
});
