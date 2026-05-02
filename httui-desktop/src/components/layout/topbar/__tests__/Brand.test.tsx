import { describe, it, expect } from "vitest";
import { renderWithProviders, screen } from "@/test/render";

import { Brand } from "@/components/layout/topbar/Brand";

describe("Brand", () => {
  it("renders the httui logo image", () => {
    renderWithProviders(<Brand />);
    const img = screen.getByAltText("httui") as HTMLImageElement;
    expect(img).toBeInTheDocument();
    expect(img.tagName).toBe("IMG");
  });

  it("points to one of the theme-aware logo assets", () => {
    renderWithProviders(<Brand />);
    const img = screen.getByAltText("httui") as HTMLImageElement;
    expect(img.src).toMatch(/httui-(light|dark)-full\.png$/);
  });

  it("tags the wrapper as data-atom='brand'", () => {
    const { container } = renderWithProviders(<Brand />);
    expect(container.querySelector('[data-atom="brand"]')).toBeTruthy();
  });

  it("renders the divider as a non-interactive aria-hidden element", () => {
    const { container } = renderWithProviders(<Brand />);
    expect(
      container.querySelectorAll('[aria-hidden="true"]').length,
    ).toBeGreaterThanOrEqual(1);
  });
});
