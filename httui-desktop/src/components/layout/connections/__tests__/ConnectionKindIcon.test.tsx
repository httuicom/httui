import { describe, it, expect } from "vitest";
import { renderWithProviders, screen } from "@/test/render";

import { ConnectionKindIcon } from "@/components/layout/connections/ConnectionKindIcon";
import { CONNECTION_KIND_ORDER } from "@/components/layout/connections/connection-kinds";

describe("ConnectionKindIcon", () => {
  it("renders the icon glyph and label aria for every kind", () => {
    for (const kind of CONNECTION_KIND_ORDER) {
      const { unmount } = renderWithProviders(
        <ConnectionKindIcon kind={kind} />,
      );
      const icon = screen.getByRole("img");
      expect(icon.getAttribute("data-kind")).toBe(kind);
      expect(icon.getAttribute("aria-label")).toBeTruthy();
      unmount();
    }
  });

  it("renders the kind label as title (tooltip)", () => {
    renderWithProviders(<ConnectionKindIcon kind="postgres" />);
    expect(screen.getByRole("img").getAttribute("title")).toBe("PostgreSQL");
  });

  it("renders an SVG icon from the kind metadata (lucide)", () => {
    renderWithProviders(<ConnectionKindIcon kind="mysql" />);
    const svg = screen.getByRole("img").querySelector("svg");
    expect(svg).toBeTruthy();
  });
});
