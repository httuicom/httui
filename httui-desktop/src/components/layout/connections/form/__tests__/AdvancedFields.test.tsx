import { describe, it, expect, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/render";
import userEvent from "@testing-library/user-event";

import { AdvancedFields } from "@/components/layout/connections/form/AdvancedFields";

function defaults() {
  return {
    open: false,
    onToggle: vi.fn(),
    timeoutMs: "10000",
    onTimeoutMsChange: vi.fn(),
    queryTimeoutMs: "30000",
    onQueryTimeoutMsChange: vi.fn(),
    ttlSeconds: "300",
    onTtlSecondsChange: vi.fn(),
    maxPoolSize: "5",
    onMaxPoolSizeChange: vi.fn(),
  };
}

describe("AdvancedFields", () => {
  it("renders the Advanced toggle even when collapsed", () => {
    renderWithProviders(<AdvancedFields {...defaults()} />);
    expect(screen.getByText("Advanced")).toBeInTheDocument();
    expect(screen.queryByText("CONNECT TIMEOUT")).toBeNull();
  });

  it("dispatches onToggle when the chevron row is clicked", async () => {
    const onToggle = vi.fn();
    renderWithProviders(<AdvancedFields {...defaults()} onToggle={onToggle} />);
    await userEvent.setup().click(screen.getByTestId("advanced-toggle"));
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it("renders all 4 fields when open", () => {
    renderWithProviders(<AdvancedFields {...defaults()} open={true} />);
    expect(screen.getByText("CONNECT TIMEOUT")).toBeInTheDocument();
    expect(screen.getByText("QUERY TIMEOUT")).toBeInTheDocument();
    expect(screen.getByText("TTL (SECONDS)")).toBeInTheDocument();
    expect(screen.getByText("MAX POOL SIZE")).toBeInTheDocument();
  });

  it("dispatches each field's onChange handler", async () => {
    const onTimeoutMsChange = vi.fn();
    const onQueryTimeoutMsChange = vi.fn();
    const onTtlSecondsChange = vi.fn();
    const onMaxPoolSizeChange = vi.fn();
    renderWithProviders(
      <AdvancedFields
        {...defaults()}
        open={true}
        onTimeoutMsChange={onTimeoutMsChange}
        onQueryTimeoutMsChange={onQueryTimeoutMsChange}
        onTtlSecondsChange={onTtlSecondsChange}
        onMaxPoolSizeChange={onMaxPoolSizeChange}
      />,
    );
    const inputs = screen.getAllByRole("textbox") as HTMLInputElement[];
    expect(inputs).toHaveLength(4);
    const user = userEvent.setup();
    await user.type(inputs[0], "1");
    await user.type(inputs[1], "2");
    await user.type(inputs[2], "3");
    await user.type(inputs[3], "4");
    expect(onTimeoutMsChange).toHaveBeenCalled();
    expect(onQueryTimeoutMsChange).toHaveBeenCalled();
    expect(onTtlSecondsChange).toHaveBeenCalled();
    expect(onMaxPoolSizeChange).toHaveBeenCalled();
  });

  it("renders the chevron-down icon when open and chevron-right when collapsed", () => {
    const { rerender } = renderWithProviders(
      <AdvancedFields {...defaults()} />,
    );
    // Collapsed: only the toggle row visible.
    expect(screen.queryByText("CONNECT TIMEOUT")).toBeNull();
    rerender(<AdvancedFields {...defaults()} open={true} />);
    expect(screen.getByText("CONNECT TIMEOUT")).toBeInTheDocument();
  });
});
