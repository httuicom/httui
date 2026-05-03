import { describe, it, expect } from "vitest";

import { renderWithProviders, screen } from "@/test/render";
import { NewConnectionSshTab } from "@/components/layout/connections/NewConnectionSshTab";

describe("NewConnectionSshTab", () => {
  it("renders the coming-soon banner + example block", () => {
    renderWithProviders(<NewConnectionSshTab />);
    expect(screen.getByTestId("new-connection-ssh-tab")).toBeInTheDocument();
    expect(
      screen.getByTestId("new-connection-ssh-coming-soon"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("new-connection-ssh-example"),
    ).toBeInTheDocument();
    expect(screen.getByText(/SSH tunnel — coming soon/)).toBeInTheDocument();
    expect(screen.getByText(/ssh -L 6432/)).toBeInTheDocument();
  });
});
