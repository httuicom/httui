import { describe, it, expect } from "vitest";
import { renderWithProviders, screen } from "@/test/render";

import { OverrideBadge } from "@/components/layout/settings/OverrideBadge";

describe("OverrideBadge", () => {
  it("renders the label text", () => {
    renderWithProviders(
      <OverrideBadge label="overridden locally" tooltip="..." />,
    );
    expect(screen.getByText("overridden locally")).toBeTruthy();
  });

  it("exposes the tooltip via the title attribute", () => {
    renderWithProviders(
      <OverrideBadge
        data-testid="badge"
        label="overridden locally"
        tooltip="set in .httui/workspace.local.toml"
      />,
    );
    expect(
      screen.getByTestId("badge").getAttribute("title"),
    ).toBe("set in .httui/workspace.local.toml");
  });
});
